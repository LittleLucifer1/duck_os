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

pub struct MountsDentry {
    pub meta: DentryMeta,
}

impl MountsDentry {
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

impl Dentry for MountsDentry {
    fn open(&self, dentry: Arc<dyn Dentry>, _flags: OpenFlags) -> OSResult<Arc<dyn File>> {
        dentry.metadata().inner.lock().d_inode.metadata().inner.lock().i_open_count += 1;
        let file = MountsFile::new(
            Arc::clone(&dentry), 
            Arc::downgrade(&Arc::clone(&dentry.metadata().inner.lock().d_inode))
        );
        let file_arc: Arc<MountsFile> = Arc::new(file);
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

pub struct MountsInode {
    pub meta: InodeMeta,
}

impl MountsInode {
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

impl Inode for MountsInode {
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

pub struct MountsFile {
    pub meta: FileMeta,
}

impl MountsFile {
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

impl File for MountsFile {
    fn metadata(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, buf: &mut [u8], _flags: OpenFlags) -> OSResult<usize> {
        // TODO: 还没完成！同时根据具体的例子来判断这里需不需要更新 file_pos? 难道不可以一口气全读完吗
        let message = "TODO: Not implemented!";
        let len = message.len();
        let mut file_pos = self.meta.inner.lock().f_pos;
        if file_pos >= len { 
            // file_pos记录着此时文件所读取的位置，如果超过了len，说明没有可以读的了
            return Ok(0);
        }
        buf[..len].copy_from_slice(message.as_bytes());
        file_pos += len;
        Ok(len)
    }

    fn write(&self, _buf: &[u8], _flags: OpenFlags) -> OSResult<usize> {
        Err(Errno::EACCES)
    }
}