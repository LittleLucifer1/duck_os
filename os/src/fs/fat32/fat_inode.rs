//! fat32文件系统对 VFS Inode 的具体实现
//! 

use alloc::{sync::Arc, vec::Vec};
use hashbrown::HashMap;

use crate::{
    fs::{
        info::{InodeMode, TimeSpec}, 
        inode::{BlockDevWrapper, Inode, InodeDev, InodeMeta}
    },
    sync::SpinLock, utils::path::dentry_name
};

use super::{data::{DirAttr, ShortDirEntry}, fat::FatInfo, fat_dentry::Position, fat_file::FatDiskFile};

// TODO： 不知道这里的meta需不需要Arc？
// 重构：1.inode是目录，InodeMode = Directory，同时在某个cache中有它内容中下一个空位的位置
//      2.inode是文件，InodeMode = Regular，此时应该要有提供读写的功能！
pub struct FatInode {
    pub meta: Option<InodeMeta>,
    pub pos: Position,
    pub fat_info: Arc<FatInfo>,
    pub fat_file: SpinLock<FatDiskFile>,
}

impl Inode for FatInode {
    fn metadata(&self) -> &InodeMeta {
        if let Some(meta) = &self.meta {
            meta 
        } else {
            todo!()
        }
    }

    // 调用底层的函数，删除磁盘上的数据
    fn delete_data(&self) {
        
    }

    fn read(&self, offset: usize, buf: &mut [u8]) {
        self.fat_file.lock().read(buf, offset);
    }
    
    fn write(&self, offset: usize, buf: &mut [u8]) {
        self.fat_file.lock().write(buf, offset, self.pos);
    }

    fn read_all(&self) -> Vec<u8> {
        self.fat_file.lock().read_all()
    }
}

impl FatInode {
    // 根目录Inode初始化
    pub fn new_from_root(fat_info: Arc<FatInfo>) -> Self {
        let pos = Position::new_from_root();
        let fat_info = Arc::clone(&fat_info);
        let fat_file = FatDiskFile::init(
            Arc::clone(&fat_info), Position::new_from_root()
        );
        let meta = Some(InodeMeta::new(
            InodeMode::Directory, 
            0, 
            InodeDev::BlockDev(BlockDevWrapper {
                block_device: Arc::clone(fat_info.dev.as_ref().expect("Block device is None")),
                id: 0,
            }), 
            fat_file.size, 
            TimeSpec::new(),
            TimeSpec::new(),
            TimeSpec::new(),
        ));
        // NXTFREEPOS_CACHE
        Self {
            meta,
            pos,
            fat_info,
            fat_file: SpinLock::new(fat_file),
        }
        
    }

    // Inode的初始化需要的信息：short_direntry, pos 和 fat_info(实际上是Device)
    pub fn new_from_entry(s_entry: &ShortDirEntry, pos: Position, fat_info: Arc<FatInfo>) -> Self {
        let mode = if let Some(attr) = DirAttr::from_bits(s_entry.attr) {
            if attr.contains(DirAttr::DIRECTORY) {
                InodeMode::Directory
            } else {
                InodeMode::Regular
            }
        } else {
            panic!("[kernel](new_from_entry): No attr");
        };
        let times = s_entry.bit_to_timespec();
        let fat_inode = Self {
            meta: Some(InodeMeta::new(
                mode, 
                0, 
                // TODO：不知道这个id的值应该为多少！
                // DONE: 好像不用处理这个值，因为这是磁盘上的文件系统
                InodeDev::BlockDev(BlockDevWrapper {
                    block_device: Arc::clone(fat_info.dev.as_ref().expect("Block device is None")),
                    id: 0,
                }),
                s_entry.file_size as usize,
                times[0],
                times[1],
                times[2], 
            )),
            pos,
            fat_info: Arc::clone(&fat_info),
            fat_file: SpinLock::new(FatDiskFile::init(Arc::clone(&fat_info), pos)),
        };
        fat_inode
    }

