//! fat32文件系统对 VFS Dentry 的具体实现
//! 
use core::fmt::Debug;

use alloc::{collections::BTreeMap, string::{String, ToString}, sync::Arc, vec::Vec};

use crate::{config::fs::ROOT_CLUSTER_NUM, fs::{dentry::{Dentry, DentryMeta, DentryMetaInner, DENTRY_CACHE}, file::{File, FileMeta, FileMetaInner, SeekFrom}, info::{InodeMode, OpenFlags}, inode::Inode, page_cache::PageCache}, sync::SpinLock, syscall::error::OSResult, utils::path::{cwd_and_name, dentry_name}};

use super::{block_cache::get_block_cache, data::{parse_child, DirEntry}, fat::{find_all_cluster, FatInfo}, fat_file::FatMemFile, fat_inode::{FatInode, NxtFreePos, NXTFREEPOS_CACHE}, utility::cluster_to_sector, DirEntryStatus};

// 目录项的位置信息 （自身的cluster——通常在父目录中， offset——dentry的编号，内容的所在的cluster）
// 如果self_cluster == 0，说明没有父目录，即是根目录
#[derive(Debug, Clone, Copy)]
pub struct Position {
    // 对应目录项所在的位置
    pub self_cluster: usize,
    pub self_sector: usize,
    pub offset: usize,
    // 目录中的内容所在的位置
    pub data_cluster: usize,
}

impl Position {
    pub fn new_from_root() -> Self {
    /*  根目录中的信息：
        因为没有父目录，所以父目录有关的信息都为零
        根目录的data起始簇是2, 当前的空闲簇为2, 空闲的sector为0, 空闲的dentry位置为0
    */
        Self {
            self_cluster: 0,
            self_sector: 0,
            offset: 0,
            data_cluster: ROOT_CLUSTER_NUM,
        }
    }
    
    pub fn new_from_nxtpos(pos: NxtFreePos, data_clu: usize) -> Self {
        Self {
            self_cluster: pos.cluster,
            self_sector: pos.sector,
            offset: pos.offset,
            data_cluster: data_clu,
        }
    }
}

pub struct FatDentry {
    pub meta: DentryMeta,
    pub pos: Position,
    pub fat_info: Arc<FatInfo>,
}

impl Dentry for FatDentry {
    fn metadata(&self) -> &DentryMeta {
        &self.meta
    }

    // Assumption: path是合法的，format过的
    // function: 创建子inode和子目录，同时将数据都写在了磁盘上。
    // return: 子目录 Arc<dyn Dentry>
    // 在openat函数和mkdir函数中均有使用
    fn mkdir(&self, path: &str, mode: InodeMode) -> Arc<dyn Dentry> {
        let inode = Arc::clone(&self.meta.inner.lock().d_inode);
        let child_inode = FatInode::mkdir(
            Arc::clone(&inode), 
            path, 
            mode, 
            Arc::clone(&self.fat_info));
        Arc::new(FatDentry::new_from_inode(child_inode,self.fat_info.clone(), path))
    }

    // TODO: 这个函数和 mkdir 暂时不知道有什么区别，所以实现一样！
    fn mknod(&self, path: &str, mode: InodeMode, dev_id: Option<usize>) -> Arc<dyn Dentry> {
        let inode = Arc::clone(&self.meta.inner.lock().d_inode);
        let child_inode = FatInode::mknod(
            inode, 
            path, 
            mode, 
            Arc::clone(&self.fat_info),
            dev_id,
        );
        Arc::new(FatDentry::new_from_inode(child_inode,Arc::clone(&self.fat_info), path))
    }

    // Assumption: name是单个名字
    // function：此时flags == CREATE, 所以需要创建对应的文件
    // 这个函数是mkdir和mknod的结合，因为要处理关系，所以直接使用mkdir和mknod多有不便。
    // 系统调用中的openat和mkdirat都是使用着这个函数去创建文件或者目录
    fn create(&self, this: Arc<dyn Dentry>, name: &str, mode: InodeMode) -> OSResult<Arc<dyn Dentry>> {
        let child_dentry: Arc<dyn Dentry>;
        if mode.eq(&InodeMode::Directory) {
            child_dentry = self.mkdir(
                &cwd_and_name(name, &self.path()),
                InodeMode::Directory);
        } else if mode.eq(&InodeMode::Regular) {
            child_dentry = self.mknod(
                &cwd_and_name(name, &self.path()),
                InodeMode::Regular,
                None,
            );
        } else { // 其他的类型，暂时不支持！
            todo!();
        }
        self.meta.inner.lock().d_child.insert(name.to_string(), Arc::clone(&child_dentry));
        child_dentry.metadata().inner.lock().d_parent = Some(Arc::downgrade(&this));
        Ok(child_dentry)
    }

    // Assumption: name是单个名字
    // function：此时flags == CREATE, 所以需要创建对应的文件
    // TODO: 当flags == TRUNCATE时，需要修改f_pos! DONE: 就是修改file_size!
    fn open(&self, dentry: Arc<dyn Dentry>, flags: OpenFlags) -> OSResult<Arc<dyn File>> {
        let file_meta = FileMeta {
            f_mode: flags.clone().into(),
            page_cache: Some(Arc::new(PageCache::new())),
            f_dentry: Arc::clone(&dentry),
            f_inode: Arc::downgrade(&Arc::clone(&dentry.metadata().inner.lock().d_inode)),
            inner: SpinLock::new(FileMetaInner {
                f_pos: 0,
                dirent_index: 0,
            })
        };
        let file = FatMemFile::init(file_meta);
        if flags.contains(OpenFlags::O_TRUNC) {
            file.truncate(0)?;   
        }
        if flags.contains(OpenFlags::O_APPEND) {
            file.seek(SeekFrom::End(0))?;
        }
        Ok(Arc::new(file))
    }

