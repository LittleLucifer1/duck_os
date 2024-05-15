//! 简化组合版 superblock + vfsmount


/*
    1.数据结构
        1）dev: 设备标识符
        2) type: 文件系统类型
        3) flags: 挂载标志
        4) root: 目录挂载点（Dentry）
        5) inode: 文件系统根inode
        6) dirty: 回写链表（待定）
        7) mnt_parent: 父文件系统（待定）

    2. 功能
        1）得到根 inode

    3. 一个全局的管理器
        负责挂载和解挂载，同时负责找到根文件系统
        2) mount unmount
*/

use alloc::{collections::BTreeMap, string::{String, ToString}, sync::Arc};
use crate::{fs::dentry::DENTRY_CACHE, sync::SpinLock, syscall::{error::{Errno, OSResult}, FSFlags, FSType}, utils::path::{dentry_name, parent_path}};

use crate::driver::BlockDevice;

use super::{dentry::{path_to_dentry, Dentry}, fat32::fat_fs::Fat32FileSystem, inode::Inode};


pub struct FileSystemMeta {
    pub f_dev: String,
    pub f_type: FSType,
    pub f_flags: FSFlags,
    pub root_dentry: Arc<dyn Dentry>,
    pub root_inode: Arc<dyn Inode>,
    /*
    pub mnt_parent: Option<Arc<dyn FileSystem>>,
    pub is_root_mnt: bool,
    pub dirty_inode: Vec<Inode>,
     */
}

#[derive(Default)]
pub struct EmptyFileSystem;

impl EmptyFileSystem {
    pub fn new() -> Arc<dyn FileSystem> {
        Arc::new(Self::default())
    }
}

impl FileSystem for EmptyFileSystem {
    fn metadata(&self) -> &FileSystemMeta {
        todo!()
    }
    fn root_dentry(&self) -> Arc<dyn Dentry> {
        todo!()
    }
}

pub trait FileSystem: Send + Sync {
    fn root_dentry(&self) -> Arc<dyn Dentry>;
    fn metadata(&self) -> &FileSystemMeta;
}

pub struct FileSystemManager {
    // (mounting point name, FileSystem)
    // 可以换成 hashmap
    pub manager: SpinLock<BTreeMap<String, Arc<dyn FileSystem>>>,
}

impl FileSystemManager {
    pub fn new() -> FileSystemManager {
        FileSystemManager { 
            manager: SpinLock::new(BTreeMap::new()), 
        }
    }

    // 返回根文件系统的引用
    pub fn root_fs(&self) -> Arc<dyn FileSystem> {
        self.manager.lock().get("/").unwrap().clone()
    }

    pub fn root_dentry(&self) -> Arc<dyn Dentry> {
        self.manager.lock().get("/").unwrap().root_dentry()
    }

    pub fn mount(
        &self,
        mount_point: &str,
        dev_name: &str,
        device: Option<Arc<dyn BlockDevice>>,
        fs_type: FSType,
        flags: FSFlags,
    ) {
        if device.is_none() {
            FILE_SYSTEM_MANAGER.manager.lock().insert(
                mount_point.to_string(),
                EmptyFileSystem::new(),
            );
            return;
        }

        let fs: Arc<dyn FileSystem> = match fs_type {
            FSType::VFAT => {
                Arc::new(Fat32FileSystem::new(
                    mount_point, 
                    dev_name, 
                    Arc::clone(&device.unwrap()),
                    flags,
                ))
            }
            _ => {
                todo!()
            }
        };
        // DENTRY_CACHE.lock().insert(
        //     mount_point.to_string(), 
        //     fs.metadata().root_dentry.clone()
        // );
        FILE_SYSTEM_MANAGER.manager.lock().insert(
            mount_point.to_string(),
            Arc::clone(&fs),
        );
    }

    // 找到fs，和fs中的meta, 移除inode_cache, fs_manager中的数据。
    // TODO: 这里可能需要 sync 同步相关的数据
    pub fn unmount(&self, mount_point: &str) -> OSResult<usize> {
        let mut fs_manager = FILE_SYSTEM_MANAGER.manager.lock();
        let fs_op = fs_manager.get(mount_point);
        if fs_op.is_none() {
            return Err(Errno::ENOENT);   
        }
        let pa_path = parent_path(mount_point);
        let name = dentry_name(mount_point);
        match path_to_dentry(&pa_path) {
            Some(dentry) => {
                dentry.metadata().inner.lock().d_child.remove(name);
            }
            None => {},
        };
        #[cfg(feature = "preliminary")] 
        if mount_point != "/mnt" {
            DENTRY_CACHE.lock().remove(mount_point);
        }
        #[cfg(not(feature = "preliminary"))]
        DENTRY_CACHE.lock().remove(mount_point);
        fs_manager.remove(mount_point);
        Ok(0)
    }

}

lazy_static::lazy_static! {
    pub static ref FILE_SYSTEM_MANAGER: FileSystemManager = FileSystemManager::new(); 
}
