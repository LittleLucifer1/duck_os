//! fat32文件系统中的data部分

use alloc::{collections::BTreeMap, string::{String, ToString}, sync::Arc, vec::Vec};
use bitflags::bitflags;

use crate::{fs::{dentry::{DentryMeta, DentryMetaInner}, info::TimeSpec, inode::Inode}, sync::SpinLock};

use super::{
    fat::FatInfo, 
    fat_dentry::{FatDentry, Position}, 
    fat_inode::FatInode, 
    utility::{fat_to_unix_time, unix_time_to_timespec}, 
    DirEntryStatus
};

// default：以短名来解析，如果不对的话，就转换为长名。
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct DirEntry {
    data: [u8; 32]
}

bitflags! {
    pub struct DirAttr: u8 {
        const READ_ONLY = 0x01;
        const HIDDEN = 0x02;
        const SYSTEM = 0x04;
        const VOLUME = 0x08;
        const DIRECTORY = 0x10;
        const ARCHIVE = 0x20;
    }
}

impl DirEntry {
    pub fn status(&self) -> DirEntryStatus {
        if self.data[0] == 0x0 {
            DirEntryStatus::Empty
        } else if self.data[0] == 0xE5 {
            DirEntryStatus::Free
        } else if self.data[0] == 0x2E {
            DirEntryStatus::Special
        } else {
            DirEntryStatus::Normal
        }
    }
    pub fn is_long(&self) -> bool {
        self.data[11] == 0x0F
    }

    pub fn is_special(&self) -> bool {
        self.data[0] == 0x2E
    }
}

#[derive(Default, Clone, Copy)]
#[repr(C)]
pub struct LongDirEntry {
    pub ord: u8,
    pub name1: [u8; 10],
    pub attr: u8,
    pub ttype: u8,
    pub chksum: u8,
    pub name2: [u8; 12],
    pub fstcluslo: [u8;2],
    pub name3: [u8; 4],
}

impl LongDirEntry {

    fn byte_to_unicode(b1: u8, b2: u8) -> char {
        let code_point = u16::from_le_bytes([b1, b2]);
        core::char::from_u32(code_point as u32).unwrap_or(core::char::REPLACEMENT_CHARACTER)
    }

    pub fn bit_to_name(&self) -> String {
        let mut name_string = String::new();
        let mut i = 0;
        let mut name_vec: Vec<u8> = Vec::new();
        name_vec.extend_from_slice(&self.name1);
        name_vec.extend_from_slice(&self.name2);
        name_vec.extend_from_slice(&self.name3);

        let mut name_array: [u8; 26] = [0; 26];
        name_array.copy_from_slice(&name_vec[..26]); 

        while i < name_array.len() {
            if name_array[i] == 0x00 && name_array[i+1] == 0x00 {
                break;
            }
            let ch = Self::byte_to_unicode(name_array[i], name_array[i+1]);
            name_string.push(ch);
            i += 2;
        }
        name_string
    }
}

pub fn change_to_long_dentry(entry: &DirEntry) -> LongDirEntry {
    let mut l_entry = LongDirEntry::default();
    unsafe {
        let pointer: *mut DirEntry = &mut l_entry as *mut LongDirEntry as *mut DirEntry;
        core::ptr::copy_nonoverlapping(entry ,pointer, 1);
    }
    l_entry
}

#[derive(Default, Clone, Copy)]
#[repr(C)]
pub struct ShortDirEntry {
    pub name: [u8; 11],
    pub attr: u8,
    pub nt_res: u8,
    pub crt_time_tenth: u8,
    pub crt_time: u16,
    pub crt_date: u16,
    pub lst_acc_date: u16,
    pub fst_clus_hi: u16,
    pub wrt_time: u16,
    pub wrt_date: u16,
    pub fst_clus_lo: u16,
    pub file_size: u32,
}

impl ShortDirEntry {
    // TODO：会不会太简单了？
    // DONE: 好像是有用的！
    pub fn bit_to_name(&self) -> String {
        let mut name = String::new();
        for &b in self.name.iter() {
            name.push(b as char)
        } 
        name.insert(8, '.');
        name
    }

