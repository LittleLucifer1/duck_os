use alloc::{collections::btree_map::BTreeMap, string::String, sync::{Arc, Weak}, vec::Vec};

use crate::{
    config::fs::SECTOR_SIZE, 
    fs::{
        dentry::{Dentry, DentryMeta}, 
        file::{File, FileMeta, FileMetaInner}, 
        info::{FileMode, InodeMode, OpenFlags, TimeSpec}, 
        inode::{Inode, InodeDev, InodeMeta}
    }, 
    sync::SpinLock, 
    syscall::error::{Errno, OSResult}, utils::random::RANDOM_GENERATOR
};

pub struct UrandomDentry {
    pub meta: DentryMeta,
}

impl UrandomDentry {
    pub fn new(
        name: String,
        path: String,
        inode: Arc<dyn Inode>,
        parent: Option<Arc<dyn Dentry>>
    ) -> Self {
        Self { meta: DentryMeta::new(
            name, 
            path, 
            inode, 
            parent, 
            BTreeMap::new(),
        ) }
    }
}

impl Dentry for UrandomDentry {
    fn open(&self, dentry: Arc<dyn Dentry>, _flags: OpenFlags) -> OSResult<Arc<dyn File>> {
        dentry.metadata().inner.lock().d_inode.metadata().inner.lock().i_open_count += 1;
        let file = UrandomFile::new(
            Arc::clone(&dentry), 
            Arc::downgrade(&Arc::clone(&dentry.metadata().inner.lock().d_inode))
        );
        let file_arc: Arc<UrandomFile> = Arc::new(file);
        Ok(file_arc)
    }
    
    fn create(&self, _this: Arc<dyn Dentry>, _name: &str, _mode: InodeMode) -> OSResult<Arc<dyn Dentry>> {
        Err(Errno::ENOTDIR)
    }

    fn metadata(&self) -> &DentryMeta {
        &self.meta
    }

    fn unlink(&self, _child: Arc<dyn Dentry>) -> OSResult<()> {
        Err(Errno::ENOTDIR)
    }
}

pub struct UrandomInode {
    pub meta: InodeMeta,
}

impl UrandomInode {
    pub fn new(mode: InodeMode) -> Self {
        Self { 
            meta: InodeMeta::new(
                mode, 
                0, 
                InodeDev::Todo, 
                SECTOR_SIZE, 
                TimeSpec::new(), 
                TimeSpec::new(),
                TimeSpec::new()
            )
        }
    }
}

impl Inode for UrandomInode {
    fn metadata(&self) -> &InodeMeta {
        &self.meta
    }

    fn read(&self, _offset: usize, _buf: &mut [u8]) -> OSResult<usize> {
        todo!()
    }

    fn write(&self, _offset: usize, _buf: &mut [u8]) -> OSResult<usize> {
        todo!()
    }

    fn delete_data(&self) -> OSResult<()> {
        todo!()
    }

    fn read_all(&self) -> OSResult<Vec<u8>> {
        todo!()
    }
}

pub struct UrandomFile {
    pub meta: FileMeta,
}

impl UrandomFile {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Weak<dyn Inode>) -> Self {
        Self {
            meta: FileMeta { 
                f_mode: FileMode::all(), 
                page_cache: None,
                f_dentry: Some(dentry),
                f_inode: inode,
                inner: SpinLock::new(FileMetaInner {
                    f_pos: 0,
                    dirent_index: 0,
                }),
            }
        }
    }
}

impl File for UrandomFile {
    fn metadata(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, buf: &mut [u8], _flags: OpenFlags) -> OSResult<usize> {
        let buf_len = buf.len();
        let mut offset = 0;
        loop {
            if offset >= buf_len {
                break;
            }
            let random = RANDOM_GENERATOR.lock().genrand_u32() as u32;
            let random_bytes = random.to_le_bytes();
            let chunk_size = (buf_len - offset).min(4);
            buf[offset..offset+chunk_size].copy_from_slice(&random_bytes[..chunk_size]);
            offset += chunk_size;
        }
        Ok(buf_len)
    }

    fn write(&self, buf: &[u8], _flags: OpenFlags) -> OSResult<usize> {
        Ok(buf.len())
    }
}
