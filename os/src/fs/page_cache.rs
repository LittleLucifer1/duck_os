use alloc::{collections::BTreeMap, sync::{Arc, Weak}};

use crate::{config::mm::PAGE_SIZE_BITS, mm::{pma::Page, type_cast::PagePermission}, sync::SpinLock};

use super::inode::Inode;

pub struct PageCache {
    // (file_offset, page)
    pub pages: SpinLock<BTreeMap<usize, Arc<Page>>>,
}

impl PageCache {
    pub fn new() -> Self {
        Self { pages: SpinLock::new(BTreeMap::new()) }
    }

    fn to_offset(file_offset: usize) -> usize {
        file_offset >> PAGE_SIZE_BITS
    }
    
    pub fn find_page(&self, file_offset: usize, inode: Weak<dyn Inode>) -> Arc<Page> {
        let page_lock = self.pages.lock();
        let page = page_lock.get(&Self::to_offset(file_offset));
        if page.is_some() {
            Arc::clone(&page.unwrap())
        } else {
            drop(page_lock);
            Self::find_page_from_disk(&self, Self::to_offset(file_offset), inode)
        }
    }

    // TODO：这里需要添加permission的相关操作，
    fn find_page_from_disk(&self, offset: usize, inode: Weak<dyn Inode>) -> Arc<Page> {
        let page = Page::new_disk_page(PagePermission::all(), inode, offset);
        let page_arc = Arc::new(page);
        self.pages.lock().insert(offset, page_arc.clone());
        page_arc
    }
}