    // 返回的是acc, wrt, crt
    pub fn bit_to_timespec(&self) -> [TimeSpec; 3] {
        let mut times: [TimeSpec; 3] = [TimeSpec::default(); 3];
        times[0] = unix_time_to_timespec(
            fat_to_unix_time(self.lst_acc_date, 0, 0)
        );
        times[1] = unix_time_to_timespec(
            fat_to_unix_time(self.wrt_date, self.wrt_time, 0)
        );
        times[2] = unix_time_to_timespec(
            fat_to_unix_time(self.crt_date, self.crt_time, self.crt_time_tenth)
        );
        times
    }
}

// Warning: 函数不知道是否可以正常工作！
pub fn change_to_short_dentry(entry: &DirEntry) -> ShortDirEntry {
    let mut s_entry = ShortDirEntry::default();
    unsafe {
        let pointer: *mut DirEntry = &mut s_entry as *mut ShortDirEntry as *mut DirEntry;
        core::ptr::copy_nonoverlapping(entry ,pointer, 1);
    }
    s_entry
}

pub fn parse_s_name(dir_pos: &(DirEntry, Position), fat_info: Arc<FatInfo>) -> FatDentry {
    let short_entry = change_to_short_dentry(&dir_pos.0);
    let dname = short_entry.bit_to_name();

    let data_clus: u32 = short_entry.fst_clus_lo as u32 + (short_entry.fst_clus_hi as u32 ) << 16;
    let pos =  Position {
        self_cluster: dir_pos.1.self_cluster,
        self_sector: dir_pos.1.self_sector,
        offset: dir_pos.1.offset,
        data_cluster: data_clus as usize,
    };
    
    let inode = FatInode::new_from_entry(&short_entry, pos, fat_info.clone());
    let inode_rc: Arc<dyn Inode> = Arc::new(inode);

    FatDentry {
        meta: DentryMeta { 
            inner: SpinLock::new(
                DentryMetaInner {
                    d_name: dname,
                    d_path: "".to_string(),
                    d_inode: inode_rc,
                    d_parent: None,
                    d_child: BTreeMap::new(),
                }
            ) 
        },
        fat_info,
        pos,
    }
}

// 做法和短名其实很相似，就是名字处理不一样
// TODO：不知道上面这段话的理解有没有什么问题？？
// Done：读的话，就是从数据中得到我想要的名字和属性，其他的字节不管了！
pub fn parse_l_name(dir_pos: &[(DirEntry, Position)], fat_info: Arc<FatInfo>) -> FatDentry {
    let s_dir_pos = &dir_pos[dir_pos.len() - 1];
    let short_entry = change_to_short_dentry(&s_dir_pos.0);
    let mut dname = String::new();
    for (dir, _) in dir_pos[..dir_pos.len() - 1].iter().rev() {
        let l_dir = change_to_long_dentry(dir);
        dname.push_str(&l_dir.bit_to_name());
    }
    let data_clus: u32 = short_entry.fst_clus_lo as u32 | (short_entry.fst_clus_hi as u32 ) << 16;
    let pos =  Position {
        self_cluster: s_dir_pos.1.self_cluster,
        self_sector: s_dir_pos.1.self_sector,
        offset: s_dir_pos.1.offset,
        data_cluster: data_clus as usize,
    };
    let inode = Arc::new(FatInode::new_from_entry(&short_entry, pos, fat_info.clone()));

    FatDentry {
        meta: DentryMeta { 
            inner: SpinLock::new(
                DentryMetaInner {
                    d_name: dname,
                    d_path: "".to_string(),
                    d_inode: inode,
                    d_parent: None,
                    d_child: BTreeMap::new(),
                }
            ) 
        },
        fat_info,
        pos,
    }
}

pub fn parse_child(dir_pos: &Vec<(DirEntry, Position)>, fat_info:Arc<FatInfo>) -> Vec<FatDentry> {
    let mut start = 0usize;
    let mut childs: Vec<FatDentry> = Vec::new();
    for (id, (dir, _)) in dir_pos.iter().enumerate() {
        if dir.is_long() {
            continue;
        } else {
            // 短目录
            if start == id {
                // . 和 .. 特殊目录，不用建立dentry
                if dir.is_special() {
                    start = id + 1;
                    continue;
                } else {
                    start = id + 1;
                    let fat_entry = parse_s_name(&dir_pos[id], fat_info.clone());
                    childs.push(fat_entry);
                }
            } else {
                let fat_entry = parse_l_name(&dir_pos[start..=id], fat_info.clone());
                childs.push(fat_entry);
                start = id + 1;
            }   
        }
    }
    childs
}