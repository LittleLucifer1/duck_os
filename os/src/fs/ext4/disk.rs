use alloc::sync::Arc;

use lwext4_rust::{
    bindings::{SEEK_CUR, SEEK_END, SEEK_SET},
    KernelDevOp,
};

use crate::{config::fs::SECTOR_SIZE, driver::BlockDevice, syscall::error::OSResult};

/// A disk device with a cursor.
pub struct Disk {
    block_id: usize,
    offset: usize,
    dev: Arc<dyn BlockDevice>,
}

impl Disk {
    /// Create a new disk.
    pub fn new(dev: Arc<dyn BlockDevice>) -> Self {
        Self {
            block_id: 0,
            offset: 0,
            dev,
        }
    }

    /// Get the size of the disk.
    pub fn size(&self) -> u64 {
        self.dev.size()
    }

    /// Get the position of the cursor.
    pub fn position(&self) -> u64 {
        self.block_id as u64 * SECTOR_SIZE as u64 + self.offset as u64
    }

    /// Set the position of the cursor.
    pub fn set_position(&mut self, pos: u64) {
        self.block_id = pos as usize / SECTOR_SIZE;
        self.offset = pos as usize % SECTOR_SIZE;
    }

    /// Read within one block, returns the number of bytes read.
    pub fn read_one(&mut self, buf: &mut [u8]) -> OSResult<usize> {
        // trace!("block id: {}", self.block_id);
        let read_size = if self.offset == 0 && buf.len() >= SECTOR_SIZE {
            // whole block
            self.dev.read_block(self.block_id, &mut buf[0..SECTOR_SIZE]);
            self.block_id += 1;
            SECTOR_SIZE
        } else {
            // partial block
            let mut data = [0u8; SECTOR_SIZE];
            let start = self.offset;
            let count = buf.len().min(SECTOR_SIZE - self.offset);
            if start > SECTOR_SIZE {
                log::trace!("block size: {} start {}", SECTOR_SIZE, start);
            }

            self.dev.read_block(self.block_id, &mut data);
            buf[..count].copy_from_slice(&data[start..start + count]);

            self.offset += count;
            if self.offset >= SECTOR_SIZE {
                self.block_id += 1;
                self.offset -= SECTOR_SIZE;
            }
            count
        };
        Ok(read_size)
    }

    /// Write within one block, returns the number of bytes written.
    pub fn write_one(&mut self, buf: &[u8]) -> OSResult<usize> {
        let write_size = if self.offset == 0 && buf.len() >= SECTOR_SIZE {
            // whole block
            self.dev.write_block(self.block_id, &buf[0..SECTOR_SIZE]);
            self.block_id += 1;
            SECTOR_SIZE
        } else {
            // partial block
            let mut data = [0u8; SECTOR_SIZE];
            let start = self.offset;
            let count = buf.len().min(SECTOR_SIZE - self.offset);

            self.dev.read_block(self.block_id, &mut data);
            data[start..start + count].copy_from_slice(&buf[..count]);
            self.dev.write_block(self.block_id, &data);

            self.offset += count;
            if self.offset >= SECTOR_SIZE {
                self.block_id += 1;
                self.offset -= SECTOR_SIZE;
            }
            count
        };
        Ok(write_size)
    }
}

impl KernelDevOp for Disk {
    type DevType = Disk;

    fn read(dev: &mut Disk, mut buf: &mut [u8]) -> Result<usize, i32> {
        log::trace!("READ block device buf={}", buf.len());
        let mut read_len = 0;
        while !buf.is_empty() {
            match dev.read_one(buf) {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                    read_len += n;
                }
                Err(_e) => return Err(-1),
            }
        }
        log::trace!("READ rt len={}", read_len);
        Ok(read_len)
    }
    fn write(dev: &mut Self::DevType, mut buf: &[u8]) -> Result<usize, i32> {
        log::trace!("WRITE block device buf={}", buf.len());
        let mut write_len = 0;
        while !buf.is_empty() {
            match dev.write_one(buf) {
                Ok(0) => break,
                Ok(n) => {
                    buf = &buf[n..];
                    write_len += n;
                }
                Err(_e) => return Err(-1),
            }
        }
        log::trace!("WRITE rt len={}", write_len);
        Ok(write_len)
    }
    fn flush(_dev: &mut Self::DevType) -> Result<usize, i32> {
        Ok(0)
    }
    fn seek(dev: &mut Disk, off: i64, whence: i32) -> Result<i64, i32> {
        let size = dev.size();
        log::trace!(
            "SEEK block device size:{}, pos:{}, offset={}, whence={}",
            size,
            &dev.position(),
            off,
            whence
        );
        let new_pos = match whence as u32 {
            SEEK_SET => Some(off),
            SEEK_CUR => dev.position().checked_add_signed(off).map(|v| v as i64),
            SEEK_END => size.checked_add_signed(off).map(|v| v as i64),
            _ => {
                log::error!("invalid seek() whence: {}", whence);
                Some(off)
            }
        }
        .ok_or(-1)?;

        if new_pos as u64 > size {
            log::warn!("Seek beyond the end of the block device");
        }
        dev.set_position(new_pos as u64);
        Ok(new_pos)
    }
}
