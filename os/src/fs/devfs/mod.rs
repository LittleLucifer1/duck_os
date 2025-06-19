use alloc::{string::{String, ToString}, sync::Arc};
use null::{NullDentry, NullInode};
use rtc::{RtcDentry, RtcInode};
use tty::{TtyDentry, TtyInode};
use urandom::{UrandomDentry, UrandomInode};
use zero::{ZeroDentry, ZeroInode};

use crate::{driver::BlockDevice, syscall::{FSFlags, FSType}, utils::path::{dentry_name, parent_path}};

use super::{dentry::{Dentry, DENTRY_CACHE}, file_system::{FileSystem, FileSystemMeta}, info::InodeMode, inode::Inode, simplefs::{simple_dentry::SimpleDentry, simple_inode::SimpleInode}};

pub mod null;
pub mod zero;
pub mod urandom;
pub mod tty;
pub mod rtc;
pub mod cpu_dma_lantency;


pub struct DevFileSystem {
    meta: FileSystemMeta,
}

impl FileSystem for DevFileSystem {
    fn metadata(&self) -> &FileSystemMeta {
        &self.meta
    }

    fn root_dentry(&self) -> Arc<dyn Dentry> {
        self.meta.root_dentry.clone()
    }

    fn init(&self) {
        let root_dentry = self.root_dentry();
        let pa_path = &root_dentry.metadata().inner.lock().d_path;
        // /dev/zero
        let name = String::from("zero");
        let inode = ZeroInode::new(InodeMode::Char);
        let inode_arc: Arc<dyn Inode> = Arc::new(inode);
        let dentry = ZeroDentry::new(
            name.clone(),
            pa_path.clone() + &name, 
            inode_arc, 
            Some(Arc::clone(&root_dentry))
        );
        let dentry_arc: Arc<dyn Dentry> = Arc::new(dentry);
        root_dentry.metadata().inner.lock().d_child.insert(name, dentry_arc);
        
        // /dev/null
        let name = String::from("null");
        let inode = NullInode::new(InodeMode::Char);
        let inode_arc: Arc<dyn Inode> = Arc::new(inode);
        let dentry = NullDentry::new(
            name.clone(),
            pa_path.clone() + &name, 
            inode_arc, 
            Some(Arc::clone(&root_dentry))
        );
        let dentry_arc: Arc<dyn Dentry> = Arc::new(dentry);
        root_dentry.metadata().inner.lock().d_child.insert(name, dentry_arc);
        
        // /dev/rtc
        let name = String::from("rtc");
        let inode = RtcInode::new(InodeMode::Regular);
        let inode_arc: Arc<dyn Inode> = Arc::new(inode);
        let dentry = RtcDentry::new(
            name.clone(),
            pa_path.clone() + &name, 
            inode_arc, 
            Some(Arc::clone(&root_dentry))
        );
        let dentry_arc: Arc<dyn Dentry> = Arc::new(dentry);
        root_dentry.metadata().inner.lock().d_child.insert(name, dentry_arc);
        
        // /dev/cpu_dma_latency
        let name = String::from("cpu_pma_latency");
        let inode = UrandomInode::new(InodeMode::Char);
        let inode_arc: Arc<dyn Inode> = Arc::new(inode);
        let dentry = UrandomDentry::new(
            name.clone(),
            pa_path.clone() + &name, 
            inode_arc, 
            Some(Arc::clone(&root_dentry))
        );
        let dentry_arc: Arc<dyn Dentry> = Arc::new(dentry);
        root_dentry.metadata().inner.lock().d_child.insert(name, dentry_arc);

        // /dev/urandom
        let name = String::from("urandom");
        let inode = UrandomInode::new(InodeMode::Char);
        let inode_arc: Arc<dyn Inode> = Arc::new(inode);
        let dentry = UrandomDentry::new(
            name.clone(),
            pa_path.clone() + &name, 
            inode_arc, 
            Some(Arc::clone(&root_dentry))
        );
        let dentry_arc: Arc<dyn Dentry> = Arc::new(dentry);
        root_dentry.metadata().inner.lock().d_child.insert(name, dentry_arc);
        
        // /dev/tty
        let name = String::from("tty");
        let inode = TtyInode::new(InodeMode::Char);
        let inode_arc: Arc<dyn Inode> = Arc::new(inode);
        let dentry = TtyDentry::new(
            name.clone(),
            pa_path.clone() + &name, 
            inode_arc, 
            Some(Arc::clone(&root_dentry))
        );
        let dentry_arc: Arc<dyn Dentry> = Arc::new(dentry);
        root_dentry.metadata().inner.lock().d_child.insert(name, dentry_arc);
    }
}

impl DevFileSystem {
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
                f_type: FSType::DevFs, 
                f_flags: flags, 
                root_dentry: dentry_arc, 
                root_inode: inode_arc 
            }
        }
    }
}