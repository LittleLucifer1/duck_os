use core::ptr::NonNull;

use alloc::vec::Vec;
use virtio_drivers::{device::blk::VirtIOBlk, transport::mmio::{MmioTransport, VirtIOHeader}, Hal};

use crate::{config::mm::VIRTIO0, driver::BlockDevice, mm::{address::{phys_to_ppn, phys_to_virt, ppn_to_phys}, allocator::frame::{alloc_contiguous_frame, dealloc_frame, FrameTracker}, memory_set::mem_set::KERNEL_SPACE}, sync::SpinNoIrqLock};

pub struct VirtIOBlock(SpinNoIrqLock<VirtIOBlk<VirtioHal, MmioTransport>>);

unsafe impl Send for VirtIOBlock {}
unsafe impl Sync for VirtIOBlock {}

impl BlockDevice for VirtIOBlock {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let result = self.0.lock().read_blocks(block_id, buf);
        match result {
            Ok(_) => (),
            Err(_) => panic!("[Kernel](VirtIoBlock): Read error in block_id:{}", block_id)
        }
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        let result = self.0.lock().write_blocks(block_id, buf);
        match result {
            // Ok(_) => debug!("[Kernle](VirtIoBlock): Write block at {}", block_id),
            Ok(_) => (),
            Err(_) => panic!("[Kernle](VirtIoBlock): Write error in block_id:{}", block_id)
        }
    }
}

impl VirtIOBlock {
    pub fn new() -> Self {
        unsafe {
            let header = NonNull::new(VIRTIO0 as *mut VirtIOHeader).unwrap();
            Self(SpinNoIrqLock::new(
                VirtIOBlk::<VirtioHal, MmioTransport>::new(
                    MmioTransport::new(header).expect("Initial MmioTransport error"),
                )
                .unwrap(),
            ))
        }
    }
}

static QUEUE_FRAMES: SpinNoIrqLock<Vec<FrameTracker>> = SpinNoIrqLock::new(Vec::new());
pub struct VirtioHal;

unsafe impl Hal for VirtioHal {
    fn dma_alloc(
        pages: usize, 
        _direction: virtio_drivers::BufferDirection
    ) -> (virtio_drivers::PhysAddr, core::ptr::NonNull<u8>) {
        let mut queue_frame_locked = QUEUE_FRAMES.lock();
        let mut frames = alloc_contiguous_frame(pages).unwrap();
        let pa = ppn_to_phys(frames[0].ppn);
        for _ in 0..pages {
            queue_frame_locked.push(frames.pop().unwrap());
        }
        (pa, unsafe {
            NonNull::new_unchecked(phys_to_virt(pa) as *mut u8)
        })
    }

    unsafe fn dma_dealloc(
        paddr: virtio_drivers::PhysAddr, 
        _vaddr: core::ptr::NonNull<u8>, 
        pages: usize
    ) -> i32 {
        let mut ppn = phys_to_ppn(paddr);
        for _ in 0..pages {
            dealloc_frame(ppn);
            ppn += 1;
        }
        0
    }

    unsafe fn mmio_phys_to_virt(
        paddr: virtio_drivers::PhysAddr, 
        _size: usize
    ) -> core::ptr::NonNull<u8> {
        log::info!("[kernel](virt_block): paddr is {:X}", paddr);
        NonNull::new_unchecked(phys_to_virt(paddr) as *mut u8)
    }
    
    unsafe fn share(
        buffer: core::ptr::NonNull<[u8]>, 
        _direction: virtio_drivers::BufferDirection
    ) -> virtio_drivers::PhysAddr {
       unsafe {
        let pa = 
        KERNEL_SPACE.as_ref().expect("Not initial")
            .pt
            .get_unchecked_mut()
            .translate_va_to_pa(
                buffer.as_ptr() as *const usize as usize
            ).unwrap();
        pa
       }
    }

    unsafe fn unshare(
        _paddr: virtio_drivers::PhysAddr, 
        _buffer: core::ptr::NonNull<[u8]>, 
        _direction: virtio_drivers::BufferDirection
    ) {}
}