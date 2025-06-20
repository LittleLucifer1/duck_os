//! 虚拟地址 逻辑段 一段访问权限相同，位置相邻的地址空间。
//！或者可以看作是多个页 pages

/*
    1. 数据结构 vma
        1) pma, 包括当前的物理空间页（管理器） + 可能的back_file。
            而page是页，有frames + flags + file_info（页cache相关的信息）
        2) vma的类型
           elf、user_stack、mmap、user_heap
        3）page_table(可以不要，从地址空间传下来)
        4）start 和 end（用于区间变化操作）
        5）mmap的port，这个和文件的相同
        6）mmap的flag 种类
        7）page_fault_handler,实现对不同种类的vma的分发。如果使用match,则使得代码耦合度较高

    2. 功能
        1）new
        2）from_another (用于fork)
        3）page_fault
        4) map 和 unmap（在创建vma之后，需要映射到物理地址，可以懒分配或者正常分配）
        5）copy_data
            (待定，在Titanix中，这个是用做map_elf的，但是在maturin中，加载elf的部分则单独放在了
            loader模块，所以maturin中没有这个函数。因为我暂时对这个东西不了解，所以先不管它。而且这个函数
            肯定是用在装载文件，例如第一次在内核中加载一个初始化的elf和之后通过sys_exec加载的elf文件)
            maturin装载这一部分的代码我还没有看，所以我不知道如何处理?!?!?!?!?!?
        6）大致没了
*/

use core::ops::Range;

use crate::{config::mm::PAGE_SIZE, utils::cell::SyncUnsafeCell};

use alloc::sync::Arc;
use log::info;
use riscv::register::scause::Scause;

use crate::config::mm::PHY_TO_VIRT_PPN_OFFSET;

use super::{
    address::{align_down, align_up, virt_to_vpn, VirtAddr}, 
    memory_set::page_fault::PageFaultHandler, 
    page_table::PageTable, 
    pma::{Page,  PhysMemoryAddr}, 
    type_cast::{MapPermission, PTEFlags, PagePermission}, 
    vma_range::{SplitOverlap, UnmapOverlap}
};


#[derive(Clone, Copy)]
pub enum VmaType {
    Elf,
    UserStack,
    Mmap,
    UserHeap,
    PhysFrame,
    Mmio,
    Interp,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum MapType {
    Framed,
    Direct,
}

/*  TODO: 这里的数据类型是否要加锁等之类的问题还要仔细考虑
    Direct类型vma：没有page_fault_handler, 没有pma(页帧)
    Assumption: vaddr的范围头尾值都 页对齐了
*/ 
pub struct VirtMemoryAddr {
    // 这里暂时的认为一个物理地址空间只会被最多一个虚拟地址空间所拥有
    // 所以不使用Arc，同时加入更加弱的锁 SyncUnsafeCell
    pub pma: SyncUnsafeCell<PhysMemoryAddr>,
    pub start_vaddr: VirtAddr,
    pub end_vaddr: VirtAddr,

    pub map_permission: MapPermission,
    pub vma_type: VmaType,
    pub map_type: MapType,

    // TODO: 待解决
    pub page_fault_handler: Option<Arc<dyn PageFaultHandler>>,
}

impl VirtMemoryAddr {
    /*
        Assumption: 根据地址参数构建vma
        TODO：有很多函数其实都默认了addr会对齐页边。如果不对齐会发生什么？？  
     */
    pub fn new(
        // 从push中创建一个Vma的时候，不应该让上层去初始化pma这个东西
        // 因为上层不需要关心这个东西
        start_vaddr: VirtAddr,
        end_vaddr: VirtAddr,
        map_permission: MapPermission,
        map_type: MapType,
        vma_type: VmaType,
        handler: Option<Arc<dyn PageFaultHandler>>,
    ) -> Self {
        // 先检查输入的地址是否对齐页 TODO: 没想明白，之后解决
        // if (vaddr_offset(start_vaddr) == 0usize) | (vaddr_offset(end_vaddr) == 0usize) {
        //     todo!()
        // }
        Self {
            pma: SyncUnsafeCell::new(PhysMemoryAddr::new()),
            start_vaddr: align_down(start_vaddr),
            end_vaddr: align_up(end_vaddr),
            map_permission,
            vma_type,
            map_type,
            page_fault_handler: handler,
        }
    }

    pub fn from_another(another: &Self) -> Self {
        Self {
            pma: SyncUnsafeCell::new(PhysMemoryAddr::new()),
            start_vaddr: another.start_vaddr,
            end_vaddr: another.end_vaddr,
            map_permission: another.map_permission,
            vma_type: another.vma_type,
            map_type: another.map_type,
            page_fault_handler: match another.page_fault_handler.as_ref() {
                Some(handler) => Some(handler.clone()),
                None => None,
            }
        }
    }

