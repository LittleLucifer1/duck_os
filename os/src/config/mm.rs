/*!
    页大小
    页对应的字节
    内核地址的offset
    堆的大小
*/

/// 内核中堆的大小
pub const KERNEL_HEAP_SIZE: usize = 0xc0_0000; // 12MB: 1 represent 1 bit

/// 内核虚拟地址与物理地址的offset
pub const PHY_TO_VIRT_OFFSET: usize = 0xffff_ffff_0000_0000;

/// 内核虚拟页号与物理页号的offset
pub const PHY_TO_VIRT_PPN_OFFSET: usize = 0xffff_ffff_0000_0;

/// 页的大小bit
pub const PAGE_SIZE_BITS: usize = 0xc;

/// 页的大小 4kb
pub const PAGE_SIZE: usize = 0x1000;

/// vpn不同的索引对应的不同的位数
pub const SV39_VPN_1: usize = 12;
/// vpn不同的索引对应的不同的位数
pub const SV39_VPN_2: usize = 21;
/// vpn不同的索引对应的不同的位数
pub const SV39_VPN_3: usize = 30;

/// physical frame memory 终点位置
pub const MEMORY_END: usize = 0x8800_0000;

/// 内核物理地址最高处
pub const PADDR_HIGH: usize = 0x8800_0000;

/// 内核物理地址最低处
pub const PADDR_LOW: usize = 0x8020_0000;

/// 内核虚拟地址最高处
pub const VADDR_HIGH: usize = 0x8800_0000 + PHY_TO_VIRT_OFFSET;

/// 内核虚拟地址最低处
pub const VADDR_LOW: usize = 0x8020_0000 + PHY_TO_VIRT_OFFSET;

/// LOW_LIMIT mmap函数中使用的
pub const LOW_LIMIT: usize = 0x0;

/// UPPER_LIMIT mmap函数中使用的
pub const UPPER_LIMIT: usize = 0xffff_ffff_ffff_ffff;

/// 用户地址空间的最高地址 ——> 用户地址空间有 4G 
pub const USER_UPPER_LIMIT: usize = 0xffff_ffff;

/// 用户栈大小 8Mb
pub const USER_STACK_SIZE: usize = 0x800000; 

/// 用户栈的初始栈顶和栈底 （用户地址空间的倒数第二页）
pub const USER_STACK_TOP: usize = 0xFFFF_F000;

// virtio-driver的映射位置
pub const VIRTIO0: usize = 0x1000_1000 + PHY_TO_VIRT_OFFSET;

// 0xffff_ffff_8000_0000 -> 0x8000_0000的映射表项位置
pub const KERNEL_PTE_POS: usize = 510;

// 0xffff_ffff_1000_0000 ~ 0xffff_ffff_4000_0000 的区域
pub const KERNEL_MMIO_PTE_POS: usize = 508;

// Mmap区域的最高处
pub const MMAP_TOP: usize = 0x0f5f_f000;

// Mmap区域的最低处
pub const MMAP_BOTTOM: usize = 0x0e5f_f000;