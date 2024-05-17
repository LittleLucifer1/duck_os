/*！
    堆分配器，使用buddy_system中的Locked_Heap.
    TODO： 暂时先这样，之后可能要更改锁的类型
*/
use buddy_system_allocator::LockedHeap;
use crate::config::mm::KERNEL_HEAP_SIZE;


// struct GlobalHeap(SpinNoIrqLock<Heap::<32>>);

// impl GlobalHeap {
//     const fn empty() -> Self {
//         Self(SpinNoIrqLock::new(Heap::empty()))
//     }
// }

// unsafe impl GlobalAlloc for GlobalHeap {
//     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
//         self.0
//             .lock()
//             .alloc(layout)
//             .ok()
//             .map_or(0 as *mut u8, |allocation| allocation.as_ptr())
//     }

//     unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
//         self.0.lock().dealloc(NonNull::new_unchecked(ptr), layout)
//     }
// }

#[global_allocator]
// static HEAP_ALLOCATOR: GlobalHeap = GlobalHeap::empty();
static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::<32>::new();

// Initial the heap
pub fn init_heap() {
    // warning: 这里其实可以使用usize,但是要考虑不同机器上的usize大小
    static mut HEAP: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];
    log::info!("[kernel]: Initialize heap.");
    unsafe {
        HEAP_ALLOCATOR
            .lock()
            .init(HEAP.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
    log::trace!("[kernel]: Heap range: {:X}-{:X}, size: {:X}", 
        unsafe{HEAP.as_ptr() as usize}, 
        unsafe{HEAP.as_ptr() as usize + KERNEL_HEAP_SIZE},
        KERNEL_HEAP_SIZE);
}

#[allow(unused)]
pub fn heap_test() {
    log::info!("[test]: Start heap_test");
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    extern "C" {
        fn sbss();
        fn ebss();
    }
    
    let bss_range = sbss as usize..ebss as usize;
    let a = Box::new(5);
    assert_eq!(*a, 5);
    assert!(bss_range.contains(&(a.as_ref() as *const _ as usize)));
    drop(a);
    let mut v: Vec<usize> = Vec::new();
    let max_len = (KERNEL_HEAP_SIZE - 10000) / core::mem::size_of::<usize>();
    for i in 0..500.min(max_len) {
        v.push(i);
    }
    for (i, val) in v.iter().take(500).enumerate() {
        assert_eq!(*val, i);
    }
    assert!(bss_range.contains(&(v.as_ptr()as usize )));
    drop(v);
    log::info!("[test]: Heap_test passed!");
}