    // 分配物理帧、映射
    // Direct：不用分配物理帧，pma保持为空
    // Frame: 分配物理帧
    // function: 完成page table映射，并将page插入pma中
    // TODO： 每次只处理一页？？？？
    pub fn map_one(&self, pt: &mut PageTable, vpn: usize, page: Option<Arc<Page>>) -> usize {
        let pma = self.pma.get_unchecked_mut();
        let ppn: usize;
        match self.map_type {
            MapType::Direct => {
                // 有逻辑问题，万一vpn不在高地址？但是只有高地址的内核地址才会直接映射
                assert!(vpn >= PHY_TO_VIRT_PPN_OFFSET);
                ppn = vpn - PHY_TO_VIRT_PPN_OFFSET;
            },
            MapType::Framed => {
                let data_page = match page {
                    Some(p) => p,
                    None => {
                        Arc::new(Page::new(PagePermission::from(self.map_permission)))
                    }
                };
                ppn = data_page.frame.ppn;
                pma.push_pma_page(vpn, data_page);
            }
        }
        pt.map_one(vpn, ppn, PTEFlags::from(self.map_permission));
        ppn
    }

    // 暂时可能没有内核的分配，所以这里只是`MapType::Framed`的类型
    // function：完成映射（映射到物理地址为0处）+ 不插入pma中
    fn map_one_lazily(&self, pt: &mut PageTable, vpn: usize) {
        assert!(self.map_type == MapType::Framed);
        pt.map_one(vpn, 0, PTEFlags::empty());
    }
    /// 解映射
    pub fn unmap(&self, pt: &mut PageTable, vpn: usize) {
        if self.map_type == MapType::Framed {
            self.pma.get_unchecked_mut().pop_pma_page(vpn);
        }
        pt.unmap(vpn);
    }

    pub fn map_self_all_lazy(&self, pt: &mut PageTable) {
        for vpn in self.vma_range() {
            self.map_one_lazily(pt, vpn);
        }
    }

    // 默认：vma是空壳，物理帧为None
    // TODO: 这个函数有一个默认的条件？？？
    pub fn map_self_all(&self, pt:&mut PageTable) {
        for vpn in self.vma_range() {
            self.map_one(pt, vpn, None);
        }
    }

    pub fn handle_page_fault(&self, vaddr: VirtAddr, pt: &mut PageTable, scause: Scause) {
        self.page_fault_handler.as_ref().map(|handler| {
            handler.handler_page_fault(
                self.pma.get_unchecked_mut(),
                vaddr,
                self.start_vaddr, 
                self.map_permission,
                None, scause, pt
            )
        });
    }

    pub fn vma_range(&self) -> Range<usize> {
        let start_vpn = virt_to_vpn(self.start_vaddr);
        let end_vpn = virt_to_vpn(self.end_vaddr);
        start_vpn..end_vpn
    }
    
    // Function: 向物理页中写入数据，写入的长度为data_len，写入页中的开始位置为offset
    // offset: 最开始写入的地址对应页中的offset位置
    // data：长度不受限制，所以可以写入很多页
    // start_va: 最开始写入地址的虚拟地址
    pub fn write_data_to_page(&self, start_va: usize, data: &[u8], offset: usize) {
        let mut start = 0usize;
        let mut offset = offset;
        // let mut current_va = self.start_vaddr;
        let mut current_va = start_va;
        let max_len = data.len();
        loop {
            let end = max_len.min(start + PAGE_SIZE - offset);
            let vpn = virt_to_vpn(current_va);
            self.pma.get_unchecked_mut()
                .write_data_to_page(vpn, &data[start..end], offset);
            start += PAGE_SIZE - offset;
            if start >= max_len {
                break;
            }
            offset = 0;
            current_va += PAGE_SIZE;
        }
    }

    // Function: 向物理页中读出数据，写入的长度为data_len，写入页中的开始位置为offset
    // offset: 最开始读出的地址对应页中的offset位置
    // data：长度不受限制，所以可以读出很多页
    // start_va: 最开始读出地址的虚拟地址
    pub fn read_data_from_page(&self, start_va: usize, data: &mut [u8], offset: usize) {
        let mut start = 0usize;
        let mut offset = offset;
        // let mut current_va = self.start_vaddr;
        let mut current_va = start_va;
        let max_len = data.len();
        loop {
            let end = max_len.min(start + PAGE_SIZE - offset);
            let vpn = virt_to_vpn(current_va);
            self.pma.get_unchecked_mut()
                .read_data_from_page(vpn, &mut data[start..end], offset);
            start += PAGE_SIZE - offset;
            if start >= max_len {
                break;
            }
            offset = 0;
            current_va += PAGE_SIZE;
        }
    }


