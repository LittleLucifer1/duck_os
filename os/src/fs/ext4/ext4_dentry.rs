use alloc::{collections::btree_map::BTreeMap, ffi::CString, string::{String, ToString}, sync::Arc};
use log::info;
use lwext4_rust::{lwext4_check_inode_exist, lwext4_link, lwext4_mvdir, lwext4_mvfile, lwext4_readlink, lwext4_rmdir, lwext4_rmfile, lwext4_symlink, Ext4Dir, Ext4File};

use crate::{
    fs::{dentry::{Dentry, DentryMeta, DENTRY_CACHE}, 
        file::{File, FileMeta, FileMetaInner, SeekFrom}, 
        info::{InodeMode, OpenFlags}, 
        inode::Inode, page_cache::PageCache}, 
    sync::SpinLock, 
    syscall::error::{Errno, OSResult}, 
    utils::path::{cwd_and_name, dentry_name}
};

use super::{ext4_file::Ext4MemFile, ext4_inode::Ext4Inode};

pub struct Ext4Dentry {
    pub meta: DentryMeta,
}

impl Dentry for Ext4Dentry {
    fn metadata(&self) -> &DentryMeta {
        &self.meta
    }
    
    fn load_child(&self, this: Arc<dyn Dentry>) -> OSResult<()> {
        let inode= self.meta.inner.lock().d_inode.clone()
            .downcast_arc::<Ext4Inode>().unwrap_or_else(|_| unreachable!());
        let mut dir = inode.dir.as_ref().unwrap().lock();
        let iters = dir.lwext4_dir_entries(&self.path()).unwrap();

        // skip "." and ".."
        dir.next();
        dir.next();

        while let Some(dirent) = dir.next() {
            // 1.此处的dirent.name是c语言风格的字符串，可能以\0结尾，所以使用CString
            let name = CString::new(dirent.name).map_err(|_| Errno::EINVAL)?;
            let name = name.to_str().unwrap();
            // 2.拿到绝对路径
            let cwd = self.path();
            let path = cwd_and_name(&name, &cwd);
            // 3.构造inode
            let i_mode =  InodeMode::from(dirent.type_ as usize);
            let inode: Ext4Inode;
            match i_mode {
                InodeMode::Regular => {
                    let file = Ext4File::open(
                        &path, OpenFlags::O_RDWR.bits() as i32
                    ).map_err(Errno::from_i32)?;
                    inode = Ext4Inode::new_file(i_mode, file);
                },
                InodeMode::Directory => {
                    let dir = Ext4Dir::open(&path).map_err(Errno::from_i32)?;
                    inode = Ext4Inode::new_dir(i_mode, dir);
                },
                _ => {todo!()}
            }
            let inode_arc: Arc<dyn Inode> = Arc::new(inode);
            // 4.构造dentry
            let child_dir = Ext4Dentry::new(
                String::from(name), 
                path.clone(), 
                inode_arc, 
                Some(Arc::clone(&this)),
            );
            // 5.处理关系
            child_dir.meta.inner.lock().d_parent = Some(Arc::downgrade(&this));
            let child_dir_arc: Arc<dyn Dentry> = Arc::new(child_dir);
            this.metadata().inner.lock().d_child.insert(String::from(name), Arc::clone(&child_dir_arc));
            DENTRY_CACHE.lock().insert(path.clone(), Arc::clone(&child_dir_arc));
        }
        Ok(())
    }

    fn load_all_child(&self, this: Arc<dyn Dentry>) -> OSResult<()> {
        let fa = this.clone();
        if fa.metadata()
            .inner
            .lock()
            .d_inode
            .metadata().i_mode != InodeMode::Directory {
            return Ok(());
        }
        fa.load_child(fa.clone())?;
        for (_, child) in &fa.metadata().inner.lock().d_child {
            child.load_all_child(Arc::clone(child))?;
        }
        Ok(())
    }

