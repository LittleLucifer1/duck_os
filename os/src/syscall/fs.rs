//!文件系统相关系统调用

use core::{ops::Add, ptr::{self}};

use alloc::{string::ToString, sync::Arc};
use log::{debug, info};

use crate::{
    config::fs::SECTOR_SIZE, 
    fs::{
        dentry::{path_to_dentry, Dentry, DENTRY_CACHE}, 
        fd_table::FdInfo, file::SeekFrom, 
        file_system::FILE_SYSTEM_MANAGER, 
        info::{InodeMode, OpenFlags, TimeSpec}, 
        inode::InodeDev, pipe::make_pipes, AT_FDCWD
    }, 
    process::hart::{
        cpu::{get_cpu_id, get_cpu_local}, env::SumGuard
    }, 
    syscall::error::Errno, 
    utils::{
        path::{cwd_and_path, dentry_name, format_path, is_relative_path, parent_path, ptr_and_dirfd_to_path}, 
        string::c_ptr_to_string
    }
};
use super::{error::SyscallResult, Dirent64, Dirent64Type, FSFlags, FSType, UtsName, STAT};

/* Description: 从buf所在的地址上将len长度的数据写到fd中
    注意事项：1.要检查buf的地址是否合法。
        2. 实际写的数据可能小于 count，因为各种原因: RLIMIT_FSIZE资源限制、信号打断、物理媒介没有足够的空间（未考虑）
        3. 对于seekable文件，offset要随着写入的数据多少而变化。
        4. 如果是open with O_APPEND，则file offset要先设置为the end of file，再写入。
    TODO： 暂时不支持稀疏文件
*/
pub fn sys_write(fd: usize, buf: usize, count: usize) -> SyscallResult {
    info!("[sys_write]: fd {}, count {}, buf address: 0x{:x}", fd, count, buf);
    let fd_table = get_cpu_local(get_cpu_id())
        .current_pcb_clone()
        .as_ref()
        .unwrap()
        .fd_table
        .clone();
    // TODO: 这里有一个问题：设计的pcb是分了多个模块，分别上锁
    // 所以很容易造成死锁。因此，应该采取的基本措施是：按照一定的顺序上锁。
    // 这里我们应不应该让locked_fd_table的锁持续到file.write之后。如果是这样的话，可以确保
    // 有且只有一个进程可以访问fd_table（但话说哪个进程会访问别人进程的fd_table？）。
    // 或者可以在file这里上锁，保证一个文件每次只有一次的读或写请求。
    let file_info = {
        let locked_fd_table = fd_table.lock();
        locked_fd_table.fd_table.get(&fd).cloned().ok_or(Errno::EBADF)?
    };
    let flags = file_info.flags.clone();
    info!("[sys_write]: file flags is {:#?}", flags);
    if !flags.is_writable() {
        // Unsafe: Titanix中的这个值是EPERM，但是根据手册我认为是这个
        return Err(Errno::EBADF);
    }
    if count == 0 {
        return Ok(0);
    }
    let _sum = SumGuard::new();
    // TODO: 检查buf的地址，如果缺页，需要有一个中断，然后再读
    let buf = unsafe { core::slice::from_raw_parts(buf as *const u8, count)};
    let ret = file_info.file.write(buf, flags);
    ret
}

/* Description: read from a file descriptor
    注意事项：要修改文件的pos位置
    TODO： 暂时不支持稀疏文件
*/
pub fn sys_read(fd: usize, buf: usize, count: usize) -> SyscallResult {
    info!("[sys_read]: fd {}, count {}, buf address: 0x{:x}", fd, count, buf);
    let fd_table = get_cpu_local(get_cpu_id())
        .current_pcb_clone()
        .as_ref()
        .unwrap()
        .fd_table
        .clone();
    let file_info = {
        let locked_fd_table = fd_table.lock();
        locked_fd_table.fd_table.get(&fd).cloned().ok_or(Errno::EBADF)?
    };
    let flags = file_info.flags.clone();
    info!("[sys_read]: file flags is {:#?}", flags);
    if !flags.is_readable() {
        // Unsafe: Titanix中的这个值是EPERM，但是根据手册我认为是这个
        debug!("[sys_read]: flags doesn't contain read! File is not readable!");
        return Err(Errno::EBADF);
    }
    if count == 0 {
        return Ok(0);
    }
    let _sum = SumGuard::new();
    // TODO： 检查地址
    let buf = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, count)};
    let ret = file_info.file.read(buf, flags);
    info!("[sys_read]: Already read: {} bytes.", ret.as_ref().ok().unwrap());
    ret
}

