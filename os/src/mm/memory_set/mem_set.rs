//! memory_set模块

/*
    1. 懒分配
        1）maturin: 在插入虚拟逻辑段的时候，使用push_lazy函数，插入时分配None的Frame
                    同时写入物理地址为0的pte。之后在page_fault中，使用get_frame来得到frame,
                    如果没有分配，则进行分配。
        2）Tiantix: 同样在插入虚拟逻辑段的时候，使用push_lazy函数，插入的时候直接不做映射。在发生
                    page_fault时，让对应的page_fault进行分配，并做好映射。
    2. 数据结构
        1) page_table
        2) areas 不同vma的集合，使用BTreeMap管理
        3）heap_range（与brk系统调用有关，可以用上那个区间管理的东西）
    3. 函数功能
        1）new 和 new_from_global(用户态的地址空间，有内核的映射)
        2）token
        3）通过vpn找vm_area
        4) 插入vma，主要就是两个push函数
        5）读写（其实就是使用下层的
        6）page_fault
        7）克隆地址空间 用在fork、clone函数
        还有一些莫名奇妙的函数，反正先不管吧，那些函数也是需要去看大量代码才能理解的，先实现一些通用的功能。
*/

use alloc::sync::Arc;
use log::info;
use riscv::register::scause::Scause;

use crate::{
    config::{
        mm::{LOW_LIMIT, MEMORY_END, PAGE_SIZE, PHY_TO_VIRT_OFFSET, USER_UPPER_LIMIT}, 
        task::{CORE_STACK_SIZE, MAX_CORE_NUM}
    }, driver::qemu::MMIO, mm::{
        address::{align_down, byte_array, phys_to_ppn, ppn_to_phys, virt_to_vpn, vpn_to_virt}, cow::CowManager, page_table::PageTable, 
        pma::BackenFile, type_cast::{MapPermission, PTEFlags}, 
        vma::{MapType, VirtMemoryAddr, VmaType}, vma_range::vma_range::VmaRange
    }, syscall::error::{Errno, OSResult}, utils::cell::SyncUnsafeCell
};

use super::page_fault::PageFaultHandler;


pub struct MemeorySet {
    // TODO: areas需不需要加锁？？
    pub areas: VmaRange,
    // 底下没有数据结构拥有页表，所以不用Arc，没有多个所有者
    // pt的借用关系难以管理，所以使用cell,但是为什么要使用sync? (一般情况下在多线程中传递引用)
    pub pt: SyncUnsafeCell<PageTable>,
    pub heap_end: usize,
    // is_user: bool,
    // pub heap_range
    pub cow_manager: CowManager,
}

// 这里没有选择上一把大锁，而是上细粒度锁，上在page_table
pub static mut KERNEL_SPACE: Option<MemeorySet> = None;

pub fn init_kernel_space() {
    info!("[kernel]: Start to initialize kernel space.");
    unsafe {
        KERNEL_SPACE = Some(MemeorySet::new_kernel());
        KERNEL_SPACE.as_ref().unwrap().activate();
    }
    info!("[kernel]: Kernel space finished!");
}

// 切换为内核的地址空间
pub fn kernel_space_activate() {
    unsafe { KERNEL_SPACE.as_ref().map(MemeorySet::activate); }
}

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sstack();
    fn estack();
    fn sbss();
    fn ebss();
    fn ekernel();
}

