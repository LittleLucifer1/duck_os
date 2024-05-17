//! 设备驱动模块
//! 

use alloc::sync::Arc;

use crate::sync::SpinNoIrqLock;

use self::qemu::virt_block::VirtIOBlock;
pub mod qemu;

pub trait BlockDevice: Send + Sync {
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    fn write_block(&self, block_id: usize, buf: &[u8]);
}

pub trait CharDevice: Send + Sync {
    fn getchar(&self) -> u8;
    fn puts(&self, char: &[u8]);
}

pub static BLOCK_DEVICE: SpinNoIrqLock<Option<Arc<dyn BlockDevice>>> = SpinNoIrqLock::new(None);

pub fn init_block_device() {
    #[cfg(feature="qemu")]
    {
        *BLOCK_DEVICE.lock() = Some(Arc::new(VirtIOBlock::new()));
    }
}

#[allow(unused)]
pub fn block_device_test() {
    println!("[test]: Start block_device_test");
    let block_device = BLOCK_DEVICE.lock().as_ref().unwrap().clone();
    let mut write_buffer = [0u8; 512];
    let mut read_buffer = [0u8; 512];
    for i in 0..4 {
        for byte in write_buffer.iter_mut() {
            *byte = i as u8;
        }
        block_device.write_block(i as usize, &write_buffer);
        block_device.read_block(i as usize, &mut read_buffer);
        assert_eq!(write_buffer, read_buffer);
    }
    
    println!("block device test passed!");
}

