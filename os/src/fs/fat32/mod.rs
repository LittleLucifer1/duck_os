use crate::config::fs::SECTOR_SIZE;

use super::info::TimeSpec;

pub mod bpb;
pub mod utility;
pub mod fsinfo;
pub mod fat_fs;
pub mod fat_dentry;
pub mod fat_file;
pub mod fat_inode;
pub mod fat;
pub mod data;
pub mod block_cache;
/* 
    初始化的流程：
    1. 先加载出bpb.判断是否有效
    2. 加载出FS_Info，这个用来处理之后的一些操作，相当于先保存信息。
    3. 找到根目录的cluster，之后就开始建立整个的fs文件系统，即加载所有的文件和目录。
    4. 向外提供服务：
        1）读文件，写文件
        2）修改文件名字
        3）创建文件/目录
        4）删除文件/目录
        5）等
    不同模块提供的服务：
    1. BPB：
        1）提供基本的信息，还有基本的函数
        2）初始化服务
    2. FS_Info：
        1）提供一些信息，同时保存相关的信息
            例如： nxt_free 和 free_count
    3. FAT:
        1）读取相关的fat_entry内容，并根据内容作出相应的操作
        2）写入相关的内容
    4. Data:
        1) 读取相关的FDT内容，短文件名或者长文件名，建立文件或者目录


    1. 先挂载文件系统
*/
#[derive(PartialEq, PartialOrd)]
pub enum FatEntryStatus {
    Next(usize),
    EndOfFile,
    Free,
    Wrong,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DirEntryStatus {
    Normal,
    Empty,
    Free,
    Special, // . or ..
}

const SYSTEM_NAME: &str = "DuckOs";
const NODE_NAME: &str = "laptop";
const RELEASE: &str = "6.5.0-25-generic";
const VERSION: &str = "#25~1.0-duckos SMP PREEMPT_DYNAMIC Tue Feb 20 16:09:15 UTC 2";
const MACHINE: &str = "riscv-64";
// 手册上没有指定数组的大小，是动态可变的。https://man7.org/linux/man-pages/man2/uname.2.html
const ARRAY_SIZE: usize = 128;
#[repr(C)]
pub struct UnameInfo {
    pub sysname: [u8; ARRAY_SIZE],
    pub node_name: [u8; ARRAY_SIZE],
    pub release: [u8; ARRAY_SIZE],
    pub version: [u8; ARRAY_SIZE],
    pub machine: [u8; ARRAY_SIZE],
}

impl UnameInfo {
    pub fn uname() -> Self {
        let mut uinfo = Self {
            sysname: [0; ARRAY_SIZE],
            node_name: [0; ARRAY_SIZE],
            release: [0; ARRAY_SIZE],
            version: [0; ARRAY_SIZE],
            machine: [0; ARRAY_SIZE],
        };
        macro_rules! load {
            ($item: expr, $cst: expr) => {
                $item[..$cst.len()].copy_from_slice($cst.as_bytes());
            };
        }
        load!(uinfo.sysname, SYSTEM_NAME);
        load!(uinfo.node_name, NODE_NAME);
        load!(uinfo.release, RELEASE);
        load!(uinfo.version, VERSION);
        load!(uinfo.machine, MACHINE);
        uinfo
    }
}

#[repr(C)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    __pad1: usize,
    pub st_size: u64,
    pub st_blksize: u32,
    __pad2: u32,
    pub st_blocks: u64,
    pub st_atim: TimeSpec,
    pub st_mtim: TimeSpec,
    pub st_ctim: TimeSpec,
}

impl Stat {
    pub fn empty() -> Stat {
        Self {
            st_dev: 0,
            st_ino: 0,
            st_mode: 0,
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad1: 0,
            st_size: 0,
            st_blksize: SECTOR_SIZE as u32,
            __pad2: 0,
            st_blocks: 0,
            st_atim: TimeSpec::new(),
            st_mtim: TimeSpec::new(),
            st_ctim: TimeSpec::new(),
        }
    }
}