    pub fn is_backen_file(&self) -> bool {
        self.pma.get_unchecked_mut().backen_file.is_some()
    }
}


// 区间相关的操作
impl VirtMemoryAddr {
    // 修改这段区间的flags
    // TODO: 无法修改页面的flags，暂时不考虑修改页面的flags吧，最多把映射相关的pte改掉！
    // 因为现在的实现中，不需要判断page flags的值。这个属性没啥用！
    // Titanix中，貌似这里只修改了map_permission，其他的一律没有修改。
    pub fn modify(&mut self, new_flags: MapPermission, pt: &mut PageTable) {
        // 修改了区间的
        let new_pte_flags = PTEFlags::from(new_flags);
        self.map_permission = new_flags;
        // 修改page_table中的
        let page_manager = &mut self.pma.get_unchecked_mut().page_manager;
        for (&vpn, _page) in  page_manager {
            // page.set_permission(PagePermission::from(new_pte_flags));
            pt.modify_flags(vpn, new_pte_flags);
        }
    }

    // 移除整个虚拟逻辑段区间
    pub fn remove(&mut self, pt: &mut PageTable) {
        for vpn in virt_to_vpn(self.start_vaddr)..virt_to_vpn(self.end_vaddr) {
            self.unmap(pt, vpn)
        }
    }

    // 扩展地址空间,目前只有brk使用了这个函数,并且只想高处扩展,同时不会分配相关的物理页面 + 也不会映射相关的地址
    pub fn expand(&mut self, new_end: usize) {
        self.end_vaddr = new_end;
    }

    // 分裂为[start, pos），并返回[pos, end)
    pub fn split(&mut self, pos: usize) -> Self {
        let old_end = self.end_vaddr;
        self.end_vaddr = pos;
        let right_pma = 
            self.pma
            .get_unchecked_mut()
            .split(pos, pos, self.start_vaddr, old_end);
        Self {
            pma: SyncUnsafeCell::new(right_pma),
            start_vaddr: pos,
            end_vaddr: old_end,
            map_permission: self.map_permission,
            vma_type: self.vma_type,
            map_type: self.map_type,
            page_fault_handler: self.page_fault_handler.clone(),
        }
    }

    pub fn is_contain(&self, pos: usize) -> bool {
        self.start_vaddr <= pos && pos <= self.end_vaddr
    }

    pub fn unmap_if_overlap(&mut self, start: usize, end: usize, pt: &mut PageTable) -> UnmapOverlap {
        let start = align_down(start);
        let end = align_up(end);
        if !self.is_overlap(start, end) {
            UnmapOverlap::Unchange
        } else if start <= self.start_vaddr {
            if end < self.end_vaddr {
                // 左边相交
                let right_vma = self.split(end);
                self.remove(pt);
                *self = right_vma;
                UnmapOverlap::Shrink
            } else {
                // 包括了原有的区间
                self.remove(pt);
                UnmapOverlap::Removed
            }
        } else if end < self.end_vaddr {
            // 被原有的区间包括了
            let right_vma = self.split(end);
            self.split(start).remove(pt);
            UnmapOverlap::Split(right_vma)
        } else {
            // 右边有相交
            self.split(start).remove(pt);
            UnmapOverlap::Shrink
        }
    }

    pub fn split_and_modify_if_overlap(&mut self, start: usize, end: usize, new_flags: MapPermission, pt: &mut PageTable) -> SplitOverlap {
        if !self.is_overlap(start, end) {
            SplitOverlap::Unchange
        } else if start <= self.start_vaddr {
            if end < self.end_vaddr {
                // 左边相交 修改左边的值，再把右边的返回出去
                let right_vma = self.split(end);
                self.modify(new_flags, pt);
                SplitOverlap::ShrinkLeft(right_vma)
            } else {
                // 包括了原有的区间
                self.modify(new_flags, pt);
                SplitOverlap::Modified
            }
        } else if end < self.end_vaddr {
            // 被原有的区间包括了
            let right_vma = self.split(end);
            let mut middle_vma = self.split(start);
            middle_vma.modify(new_flags, pt);
            SplitOverlap::Split(middle_vma, right_vma)
        } else {
            // 右边有相交
            let mut right_vma = self.split(start);
            right_vma.modify(new_flags, pt);
            SplitOverlap::ShrinkRight(right_vma)
        }
    }

    pub fn is_overlap(&self, start: usize, end: usize) -> bool {
        assert!(start <= end);
        !(end <= self.start_vaddr || start >= self.end_vaddr)
    }
}