    fn create(&self, this: Arc<dyn Dentry>, name: &str, mode: InodeMode) -> OSResult<Arc<dyn Dentry>> {
        let child_dentry: Arc<dyn Dentry>;
        let path = cwd_and_name(name, &self.path());
        if mode.eq(&InodeMode::Directory) {
            child_dentry = self.mkdir(
                &path,
                mode
            )?;
        } else if mode.eq(&InodeMode::Regular) {
            child_dentry = self.mknod(
                &path,
                mode, 
                None,
            )?;
        } else {
            todo!()
        }
        self.meta.inner.lock().d_child.insert(String::from(name), Arc::clone(&child_dentry));
        child_dentry.metadata().inner.lock().d_parent = Some(Arc::downgrade(&this));
        DENTRY_CACHE.lock().insert(path, child_dentry.clone());
        Ok(child_dentry)
    }

    fn open(&self, dentry: Arc<dyn Dentry>, flags: OpenFlags) -> OSResult<Arc<dyn File>> {
        dentry.metadata().inner.lock().d_inode.metadata().inner.lock().i_open_count += 1;
        let file_meta = FileMeta {
            f_mode: flags.clone().into(),
            page_cache: Some(Arc::new(PageCache::new())),
            f_dentry: Some(Arc::clone(&dentry)),
            f_inode: Arc::downgrade(&Arc::clone(&dentry.metadata().inner.lock().d_inode)),
            inner: SpinLock::new(FileMetaInner {
                f_pos: 0,
                dirent_index: 0,
            })
        };
        let file = Ext4MemFile::new(file_meta);
        if flags.contains(OpenFlags::O_TRUNC) {
            file.truncate(0)?;   
        }
        if flags.contains(OpenFlags::O_APPEND) {
            file.seek(SeekFrom::End(0))?;
        }
        Ok(Arc::new(file))
    }
    // TODO: 如果child下面还有child呢？该怎么办？
    // DOWN：上面一层 sys_unlinkat 会处理这种情况
    // Description: 删除dentry对应inode的硬链接数
    // 注意事项：1. 空目录，直接删除；2. 文件（硬链接数==0 && 文件打开数==0）删除
    // 3.文件硬链接数 --；
    fn unlink(&self, child: Arc<dyn Dentry>) -> OSResult<()> {
        let mut child_lock = child.metadata().inner.lock();
        let child_name = child_lock.d_name.clone();
        let child_path = child_lock.d_path.clone();
        // 处理目录树上的关系
        child_lock.d_parent = None;
        DENTRY_CACHE.lock().remove(&child_path);
        self.meta.inner.lock().d_child.remove(&child_name);
        // 处理链接数等关系
        let inode_mode = child_lock.d_inode.metadata().i_mode;
        match inode_mode {
            InodeMode::Regular => {
                info!("[sys_unlink] Unlink File");
                let mut inode_lock = child_lock.d_inode.metadata().inner.lock();
                inode_lock.i_link_count -= 1;
                if inode_lock.i_link_count == 0 && inode_lock.i_open_count == 0{
                    info!(
                        "The File is {:?}, Arc strong count is {}, weak count is {}, which are supposed to be 1or0",
                        child_path, Arc::strong_count(&child), Arc::weak_count(&child)
                    );
                    drop(inode_lock);
                    drop(child_lock);
                    lwext4_rmfile(&child_path).map_err(Errno::from_i32)?;
                } 
            }
            InodeMode::Directory => {
                info!("[sys_unlink] Unlink Directory.");
                drop(child_lock);
                lwext4_rmdir(&child_path).map_err(Errno::from_i32)?;
            }
            // TODO：unimplemented !
            InodeMode::Link => {
                todo!()
            }
            _ => todo!()
        }
        Ok(())
    }