/* Description: Change working directory 
    TODO：更新时间  DOWN：好像是已经做了这个操作
*/
pub fn sys_chdir(path: usize) -> SyscallResult {
    info!("[sys_chdir]: path address is 0x{:x}", path);
    let path_ptr = path as *const u8;
    let _sum = SumGuard::new();
    // TODO: 检查这个path的地址正确性
    let mut path_str = c_ptr_to_string(path_ptr);
    // 1.规范化path的处理
    path_str = format_path(&path_str);
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let mut inner_lock = current_task.inner.lock();
    if is_relative_path(&path_str) {
        path_str = cwd_and_path(&path_str, &inner_lock.cwd);
    }
    info!("[sys_chdir]: path is {:?}", &path_ptr);
    // 2.找到path对应的inode,判断是否是DIR
    let dentry = path_to_dentry(&path_str)?.ok_or(Errno::ENOENT)?;
    if dentry.metadata().inner.lock().d_inode.metadata().i_mode != InodeMode::Directory {
        return Err(Errno::ENOTDIR);
    } else {
        dentry.metadata().inner.lock().d_inode.metadata().inner.lock().i_atime = TimeSpec::new();
        info!("[sys_chdir]: cwd changed! old:{}, new:{}",inner_lock.cwd, path_str);
        inner_lock.cwd = path_str.to_string();
        Ok(0)
    }
}
/* Description: Duplicate a file descriptor
    注意事项: 1. 两个dup共享同一个文件和file offset,但是不共享 flags(close-on-exec)
            2. 新的fd满足是最小未使用的 file description
            3. 成功的话,返回新的fd
    TODO: 暂时不考虑fd分配器的资源最大限制的功能
*/
pub fn sys_dup(oldfd: usize) -> SyscallResult {
    info!("[sys_dup]: The oldfd is {}", oldfd);
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let mut fd_table_lock = current_task.fd_table.lock();
    let new_fd = if let Some(fd_info) = fd_table_lock.fd_table.get(&oldfd) {
        let file = Arc::clone(&fd_info.file);
        let mut flags = fd_info.flags.clone();
        flags.remove(OpenFlags::O_CLOEXEC);
        let new_fd = fd_table_lock.insert_get_fd(FdInfo::new(file, flags))?;
        info!("[sys_dup]: The newfd is {}", new_fd);
        new_fd
    } else {
        return Err(Errno::EBADF);
    };
    Ok(new_fd)
}

/* Description: Duplicate a file descriptor
    注意事项: 1. 如果new_fd之前打开了,则要先关闭,同时关闭和重新使用的操作应该是atomical,否则会出现难以预料的问题
        2. 如果old_fd不是有效的,则直接失败,不用关闭newfd
        3. 如果两者相同,则报错
        4. 如果flags中有 close-on-exec,则可以强制的设置给新fd
*/
pub fn sys_dup3(oldfd: usize, newfd: usize, flags: u32) ->SyscallResult {
    info!("[sys_dup3]: The oldfd is {}, newfd is {}", oldfd, newfd);
    if oldfd == newfd {
        return Err(Errno::EINVAL);
    }
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let mut fd_table_lock = current_task.fd_table.lock();
    if let Some(fd_info) = fd_table_lock.fd_table.get(&oldfd) {
        let file = fd_info.file.clone();
        // 处理flags
        let flags = OpenFlags::from_bits(flags).unwrap();
        info!("[sys_dup3]: The flags is {:#?}", flags);
        let mut old_flags = fd_info.flags.clone();
        old_flags.set(OpenFlags::O_CLOEXEC, flags.contains(OpenFlags::O_CLOEXEC));
        // TODO: 目前只考虑了没有被分配和被分配的情况。还有可能fd的值会超过限制！
        // DOWN: 问题已经解决
        if !fd_table_lock.insert_spec_fd(newfd, FdInfo::new(file.clone(), old_flags.clone()))? {
            // 1.关闭这个fd，之后再重新分配
            fd_table_lock.close(newfd);
            // 2.这里要确保这两步是atomically,因为如果中间又有一个线程分配fd，则下行代码又会失败。
            // 这里通过加fd_table锁的方式避免数据竞争，应该不会出现上述的情况。
            let ret = fd_table_lock.insert_spec_fd(newfd, FdInfo::new(file, flags))?;
            assert!(ret);
        }
    } else {
        return Err(Errno::EBADF);
    }
    Ok(newfd)
}

