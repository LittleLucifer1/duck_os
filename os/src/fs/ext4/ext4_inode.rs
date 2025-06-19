use alloc::vec;
use alloc::vec::Vec;
use log::info;
use lwext4_rust::{bindings::SEEK_SET, Ext4Dir, Ext4File};

use crate::{fs::{info::{InodeMode, TimeSpec}, inode::{Inode, InodeDev, InodeMeta}}, sync::SpinNoIrqLock, syscall::error::{Errno, OSResult}};

/// Inode模块，主要维护文件（广义上）的属性和底层的操作
pub struct Ext4Inode {
    pub meta: Option<InodeMeta>,
    pub file: Option<SpinNoIrqLock<Ext4File>>,
    pub dir: Option<SpinNoIrqLock<Ext4Dir>>,
}

unsafe impl Send for Ext4Inode {}
unsafe impl Sync for Ext4Inode {}

impl Inode for Ext4Inode {
    fn metadata(&self) -> &InodeMeta {
        self.meta.as_ref().unwrap()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> OSResult<usize> {
        // 这里好像需要判断不同种类的inode？
        let inode_type = self.metadata().i_mode;
        match inode_type {
            InodeMode::Directory => {
                // Inode目录不会有读，这个操作放在了Dentry中
                return Err(Errno::EISDIR);
            }, 
            InodeMode::Regular => {
                let mut file = self.file.as_ref().unwrap().lock();
                file.seek(offset as i64, SEEK_SET)
                    .map_err(Errno::from_i32)?;
                file.read(buf).map_err(Errno::from_i32)
            },
            _ => {todo!()}
        }
    }

    fn write(&self, offset: usize, buf: &mut [u8]) -> OSResult<usize> {
        let inode_type = self.metadata().i_mode;
        match inode_type {
            InodeMode::Regular => {
                let mut file = self.file.as_ref().unwrap().lock();
                file.seek(offset as i64, SEEK_SET)
                    .map_err(Errno::from_i32)?;
                file.write(buf).map_err(Errno::from_i32)
            }
            // Inode目录不会有写，这个操作有专门的系统调用会处理
            _ => {
                todo!()
            }
        }
    }

    fn delete_data(&self) -> OSResult<()>{
        let inode_type = self.metadata().i_mode;
        match inode_type {
            InodeMode::Regular => {
                let mut file = self.file.as_ref().unwrap().lock();
                file.truncate(0).map_err(Errno::from_i32)
            }
            _ => {
                todo!()
            }
        }
    }

    fn read_all(&self) -> OSResult<Vec<u8>> {
        let inode_type = self.metadata().i_mode;
        match inode_type {
            InodeMode::Regular => {
                let size = self.metadata().inner.lock().i_size;
                let mut data_vec= vec![0u8; size];
                let mut file = self.file.as_ref().unwrap().lock();
                file.seek(0, SEEK_SET).map_err(Errno::from_i32)?;
                file.read(&mut data_vec).map_err(Errno::from_i32)?;
                Ok(data_vec)
            }
            _ => {
                todo!()
            }
        }
    }
}

impl Ext4Inode {
    pub fn new_file(mode: InodeMode, file: Ext4File) -> Self {
        let mut file = file;
        Self { 
            meta: Some(InodeMeta::new(
                mode, 
                0, 
                InodeDev::Todo, 
                file.size() as usize, 
                TimeSpec::new(), 
                TimeSpec::new(),
                TimeSpec::new(), 
            )), 
            file: Some(SpinNoIrqLock::new(file)), 
            dir: None,
        }
    }

    pub fn new_dir(mode: InodeMode, dir: Ext4Dir) -> Self {
        Self { 
            meta: Some(InodeMeta::new(
                mode, 
                0, 
                InodeDev::Todo, 
                0usize, 
                TimeSpec::new(), 
                TimeSpec::new(),
                TimeSpec::new(), 
            )), 
            file: None,
            dir: Some(SpinNoIrqLock::new(dir)),
        }
    }

    pub fn new_link(mode: InodeMode, size: usize) -> Self {
        Self { 
            meta: Some(InodeMeta::new(
                mode, 
                0, 
                InodeDev::Todo, 
                size, 
                TimeSpec::new(), 
                TimeSpec::new(),
                TimeSpec::new(), 
            )), 
            file: None,
            dir: None,
        }
    }
}
