use alloc::{collections::BTreeMap, string::{String, ToString}};
use alloc::sync::Arc;

use crate::fs::info::TimeSpec;

use super::{bpb::BootSector, fat::FatInfo};

// 返回一个手册，供查阅
pub fn init_map() -> Arc<BTreeMap<String, (usize, usize)>> {
    let mut map:BTreeMap<String, (usize, usize)> = BTreeMap::new();
    
    macro_rules! insert {
        ($name: expr, $value: expr) => {
            map.insert($name.to_string(), $value);
        };
    }
    // 按照 Name, (offset, size)的格式插入map中
    // boot sector
    insert!("BS_jmpBoot", (0, 3));
    insert!("BS_OEMName", (3, 8));

    insert!("BPB_BytsPerSec", (11, 2));
    insert!("BPB_SecPerClus", (13, 1));
    insert!("BPB_RsvdSecCnt", (14, 2));
    insert!("BPB_NumFATs", (16, 1));
    insert!("BPB_RootEntCnt", (17, 2));
    insert!("BPB_TotSec16", (19, 2));
    insert!("BPB_Media", (21, 1));
    insert!("BPB_FATSz16", (22, 2));
    insert!("BPB_SecPerTrk", (24, 2));
    insert!("BPB_NumHeads", (26, 2));
    insert!("BPB_HiddSec", (28, 4));
    insert!("BPB_TotSec32", (32, 4));

    insert!("BPB_FATSz32", (36, 4));
    insert!("BPB_ExtFlags", (40, 2));
    insert!("BPB_FSVer", (42, 2));
    insert!("BPB_RootClus", (44, 4));
    insert!("BPB_FSInfo", (48, 2));
    insert!("BPB_BkBootSec", (50, 2));
    insert!("BPB_Reserved", (52, 12));

    insert!("BS_DrvNum", (64, 1));
    insert!("BS_Reserved1", (65, 1));
    insert!("BS_BootSig", (66, 1));
    insert!("BS_VollD", (67, 4));
    insert!("BS_VolLab", (71, 11));
    insert!("BS_FilSysType", (82, 8));
    insert!("Signature_word", (510, 2));
    
    // FSInfo
    insert!("FSI_LeadSig", (0, 4));
    insert!("FSI_StrucSig", (484, 4));
    insert!("FSI_Free_Count", (488, 4));
    insert!("FSI_Nxt_Free", (492, 4));
    insert!("FSI_TrailSig", (508, 4));

    Arc::new(map)
}

// fat1的起始扇区号
pub fn fat_sector(boot_sector: &BootSector) -> usize {
    boot_sector.BPB_RsvdSecCnt as usize
}

pub fn data_sector(boot_sector: &BootSector) -> usize {
    boot_sector.BPB_RsvdSecCnt as usize 
    + (boot_sector.BPB_NumFATs as usize * boot_sector.BPB_FATSz32 as usize)
}

// 给一个cluster number N,确定FAT entry的位置，返回的是(sector number, offset)
pub fn cluster_to_entry(fat_info: Arc<FatInfo>, cluster: usize) -> (usize, usize) {
    (
        fat_info.sector + (cluster * 4) / fat_info.byte_per_sec,
        (cluster * 4) % fat_info.byte_per_sec
    )
}

// 思路和上一个是一样的，因为 cluster_id 和 entry_id 是一一对应的。
pub fn entry_pos(fat_info: Arc<FatInfo>, entry_id: usize) -> (usize, usize) {
    (
        fat_info.sector + (entry_id * 4) / fat_info.byte_per_sec,
        (entry_id * 4) % fat_info.byte_per_sec
    )
}

//给一个cluster number N, 确定它所在的起始扇区编号
pub fn cluster_to_sector(fat_info: Arc<FatInfo>, cluster: usize) -> usize {
    (cluster - 2) * fat_info.sec_per_clus
    + fat_info.sector
    + fat_info.size * fat_info.num_fat
}

pub fn count_of_clusters(boot_sector: &BootSector) -> usize {
    let data_sec = boot_sector.BPB_TotSec32 as usize
    - boot_sector.BPB_RsvdSecCnt as usize
    + boot_sector.BPB_NumFATs as usize * boot_sector.BPB_FATSz32 as usize;
    data_sec / boot_sector.BPB_SecPerClus as usize
}

// 处理时间
const MILLISEC_PER_SEC: i64 = 1000;
const SEC_PER_MIN: i64 = 60;
const MIN_PER_HR: i64 = 60;
const HR_PER_DAY: i64 = 24;

const MILLISEC_PER_MIN: i64 = MILLISEC_PER_SEC * SEC_PER_MIN;
const MILLISEC_PER_HR: i64 = MILLISEC_PER_MIN * MIN_PER_HR;
const MILLISEC_PER_DAY: i64 = MILLISEC_PER_HR * HR_PER_DAY;

const DAY_PER_YEAR: i64 = 365;
#[allow(unused)]
const DAY_PER_400YEAR: i64 = DAY_PER_YEAR * 400 + 97;
const DAY_PER_MONTH: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn leap_year_cnt(year: i64) -> i64 {
    assert!(year >= 1);
    year / 4 - year / 100 + year / 400
}

fn year_to_day_count(year: i64) -> i64 {
    let leap_year = leap_year_cnt(year - 1) - leap_year_cnt(1970 - 1);
    leap_year + (year - 1970) * DAY_PER_YEAR
}

fn month_to_day_count(month: i64, leap: bool) -> i64 {
    let mut ret: i64 = 0;
    for i in 0..month {
        ret += DAY_PER_MONTH[i as usize]
            + match i == 1 && leap {
                true => 1,
                false => 0,
            };
    }
    ret
}

pub fn fat_to_unix_time(date: u16, time: u16, tenth: u8) -> i64 {
    let year = (1980 + (date >> 9)) as i64;
    let month = (((date >> 5) & 0x0F) - 1) as i64;
    let day = ((date & 0x1F) - 1) as i64;
    let hr = ((time >> 11) & 0x1F) as i64;
    let min = ((time >> 5) & 0x3F) as i64;
    let sec = (time & 0x1F) as i64;
    let millisec = (tenth as i64) * 10;
    
    (year_to_day_count(year) + month_to_day_count(month, leap_year(year)) + day) * MILLISEC_PER_DAY
        + (((hr * MIN_PER_HR + min) * SEC_PER_MIN + sec * 2) * MILLISEC_PER_SEC)
        + millisec
}

pub fn unix_time_to_timespec(unix_time: i64) -> TimeSpec {
    if unix_time < 0 {
        TimeSpec { tv_sec: 0, tv_nsec: 0 }
    } else {
        TimeSpec {
            tv_sec: (unix_time as usize) / 1000,
            tv_nsec: (unix_time as usize) % 1000,
        }
    }
}




