use alloc::{sync::{Arc, Weak}, vec::Vec, vec};
use log::debug;
use lwext4_rust::lwext4_rmfile;

use crate::{config::mm::PAGE_SIZE, fs::{file::{File, FileMeta, SeekFrom}, info::{FileMode, InodeMode, OpenFlags, TimeSpec}, page_cache}, syscall::error::{Errno, OSResult}};

use super::ext4_inode::Ext4Inode;

pub struct Ext4MemFile {
    pub meta: FileMeta,
}

impl Ext4MemFile {
    pub fn new(meta: FileMeta) -> Self {
        Self { meta }
    }
}

impl File for Ext4MemFile {
    fn metadata(&self) -> &FileMeta {
        &self.meta
    }

    fn read_all(&self, buf: &mut Vec<u8>, flags: OpenFlags) -> OSResult<usize> {
        if flags.contains(OpenFlags::O_PATH) {
            debug!("[sys_read]: The flags contain O_PATH, file is not opened actually.");
            return Err(Errno::EBADF);
        }
        let inode = self.meta.f_inode.clone().upgrade().unwrap();
        let page_cache = Arc::clone(self.meta.page_cache.as_ref().unwrap());
        
        let mut total_len = 0usize;
        let mut file_pos = 0usize;
        loop {
            let inner_lock = inode.metadata().inner.lock();
            let file_size = inner_lock.i_size;
            drop(inner_lock);
            if file_size <= file_pos {
                break;
            }
            let page = page_cache.find_page_and_create(
                file_pos, 
                Some(Weak::clone(&self.meta.f_inode))
            ).unwrap();
            buf.resize(total_len + PAGE_SIZE, 0);
            page.read(0, buf);
            total_len += PAGE_SIZE;
            file_pos += PAGE_SIZE;
        }
        Ok(total_len)
    }

    // 文件在内存中读写数据
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
            let inner_lock = inode.metadata().inner.lock();
            let file_size = inner_lock.i_size;
            drop(inner_lock);
            // 如果超过文件尾或者大于buf的长度，则不再读了
            if file_size <= file_offset || buf_offset >= buf_len {
                break;
            }
            let page = page_cache.find_page_and_create(
                file_offset, 
                Some(Weak::clone(&self.meta.f_inode))
            ).unwrap();
            let page_offset = file_offset % PAGE_SIZE;
            let mut byte = PAGE_SIZE - page_offset;
            
            byte = byte.min(buf_len - buf_offset);
            byte = byte.min(file_size - file_offset);

            page.read(page_offset, &mut buf[buf_offset..buf_offset+byte]);
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
            let page = page_cache.find_page_and_create(
                file_offset, 
                Some(Weak::clone(&self.meta.f_inode))
            ).unwrap();
            let page_offset = file_offset % PAGE_SIZE;
            let mut byte = PAGE_SIZE - page_offset;
            byte = byte.min(buf_len - buf_offset);

            page.write(page_offset, &buf[buf_offset..buf_offset+byte]);
            buf_offset += byte;
            file_offset += byte;
            total_len += byte;
            
            file_inner.f_pos = file_offset;
            let mut inner_lock = inode.metadata().inner.lock();
            inner_lock.i_size = inner_lock.i_size.max(file_offset);
            
            drop(inner_lock);
        }
        drop(file_inner);
        let mut inner_lock = inode.metadata().inner.lock();
        inner_lock.i_atime = TimeSpec::new();
        inner_lock.i_ctime = inner_lock.i_atime;
        inner_lock.i_mtime = inner_lock.i_atime;
        Ok(total_len)
    }

    fn truncate(&self, new_size: usize) ->OSResult<usize> {
        let inode = self.metadata().f_inode.clone().upgrade().ok_or(Errno::EINVAL)?;
        let inode_lock = inode.metadata().inner.lock();
        let file_lock = self.metadata().inner.lock();
        let old_file_size = inode_lock.i_size;
        let old_pos = file_lock.f_pos;
        drop(inode_lock);
        // 1. 如果是截断，之后的所有数据都为0。
        // TODO：可以考虑将这部分的内存给释放掉！
        if new_size < old_file_size {
            let buf = vec![0; old_file_size - new_size];
            self.seek(SeekFrom::Start(new_size))?;
            self.write(&buf, OpenFlags::empty())?;
            self.seek(SeekFrom::Start(old_pos))?;
            inode.metadata().inner.lock().i_size = new_size;
        } else { // 2.如果是增长，则不需要额外处理，因为write函数会从创建新的page
            let buf = vec![0; old_file_size - new_size];
            self.seek(SeekFrom::Start(old_file_size))?;
            self.write(&buf, OpenFlags::empty())?;
            self.seek(SeekFrom::Start(old_pos))?;
            // write的时候会改变 i_size
        }
        // 3. 更新磁盘上的数据
        let inode = inode
            .clone()
            .downcast_arc::<Ext4Inode>().unwrap_or_else(|_| todo!());
        inode.file.as_ref().unwrap().lock().truncate(new_size as u64).map_err(Errno::from_i32)?;
        Ok(0)
    }

    fn close(&self) -> OSResult<()> {
        let path = self.meta.f_dentry.as_ref().unwrap().metadata().inner.lock().d_path.clone();
        let inode = self.metadata().f_inode.upgrade().unwrap();
        let mut inode_lock = inode.metadata().inner.lock();
        inode_lock.i_open_count -= 1;
        if inode_lock.i_link_count == 0 && inode_lock.i_open_count == 0 {
            assert!(inode.metadata().i_mode == InodeMode::Regular);
            drop(inode_lock);
            lwext4_rmfile(&path).map_err(Errno::from_i32)?;
        }
        Ok(())
    }
}