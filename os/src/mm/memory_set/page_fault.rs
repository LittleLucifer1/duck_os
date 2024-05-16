//! 专门处理多种不同的 page_fault
/*
    1. page_fault种类
        1） sbrk
        2） mmap
        3） user_stack
        4)  user_heap
*/

use alloc::sync::{Arc, Weak};
use riscv::register::scause::Scause;

use crate::mm::{address::{virt_to_vpn, VirtAddr}, page_table::PageTable, pma::Page, type_cast::{PTEFlags, PagePermission}, vma::VirtMemoryAddr};

use super::mem_set::MemeorySet;

pub trait PageFaultHandler: Send + Sync {
    // 懒分配：已经插入了对应的vma，只是没有做映射和物理帧分配
    // 所以只需要 映射 + 将分配的物理帧插入对应的 vma 中
    fn handler_page_fault(
        &self,
        _vma: &VirtMemoryAddr,
        _vaddr: VirtAddr,
        _ms: Option<&MemeorySet>,
        _scause: Scause,
        _pt: &mut PageTable,
    ) {}

    // TODO: 这个部分需要去参考手册，目前不懂
    fn is_legal(&self, _scause: Scause) -> bool {
        todo!()
    }
}

#[derive(Clone)]
pub struct UStackPageFaultHandler {}

impl PageFaultHandler for UStackPageFaultHandler {
    // TODO: 考虑到空间的局部连续性，其实可以往地址后面连续的多分几页!
    fn handler_page_fault(
            &self,
            vma: &VirtMemoryAddr,
            vaddr: VirtAddr,
            _ms: Option<&MemeorySet>,
            _scause: Scause,
            pt: &mut PageTable,
        ) {
        let page = Page::new(PagePermission::from(vma.map_permission));
        let ppn = page.frame.ppn;
        let vpn = virt_to_vpn(vaddr);
        vma.pma
            .get_unchecked_mut()
            .page_manager
            .insert(
                vpn,
                Arc::new(page),
            );
        let flag = PTEFlags::W | PTEFlags::R | PTEFlags::U;
        pt.map_one(vpn, ppn, flag);
        pt.activate();
    }

    fn is_legal(&self, _scause: Scause) -> bool {
        todo!()
    }
}

#[derive(Clone)]
pub struct UHeapPageFaultHandler {}

impl PageFaultHandler for UHeapPageFaultHandler {
    fn handler_page_fault(
            &self,
            vma: &VirtMemoryAddr,
            vaddr: VirtAddr,
            _ms: Option<&MemeorySet>,
            _scause: Scause,
            pt: &mut PageTable,
        ) {
            let page = Page::new(PagePermission::from(vma.map_permission));
            let ppn = page.frame.ppn;
            let vpn = virt_to_vpn(vaddr);
            vma.pma
                .get_unchecked_mut()
                .page_manager
                .insert(
                    vpn, 
                    Arc::new(page),
                );
            let flag = PTEFlags::W | PTEFlags::R | PTEFlags::U | PTEFlags::X;
            pt.map_one(vpn, ppn, flag);
            pt.activate();
    }
    fn is_legal(&self, _scause: Scause) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct MmapPageFaultHandler {}

// TODO：如果我有一个MemorySet
impl PageFaultHandler for MmapPageFaultHandler {
    fn handler_page_fault(
            &self,
            vma: &VirtMemoryAddr,
            vaddr: VirtAddr,
            _vm: Option<&MemeorySet>,
            _scause: Scause,
            pt: &mut PageTable,
        ) {
        let map_permission = vma.map_permission;
        // 2. 如果有backen file，则从文件的page cache中拿出page，同时将文件中的内容放入其中
        if vma.is_backen_file() {
            let backen_file = vma.pma.get_unchecked_mut().backen_file.as_ref().unwrap().clone();
            let offset = backen_file.offset + vaddr - vma.start_vaddr;
            let inode = Weak::clone(&backen_file.file.metadata().f_inode);
            let page = backen_file
                .file
                .metadata()
                .page_cache
                .as_ref()
                .unwrap()
                .find_page(offset, inode);
            page.load();
            let ppn = page.frame.ppn;
            let vpn = virt_to_vpn(vaddr);
            vma.pma
                .get_unchecked_mut()
                .page_manager
                .insert(
                    vpn, 
                    Arc::clone(&page),
                );
            pt.map_one(vpn, ppn, map_permission.into());
            pt.activate()
        }
        // 1. 如果没有backen file，则分配一个空页面
        else {
            let page = Page::new(PagePermission::from(map_permission));
            let ppn = page.frame.ppn;
            let vpn = virt_to_vpn(vaddr);
            vma.pma
                .get_unchecked_mut()
                .page_manager
                .insert(
                    vpn, 
                    Arc::new(page),
                );
            // DONE: 这里的PTE是根据 prot来设置的，暂时没有检查这部分的内容; 应该没有什么问题
            let flag = map_permission.into();
            pt.map_one(vpn, ppn, flag);
            pt.activate();
        }
    }

    fn is_legal(&self, _scause: Scause) -> bool {
        todo!()
    }
}


#[derive(Clone)]
pub struct CowPageFaultHandler {}

impl PageFaultHandler for CowPageFaultHandler {
    fn handler_page_fault(
            &self,
            _vma: &VirtMemoryAddr,
            vaddr: VirtAddr,
            ms: Option<&MemeorySet>,
            _scause: Scause,
            pt: &mut PageTable,
        ) {
        let pte = pt.find_pte(vaddr).unwrap();
        debug_assert!(pte.flags().contains(PTEFlags::COW));
        debug_assert!(!pte.flags().contains(PTEFlags::W));

        let mut flags = pte.flags() | PTEFlags::W;
        flags.remove(PTEFlags::COW);

        let page = ms
            .unwrap()
            .cow_manager
            .page_manager
            .get_unchecked_mut()
            .get(&virt_to_vpn(vaddr))
            .cloned()
            .unwrap();

        // 复制这个page 
        // 这里有一个暴力的做法：不管是不是最后一个指向这个页，统一的复制再创造一个新页。
        let new_page = Page::new_from_page(page.frame.ppn, page.permission);
        let vpn = virt_to_vpn(vaddr);
        pt.unmap(vpn);
        pt.map_one(vpn, new_page.frame.ppn, flags);
        pt.activate();
        ms.unwrap().cow_manager
            .page_manager
            .get_unchecked_mut()
            .remove(&vpn);

        // vma.pma.get_unchecked_mut().push_pma_page(vpn, page);
        ms.unwrap()
            .find_vm_by_vaddr(vaddr)
            .unwrap()
            .pma
            .get_unchecked_mut()
            .page_manager
            .insert(vpn, page);
        
    }

    fn is_legal(&self, _scause: Scause) -> bool {
        todo!()
    }
}
