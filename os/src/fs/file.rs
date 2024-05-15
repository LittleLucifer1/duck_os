//! file 模块
/*
    file是抽象出来的，不是物理存储介质中的file。
    这个概念用于进程，表示进程打开该文件时，文件的整体状态。

    1. 数据结构
        1）dentry：对应的目录项
        2）mode：文件打开的模式
        3）pos：文件当前的位移量（文件指针）
        4）file：绑定的文件？？？

    2. 功能函数
        1）llseek：更新偏移量指针
        2）read / write：读写
        3）ioctl: io的相关控制
        4) fsync
        5) 其他的我也不知道了。
*/

use alloc::{sync::{Arc, Weak}, vec::Vec};

use crate::{sync::SpinLock, syscall::error::{Errno, OSResult}};

use super::{dentry::Dentry, info::{FileMode, OpenFlags}, inode::Inode, page_cache::PageCache};

// TODO:一个file可能没有inode???
// TODO: 说实话，这里我使用Weak来管理inode，可能是因为我想做到 file -> dentry -> inode，并不希望 file -> inode
// Unsafe: Titanix中File的数据结构都在锁里面，我这里没有这样设计，会不会出现一些问题？？
pub struct FileMeta {
    pub f_mode: FileMode,
    pub page_cache: Option<Arc<PageCache>>,
    pub f_dentry: Arc<dyn Dentry>,
    pub f_inode: Weak<dyn Inode>,
    pub inner: SpinLock<FileMetaInner>,
    // pub file: Option<Weak<dyn File>>
}

pub struct FileMetaInner {
    pub f_pos: usize,
    pub dirent_index: usize,
}

pub trait File: Send + Sync {
    fn metadata(&self) -> &FileMeta {
        todo!()
    }

    fn read(&self, buf: &mut [u8], flags: OpenFlags) -> OSResult<usize>;
    fn write(&self, buf: &[u8], flags: OpenFlags) -> OSResult<usize>;

    // TODO：这里的seek和truncate的实现和Titanix的基本上一模一样。所以需要一模一样吗？
    // TODO：会不会漏掉某些关键的语义呢？又或者有更好的实现方式呢？
    // 这个函数本质上就是将文件的大小修改一下，可以增大也可以减小。
    // 其实只需要修改file_size，但是出于未知原因，还是习惯性的把减少的或者增加的数据设置为0；
    fn truncate(&self, new_size: usize) ->OSResult<usize> {
        let inode = self.metadata().f_inode.clone().upgrade().ok_or(Errno::EINVAL)?;
        let mut inode_lock = inode.metadata().inner.lock();
        let file_lock = self.metadata().inner.lock();
        let old_file_size = inode_lock.i_size;
        let old_pos = file_lock.f_pos;
        // 1. 如果是截断，之后的所有数据都为0。
        // TODO：可以考虑将这部分的内存给释放掉！
        if new_size < old_file_size {
            let mut buf = Vec::with_capacity(old_file_size - new_size);
            buf.fill(0u8);
            self.seek(SeekFrom::Start(new_size))?;
            self.write(&buf, OpenFlags::empty())?;
            self.seek(SeekFrom::Start(old_pos))?;
            inode_lock.i_size = new_size;
        } else { // 2.如果是增长，则不需要额外处理，因为write函数会从创建新的page
            let mut buf = Vec::with_capacity(new_size - old_file_size);
            buf.fill(0u8);
            self.seek(SeekFrom::Start(old_file_size))?;
            self.write(&buf, OpenFlags::empty())?;
            self.seek(SeekFrom::Start(old_pos))?;
            inode_lock.i_size = new_size;
        }
        Ok(0)
    }
    // TODO：这里的seek和truncate的实现和Titanix的基本上一模一样。所以需要一模一样吗？
    // TODO：会不会漏掉某些关键的语义呢？又或者有更好的实现方式呢？
    // TODO：seek这里还存在一个问题：如果seek把pos放到了文件后面的位置，这是一个什么情况？会发生什么？
    fn seek(&self, seek: SeekFrom) -> OSResult<usize> {
        let mut meta_lock = self.metadata().inner.lock();
        match seek {
            SeekFrom::Current(pos) => {
                if pos < 0 {
                    meta_lock.f_pos -= pos.abs() as usize;
                } else {
                    meta_lock.f_pos += pos as usize;
                }
            },
            SeekFrom::End(pos) => {
                let data_len = self.metadata().f_inode.upgrade().unwrap().metadata().inner.lock().i_size;
                if pos < 0 {
                    meta_lock.f_pos = data_len - pos.abs() as usize;
                } else {
                    meta_lock.f_pos = data_len + pos as usize;
                }
            },
            SeekFrom::Start(pos) => {
                meta_lock.f_pos += pos;
            }
        }
        let ret = meta_lock.f_pos;
        Ok(ret)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SeekFrom {
    Start(usize),
    Current(isize),
    End(isize),
}