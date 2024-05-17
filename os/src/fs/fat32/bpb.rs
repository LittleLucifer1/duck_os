//! FAT32中 #1 sector 即 boot sector.

use alloc::{collections::BTreeMap, string::String, sync::Arc};

use crate::config::fs::SECTOR_SIZE;


#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy, Default)]
pub struct BootSector {
    pub BS_jmpBoot: [u8; 3],
    pub BS_OEMName: u64,
    
    pub BPB_BytsPerSec: u16,
    pub BPB_SecPerClus: u8,
    pub BPB_RsvdSecCnt: u16,
    pub BPB_NumFATs: u8,
    pub BPB_RootEntCnt: u16,
    pub BPB_TotSec16: u16,
    pub BPB_Media: u8,
    pub BPB_FATSz16: u16,
    pub BPB_SecPerTrk: u16,
    pub BPB_NumHeads: u16,
    pub BPB_HiddSec: u32,
    pub BPB_TotSec32: u32,

    pub BPB_FATSz32: u32,
    pub BPB_ExtFlags: u16,
    pub BPB_FSVer: u16,
    pub BPB_RootClus: u32,
    pub BPB_FSInfo: u16,
    pub BPB_BkBootSec: u16,
    pub BPB_Reserved: [u8; 12],
    
    pub BS_DrvNum: u8,
    pub BS_Reserved1: u8,
    pub BS_BootSig: u8,
    pub BS_VollD: u32,
    pub BS_VolLab: [u8; 11],
    pub BS_FilSysType: u64,

    pub Signature_word: u16,
}

impl BootSector {
    // 先不添加报错的功能，太繁琐了。之后考虑修改为 Result<(), Error>
    pub fn is_valid(&self) -> bool {
        let mut result = true;
        if self.BS_jmpBoot[0] != 0xEB && self.BS_jmpBoot[0] != 0xE9 {
            result = false;
        }
        if self.BS_jmpBoot[0] == 0xEB && self.BS_jmpBoot[2] != 0x90 {
            result = false;
        }
        if self.BPB_BytsPerSec != SECTOR_SIZE as u16 {
            result = false;
        }
        if self.BPB_SecPerClus.count_ones() != 1 {
            result = false;
        }
        if self.BPB_SecPerClus < 1 || self.BPB_SecPerClus > 128 {
            result = false;
        }
        if self.BPB_RsvdSecCnt <= self.BPB_FSInfo {
            result = false;
        }
        if self.BPB_RsvdSecCnt <= self.BPB_BkBootSec {
            result = false;
        }
        if self.BPB_NumFATs == 0 {
            result = false;
        }
        if self.BPB_RootEntCnt != 0 {
            result = false;
        }
        if self.BPB_TotSec16 != 0 {
            result = false;
        }
        if self.BPB_FATSz16 != 0 {
            result = false;
        }
        if self.BPB_TotSec32 == 0 {
            result = false;
        }
        if self.BPB_FSVer != 0 {
            result = false;
        }
        for byte in self.BPB_Reserved {
            if byte != 0u8 {
                result = false;
            }
        }
        if self.BS_Reserved1 != 0 {
            result = false;
        }
        result
    }
}


pub fn load_fn<T: Copy>(dst: &mut T, src: &[u8], offset: usize, size: usize) {
    unsafe {
        core::ptr::copy_nonoverlapping(&src[offset], dst as *mut _ as *mut u8, size);
    }
}

pub fn load_bpb(map: Arc<BTreeMap<String, (usize, usize)>>, data: [u8; 512]) -> BootSector {
    let mut boot_sector = BootSector::default();

    macro_rules! load {
        ($a: expr, $b: expr) => {
            if let Some((offset, size)) = map.get($b) {
                load_fn(&mut $a, &data, *offset, *size);
            }
        };
    }

    load!(boot_sector.BS_jmpBoot, "BS_jmpBoot");
    load!(boot_sector.BS_OEMName, "BS_OEMName");
    load!(boot_sector.BPB_BytsPerSec, "BPB_BytsPerSec");
    load!(boot_sector.BPB_SecPerClus, "BPB_SecPerClus");
    load!(boot_sector.BPB_RsvdSecCnt, "BPB_RsvdSecCnt");
    load!(boot_sector.BPB_NumFATs, "BPB_NumFATs");
    load!(boot_sector.BPB_RootEntCnt, "BPB_RootEntCnt");
    load!(boot_sector.BPB_TotSec16, "BPB_TotSec16");
    load!(boot_sector.BPB_Media, "BPB_Media");
    load!(boot_sector.BPB_FATSz16, "BPB_FATSz16");
    load!(boot_sector.BPB_SecPerTrk, "BPB_SecPerTrk");
    load!(boot_sector.BPB_NumHeads, "BPB_NumHeads");
    load!(boot_sector.BPB_HiddSec, "BPB_HiddSec");
    load!(boot_sector.BPB_TotSec32, "BPB_TotSec32");
    load!(boot_sector.BPB_FATSz32, "BPB_FATSz32");
    load!(boot_sector.BPB_ExtFlags, "BPB_ExtFlags");
    load!(boot_sector.BPB_FSVer, "BPB_FSVer");
    load!(boot_sector.BPB_RootClus, "BPB_RootClus");
    load!(boot_sector.BPB_FSInfo, "BPB_FSInfo");
    load!(boot_sector.BPB_BkBootSec, "BPB_BkBootSec");
    load!(boot_sector.BPB_Reserved, "BPB_Reserved");
    load!(boot_sector.BS_DrvNum, "BS_DrvNum");
    load!(boot_sector.BS_Reserved1, "BS_Reserved1");
    load!(boot_sector.BS_BootSig, "BS_BootSig");
    load!(boot_sector.BS_VollD, "BS_VollD");
    load!(boot_sector.BS_VolLab, "BS_VolLab");
    load!(boot_sector.BS_FilSysType, "BS_FilSysType");
    load!(boot_sector.Signature_word, "Signature_word");
    
    boot_sector
}