/* Description: get current working directory
    注意事项：1.如果 len < cwd.len，要报错
        2. 写入的cwd应该为绝对地址
        3. 成功时，返回值即buf
*/
pub fn sys_getcwd(buf: usize, len: usize) -> SyscallResult {
    info!("[sys_getcwd]: buf address is 0x{:x}, len is {}", buf, len);
    let _sum = SumGuard::new();
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let cwd = current_task.inner.lock().cwd.clone();
    info!("[sys_getcwd]: cwd is {:?}", &cwd);
    // TODO: 检查buf的地址
    if len < cwd.len() {
        Err(Errno::ERANGE)
    } else {
        assert!(cwd.starts_with("/"));
        let data = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, len)};
        data.fill(0 as u8);
        let data = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, cwd.len())};
        data.copy_from_slice(cwd.as_bytes());
        Ok(buf)
    }
}

/* Description: get directory entries
    注意事项: 成功时，返回读入的bytes值
    TODO：File可能没有inode？？？？可能有部分的File没有吧，我现在还不知道！
    DOWN: 确实可能没有，但是一般都有，例如内存中的文件，虽然有，但实际上没有
*/
pub fn sys_getdents64(fd: usize, dirp: usize, count: usize) -> SyscallResult {
    // TODO: 先检查地址dirp在用户空间是否有效,如果无效，return EFAULT
    info!("[sys_getdents64]: The fd {}, dirp addr 0x{:x}, count {}", fd, dirp, count);
    let _sum = SumGuard::new();
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let fd_table_lock = current_task.fd_table.lock();
    let file = fd_table_lock.fd_table.get(&fd).ok_or(Errno::EBADF)?;
    let dentry = Arc::clone(&file.file.metadata().f_dentry.as_ref().unwrap());
    let inode = Arc::clone(&dentry.metadata().inner.lock().d_inode);

    if inode.metadata().i_mode != InodeMode::Directory {
        return Err(Errno::ENOTDIR);
    } else {
        inode.metadata().inner.lock().i_atime = TimeSpec::new();
        let mut buf_off = 0;
        let mut file_inner = file.file.metadata().inner.lock();
        let dirent_index = file_inner.dirent_index;
        info!("[sys_getdents64]: old dirent_index is {}", dirent_index);
        for (idx, (name, child)) in dentry
                .metadata().inner.lock().d_child.iter().enumerate() {
            if idx < dirent_index {
                continue;
            }
            let c_inode = &child.metadata().inner.lock().d_inode;
            let ino = c_inode.metadata().i_ino;
            let mode: Dirent64Type = c_inode.metadata().i_mode.into();
            let size = Dirent64::dirent_size() + name.len() + 1;
            let dirent64 = Dirent64::load_dirent64(ino as u64, mode.bits(), size as u16);
            if buf_off + size > count {
                debug!("[sys_getdents64]: Result buffer is too small");
                break;
            }
            unsafe {
                // println!("dirp:{:#x}, buf_off: {}({:#x}), name_len: {}, Dirent size: {:#x}", dirp, buf_off, buf_off, name.len(), Dirent64::dirent_size());
                let dirent64_ptr: *mut Dirent64 = dirp.add(buf_off) as *mut Dirent64;
                ptr::write(dirent64_ptr, dirent64);
                let name_buf: &mut [u8] = core::slice::from_raw_parts_mut(
                dirp.add(buf_off + Dirent64::dirent_size()) as *mut _, 
                name.len() + 1
                );
                name_buf[..name.len()].copy_from_slice(&name.as_bytes());
                name_buf[name.len()] = 0;
            }
            file_inner.dirent_index = idx + 1;
            buf_off += size;
        }
        Ok(buf_off)
    }
}