impl MemeorySet {
    /*
        function: 完成内核地址空间的初始化
        TODO: 还没有考虑其他的sections，例如Trampoline
        地址空间的创建一般都是虚拟空间转物理地址空间。（注意逻辑）但是这里我们先有了物理地址空间，于是要假装没有物理地址空间
        创建好虚拟地址空间，然后固定的映射到先前的物理地址去。
     */
    pub fn new_kernel() -> Self {
        let mut kernel_memory_set = MemeorySet {
            areas: VmaRange::new(), 
            pt: SyncUnsafeCell::new(PageTable::new()),
            heap_end: 0,
            cow_manager: CowManager::new()
        };

        kernel_memory_set.push(
            VirtMemoryAddr::new(
                stext as usize, 
                etext as usize, 
                MapPermission::R | MapPermission::X, 
                MapType::Direct, 
                VmaType::Elf,
                None
            ),
            None,
            0
        );
        info!(
            "[kernel] initial kernel. [stext..etext] is [{:#x}..{:#x}]",
            stext as usize, etext as usize,
        );

        kernel_memory_set.push(
            VirtMemoryAddr::new(
                srodata as usize, 
                erodata as usize, 
                MapPermission::R, 
                MapType::Direct, 
                VmaType::Elf,
                None
            ),
            None,
            0
        );
        info!(
            "[kernel] initial kernel. [srodata..erodata] is [{:#x}..{:#x}]",
            srodata as usize, erodata as usize,
        );
        kernel_memory_set.push(
            VirtMemoryAddr::new(
                sdata as usize,
                edata as usize, 
                MapPermission::R | MapPermission::W, 
                MapType::Direct, 
                VmaType::Elf,
                None
            ),
            None,
            0
        );
        info!(
            "[kernel] initial kernel. [sdata..edata] is [{:#x}..{:#x}]",
            sdata as usize, edata as usize,
        );

        for cpu_id in 0..MAX_CORE_NUM {
            let per_stack_top = (estack as usize) - CORE_STACK_SIZE * cpu_id;
            let per_stack_bottom = per_stack_top - CORE_STACK_SIZE + PAGE_SIZE;
            kernel_memory_set.push(
                VirtMemoryAddr::new(
                    per_stack_bottom as usize, 
                    per_stack_top as usize, 
                    MapPermission::R | MapPermission::W, 
                    MapType::Direct, 
                    VmaType::Elf,
                    None
                ),
                None,
                0
            );
            info!(
                "[kernel] initial cpu_id:{} kernel. [sstack..estack] is [{:#x}..{:#x}]",
                cpu_id, per_stack_bottom as usize, per_stack_top as usize,
            );
        }
        
        kernel_memory_set.push(
            VirtMemoryAddr::new(
                sbss as usize, 
                ebss as usize, 
                MapPermission::R | MapPermission::W, 
                MapType::Direct, 
                VmaType::Elf,
                None
            ),
            None,
            0
        );
        info!(
            "[kernel] initial kernel. [sbss..ebss] is [{:#x}..{:#x}]",
            sbss as usize, ebss as usize,
        );
        kernel_memory_set.push(
            VirtMemoryAddr::new(
                ekernel as usize, 
                MEMORY_END + PHY_TO_VIRT_OFFSET as usize, 
                MapPermission::R | MapPermission::W, 
                MapType::Direct, 
                VmaType::PhysFrame,
                None
            ),
            None,
            0
        );
        info!(
            "[kernel] initial kernel. [ekernel..MEMORY_END] is [{:#x}..{:#x}]",
            ekernel as usize, MEMORY_END + PHY_TO_VIRT_OFFSET as usize,
        );
        for (name, start, len, map_per) in MMIO {
            info!(
                "[kernel] initial kernel. [MMIO]{} is [{:#x}..{:#x}]",
                name, start + PHY_TO_VIRT_OFFSET, start + len + PHY_TO_VIRT_OFFSET,
            );
            kernel_memory_set.push(
                VirtMemoryAddr::new(
                    start + PHY_TO_VIRT_OFFSET,
                    start + len + PHY_TO_VIRT_OFFSET,
                    *map_per,
                    MapType::Direct,
                    VmaType::Mmio,
                    None,
                ),
                None,
                0,
            );
        }
        info!("[kernel] Initail kernel finished!");
        kernel_memory_set
    }

    /* Function: 创建一个用户的虚拟地址空间，并包含了内核的地址空间
       Assumption：内核的地址空间不需要加入其中，只需要做好page_table的映射即可
     */
    pub fn new_user() -> Self {
        // 从内核中的页表里映射好了相关的数据
        let pt = SyncUnsafeCell::new(PageTable::new_user());
        Self {
            areas: VmaRange::new(),
            pt,
            heap_end: 0,
            cow_manager: CowManager::new(),
        }
    }

    pub fn token(&self) -> usize {
        self.pt.get_unchecked_mut().token()
    }

    pub fn activate(&self) {
        self.pt.get_unchecked_mut().activate();
    }

