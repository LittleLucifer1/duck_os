use alloc::{string::ToString, sync::Arc};
use log::info;
use lwext4_rust::{Ext4BlockWrapper, Ext4Dir};

use crate::{driver::BlockDevice, fs::{dentry::{path_to_dentry, Dentry, DENTRY_CACHE}, file_system::{FileSystem, FileSystemMeta}, info::InodeMode, inode::Inode}, syscall::{error::{Errno, OSResult}, FSFlags, FSType}, utils::path::{dentry_name, parent_path}};

use super::{disk::Disk, ext4_dentry::Ext4Dentry, ext4_inode::Ext4Inode};

pub struct Ext4FileSystem{
    pub meta: FileSystemMeta,
    pub disk: Ext4BlockWrapper<Disk>,
}
unsafe impl Send for Ext4FileSystem {}
unsafe impl Sync for Ext4FileSystem {}

impl FileSystem for Ext4FileSystem {
    fn metadata(&self) -> &FileSystemMeta {
        &self.meta
    }

    fn root_dentry(&self) -> alloc::sync::Arc<dyn crate::fs::dentry::Dentry> {
        self.meta.root_dentry.clone()
    }
}

// TODO:这里可能要处理flush问题
impl Drop for Ext4FileSystem {
    fn drop(&mut self) {
        // todo!()
    }
}

impl Ext4FileSystem {
    pub fn new(
        mount_point: &str, // 挂载的实际路径位置
        dev_name: &str,
        dev: Option<Arc<dyn BlockDevice>>,
        flags: FSFlags,
    ) -> OSResult<Self> {
        // 1. 创建root_inode，root_dentry
        // 3. 将所有的dentry给它拿出来，构造成一棵树
        // 4. 将构造好的fs给它返回出去
        let disk = Disk::new(dev.unwrap().clone());
        let disk_wrapper = Ext4BlockWrapper::<Disk>::new(disk)
        .expect("Fail to Initialize EXT4 FileSystem");

        let name = dentry_name(mount_point);
        let root_dir = Ext4Dir::open(mount_point).map_err(Errno::from_i32)?;
        let root_inode = Ext4Inode::new_dir(InodeMode::Directory, root_dir);
        let root_inode_arc: Arc<dyn Inode> = Arc::new(root_inode);
        let mut parent: Option<Arc<dyn Dentry>> = None;
        if mount_point != "/" {
            let fa_path = parent_path(mount_point);
            parent = path_to_dentry(&fa_path)?;
        }
        let root_dentry = Ext4Dentry::new(
            name.to_string(),
            mount_point.to_string(), 
            Arc::clone(&root_inode_arc),
            parent.clone(), 
        );
        let root_dirent_arc: Arc<dyn Dentry> = Arc::new(root_dentry);
        if let Some(parent) = parent {
            parent.metadata().inner.lock().d_child.insert(name.to_string(), root_dirent_arc.clone());
        }
        root_dirent_arc.load_all_child(root_dirent_arc.clone())?;
        Ok(Self { 
            meta: FileSystemMeta { 
                f_dev: dev_name.to_string(),
                f_type:  FSType::EXT4,
                f_flags: flags,
                root_dentry: root_dirent_arc,
                root_inode: root_inode_arc,
            }, 
            disk: disk_wrapper,
        })
    }
}