/* Description: get name and information about current kernel
    注意事项: 检查buf是否有效
*/
pub fn sys_uname(buf: usize) -> SyscallResult {
    // TODO：检查地址的有效性
    info!("[sys_uname]: buf addr is 0x{:x}", buf);
    let _sum = SumGuard::new();
    let uname = UtsName::new();
    unsafe {
        let buf_ptr = buf as *mut UtsName;
        ptr::write(buf_ptr, uname);
    }
    Ok(0)
}

/* Description: get file status
    注意事项: 
*/
pub fn sys_fstat(fd: usize, buf: usize) -> SyscallResult {
    info!("[sys_fstat]: fd {}, buf addr is 0x{:x}", fd, buf);
    let _sum = SumGuard::new();
    // TODO：检查地址的有效性
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let fd_table_lock = current_task.fd_table.lock();
    let file = fd_table_lock.fd_table.get(&fd).ok_or(Errno::EBADF)?;
    let inode = file.file.metadata().f_inode.upgrade().ok_or(Errno::EBADF)?;
    let mut kstat = STAT::new();
    kstat.st_dev = match &inode.metadata().i_dev {
        Some(InodeDev::BlockDev(dev)) => dev.id as u64,
        _ => 1234567, // TODO: 如果此时的inode没有dev，未知情况，所以是随意设置的
    };
    kstat.st_ino = inode.metadata().i_ino as u64;
    kstat.st_mode = inode.metadata().i_mode as u32;
    // TODO: 如果是目录，同时目录的data_len = 0,则要计算所有child的大小
    // 如果可以保证在每次目录创建文件后，size都会更新，那么就不用考虑这种情况。
    kstat.st_size = inode.metadata().inner.lock().i_size as u64;
    kstat.st_blocks = (kstat.st_size / SECTOR_SIZE as u64) as u64;
    kstat.st_atim = inode.metadata().inner.lock().i_atime;
    kstat.st_mtim = inode.metadata().inner.lock().i_mtime;
    kstat.st_ctim = inode.metadata().inner.lock().i_ctime;
    info!("[sys_fstat]: kstat is {:?}", kstat);
    unsafe {
        let buf_ptr = buf as *mut STAT;
        ptr::write(buf_ptr, kstat);
    }
    Ok(0)
}

/* Description: open and possibly create a file
    注意事项：1. 默认情况下file的offset为0
        2. mode参数一般用于指示组权限的，这里我们不用实现这么复杂的功能
    TODO: 有少量的flags没有去处理，目前值处理了多个简单的flags，不过这些flags已经完全超过Titanix的实现
*/
pub fn sys_openat(dirfd: isize, pathname: *const u8, flags: u32, _mode: usize) -> SyscallResult {
    let mut flags = OpenFlags::from_bits_truncate(flags);
    info!("[sys_openat]: dirfd {}, pathname addr: 0x{:X}, flags: {:#?}",dirfd, pathname as usize, flags);
    // TODO:检查dirfd，必须是目录同时必须open for reading or 使用了 O_PATH
    let _sum = SumGuard::new();
    // TODO：检查pathname的地址
    let path = ptr_and_dirfd_to_path(dirfd, pathname)?;
    info!("[sys_openat]: path is {:?}", path);
    let final_dentry: Arc<dyn Dentry>;
    // 1.如果文件存在
    if let Some(dentry) = path_to_dentry(&path)? {
        let file_kind = dentry.metadata().inner.lock().d_inode.metadata().i_mode;
        if flags.contains(OpenFlags::O_TRUNC) {
            if file_kind == InodeMode::Regular && (flags.is_writable()) {
                // do nothing!
            } else {
                flags.remove(OpenFlags::O_TRUNC);
            }
        }
        if flags.contains(OpenFlags::O_DIRECTORY) && file_kind != InodeMode::Directory {
            return Err(Errno::ENOTDIR);
        }
        if flags.contains(OpenFlags::O_CREAT) && flags.contains(OpenFlags::O_EXCL){
            debug!("The file has existed!");
            return Err(Errno::EEXIST);
        }
        final_dentry = dentry;
    } else { 
        // 2.如果文件不存在
        if flags.contains(OpenFlags::O_CREAT) {
            let fa_dentry = path_to_dentry(&parent_path(&path))?.unwrap();
            let name = dentry_name(&path);
            final_dentry = fa_dentry.create(Arc::clone(&fa_dentry), name, InodeMode::Regular)?;
        }
        else {
            return Err(Errno::ENOENT);
        }
    }
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let fd = current_task.fd_table.lock().open(final_dentry, flags)?;
    info!("[sys_openat]: The fd is {}", fd);
    Ok(fd)
}

