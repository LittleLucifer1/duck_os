use alloc::sync::Arc;

use crate::{driver::BLOCK_DEVICE, syscall::{FSFlags, FSType}};

use self::file_system::FILE_SYSTEM_MANAGER;

pub mod fat32;
pub mod file;
pub mod file_system;
pub mod dentry;
pub mod inode;
pub mod info;
pub mod fd_table;
pub mod page_cache;
pub mod stdio;
pub mod pipe;

pub const AT_FDCWD: isize = -100;

pub fn init() {
    FILE_SYSTEM_MANAGER
        .mount(
            "/",
         "/dev/vad3",
        Some(Arc::clone(&BLOCK_DEVICE.lock().as_ref().unwrap())), 
        FSType::VFAT, 
        FSFlags::MS_NOSUID,
    );
}