    // 分配一个vma空壳
    // 从vma_range中找到合适的start
    pub fn alloc_vma_anywhere(
        &self, 
        hint: usize, 
        len: usize, 
        map_permission: MapPermission,
        map_type: MapType,
        handler: Option<Arc<dyn PageFaultHandler>>,
    ) -> Option<VirtMemoryAddr> {
        // TODO: if end == start ???? DONE：其实并没有关系，只是不会做映射！
        self.areas.find_anywhere(hint, len).map(|start_va| {
            VirtMemoryAddr::new(
                start_va,
                start_va + len,
                map_permission,
                map_type,
                VmaType::Mmap,
                handler,
            )
        })
    }

    // 分配 固定虚拟地址 的vma空壳
    // 非 Direct 类型
    pub fn alloc_vma_fixed(
        &mut self,
        start: usize, 
        end: usize,
        map_permission: MapPermission,
        map_type: MapType,
        handler: Option<Arc<dyn PageFaultHandler>>,
    ) -> Option<VirtMemoryAddr> {
        // TODO: if end == start ???? DONE：其实并没有关系，只是不会做映射！
        self.areas.find_fixed(start, end, self.pt.get_unchecked_mut()).map(|start_va| {
            VirtMemoryAddr::new(
                start_va,
                end,
                map_permission,
                map_type,
                VmaType::Mmap,
                handler,
            )
        })
    }

    /* Function: 为 vma（分配物理页 + 做映射 + 插入memory_set) 
                 如果有数据需要写入物理页，则先分配，再写入
    */ 
    pub fn push(&mut self, vm_area: VirtMemoryAddr, data: Option<&[u8]>, offset: usize) {
        vm_area.map_all(self.pt.get_unchecked_mut());
        // 如果是elf文件，则需要将某个段中的data内容放入物理页帧中
        if data.is_some() {
            vm_area.copy_data(&data.unwrap(), offset, self.pt.get_unchecked_mut());
        }
        self.areas.insert_raw(vm_area);
    }

    // 懒分配，为 vma (插入memory_set)
    pub fn push_no_map(&mut self, vm_area: VirtMemoryAddr) {
        self.areas.insert_raw(vm_area);
    }

    pub fn mmap(
        &mut self,
        vma: VirtMemoryAddr,
        backen_file: Option<BackenFile>,
    ) -> Option<usize> {
        let start_addr = vma.start_vaddr;
        if backen_file.is_some() {
            vma.pma.get_unchecked_mut().add_backen_file(backen_file.unwrap());
        }
        self.push_no_map(vma);
        Some(start_addr)
    }

    pub fn mprotect(&mut self, start: usize, end: usize, new_flags: MapPermission) {
        self.areas.mprotect(start, end, new_flags, &mut self.pt.get_unchecked_mut())
    }

    pub fn munmap(&mut self, start: usize, end: usize) {
        self.areas.unmap(start, end, &mut self.pt.get_unchecked_mut())
    }

    pub fn expand(&mut self, start: usize, end: usize) ->OSResult<bool> {
        self.areas.expand(start, end)
    }

    pub fn find_vm_mut_by_vpn(&mut self, vpn: usize) -> Option<&mut VirtMemoryAddr> {
        if let Some((_, vma)) = 
            self.areas
                .segments
                .iter_mut()
                .find(
                    |(_, vma)|
                    vma.vma_range().contains(&vpn)
                ) {
                    Some(vma)
                }
        else {
            None
        }
    }

    pub fn find_vm_by_vaddr(&self, vaddr: usize) -> Option<&VirtMemoryAddr> {
        if let Some((_, vma)) = 
            self.areas
                .segments
                .iter()
                .find(|(_, vma)| vma.is_contain(vaddr)) {
                    Some(vma)
                }
        else {
            None
        }
    }

    pub fn handle_page_fault(&self, vaddr: usize, scause: Scause) -> OSResult<()>{
        for (_, area) in self.areas.segments.iter() {
            info!("Vma area from {:#x} ~ {:#x}", area.start_vaddr, area.end_vaddr);
        }
        if let Some(vma) = self.find_vm_by_vaddr(vaddr) {
                vma.handle_page_fault(vaddr, self.pt.get_unchecked_mut(), scause);
                Ok(())
            }
        // 对应的虚拟地址没有对应的虚拟地址空间！
        else {
            info!("[handler_page_fault]: No corresponding vma in mem_set. va is {:x}", vaddr);
            for (_, area) in self.areas.segments.iter() {
                info!("Vma area from {:#x} ~ {:#x}", area.start_vaddr, area.end_vaddr);
            }
            Err(Errno::EFAULT)
        }
    }