/* Description: close a file descriptor
    注意事项：最后一个文件的引用：和unlink有关，暂时不知道目前的实现满不满足语义
    DOWN: 实现了部分的 unlink 语义，但是不确定实现是否正确？？？
*/
pub fn sys_close(fd: usize) -> SyscallResult {
    info!("[sys_close]: fd {}", fd);
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let mut table = current_task.fd_table.lock();
    if let Some(fd_info) = table.fd_table.get(&fd) {
        fd_info.file.close()?;
        table.close(fd);
        Ok(0)
    } else {
        Err(Errno::EBADF)
    }
}

// Description: truncate a file to specified length
pub fn sys_ftruncate(fd: usize, len: isize) -> SyscallResult {
    info!("[sys_ftruncate]: fd: {}, len: {}", fd, len);
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let table = current_task.fd_table.lock();
    if let Some(fd_info) = table.fd_table.get(&fd) {
        let file = fd_info.file.clone();
        // TODO：这里需要判断文件是否可读或者可写？？？？
        // TODO: 还需要修改时间相关的量
        file.truncate(len as usize)?;
    } else {
        return Err(Errno::EBADF);
    }
    Ok(0)
}

/* Description: create a directory
*/
pub fn sys_mkdirat(dirfd: isize, pathname: *const u8, _mode: usize) -> SyscallResult {
    info!("[sys_mkdirat]: dirfd is {}, pathname addr: 0x{:x}", dirfd, pathname as usize);
    // TODO: 检查pathname的地址
    let _sum = SumGuard::new();
    let path = ptr_and_dirfd_to_path(dirfd, pathname)?;
    if path_to_dentry(&path)?.is_some() {
        return Err(Errno::EEXIST);
    }
    let fa_dentry = path_to_dentry(&parent_path(&path))?.unwrap();
    if fa_dentry.metadata().inner.lock().d_inode.metadata().i_mode != InodeMode::Directory {
        debug!("[sys_mkdirat] parent is not a directory.");
        return Err(Errno::ENOTDIR);
    }
    let inode = fa_dentry.metadata().inner.lock().d_inode.clone();
    let mut inode_lock = inode.metadata().inner.lock();
    inode_lock.i_atime = TimeSpec::new();
    inode_lock.i_mtime = TimeSpec::new();
    drop(inode_lock);
    let name = dentry_name(&path);
    fa_dentry.create(Arc::clone(&fa_dentry), name, InodeMode::Directory)?;
    Ok(0)
}

/* Description:  delete a name and possibly the file it refers to
    注意事项：1.暂时没有考虑实现软链接和硬链接，所以这里的实现稍微简单点。之后如果时间充裕可以考虑实现！
        2. 可以去思考一下 unlink 删除缓存索引和释放资源的区别 DONN!
        3. 理论上来讲，如果inode要释放，则需要它的引用计数 = 0, 所以这就要小心使用Arc/Weak指针。这是一个稍微比较复杂的问题。
        DOWN!
*/
pub fn sys_unlinkat(dirfd: isize, pathname: *const u8, flags: u32) -> SyscallResult {
    info!("[sys_unlinkat]: dirfd {}, pathname addr: 0x{:x}", dirfd, pathname as usize);
    // TODO: 检查地址的有效性
    let _sum = SumGuard::new();
    let path = ptr_and_dirfd_to_path(dirfd, pathname)?;
    info!("[sys_unlinkat]: path is {:?}", path);
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let cwd = current_task.inner.lock().cwd.clone();
    // 不能删除当前进程的工作目录
    if path == cwd {
        return Err(Errno::EBUSY);
    }
    let dentry = path_to_dentry(&path)?;
    if dentry.is_none() {
        return Err(Errno::ENOENT);
    }
    const AT_REMOVEDIR: u32 = 0x200;
    let dentry = dentry.unwrap();
    let dentry_inner = dentry.metadata().inner.lock();
    if dentry_inner.d_inode.metadata().i_mode == InodeMode::Directory {
        if (flags & AT_REMOVEDIR) == AT_REMOVEDIR {
            if dentry_inner.d_child.is_empty() {
                let parent_dirent = dentry_inner.d_parent.clone();
                if parent_dirent.is_none() {
                    debug!("The inode is the root inode, cannot be unlinked!");
                    return Err(Errno::EPERM);
                }
                let pa_dirent = parent_dirent.unwrap().upgrade().unwrap();
                drop(dentry_inner);
                pa_dirent.unlink(Arc::clone(&dentry))?;
            } else {
                return Err(Errno::ENOTEMPTY);
            }
        } else {
            return Err(Errno::EISDIR);
        }
    } else {
        let mut inner_lock = dentry_inner.d_inode.metadata().inner.lock();
        // TODO：这里还没完善有关时间的相关处理，此处的时间处理应该是 时间归 0
        inner_lock.i_atime = TimeSpec::new();
        inner_lock.i_ctime = TimeSpec::new();
        inner_lock.i_mtime = TimeSpec::new();
        drop(inner_lock);
        let parent_dirent = dentry_inner.d_parent.clone();
        if parent_dirent.is_none() {
            return Err(Errno::EPERM);
        } else {
            let pa_dirent = parent_dirent.unwrap().upgrade().unwrap();
            drop(dentry_inner);
            pa_dirent.unlink(Arc::clone(&dentry))?;
        }
    }
    Ok(0)
}

