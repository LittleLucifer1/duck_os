//! 这个是 copy-on-write 模块 

use alloc::{collections::BTreeMap, sync::Arc};

use crate::utils::cell::SyncUnsafeCell;

use super::{address::{vpn_to_virt, VirtAddr}, memory_set::page_fault::{CowPageFaultHandler, PageFaultHandler}, page_table::PageTable, pma::Page, type_cast::PTEFlags};

// TODO： 重构，这里的cow还是存在问题
pub struct CowManager {
    // (vpn, page)
    // Warning: 这里使用 Arc 来记录被 COW了的页，其实可以不需要，会添加计数，容易造成
    // 内存泄漏，但是我们在清除 memory_set时，清空了它。所以暂时可能没有什么大问题
    pub page_manager: SyncUnsafeCell<BTreeMap<usize, Arc<Page>>>,
    // (ppn) 共享的page的ppn 和 可能要修改的page_table信息
    pub handler: Arc<dyn PageFaultHandler>,
}

impl CowManager {
    pub fn new() -> Self {
        Self {
            page_manager: SyncUnsafeCell::new(BTreeMap::new()),
            handler: Arc::new(CowPageFaultHandler {}.clone())
        }
    }
    // 清空page_manager，以防止page无法正常的被释放
    pub fn clear(&mut self) {
        self.page_manager.get_unchecked_mut().clear();
    }

    // 判断发生缺页 va是否在cow中
    pub fn is_in_cow(&self, va: VirtAddr) -> bool {
        for (vpn, _) in self.page_manager.get_ref().iter() {
            let va_start = vpn_to_virt(*vpn);
            let va_end = vpn_to_virt(*vpn+1);
            if va_start <= va && va < va_end {
                return true;
            }
        }
        return false;
    }

    // 共享页面，并且标记好是 cow(copy-on-write)
    pub fn from_other_cow(&mut self, another: &Self, pt: &mut PageTable) {
        let page_manager = 
            another
                .page_manager
                .get_unchecked_mut()
                .clone();
        // 如果之前的cow中有页，则应该是已经修改好 pte 的。
        // Titanix中则是又修改了一遍。但是我认为不需要。
        for (vpn, _) in another.page_manager.get_unchecked_mut().iter() {
                pt
                // .find_pte(vpn_to_virt(*vpn))
                .translate_va_to_pte(vpn_to_virt(*vpn))
                .map(|pte_flags| {
                    debug_assert!(pte_flags.flags().contains(PTEFlags::COW));
                    debug_assert!(!pte_flags.flags().contains(PTEFlags::W));
                });
        }

        self.page_manager = SyncUnsafeCell::new(page_manager);
        self.handler = another.handler.clone();
    }

    // pub fn page_fault_handler(

    // )
}