    fn look_up(self: Arc<Self>, name: &str) -> Option<Arc<dyn Dentry>> {
        let meta_lock = self.meta.inner.lock();
        let child = meta_lock.d_child.get(name);
        // 1.在缓存中
        if let Some(child) = child {
            Some(Arc::clone(child))
        } else {
            drop(meta_lock);
            // 检查是否在磁盘中，目前只检查 file or directory,不在则返回错误 ENOENT
            // Update: 这里选择不报磁盘上查找的错误，而是找不到就返回 None
            let path = cwd_and_name(name, &self.path());
            let new_inode: Ext4Inode;
            if lwext4_check_inode_exist(&path, lwext4_rust::InodeTypes::EXT4_DE_REG_FILE) {
                let new_file = Ext4File::open(&path, OpenFlags::O_RDWR.bits() as i32).ok();
                if let Some(new_file) = new_file {
                    new_inode = Ext4Inode::new_file(InodeMode::Regular, new_file);
                } else {
                    return None;
                }
            } else if lwext4_check_inode_exist(&path, lwext4_rust::InodeTypes::EXT4_DE_DIR) {
                let new_file = Ext4Dir::open(&path).ok();
                if let Some(new_file) = new_file {
                    new_inode = Ext4Inode::new_dir(InodeMode::Regular, new_file);
                } else {
                    return None;
                }
            } else if lwext4_check_inode_exist(&path, lwext4_rust::InodeTypes::EXT4_DE_SYMLINK) {
                new_inode = Ext4Inode::new_link(InodeMode::Link, 0);
            }
            else {
                return None;
            }
            let inode_arc: Arc<dyn Inode> = Arc::new(new_inode);
            let parent: Arc<dyn Dentry> = self.clone();
                let new_dirent = Ext4Dentry::new(
                    name.to_string(), 
                    path.clone(), 
                    inode_arc,
                    Some(parent),
                );
            let dirent_arc: Arc<dyn Dentry> = Arc::new(new_dirent);
            let mut meta_lock = self.meta.inner.lock();
            meta_lock.d_child.insert(name.to_string(), dirent_arc.clone());
            DENTRY_CACHE.lock().insert(path, dirent_arc.clone());
            return Some(dirent_arc);
        }
    }

    // Function: 在self（旧dentry）下，让new_dentry也链接到self中的Inode
    // TODO: 暂时还没有进行测试，不知道这样子的语义是否正确
    fn link(&self, parent: Arc<dyn Dentry>, new_name: &str) -> OSResult<()> {
        // 1. 更新link count 并得到 inode引用
        let meta_lock = self.metadata().inner.lock();
        let inode = Arc::clone(&meta_lock.d_inode);
        inode.metadata().inner.lock().i_link_count += 1;
        drop(meta_lock);
        // 2. 创建新的 new_dentry,处理好与parent的关系，放入缓存
        let fa_path = parent.metadata().inner.lock().d_path.clone();
        let path = cwd_and_name(&new_name, &fa_path);
        let new_dirent = Ext4Dentry::new(
            String::from(new_name),
            path.clone(), 
            inode,
            Some(parent.clone()),
        );
        let dirent_arc: Arc<dyn Dentry> = Arc::new(new_dirent);
        parent.metadata().inner.lock().d_child.insert(
            new_name.to_string(), 
            dirent_arc.clone()
        );
        DENTRY_CACHE.lock().insert(path.clone(), dirent_arc);
        // 3. 在硬盘上修改数据
        let old_path = self.path();
        lwext4_link(&old_path, &path).map_err(Errno::from_i32)?;
        Ok(())
    }

    // Limitation: 得确保同一个文件系统
    // Function: 将原dentry从一个目录树上拿走放到另一个目录树上，inode不变动
    // Description: parent目录是一定存在的，self指的是旧父目录
    // TODO: 没有考虑各种 flags，没有考虑目录的rename，目前只考虑的是文件！！！
    fn rename(&self, old_name: &str, new_parent: Arc<dyn Dentry>, new_name: &str) -> OSResult<()> {
        let mut old_lock = self.metadata().inner.lock();
        let mut new_lock = new_parent.metadata().inner.lock();
        let new_path = cwd_and_name(new_name, &new_lock.d_path);
        // 1. 检查new_name是否存在
        if new_lock.d_child.contains_key(new_name) {
            let overlap_dentry = new_lock.d_child.get(new_name).unwrap().clone();
            let inode_mode = overlap_dentry.metadata().inner.lock().d_inode.metadata().i_mode;
            let is_empty = overlap_dentry.metadata().inner.lock().d_child.is_empty();
            // 如果是文件且非目录
            if inode_mode.eq(&InodeMode::Regular) {
                new_parent.unlink(overlap_dentry)?;
            } // 如果是一个空目录
            else if inode_mode.eq(&InodeMode::Directory) && is_empty {
                new_parent.unlink(overlap_dentry)?;
            }
             // 如果是目录且非空，则拒绝操作 
            else if inode_mode.eq(&InodeMode::Directory) && !is_empty {
                return Err(Errno::ENOTEMPTY);
            } else {
                todo!()
            }
        }
        let dentry = old_lock.d_child.remove(old_name).unwrap();
        let old_type = dentry.metadata().inner.lock().d_inode.metadata().i_mode;
        let old_path = cwd_and_name(old_name, &old_lock.d_path);
        drop(old_lock);
        let mut dentry_lock = dentry.metadata().inner.lock();
        // 更新 dentry的相关信息 和 子父目录的关系
        dentry_lock.d_name = new_name.to_string();
        dentry_lock.d_path = new_path.clone();
        dentry_lock.d_parent = Some(Arc::downgrade(&new_parent));
        drop(dentry_lock);
        new_lock.d_child.insert(new_name.to_string(), dentry.clone());
        drop(new_lock);
        // 更新缓存
        DENTRY_CACHE.lock().remove(&old_path);
        DENTRY_CACHE.lock().insert(new_path.clone(), dentry);
        // 更新磁盘，同时在这之前把所有的锁给释放掉
        match old_type {
            InodeMode::Regular => {
                lwext4_mvfile(&old_path, &new_path).map_err(Errno::from_i32)?;
            },
            InodeMode::Directory => {
                lwext4_mvdir(&old_path, &new_path).map_err(Errno::from_i32)?;
            },
            _ => todo!()
        }
        Ok(())
    }