// Description: 对已有的文件创建一个新的硬链接
// 注意事项：
// TODO: 有两个高级标志还未实现，是 AT_EMPTY_PATH && AT_SYMLINK_FOLLOW
pub fn sys_linkat(old_dirfd: isize, old_path: *const u8, new_dirfd: isize, new_path: *const u8, flags: u32) -> SyscallResult {
    info!(
        "[sys_linkat]: old_dirfd is {}, old_path address is 0x{:x}, new_dirfd is {}, new_path address is 0x{:x}", 
        old_dirfd, old_path as usize, new_dirfd, new_path as usize
    );
    let _sum = SumGuard::new();
    let old_path = ptr_and_dirfd_to_path(old_dirfd, old_path)?;
    let new_path = ptr_and_dirfd_to_path(new_dirfd, new_path)?;
    info!("[sys_linkat]: The old path is {:?}, new path is {:?}", old_path, new_path);
    if DENTRY_CACHE.lock().contains_key(&new_path) {
        return Err(Errno::EEXIST);
    }
    let old_dentry = path_to_dentry(&old_path)?.ok_or(Errno::ENOENT)?;
    if old_dentry.metadata().inner.lock().d_inode.metadata().i_mode.ne(&InodeMode::Regular) {
        return Err(Errno::EPERM);
    }
    let new_parent_dentry = path_to_dentry(&parent_path(&new_path))?.ok_or(Errno::ENOENT)?;
    if new_parent_dentry.metadata().inner.lock().d_inode.metadata().i_mode.ne(&InodeMode::Directory) {
        return Err(Errno::ENOTDIR)
    }
    old_dentry.link(new_parent_dentry, dentry_name(&new_path))?;
    Ok(0)
}

// Description: 创建一个名为 link_path 的软链接文件，其中的内容包括了target内容
pub fn sys_symlinkat(target: *const u8, new_dirfd: isize, link_path: *const u8) -> SyscallResult {
    info!(
        "[sys_symlinkat]: target address is 0x{:x}, new_dirfd is {}, linkpath address is 0x{:x},",
        target as usize, new_dirfd, link_path as usize,
    );
    let _sum = SumGuard::new();
    // TODO: 这里的 ptr_and_dirfd_to_path中有一些错误需要判断，并返回的
    let link_path = ptr_and_dirfd_to_path(new_dirfd, link_path)?;
    let target = c_ptr_to_string(target);
    info!("[sys_symlinkat]: The target content is {:?}, the link_path is {:?}", target, link_path);
    if DENTRY_CACHE.lock().contains_key(&link_path) {
        return Err(Errno::EEXIST);
    }
    let fa_dentry = path_to_dentry(&parent_path(&link_path))?.ok_or(Errno::ENOENT)?;
    fa_dentry.symbol_link(dentry_name(&link_path), &target)?;
    Ok(0)    
}

