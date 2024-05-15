//! fat32文件系统 在磁盘上管理 File 和 抽象出来的内存File

use core::cmp::{max, min};

use alloc::{sync::{Arc, Weak}, vec::Vec};
use log::debug;

use crate::{config::{fs::SECTOR_SIZE, mm::PAGE_SIZE}, fs::{file::{File, FileMeta}, info::{OpenFlags, TimeSpec}}, syscall::error::{Errno, OSResult}};

use super::{block_cache::get_block_cache,fat::{alloc_cluster, find_all_cluster, free_cluster, FatInfo}, fat_dentry::Position, utility::cluster_to_sector};

// TODO：这里有一个问题要仔细想一想？
/*
    这里实现的是磁盘上的文件读写。在实际读写文件时，首先通过open函数打开文件，即应该是把文件内容加载到内存中，
    然后通过调用 trait File中的函数进行读写，Titanix中是利用了 page，
*/
// 磁盘上的文件，
pub struct FatDiskFile {
    pub fat_info: Arc<FatInfo>,
    clusters: Vec<usize>,
    pub size: usize,
}

impl FatDiskFile {
    pub fn empty(fat_info: Arc<FatInfo>) -> Self {
        Self {
            fat_info: Arc::clone(&fat_info),
            clusters: Vec::new(),
            size: 0,
        }
    }

    pub fn init(fat_info: Arc<FatInfo>, pos: Position) -> Self {
        let mut file = Self::empty(Arc::clone(&fat_info));
        file.cal_cluster_size(pos);
        file
    }

    fn cal_cluster_size(&mut self, pos: Position) {
        if pos.data_cluster == 0 {
            // TODO:这个情况是一个偷懒的选择。当系统调用mkdir 或者 mknod时需要创建FatFile并分配data_cluster，需要将数据写入磁盘。
            // 但是我还没有处理这种情况，同时想快速测试完系统调用，所以这里就偷懒将data_cluster分为 0;
            return;
        }
        let cluster = find_all_cluster(self.fat_info.clone(), pos.data_cluster);
        self.clusters.clone_from(&cluster);
        if self.size == 0 {
            self.size = self.clusters.len() * self.fat_info.sec_per_clus * SECTOR_SIZE;
        }
    }

    pub fn first_cluster(&self) -> usize {
        self.clusters[0]
    }

    pub fn last_cluster(&self) -> usize {
        *self.clusters.last().unwrap()
    }

    // 此时默认了不用进行 cluster 的重新计算
    pub fn modify_size(&mut self, diff_size: isize, pos: Position) -> usize {
        if diff_size < 0 && self.size as isize + diff_size > 0 {
            let new_sz = (self.size as isize + diff_size) as usize;
            let clus_num = (new_sz + self.fat_info.sec_per_clus * SECTOR_SIZE - 1)
                / (self.fat_info.sec_per_clus * SECTOR_SIZE);
            while self.clusters.len() > clus_num {
                let end_clu = self.clusters.pop().unwrap();
                if self.clusters.len() > 0 {
                    let pre_clu = *self.clusters.last().unwrap();
                    free_cluster(end_clu, Some(pre_clu), Arc::clone(&self.fat_info));
                } else {
                    free_cluster(end_clu, None, Arc::clone(&self.fat_info));
                }
            }
            self.size = new_sz;
        } else if diff_size > 0 {
            let new_sz = (self.size as isize + diff_size) as usize;
            let clus_num = (new_sz + self.fat_info.sec_per_clus * SECTOR_SIZE - 1)
                / (self.fat_info.sec_per_clus * SECTOR_SIZE);
            while self.clusters.len() < clus_num {
                let end_clu = *self.clusters.last().unwrap();
                let new_clu: usize;
                if self.clusters.is_empty() {
                    new_clu = alloc_cluster(None, Arc::clone(&self.fat_info)).unwrap();
                } else {
                    new_clu = alloc_cluster(Some(end_clu), Arc::clone(&self.fat_info)).unwrap();
                }
                self.clusters.push(new_clu);
            }
            self.size = new_sz;
        }
        self.cal_cluster_size(pos);
        self.size
    }

    // TODO: 再检查一下 这里是抄的Titanix的思路，其实可以自己实现！
    /* Desciption:  从磁盘上把数据读到data的数组中
        data:  接收数据的数组，其中的len比较重要
        offset： 磁盘上文件以字节为单位的offset */
    pub fn read(&mut self, data: &mut [u8], offset: usize) -> usize {
        // println!("The offset is {}, data_len is {}",offset, data.len());
        let st = min(offset, self.size);
        let ed = min(offset + data.len(), self.size);
        let st_cluster = st / (self.fat_info.sec_per_clus * SECTOR_SIZE);
        let ed_cluster = (ed + self.fat_info.sec_per_clus * SECTOR_SIZE - 1)
            / (self.fat_info.sec_per_clus * SECTOR_SIZE);
        // println!("The st_cluster is {}, ed_cluster is {}", st_cluster, ed_cluster);
        for clu_id in st_cluster..ed_cluster {
            let cluster_id = self.clusters[clu_id];
            // println!("The cluster id is {}", cluster_id);
            let sector_id = cluster_to_sector(Arc::clone(&self.fat_info), cluster_id);
            for j in 0..self.fat_info.sec_per_clus {
                let off = clu_id * self.fat_info.sec_per_clus + j;
                let sector_st = off * SECTOR_SIZE;
                let sector_ed = sector_st + SECTOR_SIZE;
                // println!("sector_st: {}, sector_ed: {}, st: {}, ed: {}", sector_st, sector_ed, st, ed);
                if sector_ed <= st || sector_st >= ed {
                    continue;
                }
                let cur_st = max(sector_st, st);
                let cur_ed = min(sector_ed, ed);
                let mut tmp_data: [u8; SECTOR_SIZE] = [0; SECTOR_SIZE];
                // println!("block_id is {}", sector_id + j);
                get_block_cache(sector_id + j, Arc::clone(&self.fat_info.dev.as_ref().unwrap()))
                    .lock()
                    .read(0, |sector: &Sector|{
                        tmp_data.copy_from_slice(sector);
                });
                for i in cur_st..cur_ed {
                    data[i - st] = tmp_data[i - sector_st];
                }
            }
        }
        ed - st
    }

