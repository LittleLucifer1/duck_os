//! 地址转换函数

/*
    1. 虚拟地址和物理地址之间的转换
    2. 页号和地址之间的转换
    3. align相关的函数，etc...
    4. 解引用物理地址直接得到值 (mut 和 不可变)（T 和 byte）
*/

use crate::config::mm::{LOW_LIMIT, PADDR_HIGH, PADDR_LOW, PAGE_SIZE, PAGE_SIZE_BITS, PHY_TO_VIRT_OFFSET, SV39_VPN_1, SV39_VPN_2, SV39_VPN_3, USER_UPPER_LIMIT, VADDR_HIGH, VADDR_LOW, VIRTIO0};

use super::page_table::PageTableEntry;

pub type VirtAddr = usize;
pub type PhysAddr = usize;

pub fn vaddr_is_align(vaddr: VirtAddr) -> bool {
    (vaddr & (PAGE_SIZE - 1)) == 0
}

pub fn check_kernel_va(vaddr: VirtAddr) {
    assert!((VADDR_LOW <= vaddr && vaddr <= VADDR_HIGH) || 
        (VIRTIO0 <= vaddr && vaddr <= VIRTIO0 + 0x4000));
}

pub fn check_user_va(vaddr: VirtAddr) {
    assert!((VADDR_LOW <= vaddr && vaddr <= VADDR_HIGH) || 
    (VIRTIO0 <= vaddr && vaddr <= VIRTIO0 + 0x4000) ||
    ((LOW_LIMIT <= vaddr && vaddr <= USER_UPPER_LIMIT))
    );
}

/// 默认了 paddr 在 低32位 仅限内核地址空间
#[allow(unused)]
pub fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    assert!(PADDR_LOW <= paddr && paddr <= PADDR_HIGH);
    paddr | PHY_TO_VIRT_OFFSET
}

/// 物理地址 ---> 物理页号
pub fn phys_to_ppn(paddr: PhysAddr) -> usize {
    assert!(PADDR_LOW <= paddr && paddr <= PADDR_HIGH);
    paddr >> PAGE_SIZE_BITS
}

/// 物理地址 ---> 下一个物理页号
pub fn phys_to_ppn_next(paddr: PhysAddr) -> usize {
    assert!(PADDR_LOW <= paddr && paddr <= PADDR_HIGH);
    ( paddr >> PAGE_SIZE_BITS ) + 1
}

/// 虚拟地址转为物理地址 仅限内核地址空间
pub fn virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
    check_kernel_va(vaddr);
    vaddr - PHY_TO_VIRT_OFFSET
}

/// 虚拟地址转页号 (页首)
pub fn virt_to_vpn(vaddr: VirtAddr) -> usize {
    check_user_va(vaddr);
    vaddr >> PAGE_SIZE_BITS
}

/// 虚拟地址转下一页的页号
pub fn virt_to_next_vpn(vaddr: VirtAddr) -> usize {
    check_user_va(vaddr);
    (vaddr >> PAGE_SIZE_BITS) + 1
}

/// 虚拟地址所在页的页首地址
pub fn align_down(vaddr: VirtAddr) -> VirtAddr {
    check_user_va(vaddr);
    vaddr & !(PAGE_SIZE - 1)
}

