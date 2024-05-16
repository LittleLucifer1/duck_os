//！ 加载模块 —— 目前只实现静态加载

use alloc::{string::String, sync::Arc, vec::Vec};
use log::info;

use crate::{
    config::mm::{USER_STACK_TOP, USER_STACK_SIZE}, 
    mm::{
        address::{align_up, vaddr_offset}, 
        memory_set::{mem_set::MemeorySet, page_fault::{UHeapPageFaultHandler, UStackPageFaultHandler}}, 
        type_cast::MapPermission, 
        vma::{MapType, VirtMemoryAddr, VmaType}
    }
};
use self::stack::{StackInfo, StackLayout};

pub mod dynamic;
pub mod stack;

pub fn check_magic(elf: &xmas_elf::ElfFile) -> bool {
    let mut ans: bool = true;
    let magic_num:[u8; 4] = [0x7f, 0x45, 0x4c, 0x46];
    for i in 0..magic_num.len() {
       if magic_num[i] != elf.header.pt1.magic[i] {
            ans = false;
       }
    }
    ans
}

// 返回值：(entry_point, ustack_sp, StackLayout)
pub fn load_elf(data: &[u8], vm: &mut MemeorySet, args: Vec<String>, envs: Vec<String>) -> (usize, usize, Option<StackLayout>) {
    let elf = xmas_elf::ElfFile::new(&data).unwrap();
    // 检查魔数
    if !check_magic(&elf) {
        panic!("ELF magic wrong");
    }
    // 开始映射
    let ph_count = elf.header.pt2.ph_count();
    // TODO：当这个为动态链接时，会被修改
    let mut entry_point = elf.header.pt2.entry_point() as usize;
    let mut heap_start: usize = 0;
    for i in 0..ph_count {
        let ph = elf.program_header(i).unwrap();
        if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
            let start_va = ph.virtual_addr() as usize;
            let end_va = start_va + ph.mem_size() as usize;
            let mut map_permission = MapPermission::U;
            let ph_flags = ph.flags();
            if ph_flags.is_read() {
                map_permission |= MapPermission::R;
            }
            if ph_flags.is_write() {
                map_permission |= MapPermission::W;
            }
            if ph_flags.is_execute() {
                map_permission |= MapPermission::X;
            }
            // TODO：优化的地方：首先可以对elf的内容进行lazy复制
            // 其次如果elf的内容已经在内存中，且是不可写，则可以共享page
            vm.push(VirtMemoryAddr::new(
                start_va,
                end_va, 
                map_permission, 
                MapType::Framed,
                VmaType::Elf,
                None,),
                Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                vaddr_offset(start_va)
            );
            heap_start = heap_start.max(align_up(end_va));
        }
    }
    // 映射用户栈
    let user_stack_top = USER_STACK_TOP;
    let user_stack_bottom = user_stack_top - USER_STACK_SIZE;
    // TODO： 修改为push_no_map的形式
    vm.push(VirtMemoryAddr::new(
        user_stack_bottom, 
        user_stack_top,
        MapPermission::U | MapPermission::R | MapPermission::W, 
        MapType::Framed, 
        VmaType::UserStack,
        Some(Arc::new(UStackPageFaultHandler {}))
        ),
        None,
        0
    );
    // TODO: 这里的堆到底有没有成功映射 DONE：有，只不过没有任何的映射数据罢了。
    let heap_end = heap_start;
    vm.push(VirtMemoryAddr::new(
        heap_start, 
        heap_end, 
        MapPermission::U | MapPermission::R | MapPermission::W, 
        MapType::Framed, 
        VmaType::UserHeap,
        Some(Arc::new(UHeapPageFaultHandler {})) 
        ),
        None,
        0
    );
    vm.heap_end = heap_end;
    info!("The heap start is 0x{:x}, heap end is 0x{:X}", heap_start, heap_end);
    let mut stack_layout: Option<StackLayout> = None;
    // 需要构建user stack中的内容
    if !args.is_empty() || !envs.is_empty() {
        // 传递auxv的相关值
        let mut stack_info = StackInfo::empty();
        stack_info.init_arg(args, envs);
        stack_info.init_auxv(&elf);

        let (_sp, layout) = stack_info.build_stack(user_stack_top);
        stack_layout = Some(layout);
    }
    info!("The entry_point is {:x}, user_stack_top is {:x}, user_stack_bottom is {:x}", entry_point, user_stack_top, user_stack_bottom);
    (entry_point, user_stack_top, stack_layout)
    
}