//! File System Information(FSInfo) Structure

use core::fmt::Debug;

use alloc::{collections::BTreeMap, string::String, sync::Arc};

use crate::{config::fs:: SECTOR_SIZE, driver::BlockDevice, sync::SpinLock};

use super::block_cache::{get_block_cache, BlockCache};

#[allow(non_snake_case)]
// 有两个数据不记录，因为为0. 没什么用
pub struct FSInfo {
    pub FSI_LeadSig: u32,
    pub FSI_StrucSig: u32,
    pub FSI_Free_Count: u32,
    pub FSI_Nxt_Free: u32,
    pub FSI_TrailSig: u32,
    pub block_cache: Option<Arc<SpinLock<BlockCache>>>,
}

impl FSInfo {
    pub const fn empty() -> Self {
        Self {
            FSI_LeadSig: 0, 
            FSI_StrucSig: 0, 
            FSI_Free_Count: 0, 
            FSI_Nxt_Free: 0, 
            FSI_TrailSig: 0,
            block_cache: None,
        }
    }

    /* Function：根据map中的规定，初始化FSINFO
       Warning: 其中的load_fn函数没有经过测试，潜在风险很大！
     */
    pub fn init(&mut self, map: Arc<BTreeMap<String, (usize, usize)>>, dev: Arc<dyn BlockDevice>, sector_id: usize) {
        self.block_cache = Some(get_block_cache(sector_id, dev));
        let mut fsinfo_sec_data:[u8; SECTOR_SIZE] = [0; SECTOR_SIZE];
        self.block_cache
            .as_ref()
            .unwrap()
            .lock()
            .read(0, |data: &[u8; SECTOR_SIZE]|{
                fsinfo_sec_data.copy_from_slice(data);
            });
            
            macro_rules! load {
                ($a: expr, $b: expr) => {
                    if let Some((offset, size)) = map.get($b) {
                        Self::load_fn(&mut $a, &fsinfo_sec_data, *offset, *size);
                    }
                };
            }

            load!(self.FSI_LeadSig, "FSI_LeadSig");
            load!(self.FSI_StrucSig, "FSI_StrucSig");
            load!(self.FSI_Free_Count, "FSI_Free_Count");
            load!(self.FSI_Nxt_Free, "FSI_Nxt_Free");
            load!(self.FSI_TrailSig, "FSI_TrailSig");
    }

    fn load_fn<T: Copy>(dst: &mut T, src: &[u8], offset: usize, size: usize) {
        unsafe {
            core::ptr::copy_nonoverlapping(&src[offset], dst as *mut _ as *mut u8, size);
        }
    }

    fn store_fn<T: Copy>(src: &T, dst: &mut [u8], offset: usize, size: usize) {
        unsafe {
            core::ptr::copy_nonoverlapping(src as *const _ as *const u8, &mut dst[offset], size);
        }
    }

    pub fn update(&mut self, free_count: u32, nxt_free: u32) {
        self.FSI_Free_Count = free_count;
        self.FSI_Nxt_Free = nxt_free;
    }

    pub fn free_count(&self) -> usize {
        self.FSI_Free_Count as usize
    }

    pub fn nxt_free(&self) -> usize {
        self.FSI_Nxt_Free as usize
    }

    pub fn alloc_cluster(&mut self) -> usize {
        let nxt_free = self.nxt_free();
        self.FSI_Free_Count -= 1;
        self.FSI_Nxt_Free += 1;
        nxt_free
    }

    pub fn free_cluster(&mut self) {
        self.FSI_Free_Count += 1;
    }

}

/*  Function: 在FSINFO全局变量结束之后，将其中的数据写入block中，再回写到磁盘上
    Warning: store函数没有经过测试，store_fn函数同理，可能有问题！
*/
impl Drop for FSInfo {
    fn drop(&mut self) {
        let mut data:[u8; SECTOR_SIZE] = [0; SECTOR_SIZE];
        self.block_cache
            .as_ref()
            .unwrap()
            .lock()
            .read(0, |da: &[u8; SECTOR_SIZE]|{
                data.copy_from_slice(da);
            });
        
        macro_rules! store {
            ($a: expr, $b: expr, $c: expr, $d: expr) => {
                Self::store_fn(&$a, $b, $c, $d);
            };
        }
        store!(self.FSI_LeadSig, &mut data, 0, 4);
        store!(self.FSI_StrucSig, &mut data, 484, 4);
        store!(self.FSI_Free_Count, &mut data, 488, 4);
        store!(self.FSI_Nxt_Free, &mut data, 492, 4);
        store!(self.FSI_TrailSig, &mut data, 508, 4);

        let block_cache = self.block_cache.as_ref().unwrap();
        block_cache.lock().write(0, |sec_data: &mut [u8; SECTOR_SIZE]| {
            sec_data.copy_from_slice(&data);
        });
        block_cache.lock().sync();
    }
}

impl FSInfo {
    // free cluster 的数量
    pub fn set_free_count(&mut self, new_free_count: usize) {
        self.FSI_Free_Count = new_free_count as u32;
    }

    // 下一个空闲的cluster number
    pub fn next_free(&mut self, new_free: usize) {
        self.FSI_Nxt_Free = new_free as u32;
    }
}

impl Debug for FSInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("FSInfo is [free_count:{}, next_free_cluter_num: {}]", self.FSI_Free_Count, self.FSI_Nxt_Free))
    }
}
