use alloc::{collections::BTreeMap, sync::{Arc, Weak}};

use crate::{config::mm::PAGE_SIZE_BITS, mm::{pma::Page, type_cast::PagePermission}, sync::SpinLock};

use super::inode::Inode;

// 这个 page_cache 是磁盘文件在内存中的buffer
pub struct PageCache {
    // (page_num_offset in file, page)
    pub pages: SpinLock<BTreeMap<usize, Arc<Page>>>,
}

impl PageCache {
    pub fn new() -> Self {
        Self { pages: SpinLock::new(BTreeMap::new()) }
    }

    fn to_offset(file_offset: usize) -> usize {
        file_offset >> PAGE_SIZE_BITS
    }
    
    // Function: file使用page_cache中的find_page函数去找相关的page
    // 1、如果没有找到，则如果有disk，就创建一个与之向联系；如果没有disk，则返回None
    // 在 file.read / file.write中，如果文件没有关联硬件，我们会直接将 inode=None
    pub fn find_page_and_create(&self, file_offset: usize, inode: Option<Weak<dyn Inode>>) -> Option<Arc<Page>> {
        let page_lock = self.pages.lock();
        let page = page_lock.get(&Self::to_offset(file_offset));
        if page.is_some() {
            Some(Arc::clone(&page.unwrap()))
        } else {
            drop(page_lock);
            // 1、如果是磁盘上的文件，则从disk中找数据过来
            if inode.is_some() {
                Self::find_page_from_disk(&self, Self::to_offset(file_offset), inode.unwrap())
            } 
            // 2、如果是内存上的文件，例如/proc，则没有对应的disk
            else {
                None
            }
        }
    }

    pub fn find_page(&self, file_offset: usize) -> Option<Arc<Page>> {
        let page_lock = self.pages.lock();
        let page = page_lock.get(&Self::to_offset(file_offset));
        if page.is_some() {
            Some(Arc::clone(&page.unwrap()))
        } else {
            None
        }
    }


    // TODO：这里需要添加permission的相关操作，
    fn find_page_from_disk(&self, page_num_offset: usize, inode: Weak<dyn Inode>) -> Option<Arc<Page>> {
        let page = Page::new_disk_page(PagePermission::all(), inode, page_num_offset);
        let page_arc = Arc::new(page);
        self.pages.lock().insert(page_num_offset, page_arc.clone());
        Some(page_arc)
    }
}