//! fat32文件系统中的fat部分

use core::fmt::Debug;

use alloc::{sync::Arc, vec::Vec};

use crate::driver::BlockDevice;

use super::{block_cache::get_block_cache, fat_fs::FSINFO, utility::{cluster_to_entry, entry_pos}, FatEntryStatus};

// TODO: 暂时设置为这个值，正确的值应该是 count_cluster + 1, 但是暂时不好处理。
const MAX_CLUSTER: u32 = 0x0FFF_FFEF;

// 表示当前簇的状态，fat的编号和簇是一一对应的
// 即 2号fat_entry对应的就是2号簇
#[derive(Debug)]
#[repr(C)]
pub struct FatEntry {
    pub value: u32,
}

impl FatEntry {
    pub fn status(&self) -> FatEntryStatus {
        if self.value == 0 {
            FatEntryStatus::Free
        } else if self.value == 0x0FFF_FFFF || self.value == 0x0FFF_FFF8 {
            FatEntryStatus::EndOfFile
        } else if self.value >= 0x2 && self.value <= MAX_CLUSTER {
            FatEntryStatus::Next(self.value as usize)
        } else {
            FatEntryStatus::Wrong
        }
    }

    pub fn next_cluster(&self) -> Option<usize> {
        let next_cluster = match self.status() {
            FatEntryStatus::Next(next) => Some(next),
            FatEntryStatus::EndOfFile => None,
            _ => panic!("Invalid status"),
        };
        next_cluster
    }
}

// 根据 FSINFO 来分配相关的空闲 cluster
pub fn alloc_cluster(prev_id: Option<usize>, fat_info: Arc<FatInfo>) -> Option<usize> {
    let nxt_cluster = FSINFO.lock().alloc_cluster();
    if let Some(p_id) = prev_id {
        let (sector_id, offset) = entry_pos(fat_info.clone(), p_id);
        get_block_cache(sector_id, Arc::clone(&fat_info.dev.as_ref().unwrap()))
            .lock()
            .write(offset, |fat_dentry: &mut FatEntry|{
                assert!(fat_dentry.status() == FatEntryStatus::EndOfFile);
                fat_dentry.value = nxt_cluster as u32;
        });
    }
    let (sector_id, offset) = entry_pos(fat_info.clone(), nxt_cluster);
        get_block_cache(sector_id, Arc::clone(&fat_info.dev.as_ref().unwrap()))
            .lock()
            .write(offset, |fat_dentry: &mut FatEntry|{
                fat_dentry.value = 0x0FFF_FFFF;
        });
    
    Some(nxt_cluster)
}

// 根据 FSINFO 来释放相关的 cluster
pub fn free_cluster(id: usize, prev_id: Option<usize>, fat_info: Arc<FatInfo>) {
    if let Some(p_id) = prev_id {
        let (sector_id, offset) = entry_pos(fat_info.clone(), p_id);
            get_block_cache(sector_id, Arc::clone(&fat_info.dev.as_ref().unwrap()))
            .lock()
            .write(offset, |fat_dentry: &mut FatEntry|{
                assert!(fat_dentry.value == id as u32);
                fat_dentry.value = 0x0FFF_FFFF;
        });
    }
    FSINFO.lock().free_cluster();
    let (sector_id, offset) = entry_pos(fat_info.clone(), id);
    get_block_cache(sector_id, Arc::clone(&fat_info.dev.as_ref().unwrap()))
        .lock()
        .write(offset, |fat_dentry: &mut FatEntry|{
            assert!(fat_dentry.value == 0x0FFF_FFFF as u32);
            fat_dentry.value = 0x0;
        });
}

// 找到一个目录的内容对应的所有簇集合
pub fn find_all_cluster(fat_info: Arc<FatInfo>, cluster: usize) -> Vec<usize> {
    let mut clusters: Vec<usize> = Vec::new();
    clusters.push(cluster);
    let dev = Arc::clone(fat_info.dev.as_ref().unwrap());
    let (mut sec, mut offst) = cluster_to_entry(fat_info.clone(), cluster);
    loop {
        let nxt = get_block_cache(sec, dev.clone())
        .lock()
        .read(offst, |fat_entry: &FatEntry| {
                fat_entry.next_cluster()
            }
        );
        if let Some(nxt) = nxt {
            clusters.push(nxt);
            (sec, offst) = cluster_to_entry(fat_info.clone(), nxt);
            continue;
        } else {
            break;
        }
    }
    clusters
}

// fat 中需要的基本信息，这些信息不会发生变化，都是常量
#[derive(Clone)]
pub struct FatInfo {
    // fat position
    pub sector: usize, 
    pub size: usize,

    // const
    pub byte_per_sec: usize,
    pub sec_per_clus: usize,
    pub num_fat: usize,

    // 设备
    pub dev: Option<Arc<dyn BlockDevice>>,
}

impl FatInfo {
    pub fn init(
        sector: usize, 
        size: usize, 
        byte_per_sec: usize,
        sec_per_clus: usize,
        num_fat: usize,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> Self {
        Self {
            sector,
            size,
            byte_per_sec,
            sec_per_clus,
            num_fat,
            dev,
        }
    }
}

impl Debug for FatInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "FatInfo is [
                fat's sector: {}, 
                fat's size: {}, 
                byte_per_sec: {}, 
                sec_per_cluster: {},
                num_fat: {},
            ]", self.sector, self.size, self.byte_per_sec, self.sec_per_clus, self.num_fat
        ))
    }
}
