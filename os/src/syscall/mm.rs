//! 系统调用模块

use alloc::sync::Arc;
use log::{debug, info};

use crate::{mm::{address::vaddr_is_align, memory_set::page_fault::MmapPageFaultHandler, pma::BackenFile, type_cast::{MapPermission, MmapFlags, MmapProt}, vma::MapType}, process::hart::cpu::{get_cpu_id, get_cpu_local}};

use super::error::{Errno, SyscallResult};

/* Description: map or unmap files or devices into memory
    addr： 1. 如果是NULL，则随机选择移一处地址？
           2. 如果不是NULL，则将其做为hint，选择一处合适的地址
    prot: 访问权限
    flags：1. MAP_SHARED：进程间映射相同位置的修改是可见的，同时使用write through同步策略
           2. MAP_PRIVATE: 创建一个私有的COW映射。和上述的相反
           3. MAP_ANONYMOUS: 这段映射空间没有backen file，内容初始化为0,fd忽略，offset必须为0
           4. MAP_FIXED：放在addr确切的位置，同时addr必须是aligned。如果无法使用特定的address，则失败。
    fd/len/offset：在文件offset的位置，长度为len的字节流
    注意事项：1）len一定要大于0
            2）offset一定是 page alignment
            4）prot必须和文件的打开方式不冲突
            3）mmap() call 返回之后，文件描述符fd可以立即关闭而不会使映射失效
            5）文件支持的映射，可能要更新文件的相关时间
    返回值: 新映射空间地址的首地址
    TODO: 还有部分的flags没有考虑。
 */
pub fn sys_mmap(
    addr: usize,
    length: usize,
    prot: i32,
    flags: i32,
    fd: usize,
    offset: usize,
) -> SyscallResult {
    info!("[sys_mmap]: addr: 0x{:x}, length: {}, fd: {}, offset: {}", addr, length, fd, offset);
    if !vaddr_is_align(offset) || 
        length == 0 || 
        !vaddr_is_align(addr) {
        debug!("I don't like the parameter!");
        return Err(Errno::EINVAL);
    }
    let prot = MmapProt::from_bits_truncate(prot as u32);
    let mut flags = MmapFlags::from_bits_truncate(flags as u32);
    if !flags.intersects(MmapFlags::MAP_PRIVATE | MmapFlags::MAP_SHARED) {
        debug!("flags contained none of MAP_PRIVATE, MAP_SHARED, or MAP_SHARED_VALIDATE");
        return Err(Errno::EINVAL);
    };
    flags.remove(MmapFlags::MAP_PRIVATE); // TODO：暂时不处理关于这个flags的操作，所以固定规定是SHARED
    flags.insert(MmapFlags::MAP_SHARED);
    // Debug: 这个地方的bug在于，映射时的权限没有U，但是在内核中访问用户地址空间，是需要U这个权限的。
    let mut map_permission = prot.into();
    map_permission |= MapPermission::U;
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let mut vm_lock = current_task.vm.lock();
    let start_addr: Option<usize>;
    let handler = MmapPageFaultHandler {};
    // 1. 分配一个vma
    let vma = if flags.contains(MmapFlags::MAP_FIXED) {
        if addr == 0 {
            return Err(Errno::EINVAL);
        }
        vm_lock.alloc_vma_fixed(
                addr, 
                addr + length,
                map_permission, 
                MapType::Framed, 
                Some(Arc::new(handler))
            )
    } else {
        vm_lock.alloc_vma_anywhere(
                addr, 
                length, 
                map_permission, 
                MapType::Framed, 
                Some(Arc::new(handler))
            )
    }.ok_or(Errno::ENOMEM)?;
    // 2. 处理backen_file
    if flags.contains(MmapFlags::MAP_ANONYMOUS) {
        if offset != 0 {
            return Err(Errno::EINVAL);
        }
        start_addr = vm_lock.mmap(vma, None);
    }
    else {
        let fd_table_lock = current_task.fd_table.lock();
        let file = fd_table_lock.fd_table.get(&fd).ok_or(Errno::EBADF)?;
        let backen_file = BackenFile::new(
            offset, 
            Arc::clone(&file.file),
        );
        start_addr = vm_lock.mmap(vma, Some(backen_file));
    }
    // vm_lock.activate();
    info!("[sys_mmap]: start_addr is 0x{:x}", start_addr.unwrap());
    Ok(start_addr.unwrap())
}

/* Description: changes the access protections on a region of memory
    注意事项：1. address range: [addr, addr + len - 1]
            2. addr必须是 page alignment
            3. 如果进程尝试访问内存，其方式和protections冲突，则会发送SIGSEGV信号
 */
pub fn sys_mprotect(addr: usize, len: usize, prot: u32) -> SyscallResult {
    if !vaddr_is_align(addr) {
        return Err(Errno::EINVAL);
    }
    let flags = MmapProt::from_bits(prot).ok_or(Errno::EINVAL)?;
    info!("[sys_mprotect]: addr is 0x{:x} ~ 0x{:x}, prot is {:?}", addr, addr + len, flags);
    if flags.contains(MmapProt::PROT_NONE) {
        return Ok(0);
    }
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    current_task.vm.lock().mprotect(addr, addr+len, flags.into());
    current_task.vm.lock().activate();
    Ok(0)
}

/* Description: unmap
    addr: 必须是page alignment
    length: 不必是 page alignment
    注意事项：如果指定的范围不包含任何映射页面，则不会报错
    TODO: 无法细粒度的munmap一页的内容！例如我在0x0 ~ 0x500处有一个映射内容，我想munmap 0x0 ～ 0x200的内容，结果是会把整个一页全都删了。
*/
pub fn sys_munmap(addr: usize, length: usize) -> SyscallResult {
    info!("[sys_munmap]: addr range 0x{:x} ~ 0x{:x}, length: 0x{:x}", addr, addr + length, length);
    if !vaddr_is_align(addr) ||
        length == 0 {
        return Err(Errno::EINVAL);
    }
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    current_task.vm.lock().munmap(addr, addr + length);
    current_task.vm.lock().activate();
    Ok(0)
}

/* Description: change data segment size(heap size)
    addr: 如果为0,则返回堆顶的位置值
    注意事项：1. 在data段之上，不能超过一定的值。
          2. 不能和mmap区域的映射重叠
          3. 必须是page alignment
*/
pub fn sys_brk(addr: usize) -> SyscallResult {
    info!("[sys_brk]: addr: 0x{:x}", addr);
    // if !vaddr_is_align(addr) {
    //     return Err(Errno::EINVAL);
    // }
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let mut vm_lock = current_task.vm.lock();
    info!("[sys_brk]: old_heap_end: 0x{:x}", vm_lock.heap_end);
    if addr == 0 {
        return Ok(vm_lock.heap_end);
    }
    let heap_end = vm_lock.heap_end;
    // TODO: 这里地址相关的逻辑判断还不确定！
    if addr > heap_end {
        vm_lock.expand(heap_end, addr)?;
        vm_lock.heap_end = addr;
    } else if addr < heap_end {
        vm_lock.munmap(addr, heap_end);
    }
    vm_lock.activate();
    // 如果addr == heap_end, 就啥也不做
    Ok(0)
}

// pub fn sys_msync() {

// }