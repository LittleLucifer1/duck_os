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
    syscall::error::{Errno, OSResult}
};

pub struct CpuDmaLatencyDentry {
    pub meta: DentryMeta,
}

impl CpuDmaLatencyDentry {
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

impl Dentry for CpuDmaLatencyDentry {
    fn open(&self, dentry: Arc<dyn Dentry>, _flags: OpenFlags) -> OSResult<Arc<dyn File>> {
        dentry.metadata().inner.lock().d_inode.metadata().inner.lock().i_open_count += 1;
        let file = CpuDmaLatencyFile::new(
            Arc::clone(&dentry), 
            Arc::downgrade(&Arc::clone(&dentry.metadata().inner.lock().d_inode))
        );
        let file_arc: Arc<CpuDmaLatencyFile> = Arc::new(file);
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

pub struct CpuDmaLatencyInode {
    pub meta: InodeMeta,
}

impl CpuDmaLatencyInode {
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

impl Inode for CpuDmaLatencyInode {
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

pub struct CpuDmaLatencyFile {
    pub meta: FileMeta,
}

impl CpuDmaLatencyFile {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Weak<dyn Inode>) -> Self {
        Self {
            meta: FileMeta { 
                f_mode: FileMode::empty(), 
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

impl File for CpuDmaLatencyFile {
    fn metadata(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, buf: &mut [u8], _flags: OpenFlags) -> OSResult<usize> {
        buf.fill(0);
        Ok(buf.len())
    }

    fn write(&self, buf: &[u8], _flags: OpenFlags) -> OSResult<usize> {
        Ok(buf.len())
    }
}