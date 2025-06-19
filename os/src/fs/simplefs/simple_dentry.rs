use alloc::{collections::btree_map::BTreeMap, string::{String, ToString}, sync::Arc};
use log::info;

use crate::{
    fs::{
        dentry::{Dentry, DentryMeta, DENTRY_CACHE}, 
        file::{File, FileMeta, FileMetaInner}, 
        info::{InodeMode, OpenFlags}, 
        inode::Inode, page_cache::PageCache
    }, 
    sync::SpinLock, syscall::error::OSResult, 
    utils::path::{cwd_and_name, dentry_name}
};

use super::{simple_file::SimpleFile, simple_inode::SimpleInode};

pub struct SimpleDentry {
    pub meta: DentryMeta
}

impl SimpleDentry {
    pub fn new(
        name: String,
        path: String,
        inode: Arc<dyn Inode>,
        parent: Option<Arc<dyn Dentry>>,
    ) -> SimpleDentry {
        SimpleDentry {
            meta: DentryMeta::new(
                name, 
                path, 
                inode, 
                parent,
                BTreeMap::new(),
            )
        }
    }

    pub fn path(&self) -> String {
        self.meta.inner.lock().d_path.clone()
    }

    fn mkdir(&self, path: &str, mode: InodeMode) -> OSResult<Arc<dyn Dentry>> {
        let new_inode = SimpleInode::new(mode, 0);
        let new_inode_arc: Arc<dyn Inode> = Arc::new(new_inode);
        let new_dirent = SimpleDentry::new(
            dentry_name(path).to_string(), 
            path.to_string(), 
            new_inode_arc,
            None,
        );
        
        let new_dirent_arc: Arc<SimpleDentry> = Arc::new(new_dirent);
        Ok(new_dirent_arc)
    }

    fn mknod(&self, path: &str, mode: InodeMode, _dev_id: Option<usize>) -> OSResult<Arc<dyn Dentry>> {
        let new_inode = SimpleInode::new(mode, 0);
        let new_inode_arc: Arc<dyn Inode> = Arc::new(new_inode);
        let new_dirent = SimpleDentry::new(
            dentry_name(path).to_string(), 
            path.to_string(), 
            new_inode_arc,
            None,
        );
        
        let new_dirent_arc: Arc<SimpleDentry> = Arc::new(new_dirent);
        Ok(new_dirent_arc)
    }

}

impl Dentry for SimpleDentry {
    fn metadata(&self) -> &DentryMeta {
        &self.meta
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
        let file = SimpleFile::new(file_meta);
        if flags.contains(OpenFlags::O_TRUNC) {
            file.truncate(0)?;   
        }
        if flags.contains(OpenFlags::O_APPEND) {
            file.seek(crate::fs::file::SeekFrom::End(0))?;
        }
        Ok(Arc::new(file))
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
            }
            InodeMode::Directory => {
                info!("[sys_unlink] Unlink Directory.");
            }
            // TODO：unimplemented !
            InodeMode::Link => {
                todo!()
            }
            _ => todo!()
        }
        Ok(())
    }

    // TODO：重构，小心这里的语义。其他操作系统的实现是找文件的时候再去磁盘中查询
    // 而我是一开始就把所有的文件全部加载进来。但是对于没有磁盘文件系统的simple_fs，
    // 我不知道如何去加载所有的文件。关键是不知道这里的语义！！！！！！
    // DOWN: 对于这种fs来说，在初始化的时候就会创建好该有的dentry,所以不需要load
    fn load_all_child(&self, this: Arc<dyn Dentry>) -> OSResult<()> {
        todo!()
    }

    fn load_child(&self, this: Arc<dyn Dentry>) -> OSResult<()> {
        todo!()
    }

    fn look_up(self:Arc<Self>, name: &str) -> Option<Arc<dyn Dentry>> {
        let meta_lock = self.meta.inner.lock();
        let child = meta_lock.d_child.get(name);
        // 1.在缓存中
        if let Some(child) = child {
            Some(Arc::clone(child))
        } else {
            return None;
        }
    }

    fn link(&self, parent: Arc<dyn Dentry>, new_name: &str) -> OSResult<()> {
        // 1. 更新link count 并得到 inode引用
        let meta_lock = self.metadata().inner.lock();
        let inode = Arc::clone(&meta_lock.d_inode);
        inode.metadata().inner.lock().i_link_count += 1;
        drop(meta_lock);
        // 2. 创建新的 new_dentry,处理好与parent的关系，放入缓存
        let fa_path = parent.metadata().inner.lock().d_path.clone();
        let path = cwd_and_name(&new_name, &fa_path);
        let new_dirent = SimpleDentry::new(
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
        Ok(())
    }
}