    fn load_child(&self, this: Arc<dyn Dentry>) {
        // 1. 找目录中所有的数据cluster
        let mut nxt_free_pos = NxtFreePos::empty();
        let dev = Arc::clone(self.fat_info.dev.as_ref().expect("Block device is None"));
        let clusters: Vec<usize> = find_all_cluster(self.fat_info.clone(), self.data_cluster());
        // 2. 分别从其中的cluster读出所需要的数据，记录direntry的数据以及位置
        let mut dir_pos: Vec<(DirEntry, Position)> = Vec::new();
        'outer: for current_cluster in clusters.iter() {
            let start_sector = cluster_to_sector(self.fat_info.clone(), *current_cluster);
            for sec_id in start_sector..start_sector + self.fat_info.sec_per_clus {
                for num in 0..16usize {
                    let dir = get_block_cache(sec_id, dev.clone())
                        .lock()
                        .read(num * core::mem::size_of::<DirEntry>(), |dir: &DirEntry| {
                            *dir
                        });
                    // TODO：direntry的类型暂时只考虑四种
                    if dir.status() == DirEntryStatus::Empty {
                        // 记录该目录在磁盘中可以写入的空entry的位置
                        nxt_free_pos.update(*current_cluster, sec_id, num * core::mem::size_of::<DirEntry>());
                        break 'outer;
                    } else if dir.status() == DirEntryStatus::Free {
                        continue;
                    } else {
                        let pos = Position {
                            self_cluster: *current_cluster,
                            self_sector: sec_id,
                            offset: num * core::mem::size_of::<DirEntry>(),
                            data_cluster: 0,
                        };
                        dir_pos.push((dir, pos));
                    }
                }
            }
        }
        // 将目录中空闲的下一位置给缓存起来
        let ino = this.metadata().inner.lock().d_inode.metadata().i_ino;
        NXTFREEPOS_CACHE.0.lock().as_mut().unwrap().insert(ino, nxt_free_pos);
        // 3. 解析相关数据并转换为inode和dentry
        let childs = parse_child(&dir_pos, self.fat_info.clone());
        for child in childs.into_iter() {
            let name = child.meta.inner.lock().d_name.clone();
            let cwd = self.meta.inner.lock().d_path.clone();
            let path = cwd_and_name(&name, &cwd);
            child.meta.inner.lock().d_path = path.clone();
            // 维护好关系
            child.meta.inner.lock().d_parent = Some(Arc::downgrade(&this));
            let child_rc: Arc<dyn Dentry> = Arc::new(child);
            DENTRY_CACHE.lock().insert(path, Arc::clone(&child_rc));
            self.meta.inner.lock().d_child.insert(name, Arc::clone(&child_rc));
        }
    }
    
    // TODO: 不确定这种写法能不能正确的运行???? 如果不行,则要替换成每次只load一层.
    fn load_all_child(&self, this: Arc<dyn Dentry>) {
        let fa = this.clone();
        if fa.metadata()
            .inner
            .lock()
            .d_inode
            .metadata().i_mode != InodeMode::Directory {
            return;
        }
        fa.load_child(fa.clone());
        for (_, child) in &fa.metadata().inner.lock().d_child {
            child.load_all_child(Arc::clone(child));
        }
    }

    // 这个函数的功能暂时很简单，就是删除cache中的数据，同时删除关系和磁盘上的数据（其实不用删除？？）
    fn unlink(&self, child: Arc<dyn Dentry>) {
        let child_name = child.metadata().inner.lock().d_name.clone();
        DENTRY_CACHE.lock().remove(&child.metadata().inner.lock().d_path);
        child.metadata().inner.lock().d_inode.delete_data();
        self.meta.inner.lock().d_child.remove(&child_name);
    }
}

impl FatDentry {
    pub fn new_from_root(fat_info: Arc<FatInfo>, mount_point: &str, inode: Arc<dyn Inode> ) -> Self {
        Self {
            meta: DentryMeta {
                inner: SpinLock::new(DentryMetaInner {
                    d_name: mount_point.to_string(),
                    d_path: mount_point.to_string(),
                    d_inode: inode,
                    d_parent: None,
                    d_child: BTreeMap::new(),
                })
            },
            pos: Position { 
                /*  根目录中的信息：
                    因为没有父目录，所以父目录有关的信息都为零
                    根目录的data起始簇是2, 当前的空闲簇为2, 空闲的sector为0, 空闲的dentry位置为0
                */
                self_cluster: 0, 
                self_sector: 0, 
                offset: 0, 
                data_cluster: ROOT_CLUSTER_NUM, 
            },
            fat_info,
        }
    }

    fn data_cluster(&self) -> usize {
        self.pos.data_cluster
    }

    pub fn new_from_inode(inode: FatInode, fat_info: Arc<FatInfo>, path: &str) -> Self {
        let pos = inode.pos;
        Self {
            meta: DentryMeta {
                inner: SpinLock::new(DentryMetaInner {
                    d_name: dentry_name(path).to_string(),
                    d_path: path.to_string(),
                    d_inode: Arc::new(inode),
                    d_parent: None,
                    d_child: BTreeMap::new(),
                })
            },
            pos,
            fat_info,
        }
    }

    pub fn path(&self) -> String {
        self.meta.inner.lock().d_path.clone()
    }
}


    
