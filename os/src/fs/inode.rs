//！ inode模块

/*
用于存储文件的各种属性
    - 所有者信息：文件的owner，group；
    - 权限信息：read、write和excite；
    -时间信息：建立或改变时间（ctime）、最后读取时间（atime）、最后修改时间（mtime）；
    - 标志信息：一些flags；
    - 内容信息：type，size，以及相应的block的位置信息。

    1. 数据结构
        1）ino: inode 的编号
        2) mode: 访问权限
        3) rdev: id，表示与哪个设备通信
        4）dev：设备，可以为block / char / pipe
        5) i_atime、i_mtime、i_ctime
        6) size: 文件长度
        7）其他的再说吧，关键是根据需求确定功能！！

    2. 功能 (按照linux中的设计，有很多函数。但是我现在还不了解整体，所以先照猫画虎)
        1）create: 在某一目录下，为与目录项对想相关的普通文件创建一个磁盘索引节点
        2）mkdir / mknod：在某一目录下，为与目录项对想相关的特殊文件/目录 创建一个磁盘索引节点
        3) metadata: 获得相关的元数据
        // 4）link：创建硬链接
        5）lookup: 为包含在一个目录项对想的文件名对应的索引节点查找目录
        6）rename：移动文件
    
*/

use core::sync::atomic::AtomicUsize;

use alloc::{sync::Arc, vec::Vec};

use crate::{driver::BlockDevice, sync::SpinLock};

use super::info::{InodeMode, TimeSpec};

pub struct InodeMeta{
    pub i_ino: usize,
    pub i_mode: InodeMode,
    pub i_rdev: usize, // 设备通信的id
    pub i_dev: Option<InodeDev>,
    pub inner: SpinLock<InodeMetaInner>,
}

pub struct InodeMetaInner {
    pub i_atime: TimeSpec, /* Time of last access 每当文件数据被进程读取时，它就会更新。*/
    pub i_mtime: TimeSpec, /* Time of last modification 每当文件数据被修改，例如文件被写入或截断时，它就会更新。*/
    pub i_ctime: TimeSpec, /* Time of last status change 每当inode关联的元数据发生变化时，比如文件权限被修改或文件被重命名时，它就会更新。*/
    pub i_size: usize,
}

impl InodeMeta {
    pub fn new(
        mode: InodeMode,
        rdev: usize,
        dev: InodeDev,
        size: usize,
        atime: TimeSpec,
        mtime: TimeSpec,
        ctime: TimeSpec,
    ) -> Self {
        let ino = INODE_NUM_ALLOCATOR.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        Self {
            i_ino: ino,
            i_mode: mode,
            i_rdev: rdev,
            i_dev: Some(dev),
            inner: SpinLock::new(
                InodeMetaInner {
                    i_atime: atime,
                    i_mtime: mtime,
                    i_ctime: ctime,
                    i_size: size,
                }
            )
        }
    }
}

static INODE_NUM_ALLOCATOR: AtomicUsize = AtomicUsize::new(0);

pub trait Inode: Sync + Send {
    fn metadata(&self) -> &InodeMeta;

    fn delete_data(&self);
    fn read(&self, offset: usize, buf: &mut [u8]);
    fn write(&self, offset: usize, buf: &mut [u8]);
    // 用于读整个elf文件
    fn read_all(&self) -> Vec<u8>;
}

pub enum InodeDev {
    BlockDev(BlockDevWrapper),
    // TODO: 
}

// TODO：这里的id没有弄明白是什么？
pub struct BlockDevWrapper {
    pub block_device: Arc<dyn BlockDevice>,
    pub id: usize,
}