    // 在fork, clone, exec 等系统调用中，用于创建一个新的地址空间。
    // 同时做好 COW （copy-on-write）
    pub fn from_user_lazily(&self) -> Self {
        let mut ms = MemeorySet::new_user();
        ms.cow_manager.from_other_cow(
            &self.cow_manager, 
            &mut ms.pt.get_unchecked_mut()
        );
        for (_, vma) in self
            .areas
            .segments
            .iter() {
                // 复制一模一样的虚拟逻辑段 TODO：有没有可能虚拟地址会重合？？？ 重合没有关系
                let new_vma = VirtMemoryAddr::from_another(&vma);
                for vpn in vma.vma_range() {
                    if let Some(page) = vma
                        .pma
                        .get_unchecked_mut()
                        .page_manager
                        .get(&vpn) {
                            // 这里存在 physical frame，所以要做一个特殊的映射
                            let old_pte = self
                                .pt
                                .get_unchecked_mut()
                                .find_pte(vpn)
                                .unwrap();
                            let mut new_flags = old_pte.flags();
                            new_flags |= PTEFlags::COW;
                            new_flags.remove(PTEFlags::W);
                            old_pte.set_flags(new_flags);
                            let ppn = page.frame.ppn;
                            ms.pt.get_unchecked_mut().map_one(vpn, ppn, new_flags);
                            ms.cow_manager
                                .page_manager
                                .get_unchecked_mut()
                                .insert(vpn, page.clone());
                            self.cow_manager
                                .page_manager
                                .get_unchecked_mut()
                                .insert(vpn, page.clone());
                        }
                    // 没有页，则可能是懒分配，或者是 Direct类型
                    else {
                        todo!()
                    }
                }
            ms.push_no_map(new_vma);
        }
        ms
    }

    pub fn from_user(&self) -> Self {
        let mut ms = MemeorySet::new_user();
        for (_, vma) in self
            .areas
            .segments
            .iter() {
                let new_vma = VirtMemoryAddr::from_another(&vma);
                for vpn in vma.vma_range() {
                    if let Some(pa) = self.pt.get_unchecked_mut().translate_va_to_pa(vpn_to_virt(vpn)) {
                        let src_ppn = phys_to_ppn(pa);
                        let dst_ppn = new_vma.map_one(ms.pt.get_unchecked_mut(), vpn, None);
                        byte_array(ppn_to_phys(dst_ppn)).copy_from_slice(&byte_array(ppn_to_phys(src_ppn)));
                    }
                }
            ms.push_no_map(new_vma);
        }
        ms.heap_end = self.heap_end;
        ms
    }

    /* Function: 清理用户地址空间中的数据
     */
    pub fn clear_user_space(&mut self) {
        self.areas.unmap(LOW_LIMIT, USER_UPPER_LIMIT, self.pt.get_mut());
        // self.pt.get_unchecked_mut().activate();
        // TODO: 待修改
        self.heap_end = 0;
    }

}

#[allow(unused)]
/// 检查 page_table
pub fn remap_test() {
    info!("remap_test start...");
    let kernel_space = unsafe { KERNEL_SPACE.as_ref().unwrap() };
    let mid_text = (stext as usize + (etext as usize - stext as usize) / 2);
    let mid_rodata =
        (srodata as usize + (erodata as usize - srodata as usize) / 2);
    let mid_data = (sdata as usize + (edata as usize - sdata as usize) / 2);
    // log::info!(
    //     "mid text {:#x}, mid rodata {:#x}, mid data {:#x}",
    //     mid_text, mid_rodata, mid_data
    // );
    unsafe {
        assert!(!(*kernel_space.pt.get())
            .translate_vpn_to_pte(virt_to_vpn(align_down(mid_text)))
            .unwrap()
            .is_writable()
            );
        assert!(!(*kernel_space.pt.get())
            .translate_vpn_to_pte(virt_to_vpn(align_down(mid_rodata)))
            .unwrap()
            .is_writable());
        assert!(!(*kernel_space.pt.get())
            .translate_vpn_to_pte(virt_to_vpn(align_down(mid_data)))
            .unwrap()
            .is_executable());
    }
    info!("remap_test passed!");
}