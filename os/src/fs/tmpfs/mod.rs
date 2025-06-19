use alloc::{string::{String, ToString}, sync::Arc};

use crate::{
    driver::BlockDevice, 
    syscall::{FSFlags, FSType}, 
    utils::path::{dentry_name, parent_path}
};

use super::{
    dentry::{Dentry, DENTRY_CACHE}, 
    file_system::{FileSystem, FileSystemMeta}, 
    info::InodeMode, inode::Inode, 
    simplefs::{simple_dentry::SimpleDentry, simple_inode::SimpleInode}
};

pub struct TmpFileSystem {
    meta: FileSystemMeta,
}

impl FileSystem for TmpFileSystem {
    fn metadata(&self) -> &FileSystemMeta {
        &self.meta
    }

    fn root_dentry(&self) -> Arc<dyn Dentry> {
        self.meta.root_dentry.clone()
    }
}

impl TmpFileSystem {
    pub fn new(
        mount_point: &str,
        dev_name: &str,
        device: Option<Arc<dyn BlockDevice>>,
        flags: FSFlags,
    ) -> Self {
        let inode = SimpleInode::new(InodeMode::Directory);
        let inode_arc:Arc<dyn Inode> = Arc::new(inode);
        let name = dentry_name(mount_point);
        
        let mut parent: Option<Arc<dyn Dentry>> = None;
        if mount_point != "/" {
            let fa_path = parent_path(mount_point);
            parent = Some(DENTRY_CACHE.lock().get(&fa_path).unwrap().clone());
        } 
        let dentry = SimpleDentry::new(
            name.to_string(),
            mount_point.to_string(), 
            Arc::clone(&inode_arc), 
            parent.clone()
        );
        dentry.metadata().inner.lock().d_inode = Arc::clone(&inode_arc);
        let dentry_arc: Arc<dyn Dentry> = Arc::new(dentry);
        if let Some(parent) = parent {
            parent.metadata().inner.lock().d_child.insert(name.to_string(), Arc::clone(&dentry_arc));
        }
        Self { 
            meta: FileSystemMeta { 
                f_dev: String::from("TODO Unimplemented"), 
                f_type: FSType::TmpFs, 
                f_flags: flags, 
                root_dentry: dentry_arc, 
                root_inode: inode_arc 
            }
        }
    }
}