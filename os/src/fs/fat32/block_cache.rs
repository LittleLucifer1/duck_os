//! 做为磁盘的块缓存

use alloc::{
    collections::VecDeque, sync::Arc, vec::Vec, vec,
};

use crate::{config::fs::{SECTOR_CACHE_SIZE, SECTOR_SIZE}, driver::BlockDevice, sync::SpinLock};

pub struct BlockCache {
    cache: Vec<u8>,
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
    modified: bool,
}

impl BlockCache {
    /// Load a new BlockCache from disk.
    pub fn new(block_id: usize, block_device: Arc<dyn BlockDevice>) -> Self {
        // for alignment and move effciency
        
        let mut cache = vec![0u8; SECTOR_SIZE];
        block_device.read_block(block_id, &mut cache);
        Self {
            cache,
            block_id,
            block_device,
            modified: false,
        }
    }
    /// Get the slice in the block cache according to the offset.
    fn addr_of_offset(&self, offset: usize) -> usize {
        &self.cache[offset] as *const _ as usize
    }
    /// Get a immutable reference to the data in the block cache according to the offset.
    pub fn get_ref<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= SECTOR_SIZE);
        let addr = self.addr_of_offset(offset);
        unsafe { &*(addr as *const T) }
    }
    /// Get a mutable reference to the data in the block cache according to the offset.
    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= SECTOR_SIZE);
        self.modified = true;
        let addr = self.addr_of_offset(offset);
        unsafe { &mut *(addr as *mut T) }
    }
    /// Read the data from the block cache according to the offset.
    pub fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }
    /// Write the data into the block cache according to the offset.
    pub fn write<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }
    /// Sync(write) the block cache to disk.
    pub fn sync(&mut self) {
        if self.modified {
            self.modified = false;
            self.block_device.write_block(self.block_id, &self.cache);
        }
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync()
    }
}

/// BlockCacheManager is a manager for BlockCache.
pub struct BlockCacheManager {
    /// (block_id, block_cache, flag)
    queue: VecDeque<(usize, Arc<SpinLock<BlockCache>>, usize)>,
    clock: usize,
}

impl BlockCacheManager {
    /// Create a new BlockCacheManager with an empty queue (block_id, block_cache)
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            clock: 0,
        }
    }
    /// Get a block cache from the queue. according to the block_id.
    /// clock frame algorithm
    pub fn get_block_cache(
        &mut self,
        block_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<SpinLock<BlockCache>> {
        if let Some(pair) = self.queue.iter().find(|pair| pair.0 == block_id) {
            Arc::clone(&pair.1)
        } else {
            let block_cache = Arc::new(SpinLock::new(BlockCache::new(
                block_id,
                Arc::clone(&block_device),
            )));
            if self.queue.len() == SECTOR_CACHE_SIZE {
                loop {
                    if Arc::strong_count(&self.queue[self.clock].1) == 1 {
                        if self.queue[self.clock].2 == 1 {
                            self.queue[self.clock].2 = 0;
                            self.clock = (self.clock + 1) % SECTOR_CACHE_SIZE;
                            continue;
                        } else {
                            self.queue.drain(self.clock..=self.clock);
                            break;
                        }
                    } else {
                        self.clock = (self.clock + 1) % SECTOR_CACHE_SIZE;
                    }
                }
                self.queue[self.clock] = (block_id, Arc::clone(&block_cache), 1);
                self.clock = (self.clock + 1) % SECTOR_CACHE_SIZE;
            } else {
                self.queue.push_back((block_id, Arc::clone(&block_cache), 1));
            }
            block_cache
        }
    }
}

pub static BLOCK_CACHE_MANAGER: SpinLock<BlockCacheManager> = 
    SpinLock::new(BlockCacheManager::new());

/// Get a block cache from the queue. according to the block_id.
pub fn get_block_cache(
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<SpinLock<BlockCache>> {
    BLOCK_CACHE_MANAGER
        .lock()
        .get_block_cache(block_id, block_device)
}
/// Sync(write) all the block cache to disk.
pub fn block_cache_sync_all() {
    let manager = BLOCK_CACHE_MANAGER.lock();
    for (_, cache,_) in manager.queue.iter() {
        cache.lock().sync();
    }
}