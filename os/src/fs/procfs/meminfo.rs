use alloc::{collections::btree_map::BTreeMap, string::{String, ToString}, sync::{Arc, Weak}, vec::Vec};
use lazy_static::lazy_static;

use crate::{
    config::{
        fs::SECTOR_SIZE, 
        mm::{
            AVAIL_MEM_SIZE, BUFFER_SIZE, CACHE_SIZE, FREE_MEM_SIZE, FREE_SWAP_SIZE, SHARED_MEMORY_SIZE, SLAB_SIZE, TOTAL_MEM_SIZE, TOTAL_SWAP_SIZE
        }
    }, 
    fs::{
        dentry::{Dentry, DentryMeta}, 
        file::{File, FileMeta, FileMetaInner}, 
        info::{FileMode, InodeMode, OpenFlags, TimeSpec}, 
        inode::{Inode, InodeDev, InodeMeta}
    }, 
    sync::{SpinLock, SpinNoIrqLock}, 
    syscall::error::{Errno, OSResult}
};

pub struct MemInfoDentry {
    pub meta: DentryMeta,
}

impl MemInfoDentry {
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

impl Dentry for MemInfoDentry {
    fn open(&self, dentry: Arc<dyn Dentry>, _flags: OpenFlags) -> OSResult<Arc<dyn File>> {
        dentry.metadata().inner.lock().d_inode.metadata().inner.lock().i_open_count += 1;
        let file = MemInfoFile::new(
            Arc::clone(&dentry), 
            Arc::downgrade(&Arc::clone(&dentry.metadata().inner.lock().d_inode))
        );
        let file_arc: Arc<MemInfoFile> = Arc::new(file);
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

pub struct MemInfoInode {
    pub meta: InodeMeta,
}

impl MemInfoInode {
    pub fn new(mode: InodeMode) -> Self {
        let size = MEM_INFO.lock().serialize().len();
        Self { 
            meta: InodeMeta::new(
                mode, 
                0, 
                InodeDev::Todo, 
                size,
                TimeSpec::new(), 
                TimeSpec::new(),
                TimeSpec::new()
            )
        }
    }
}

impl Inode for MemInfoInode {
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

pub struct MemInfoFile {
    pub meta: FileMeta,
}

impl MemInfoFile {
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

impl File for MemInfoFile {
    fn metadata(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, buf: &mut [u8], _flags: OpenFlags) -> OSResult<usize> {
        // TODO: 这里需要判断这个 file_offset 吗？？
        let message = MEM_INFO.lock().serialize();
        let mut file_lock = self.meta.inner.lock();
        let file_offset = file_lock.f_pos;
        let len = (message.len() - file_offset).min(buf.len());
        buf[..len].copy_from_slice(&message.as_bytes()[file_offset..file_offset+len]);
        file_lock.f_pos = file_offset + len;
        Ok(len)
    }

    fn write(&self, _buf: &[u8], _flags: OpenFlags) -> OSResult<usize> {
        Err(Errno::EACCES)
    }
}

// Reference: https://access.redhat.com/solutions/406773.
pub struct MemInfo {
    // memory
    pub total_mem: usize,
    pub free_mem: usize,
    pub avail_mem: usize,
    // buffer and cache
    pub buffer: usize,
    pub cache: usize,
    // swap space
    pub total_swap: usize,
    pub free_swap: usize,
    // shared memory
    pub shmem: usize,
    pub slab: usize,
}

impl MemInfo {
    pub const fn new() -> Self {
        Self { total_mem: TOTAL_MEM_SIZE,
            free_mem: FREE_MEM_SIZE, 
            avail_mem: AVAIL_MEM_SIZE, 
            buffer: BUFFER_SIZE, 
            cache: CACHE_SIZE, 
            total_swap: TOTAL_SWAP_SIZE, 
            free_swap: FREE_SWAP_SIZE, 
            shmem: SHARED_MEMORY_SIZE, 
            slab: SLAB_SIZE, 
        }
    }

    pub fn serialize(&self) -> String {
        let mut message = String::new();
        let end = " KB\n";
        let total_mem = "MemTotal:\t".to_string() + self.total_mem.to_string().as_str() + end;
        let free_mem = "MemFree:\t".to_string() + self.free_mem.to_string().as_str() + end;
        let avail_mem = "MemAvailable:\t".to_string() + self.avail_mem.to_string().as_str() + end;
        let buffers = "Buffers:\t".to_string() + self.buffer.to_string().as_str() + end;
        let cached = "Cached:\t".to_string() + self.cache.to_string().as_str() + end;
        let cached_swap = "SwapCached:\t".to_string() + 0.to_string().as_str() + end;
        let total_swap = "SwapTotal:\t".to_string() + self.total_swap.to_string().as_str() + end;
        let free_swap = "SwapFree:\t".to_string() + self.free_swap.to_string().as_str() + end;
        let shmem = "Shmem:\t".to_string() + self.shmem.to_string().as_str() + end;
        let slab = "Slab:\t".to_string() + self.slab.to_string().as_str() + end;
        message += total_mem.as_str();
        message += free_mem.as_str();
        message += avail_mem.as_str();
        message += buffers.as_str();
        message += cached.as_str();
        message += cached_swap.as_str();
        message += total_swap.as_str();
        message += free_swap.as_str();
        message += shmem.as_str();
        message += slab.as_str();
        message
    }
}

lazy_static! {
    pub static ref MEM_INFO: SpinNoIrqLock<MemInfo> = SpinNoIrqLock::new(MemInfo::new());
}