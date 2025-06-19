pub mod exe;
pub mod meminfo;
pub mod mounts;

use alloc::{string::{String, ToString}, sync::Arc};
use meminfo::{MemInfoDentry, MemInfoInode};
use mounts::{MountsDentry, MountsInode};

use crate::{driver::BlockDevice, syscall::{FSFlags, FSType}, utils::path::{dentry_name, parent_path}};

use super::{dentry::{Dentry, DENTRY_CACHE}, file_system::{FileSystem, FileSystemMeta}, info::InodeMode, inode::Inode, simplefs::{simple_dentry::SimpleDentry, simple_inode::SimpleInode}};

pub struct ProcFileSystem {
    meta: FileSystemMeta,
}

impl FileSystem for ProcFileSystem {
    fn metadata(&self) -> &FileSystemMeta {
        &self.meta
    }

    fn root_dentry(&self) -> Arc<dyn Dentry> {
        self.meta.root_dentry.clone()
    }

    fn init(&self) {
        let root_dentry = self.root_dentry();
        let pa_path = root_dentry.metadata().inner.lock().d_path.clone();
        // /proc/Meminfo
        let name = String::from("meminfo");
        let inode = MemInfoInode::new(InodeMode::Regular);
        let inode_arc: Arc<dyn Inode> = Arc::new(inode);
        let path = pa_path.clone() + &name;
        let dentry = MemInfoDentry::new(
            name.clone(),
            path.clone(), 
            inode_arc, 
            Some(Arc::clone(&root_dentry))
        );
        let dentry_arc: Arc<dyn Dentry> = Arc::new(dentry);
        root_dentry.metadata().inner.lock().d_child.insert(name, dentry_arc.clone());
        DENTRY_CACHE.lock().insert(path, dentry_arc);

        // /proc/mounts
        let name = String::from("mounts");
        let inode = MountsInode::new(InodeMode::Regular);
        let inode_arc: Arc<dyn Inode> = Arc::new(inode);
        let path = pa_path.clone() + &name;
        let dentry = MountsDentry::new(
            name.clone(),
            path.clone(), 
            inode_arc, 
            Some(Arc::clone(&root_dentry))
        );
        let dentry_arc: Arc<dyn Dentry> = Arc::new(dentry);
        root_dentry.metadata().inner.lock().d_child.insert(name, dentry_arc.clone());
        DENTRY_CACHE.lock().insert(path, dentry_arc);

        // /proc/exec
        // let name = String::from("meminfo");
        // let inode = MemInfoInode::new(InodeMode::Regular);
        // let inode_arc: Arc<dyn Inode> = Arc::new(inode);
        // let dentry = MemInfoDentry::new(
        //     name.clone(),
        //     pa_path + &name, 
        //     inode_arc, 
        //     Some(Arc::clone(&root_dentry))
        // );
        // let dentry_arc: Arc<dyn Dentry> = Arc::new(dentry);
        // root_dentry.metadata().inner.lock().d_child.insert(name, dentry_arc);
    }
}

impl ProcFileSystem {
    pub fn new(
        mount_point: &str,
        dev_name: &str,
        device: Option<Arc<dyn BlockDevice>>,
        flags: FSFlags,
    ) -> Self {
        let inode = SimpleInode::new(InodeMode::Directory, 0);
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
        ProcFileSystem { 
            meta: FileSystemMeta { 
                f_dev: String::from("TODO Unimplemented"), 
                f_type: FSType::ProcFs, 
                f_flags: flags, 
                root_dentry: dentry_arc, 
                root_inode: inode_arc 
            }
        }
    }
}