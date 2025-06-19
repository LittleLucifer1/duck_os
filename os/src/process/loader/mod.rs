//！ 加载模块 —— 静态 + 动态加载器模块，参考 2023年作品 Titanix 

use core::str::from_utf8;

use alloc::{string::{String, ToString}, sync::Arc, vec::Vec, vec};
use log::info;
use virtio_drivers::PAGE_SIZE;

use crate::{
    config::mm::{DL_INTERP_OFFSET, USER_STACK_SIZE, USER_STACK_TOP}, fs::{dentry::path_to_dentry, file::File, info::OpenFlags, page_cache::PageCache}, mm::{
        address::{align_up, vaddr_offset}, 
        memory_set::{mem_set::MemorySet, page_fault::{UHeapPageFaultHandler, UStackPageFaultHandler}}, 
        type_cast::MapPermission, 
        vma::{MapType, VirtMemoryAddr, VmaType}
    }, syscall::error::OSResult
};
use self::stack::StackInfo;

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

// Function: 映射不同段、映射进程的 user_stack、heap、处理栈中的auxv、argc、argv
// Return：(entry_point, ustack_sp, StackLayout)
pub fn load_elf(data: &[u8], vm: &mut MemorySet, args: Vec<String>, envs: Vec<String>) -> (usize, usize, StackInfo) {
    let elf = xmas_elf::ElfFile::new(&data).unwrap();
    // 检查魔数
    if !check_magic(&elf) {
        panic!("ELF magic wrong");
    }
    // 开始映射
    let mut entry_point = elf.header.pt2.entry_point() as usize;
    let dl_value = load_dl_interp(&elf, vm);
    let heap_start: usize = map_elf_at(&elf, None, vm, 0).expect("[loader mod.rs] Wrong");
    
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
    info!("The ustack start is 0x{:x}, ustack end is 0x{:x}", user_stack_bottom, user_stack_top);
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
    info!("The heap start is 0x{:x}, heap end is 0x{:x}", heap_start, heap_end);
    
    // 需要构建user stack中的内容，无论什么情况，都需要构建argc,argv,auxv的结构
    let mut stack_info = StackInfo::empty();
    stack_info.init_arg(args, envs);
    stack_info.init_auxv(&elf);
    if let Some((entry, base)) = dl_value {
        stack_info.set_auxv_at_base(base);
        entry_point = entry;
    } else {
        stack_info.set_auxv_at_base(0);
    }
    
    info!("The entry_point is {:x}, user_stack_top is {:x}, user_stack_bottom is {:x}", entry_point, user_stack_top, user_stack_bottom);
    (entry_point, user_stack_top, stack_info)
    
}

// Function: 根据elf决定映射相关的段，返回映射所有段中段最高的位置
// 如果文件中有 Page_cache，说明这个文件数据被加载到了内存中，不需要再分配page了
fn map_elf_at(elf: &xmas_elf::ElfFile, file: Option<Arc<dyn File>>, vm: &mut MemorySet, base_addr: usize) -> OSResult<usize> {
    let mut max_end = 0usize;
    
    for ph in elf.program_iter() {
        if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
            let start_va = ph.virtual_addr() as usize + base_addr;
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

            let mut page_cache: Option<Arc<PageCache>> = None;
            if let Some(file) = file.as_ref() {
                page_cache = Some(file.metadata().page_cache.as_ref().unwrap().clone());
            }

            if !map_permission.contains(MapPermission::W) && page_cache.is_some() {
                let mut file_offset = ph.offset() as usize;
                let page_cache = page_cache.unwrap();
                let vma = VirtMemoryAddr::new(
                    start_va,
                    end_va, 
                    map_permission, 
                    MapType::Framed,
                    VmaType::Elf,
                    None);
                for vpn in vma.vma_range() {
                    let page = page_cache.find_page(file_offset);
                    vma.map_one(&mut vm.pt.get_unchecked_mut(), vpn, page);
                    file_offset += PAGE_SIZE;
                }
                vm.push_no_map(vma);
            } else {
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
            }
            info!("[map_elf_at]: map vma from elf, start addr is 0x{:x?}, end addr is 0x{:x?}", start_va, end_va);
            max_end = max_end.max(align_up(end_va));
        }
    }
    Ok(max_end)
}

fn load_dl_interp(elf: &xmas_elf::ElfFile, vm: &mut MemorySet) -> Option<(usize, usize)> {
    let mut interp_path: Option<String> = None;

    for ph in elf.program_iter() {
        if let Ok(xmas_elf::program::Type::Interp) = ph.get_type() {
            let offset = ph.offset() as usize;
            let size = ph.file_size() as usize;
            let raw = &elf.input[offset..offset + size];
            let path = from_utf8(raw).unwrap().trim_end_matches('\0').to_string();
            interp_path = Some(path);
            break;
        }
    }

    let interp_path = interp_path?;

    let mut candidates = vec![interp_path.clone()];
    if interp_path == "/lib/ld-musl-riscv64.so.1" || interp_path == "/lib/ld-musl-riscv64-sf.so.1" {
        candidates.push("/libc.so".to_string());
        candidates.push("/lib/libc.so".to_string());
    }

    let mut file: Option<Arc<dyn File>> = None;
    for path in candidates {
        if let Some(dentry) = path_to_dentry(&path).expect("[loader mod.rs] Unimplemented") {
            file = dentry.open(dentry.clone(), OpenFlags::O_RDONLY).ok();
            break;
        }
    }

    let file = file?;
    let mut data = Vec::new();
    file.read_all(&mut data, OpenFlags::O_RDONLY).ok()?;

    let interp_elf = xmas_elf::ElfFile::new(&data).ok()?;

    // 将动态链接器加载到固定基址
    let interp_base = DL_INTERP_OFFSET;
    map_elf_at(&interp_elf, Some(file), vm, interp_base).ok()?;

    Some((interp_elf.header.pt2.entry_point() as usize + DL_INTERP_OFFSET, interp_base))
}