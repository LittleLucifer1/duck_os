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
mod ext4;
mod simplefs;
mod tmpfs;
mod procfs;
mod devfs;


pub const AT_FDCWD: isize = -100;

pub fn init() {
    FILE_SYSTEM_MANAGER
        .mount(
            "/",
        "/dev/vda3",
        Some(Arc::clone(&BLOCK_DEVICE.lock().as_ref().unwrap())), 
        FSType::EXT4, 
        FSFlags::MS_NOSUID,
    ).expect("Mounting root filesystem wrong, reason is: ");
    // FILE_SYSTEM_MANAGER
    //     .mount(
    //         "/dev", 
    //         "Not implemented yet", 
    //         None, 
    //         FSType::DevFs, 
    //         FSFlags::MS_NOSUID,
    //     );
    // FILE_SYSTEM_MANAGER.manager.lock().get("/dev").unwrap().init();

    // FILE_SYSTEM_MANAGER
    //     .mount(
    //         "/proc", 
    //         "Not implemented yet", 
    //         None, 
    //         FSType::ProcFs, 
    //         FSFlags::MS_NOSUID,
    //     );
    // FILE_SYSTEM_MANAGER.manager.lock().get("/proc").unwrap().init();

    // FILE_SYSTEM_MANAGER
    // .mount(
    //     "/tmp", 
    //     "Not implemented yet", 
    //     None, 
    //     FSType::TmpFs, 
    //     FSFlags::MS_NOSUID,
    // );


}