    // TODO: 待完善
    // DONE: 已完善，此时inode为新建的目录或者文件，所以 size = 0;
    pub fn new(mode: InodeMode, fat_info: Arc<FatInfo>, pos: NxtFreePos, data_clu: usize) -> Self {
        Self { 
            meta: Some(
                InodeMeta::new(
                    mode, 
                    0, 
                    InodeDev::BlockDev(BlockDevWrapper {
                        block_device: Arc::clone(fat_info.dev.as_ref().expect("Block device is None")),
                        id: 0,
                    }),
                    0,
                    TimeSpec::new(),
                    TimeSpec::new(),
                    TimeSpec::new(), 
            )),
            pos: Position::new_from_nxtpos(pos, data_clu),
            fat_info: Arc::clone(&fat_info),
            fat_file: SpinLock::new(FatDiskFile::init(
                Arc::clone(&fat_info), 
                Position::new_from_nxtpos(pos, data_clu)
                )
            )
        }
    }

    // TODO: 应该要返回新的 pos，以便于重新插入到cache中
    // TODO: 这里的操作涉及磁盘的写，或许暂时用不到？
    pub fn write_dentry(pos: NxtFreePos, _name: &str) -> (NxtFreePos, usize) {
        (pos, 0)
    }

    // 创建目录，需要在磁盘上写入相关的数据
    pub fn mkdir(fa:Arc<dyn Inode>, path: &str, mode: InodeMode,fat_info: Arc<FatInfo>) -> FatInode {
        if fa.metadata().i_mode != InodeMode::Directory {
            panic!("[Kernel](inode mkdir): Father inode is not a directory.");
        }
        // 在 data_cluster中写入相关的信息 
        // 1. 从cache中拿到相关的pos
        let nxt_pos: NxtFreePos;
        let pos = NXTFREEPOS_CACHE
            .0
            .lock()
            .as_ref()
            .unwrap()
            .get(&fa.metadata().i_ino)
            .cloned();
        if pos.is_none() {
            panic!("[Kernel](inode mkdir): nxt_free_pos cache has no corresponding pos of the inode fa");
        } else {
            nxt_pos = pos.unwrap();
        }
        // 在磁盘中写入对应的数据，并且要返回出pos, 插入新的数据进去
        let (nxt, data_cluster) = FatInode::write_dentry(nxt_pos, dentry_name(path));
        NXTFREEPOS_CACHE.0.lock().as_mut().unwrap().insert(fa.metadata().i_ino, nxt);
        // 返回出这个inode
        FatInode::new(mode, Arc::clone(&fat_info), nxt_pos, data_cluster)
    }

    pub fn mknod(fa:Arc<dyn Inode>, path: &str, mode: InodeMode,fat_info: Arc<FatInfo>, _dev_id: Option<usize>) -> FatInode {
        if fa.metadata().i_mode != InodeMode::Directory {
            todo!()
        }
        // 在 data_cluster中写入相关的信息 
        // 1. 从cache中拿到相关的pos
        let nxt_pos: NxtFreePos;
        let pos = NXTFREEPOS_CACHE
            .0
            .lock()
            .as_ref()
            .unwrap()
            .get(&fa.metadata().i_ino)
            .cloned();
        // println!("fa ino:{}", fa.metadata().i_ino);
        if pos.is_none() {
            todo!()
        } else {
            nxt_pos = pos.unwrap();
        }
        // 在磁盘中写入对应的数据，并且要返回出pos, 插入新的数据进去
        let (nxt, data_cluster) = FatInode::write_dentry(nxt_pos, dentry_name(path));
        NXTFREEPOS_CACHE.0.lock().as_mut().unwrap().insert(fa.metadata().i_ino, nxt);
        // 返回出这个inode
        FatInode::new(mode, Arc::clone(&fat_info), nxt_pos, data_cluster)
    }

}


// 记录着每一个inode中下一个空dentry的位置,由此可以直接写.
pub static NXTFREEPOS_CACHE: NxtFreePosCache = NxtFreePosCache::new();

pub struct NxtFreePosCache(pub SpinLock<Option<HashMap<usize, NxtFreePos>>>);

impl NxtFreePosCache {
    pub const fn new() -> Self {
        Self(SpinLock::new(None))
    }

    pub fn init(&self) {
        *self.0.lock() = Some(HashMap::new());
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
// 只有目录会在NXT_POS_CACHE中有这个数据，
pub struct NxtFreePos {
    // 记录着下一个内容cluster中可以分配具体位置
    pub cluster: usize,
    pub sector: usize,
    pub offset: usize,
}

impl NxtFreePos {
    pub fn empty() -> Self {
        Self { cluster: 0, sector: 0, offset: 0 }
    }

    pub fn update(&mut self, cluster: usize, sector: usize, offset: usize) {
        self.cluster = cluster;
        self.sector = sector;
        self.offset = offset;
    }
}