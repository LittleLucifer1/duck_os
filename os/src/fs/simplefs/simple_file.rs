use alloc::sync::Arc;
use log::debug;

use crate::{config::mm::PAGE_SIZE, fs::{file::{File, FileMeta}, info::{OpenFlags, TimeSpec}}, syscall::error::{Errno, OSResult}};

pub struct SimpleFile {
    pub meta: FileMeta,
}

impl SimpleFile {
    pub fn new(meta: FileMeta) -> Self {
        Self { meta }
    }
}

impl File for SimpleFile {
    fn metadata(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, buf: &mut [u8], flags: OpenFlags) -> OSResult<usize> {
        if flags.contains(OpenFlags::O_PATH) {
            debug!("[sys_read]: The flags contain O_PATH, file is not opened actually.");
            return Err(Errno::EBADF);
        }
        let inode = self.meta.f_inode.clone().upgrade().unwrap();
        let mut file_inner = self.meta.inner.lock();
        let pos = file_inner.f_pos;
        let page_cache = Arc::clone(self.meta.page_cache.as_ref().unwrap());

        let mut buf_offset = 0usize;
        let mut file_offset = pos;
        let mut total_len = 0usize;
        let buf_len = buf.len();
        
        loop {
            // 如果超过文件尾或者大于buf的长度，则不再读了
            if buf_offset >= buf_len {
                break;
            }
            let page = page_cache.find_page_and_create(file_offset, None);
            if page.is_none() {
                break; // EOF! 
            }
            let page_offset = file_offset % PAGE_SIZE;
            let mut byte = PAGE_SIZE - page_offset;
            byte = byte.min(buf_len - buf_offset);

            page.unwrap().read(page_offset, &mut buf[buf_offset..buf_offset+byte]);
            buf_offset += byte;
            file_offset += byte;
            total_len += byte;
            file_inner.f_pos = file_offset;
        }
        drop(file_inner);
        // TODO: 没搞懂这个东西的逻辑
        if !flags.contains(OpenFlags::O_NOATIME) {
            inode.metadata().inner.lock().i_atime = TimeSpec::new();
        }
        Ok(total_len)
    }

    fn write(&self, buf: &[u8], flags: OpenFlags) -> OSResult<usize> {
        if flags.contains(OpenFlags::O_PATH) {
            debug!("[sys_write]: The flags contain O_PATH, file is not opened actually.");
            return Err(Errno::EBADF);
        }
        let inode = self.meta.f_inode.clone().upgrade().unwrap();
        // 防止其他进程修改这里的pos，统一在成功读完之后，再释放这个地方的锁
        let mut file_inner = self.meta.inner.lock();
        let pos = file_inner.f_pos;
        let page_cache = Arc::clone(self.meta.page_cache.as_ref().unwrap());

        let mut buf_offset = 0usize;
        let mut file_offset = pos;
        let mut total_len = 0usize;
        let buf_len = buf.len();
        loop {
            // Unsafe: 这里上了一把锁，目前感觉好像没有必要，不过如果没有问题，暂时不处理这个。
            if buf_offset >= buf_len {
                break;
            }
            let page = page_cache.find_page_and_create(file_offset, None);
            if page.is_none() {
                break; // EOF!
            }
            let page_offset = file_offset % PAGE_SIZE;
            let mut byte = PAGE_SIZE - page_offset;
            byte = byte.min(buf_len - buf_offset);
            
            page.unwrap().write(page_offset, &buf[buf_offset..buf_offset+byte]);
            buf_offset += byte;
            file_offset += byte;
            total_len += byte;
            
            file_inner.f_pos = file_offset;
        }
        drop(file_inner);
        let mut inner_lock = inode.metadata().inner.lock();
        inner_lock.i_atime = TimeSpec::new();
        inner_lock.i_ctime = inner_lock.i_atime;
        inner_lock.i_mtime = inner_lock.i_atime;
        Ok(total_len)
    }
}