    // TODO: 再检查一下 这里是抄的Titanix的思路，其实可以自己实现！
    pub fn write(&mut self, data: &mut [u8], offset: usize, pos: Position) -> usize {
        let st = min(offset, self.size);
        let ed = min(offset + data.len(), self.size);
        if self.size < ed {
            self.modify_size((ed - self.size) as isize, pos);
        }
        let st_cluster = st / (self.fat_info.sec_per_clus * SECTOR_SIZE);
        let ed_cluster = (ed + self.fat_info.sec_per_clus * SECTOR_SIZE - 1)
            / (self.fat_info.sec_per_clus * SECTOR_SIZE);
        for clu_id in st_cluster..ed_cluster {
            let cluster_id = self.clusters[clu_id];
            let sector_id = cluster_to_sector(Arc::clone(&self.fat_info), cluster_id);
            for j in 0..self.fat_info.sec_per_clus {
                let off = clu_id * self.fat_info.sec_per_clus + j;
                let sector_st = off * SECTOR_SIZE;
                let sector_ed = sector_st + SECTOR_SIZE;
                if sector_ed <= st || sector_st >= ed {
                    continue;
                }
                let cur_st = max(sector_st, st);
                let cur_ed = min(sector_ed, ed);
                let mut tmp_data: [u8; SECTOR_SIZE] = [0; SECTOR_SIZE];
                if cur_st != sector_st || cur_ed != sector_ed {
                    get_block_cache(sector_id + j, Arc::clone(&self.fat_info.dev.as_ref().unwrap()))
                    .lock()
                    .read(0, |sector: &Sector|{
                        // tmp_data = sector.data;
                        tmp_data.copy_from_slice(sector);
                    });
                }
                for i in cur_st..cur_ed {
                    data[i - st] = tmp_data[i - sector_st];
                }
                get_block_cache(sector_id + j, Arc::clone(&self.fat_info.dev.as_ref().unwrap()))
                    .lock()
                    .write(0, |sector: &mut Sector|{
                        // sector.data = tmp_data;
                        sector.copy_from_slice(&tmp_data);
                    })
            }
        }
        ed - st
    }

    pub fn read_all(&mut self) -> Vec<u8> {
        let mut data_vec = Vec::new();
        data_vec.resize(self.size, 0);
        self.read(&mut data_vec, 0);
        data_vec
    }

}

type Sector = [u8; SECTOR_SIZE];

// fat 抽象出来的内存上的文件
pub struct FatMemFile {
    pub meta: FileMeta,
}

impl FatMemFile {
    pub fn init(meta: FileMeta) -> Self {
        Self { meta }
    }
}

impl File for FatMemFile {
    fn metadata(&self) -> &FileMeta {
        &self.meta
    }

    // 将文件的offset(即pos)之后的数据读入buf中
    // offset >> PAGE_SIZE: page的索引值； 后几位：page中的offset
    // 如果读到了文件的尾，则不用再读了。
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
            // 如果超过文件尾或者大于buf的长度，则不再读了
            if file_size <= file_offset || buf_offset >= buf_len {
                break;
            }
            let page = page_cache.find_page(file_offset, Weak::clone(&self.meta.f_inode));
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

    // TODO: 多个进程访问一个文件的问题？
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
        let max_len = buf.len();
        loop {
            // Unsafe: 这里上了一把锁，目前感觉好像没有必要，不过如果没有问题，暂时不处理这个。
            // let mut inner_lock = inode.metadata().inner.lock();
            let page = page_cache.find_page(file_offset, Weak::clone(&self.meta.f_inode));
            let page_offset = file_offset % PAGE_SIZE;
            let mut byte = PAGE_SIZE - page_offset;
            if byte + buf_offset > max_len {
                byte = max_len - buf_offset;
            }
            page.write(page_offset, &buf[buf_offset..buf_offset+byte]);
            buf_offset += byte;
            file_offset += byte;
            total_len += byte;
            
            file_inner.f_pos = file_offset;
            let mut inner_lock = inode.metadata().inner.lock();
            inner_lock.i_size = inner_lock.i_size.max(file_offset);
            if buf_offset >= max_len {
                break;
            }
            drop(inner_lock);
        }
        drop(file_inner);
        let mut inner_lock = inode.metadata().inner.lock();
        inner_lock.i_atime = TimeSpec::new();
        inner_lock.i_ctime = inner_lock.i_atime;
        inner_lock.i_mtime = inner_lock.i_atime;
        Ok(total_len)
    }
}


