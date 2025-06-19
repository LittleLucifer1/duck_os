use alloc::vec::Vec;

use crate::{fs::{info::{InodeMode, TimeSpec}, inode::{Inode, InodeDev, InodeMeta}}, syscall::error::OSResult};

pub struct SimpleInode {
    pub meta: InodeMeta,
}

impl Inode for SimpleInode {
    fn metadata(&self) -> &InodeMeta {
        &self.meta
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> OSResult<usize> {
        todo!()
    }

    fn write(&self, offset: usize, buf: &mut [u8]) -> OSResult<usize> {
        todo!()
    }

    fn delete_data(&self) -> OSResult<()> {
        todo!()
    }

    fn read_all(&self) -> OSResult<Vec<u8>> {
        todo!()
    }
}


impl SimpleInode {
    pub fn new(mode: InodeMode) -> Self {
        Self {
            meta: InodeMeta::new(
                mode, 
                0, 
                InodeDev::Todo, 
                0, 
                TimeSpec::new(), 
                TimeSpec::new(),
                TimeSpec::new(),
            ),
        }
    }

    pub fn set_size(&mut self, size: usize) {
        self.meta.inner.lock().i_size = size;
    }
}
