//! 内存模块
/*
暂时先处理 堆、页帧、虚拟逻辑段、地址之间转换、地址空间、物理地址（pma）；
*/
mod page_table;
pub mod vma;
pub mod allocator;
pub mod memory_set;
pub mod address;
pub mod pma;
mod vma_range;
pub mod cow;
/// 
pub mod type_cast;

// 内存方面的初始化 heap, frame, kernel_space
pub fn init() {
    log::info!("[kernel]: Initialize memory");
    allocator::heap::init_heap();
    // allocator::heap::heap_test();
    allocator::frame::init_frame_allocator();
    // allocator::frame::frame_test();
    // address::address_test();
    memory_set::mem_set::init_kernel_space();
    // memory_set::mem_set::remap_test();
}