// Description: read value of a symbolic link
// 注意事项：无，主要是通过 ext4中的 api 来管理
pub fn sys_readlinkat(dirfd: isize, pathname: *const u8, buf: *mut u8, bufsize: isize) -> SyscallResult {
    info!(
        "[sys_readlinkat]: dirfd is {}, pathname address is 0x{:x}, buf_size is {}", 
        dirfd, pathname as usize, bufsize
    );
    let _sum = SumGuard::new();
    if bufsize < 0 {
        return Err(Errno::EINVAL);
    }
    let bufsize = bufsize as usize;
    let path = ptr_and_dirfd_to_path(dirfd, pathname)?;
    let dentry = path_to_dentry(&path)?.ok_or(Errno::ENOENT)?;
    let buf: &mut[u8] = unsafe { core::slice::from_raw_parts_mut(buf, bufsize) };
    if dentry.metadata().inner.lock().d_inode.metadata().i_mode.ne(&InodeMode::Link) {
        return Err(Errno::EINVAL);
    }
    dentry.read_symlink(buf)?;
    Ok(0)
}

/* Description: mount filesystem
    source: 需要挂载的东西的位置，指向设备 或者 目录/文件/空字符串
    target: 被挂载的位置(目录或者文件)
    fs_type: 就是文件系统的类型
    data: 暂时没啥用
    注意事项：这里基本上没有考虑任何的挂载标识！
    Detail：系统调用只处理路径，FILE_MANAGER只加入挂载点，不同的FS自己处理不同的初始化，
*/
pub fn sys_mount(source: *const u8, target: *const u8, fs_type: *const u8, flags: u32, _data: usize) -> SyscallResult {
    info!( "[sys_mount]: source addr: 0x{:x}, target addr: 0x{:x}, fs_type addr: 0x{:x}", 
        source as usize, target as usize, fs_type as usize);
    let _sum = SumGuard::new();
    // TODO：检查地址的有效性
    let dev_path = ptr_and_dirfd_to_path(AT_FDCWD, source)?;
    let tar_path = ptr_and_dirfd_to_path(AT_FDCWD, target)?;
    let fs_type_str = c_ptr_to_string(fs_type);
    let fs_type = FSType::str_to_type(&fs_type_str);
    let flags = FSFlags::from_bits(flags & 511).ok_or(Errno::EINVAL)?;
    info!("[sys_mount]: The dev_path: {:?}, tar_path: {:?}", dev_path, tar_path);

    // TODO： 实现的有问题，按linux语义来说，有虚拟的/dev路径指定硬件，但是我们没有，所以这里还是有点问题
    let dev_dentry = path_to_dentry(&dev_path)?;
    let dev = match dev_dentry {
        Some(dentry) => {
            let inode = dentry.metadata().inner.lock().d_inode.clone();
            let dev = match &inode.metadata().i_dev {
                Some(InodeDev::BlockDev(block_dev)) => {
                    block_dev.block_device.clone()
                }
                _ => todo!(),
            };
            Some(dev)
        },
        None => None
    };
    FILE_SYSTEM_MANAGER.mount(&tar_path, &dev_path, dev, fs_type, flags)?;
    Ok(0)
}

/* Description：unmount filesystem
    注意事项：基本上没有考虑flags的功能
*/
pub fn sys_umount2(target: *const u8, _flags: usize) -> SyscallResult {
    info!("[sys_umount2]: target addr: 0x{:x}", target as usize);
    let _sum = SumGuard::new();
    // TODO：检查地址的有效性
    let path = ptr_and_dirfd_to_path(AT_FDCWD, target)?;
    info!("[sys_umount2]: path is {:?}", path);
    if path == "/" {
        return Err(Errno::EPERM);
    }
    // TODO: 考虑文件sync相关的事宜！
    FILE_SYSTEM_MANAGER.unmount(&path)?;
    Ok(0)
}

/* Description: reposition read/write file offset
    注意事项：允许 offset 放在 EOF 之后，会产生空洞
*/
#[allow(unused)]
pub fn sys_lseek(fd: usize, offset: isize, whence: u8) -> SyscallResult {
    info!("[sys_lseek]: fd: {}, offset: {}, whence: {}", fd, offset, whence);
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let fd_table_lock = current_task.fd_table.lock();
    let fdinfo = fd_table_lock.fd_table.get(&fd).ok_or(Errno::EBADF)?;
    if !fdinfo.flags.is_readable() {
        return Err(Errno::EACCES);
    }
    let mut cur_offset: usize = 0;
    const SEEK_START: u8 = 0;
    const SEEK_CUR: u8 = 1;
    const SEEK_END: u8 = 2;
    match whence {
        SEEK_START => {
            if offset < 0 {
                return Err(Errno::EINVAL);
            }
            cur_offset = fdinfo.file.seek(SeekFrom::Start(offset as usize))?;
            
        }
        SEEK_CUR => {
            cur_offset = fdinfo.file.seek(SeekFrom::Current(offset))?;
        }
        SEEK_END => {
            cur_offset = fdinfo.file.seek(SeekFrom::End(offset))?;
        }
        _  => { return Err(Errno::EINVAL); }
    }
    info!("[sys_lseek] Final offset is {}", cur_offset);
    Ok(cur_offset)
}