/// 虚拟地址 页首则保持为页首位置，页中值则为下一页的页首位置
pub fn align_up(vaddr: VirtAddr) -> VirtAddr {
    check_user_va(vaddr);
    (vaddr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

/// 虚拟页号 --> 虚拟地址
pub fn vpn_to_virt(vpn: usize) -> VirtAddr {
    // assert!(VADDR_LOW <= (vpn << PAGE_SIZE_BITS) 
    //     && (vpn << PAGE_SIZE_BITS) <= VADDR_HIGH);
    check_user_va(vpn << PAGE_SIZE_BITS);
    vpn << PAGE_SIZE_BITS
}

/// 物理页号 ---> 物理地址
pub fn ppn_to_phys(ppn: usize) -> PhysAddr {
    ppn << PAGE_SIZE_BITS
}

/// 地址 -> 页内偏移 offset
pub fn vaddr_offset(vaddr: VirtAddr) -> usize {
    check_user_va(vaddr);
    vaddr & (PAGE_SIZE - 1)
}

/// 用户地址空间中 虚拟地址查询物理地址 pte的三级 (vpn2 vpn1 vpn0)
pub fn vaddr_to_pte_vpn(vaddr: VirtAddr) -> (usize, usize, usize) {
    check_user_va(vaddr);
    (
        (vaddr >> SV39_VPN_3) & 0x1ff,
        (vaddr >> SV39_VPN_2) & 0x1ff,
        (vaddr >> SV39_VPN_1) & 0x1ff
    )
}

/// 此处的物理地址，由于在内核中不能直接访问，所以需要将其转换为虚拟地址。
/// 一般情况下，无法从pa转换为va,但是内核地址空间是我们人为的映射过去的，所以这个offset已经确定了。
/// 于是就可以转换。但是仅限内核地址空间使用，用户地址空间无法使用这个函数！
/// 直接从物理地址 解引用获得pte array  4k的页面有512个usize
pub fn pte_array(paddr: PhysAddr) -> &'static mut [PageTableEntry] {
    assert!(PADDR_LOW <= paddr && paddr <= PADDR_HIGH);
    unsafe {
        core::slice::from_raw_parts_mut(phys_to_virt(paddr) as *mut PageTableEntry, 512)
    }
}

/// 从物理地址 解引用获得byte array
pub fn byte_array(paddr: PhysAddr) -> &'static mut [u8] {
    assert!(PADDR_LOW <= paddr && paddr <= PADDR_HIGH);
    unsafe {
        core::slice::from_raw_parts_mut(phys_to_virt(paddr) as *mut u8, PAGE_SIZE)
    }
}

/// 从物理地址 解引用获得结构体 T immutable
pub fn get_ref<T>(paddr: PhysAddr) -> &'static T {
    assert!(PADDR_LOW <= paddr && paddr <= PADDR_HIGH);
    unsafe {
        (phys_to_virt(paddr) as *const T).as_ref().unwrap()
    }
}
/// 从物理地址 解引用获得结构体 T mutable
pub fn get_mut<T>(paddr: PhysAddr) -> &'static mut T {
    assert!(PADDR_LOW <= paddr && paddr <= PADDR_HIGH);
    unsafe {
        (phys_to_virt(paddr) as *mut T).as_mut().unwrap()
    }
}

#[allow(unused)]
pub fn address_test() {
    log::info!("[test]: Start address test");
    use crate::mm::address::*;
    let va = 0xffff_ffff_8065_4321usize;
    let vpn = 0xffff_ffff_8065_4usize;
    let nxt_vpn = 0xffff_ffff_8065_5usize;
    let align_d = 0xffff_ffff_8065_4000usize;
    let align_u = 0xffff_ffff_8065_5000usize;
    let va_off = 0x321usize;
    let pa = 0x8065_4321usize;
    let ppn = 0x8065_4usize;
    let nxt_ppn = 0x8065_5usize;
    assert_eq!(phys_to_virt(pa), va);
    assert_eq!(phys_to_ppn(pa), ppn);
    assert_eq!(phys_to_ppn_next(pa), nxt_ppn);
    assert_eq!(virt_to_phys(va), pa);
    assert_eq!(virt_to_vpn(va), vpn);
    assert_eq!(virt_to_next_vpn(va), nxt_vpn);
    assert_eq!(align_down(va), align_d);
    assert_eq!(align_up(va), align_u);
    assert_eq!(vpn_to_virt(vpn), align_d);
    assert_eq!(ppn_to_phys(ppn), 0x8065_4000);
    assert_eq!(vaddr_offset(va), va_off);
    let (va1, va2, va3) = vaddr_to_pte_vpn(va);
    println!("{}, {}, {}", va1, va2, va3);
    log::info!("[test]: Address_test passed!");
}