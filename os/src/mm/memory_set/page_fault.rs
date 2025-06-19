//! 专门处理多种不同的 page_fault
/*
    1. page_fault种类
        1） sbrk
        2） mmap
        3） user_stack
        4)  user_heap
*/

use alloc::{collections::btree_map::BTreeMap, sync::{Arc, Weak}};
use riscv::register::scause::Scause;

use crate::mm::{address::{virt_to_vpn, VirtAddr}, page_table::PageTable, pma::{Page, PhysMemoryAddr}, type_cast::{MapPermission, PTEFlags, PagePermission}};

pub trait PageFaultHandler: Send + Sync {
    // 懒分配：已经插入了对应的vma，只是没有做映射和物理帧分配
    // 所以只需要 映射 + 将分配的物理帧插入对应的 vma 中
    fn handler_page_fault(
        &self,
        _pma: &mut PhysMemoryAddr,
        _vaddr: VirtAddr,
        _start_va: VirtAddr,
        _permission: MapPermission,
        _cow_page_manager: Option<&mut BTreeMap<usize, Arc<Page>>>,
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
            pma: &mut PhysMemoryAddr,
            vaddr: VirtAddr,
            _start_va: VirtAddr,
            permission: MapPermission,
            _cow_page_manager: Option<&mut BTreeMap<usize, Arc<Page>>>,
            _scause: Scause,
            pt: &mut PageTable,
        ) {
        let page = Page::new(PagePermission::from(permission));
        let ppn = page.frame.ppn;
        let vpn = virt_to_vpn(vaddr);
        pma.page_manager
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
            pma: &mut PhysMemoryAddr,
            vaddr: VirtAddr,
            _start_va: VirtAddr,
            permission: MapPermission,
            _cow_page_manager: Option<&mut BTreeMap<usize, Arc<Page>>>,
            _scause: Scause,
            pt: &mut PageTable,
        ) {
            let page = Page::new(PagePermission::from(permission));
            let ppn = page.frame.ppn;
            let vpn = virt_to_vpn(vaddr);
            pma.page_manager
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
            pma: &mut PhysMemoryAddr,
            vaddr: VirtAddr,
            start_va: VirtAddr,
            permission: MapPermission,
            _cow_page_manager: Option<&mut BTreeMap<usize, Arc<Page>>>,
            _scause: Scause,
            pt: &mut PageTable,
        ) {
        // 2. 如果有backen file，则从文件的page cache中拿出page，同时将文件中的内容放入其中
        if pma.backen_file.is_some() {
            let backen_file = pma.backen_file.as_ref().unwrap().clone();
            let offset = backen_file.offset + vaddr - start_va;
            let inode = Weak::clone(&backen_file.file.metadata().f_inode);
            let page = backen_file
                .file
                .metadata()
                .page_cache
                .as_ref()
                .unwrap()
                .find_page_and_create(offset, Some(inode))
                .unwrap_or_else(|| panic!("[page_fault.rs] Read page wrong"));
            page.load();
            let ppn = page.frame.ppn;
            let vpn = virt_to_vpn(vaddr);
            pma.page_manager
                .insert(
                    vpn, 
                    Arc::clone(&page),
                );
            pt.map_one(vpn, ppn, permission.into());
            pt.activate()
        }
        // 1. 如果没有backen file，则分配一个空页面
        else {
            let page = Page::new(PagePermission::from(permission));
            let ppn = page.frame.ppn;
            let vpn = virt_to_vpn(vaddr);
            pma.page_manager
                .insert(
                    vpn, 
                    Arc::new(page),
                );
            // DONE: 这里的PTE是根据 prot来设置的，暂时没有检查这部分的内容; 应该没有什么问题
            let flag = permission.into();
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
            pma: &mut PhysMemoryAddr,
            vaddr: VirtAddr,
            _start_va: VirtAddr,
            _permission: MapPermission,
            cow_page_manager: Option<&mut BTreeMap<usize, Arc<Page>>>,
            _scause: Scause,
            pt: &mut PageTable,
        ) {
        // let pte = pt.find_pte(vaddr).unwrap();
        let pte = pt.translate_va_to_pte(vaddr).unwrap();
        debug_assert!(pte.flags().contains(PTEFlags::COW));
        debug_assert!(!pte.flags().contains(PTEFlags::W));

        let mut flags = pte.flags() | PTEFlags::W;
        flags.remove(PTEFlags::COW);
        
        let vpn = virt_to_vpn(vaddr);
        let page = cow_page_manager.unwrap()
            .remove(&vpn)
            .unwrap();

        // 复制这个page 
        // 这里有一个暴力的做法：不管是不是最后一个指向这个页，统一的复制再创造一个新页。
        let new_page = Page::new_from_page(page.frame.ppn, page.permission);
        
        
        pt.unmap(vpn);
        pt.map_one(vpn, new_page.frame.ppn, flags);
        pt.activate();
        pma.page_manager.insert(vpn, Arc::new(new_page));

        // vma.pma.get_unchecked_mut().push_pma_page(vpn, page);

        
    }

    fn is_legal(&self, _scause: Scause) -> bool {
        todo!()
    }
}