// 目前只用于创建管道，其实还有其他的作用，后续可能需要修改相关的函数接口
// 没有实现文件的meta,因为可能需要修改相关的数据结构。同时，在make_pipes中的Error的报错机制还不是很完善，稍微有点不完善。
// Description: 创建 pipe，这里的不完善指的是 传统的用户地址检查 和 暂时不太需要处理的 flags 
pub fn sys_pipe2(buf: *mut i32, flags: u32) -> SyscallResult {
    info!("[sys_pipe2]: buf addr: {:#x}, flags: {}", buf as usize, flags);
    let flags = OpenFlags::from_bits_truncate(flags);
    let (pipe_read, pipe_write) = make_pipes()?;
    let current_task = get_cpu_local(get_cpu_id()).current_pcb_clone().unwrap();
    let mut table = current_task.fd_table.lock();
    let read_fd = table.insert_get_fd(FdInfo::new(pipe_read, flags | OpenFlags::O_RDONLY))?;
    let write_fd = table.insert_get_fd(FdInfo::new(pipe_write, flags | OpenFlags::O_WRONLY))?;
    
    let _sum  = SumGuard::new();
    // TODO: 检查地址
    let buf = unsafe { core::slice::from_raw_parts_mut(buf, 2 * core::mem::size_of::<i32>()) };
    buf[0] = read_fd as i32;
    buf[1] = write_fd as i32;
    info!("[sys_pipe2]: read_fd: {}, write_fd: {}", read_fd, write_fd);
    Ok(0)
}

// Description: Change the name or location of a file
pub fn sys_renameat2(old_dirfd: isize, old_path: *const u8, new_dirfd: isize, new_path: *const u8, flags: u32) -> SyscallResult {
    info!(
        "[sys_renameat2]: old dirfd is {}, old_path address is 0x{:x}, new dirfd is {}, new_path address is 0x{:x}, flags is {}",
        old_dirfd, old_path as usize, new_dirfd, new_path as usize, flags
    );
    let _sum = SumGuard::new();
    // TODO：检查path是否为空指针？？？
    let old_path = ptr_and_dirfd_to_path(old_dirfd, old_path)?;
    let new_path = ptr_and_dirfd_to_path(new_dirfd, new_path)?;
    info!("[sys_renameat2]: old_path is {:?}, new_path is {:?}", old_path, new_path);
    let old_pa_dentry = path_to_dentry(&parent_path(&old_path))?.ok_or(Errno::ENOENT)?;
    let new_pa_dentry = path_to_dentry(&parent_path(&new_path))?.ok_or(Errno::ENOENT)?;
    let old_name = dentry_name(&old_path);
    let new_name = dentry_name(&new_path);
    let old_inode_ino = old_pa_dentry.metadata().inner.lock().d_inode.metadata().i_ino;
    let new_inode_ino = new_pa_dentry.metadata().inner.lock().d_inode.metadata().i_ino;
    // 两个路径指向同一个 inode，则啥也不做
    if old_inode_ino == new_inode_ino {
        return Ok(0);
    }
    let old_mode = old_pa_dentry.metadata().inner.lock().d_inode.metadata().i_mode;
    let new_mode = new_pa_dentry.metadata().inner.lock().d_inode.metadata().i_mode;
    if old_mode != new_mode {
        return match (old_mode, new_mode) {
            (InodeMode::Regular, InodeMode::Directory) => Err(Errno::EISDIR),
            (InodeMode::Directory, InodeMode::Regular) => Err(Errno::ENOTDIR),
            _ => todo!()
        }
    }
    old_pa_dentry.rename(old_name, new_pa_dentry, new_name)?;

    Ok(0)
}