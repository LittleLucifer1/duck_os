//! 这个是 copy-on-write 模块 

use alloc::{collections::BTreeMap, sync::Arc};

use crate::utils::cell::SyncUnsafeCell;

use super::{address::vpn_to_virt, memory_set::page_fault::{CowPageFaultHandler, PageFaultHandler}, page_table::PageTable, pma::Page, type_cast::PTEFlags};

pub struct CowManager {
    // (vpn, page)
    pub page_manager: SyncUnsafeCell<BTreeMap<usize, Arc<Page>>>,
    // (ppn) 共享的page的ppn 和 可能要修改的page_table信息
    // pub page_manager: Vec<usize>,
    // pub fa_pt_paddr: Option<usize>,
    pub handler: Arc<dyn PageFaultHandler>,
}

impl CowManager {
    pub fn new() -> Self {
        Self {
            page_manager: SyncUnsafeCell::new(BTreeMap::new()),
            // page_manager: Vec::new(),
            // fa_pt_raddr: None,
            handler: Arc::new(CowPageFaultHandler {}.clone())
        }
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
                .find_pte(vpn_to_virt(*vpn))
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


