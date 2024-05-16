//! fat32文件系统对 VFS File System 的具体实现

use alloc::{string::ToString, sync::Arc};

use crate::{config::fs::{BOOT_SECTOR_ID, SECTOR_SIZE}, driver::BlockDevice, fs::{dentry::{Dentry, DENTRY_CACHE}, fat32::fat_inode::NXTFREEPOS_CACHE, file_system::{FileSystem, FileSystemMeta}, inode::Inode}, sync::SpinLock, syscall::{FSFlags, FSType}};

use super::{bpb::load_bpb, fat::FatInfo, fat_dentry::FatDentry, fat_inode::FatInode, fsinfo::FSInfo, utility::{fat_sector, init_map}};

/// fat中的文件系统
pub struct Fat32FileSystem {
    pub meta: FileSystemMeta,
}

impl FileSystem for Fat32FileSystem {
    fn metadata(&self) -> &FileSystemMeta {
        &self.meta
    }

    fn root_dentry(&self) -> Arc<dyn Dentry> {
        self.meta.root_dentry.clone()
    }
}

// TODO: 处理缓冲区的sync问题
impl Drop for Fat32FileSystem {
    fn drop(&mut self) {
        
    }
}

impl Fat32FileSystem {
    // 初始化文件系统
    pub fn new(
        mount_point: &str,
        dev_name: &str,
        dev: Arc<dyn BlockDevice>,
        flags: FSFlags,
    ) -> Self {
        // 初始化相关的cache
        NXTFREEPOS_CACHE.init();
        // 1. 读 bpb —— 从disk到block cache,同时检查bpb
        let mut boot_sec_data: [u8; SECTOR_SIZE] = [0; SECTOR_SIZE];
        // 这个数据只需要读一次即可，所以不需要使用cache
        dev.read_block(BOOT_SECTOR_ID, &mut boot_sec_data[..]);
        let map = init_map();
        let bpb = load_bpb(map.clone(), boot_sec_data);
        if !bpb.is_valid() {
            panic!("The fat32 magic is wrong!");
        };
        // 2. 读 fsinfo，从disk到block，系统结束时回写，期间所有的修改保存在FSINFO中
        
        // let mut fsinfo_sec_data: [u8; SECTOR_SIZE] = [0; SECTOR_SIZE];
        // // TODO：这个部分的代码要修改，应该使用缓存方式的读写，因为这个数据会被修改
        // // dev.read_block(FSINFO_SECTOR_ID, &mut fsinfo_sec_data[..]);
        // get_block_cache(FSINFO_SECTOR_ID, Arc::clone(&dev))
        //     .lock()
        //     .read(0, |data: &[u8; SECTOR_SIZE]|{
        //         fsinfo_sec_data.copy_from_slice(data);
        //     });
        // let fsinfo = load_fsinfo(map.clone(), fsinfo_sec_data);
        // FSINFO.lock().from_another(fsinfo);
        let fsinfo_sec_id = bpb.BPB_FSInfo as usize;
        FSINFO.lock().init(Arc::clone(&map), Arc::clone(&dev), fsinfo_sec_id);

        // 3. 读 fat
        let fat_info = Arc::new(FatInfo::init(
            fat_sector(&bpb), 
            bpb.BPB_FATSz32 as usize,
            bpb.BPB_BytsPerSec as usize,
            bpb.BPB_SecPerClus as usize,
            bpb.BPB_NumFATs as usize,
            Some(dev.clone()))
        );
        // 4. 读data块的位置，构建root_dentry和root_inode
        let root_inode = FatInode::new_from_root(fat_info.clone());
        let r_inode: Arc<dyn Inode> = Arc::new(root_inode);
        let root_dentry = FatDentry::new_from_root(fat_info.clone(), mount_point, Arc::clone(&r_inode));
        root_dentry.meta.inner.lock().d_inode = Arc::clone(&r_inode);
        let r_dentry: Arc<dyn Dentry> = Arc::new(root_dentry);
        DENTRY_CACHE.lock().insert(mount_point.to_string(), r_dentry.clone());
        // 5. 让root_dentry 加载所有的子结点
        r_dentry.load_all_child(Arc::clone(&r_dentry));
        // r_dentry.list_child();
        Self {
            meta: FileSystemMeta {
                f_dev: dev_name.to_string(),
                f_type: FSType::VFAT,
                f_flags: flags,
                root_dentry: r_dentry,
                root_inode: r_inode,
            }
        }
    }
}

// 多个进程可能同时使用FSINFO,所以要上锁。同时FSINFO实现了RAII
pub static FSINFO: SpinLock<FSInfo> = SpinLock::new(FSInfo::empty());