    // self 是父目录的dentry
    fn symbol_link(self: Arc<Self>, name: &str, target: &str) -> OSResult<()> {
        let path = cwd_and_name(name, &self.path());
        let inode = Ext4Inode::new_link(InodeMode::Link, target.len());
        let inode_arc: Arc<dyn Inode> = Arc::new(inode);
        let new_dirent = Ext4Dentry::new(
            String::from(name),
            path.clone(), 
            inode_arc,
            Some(self.clone()),
        );
        let new_dentry_arc: Arc<dyn Dentry> = Arc::new(new_dirent);
        DENTRY_CACHE.lock().insert(path.clone(), new_dentry_arc.clone());
        self.meta.inner.lock().d_child.insert(String::from(name), new_dentry_arc.clone());
        lwext4_symlink(target, &path).map_err(Errno::from_i32)?;
        Ok(())
    }

    fn read_symlink(&self, buf: &mut [u8]) -> OSResult<()> {
        lwext4_readlink(&self.path(), buf).map_err(Errno::from_i32)?;
        Ok(())
    }
}

impl Ext4Dentry {
    pub fn new(
        name: String, 
        path: String, 
        inode: Arc<dyn Inode>, 
        parent: Option<Arc<dyn Dentry>>,
    ) -> Ext4Dentry {
        Ext4Dentry { 
            meta: DentryMeta::new(
                name,
                path, 
                inode, 
                parent,
                BTreeMap::new(),
            ), 
        }
    }

    pub fn path(&self) -> String {
        self.meta.inner.lock().d_path.clone()
    }

    fn mkdir(&self, path: &str, mode: InodeMode) -> OSResult<Arc<dyn Dentry>> {
        // let inode = Arc::clone(&self.meta.inner.lock().d_inode);
        let new_dir = Ext4Dir::create(path).map_err(Errno::from_i32)?;
        let new_inode = Ext4Inode::new_dir(mode, new_dir);
        // 构建好dirent，然后剩下的就很简单了。
        let new_inode_arc: Arc<dyn Inode> = Arc::new(new_inode);
        let new_dirent = Ext4Dentry::new(
            dentry_name(path).to_string(), 
            path.to_string(), 
            new_inode_arc,
            None,
        );
        let new_dirent_arc: Arc<Ext4Dentry> = Arc::new(new_dirent);
        Ok(new_dirent_arc)
    }

    fn mknod(&self, path: &str, mode: InodeMode, _dev_id: Option<usize>) -> OSResult<Arc<dyn Dentry>> {
        let new_file = Ext4File::open(
            path, (OpenFlags::O_RDWR | OpenFlags::O_CREAT | OpenFlags::O_TRUNC).bits() as i32
        ).map_err(Errno::from_i32)?;
        let new_inode = Ext4Inode::new_file(mode, new_file);
        let new_inode_arc: Arc<dyn Inode> = Arc::new(new_inode);
        let new_dirent = Ext4Dentry::new(
            dentry_name(path).to_string(), 
            path.to_string(), 
            new_inode_arc,
            None,
        );
        let new_dirent_arc: Arc<Ext4Dentry> = Arc::new(new_dirent);
        Ok(new_dirent_arc)
    }
}

