use crate::{fs::info::TimeSpec, process::hart::{cpu::suspend_current_task, env::SumGuard}, timer::current_time};

use super::{error:: SyscallResult, TimeVal, Tms};

// TODO: 这个部分的syscall实现的很丑陋，以后有时间需要仔细研究一下操作系统中的时间是如何表示的？
// TODO：整个的机制是怎么样的？？

pub fn sys_gettimeofday(tv: *mut TimeVal, _tz: usize) -> SyscallResult {
    // TODO：检查地址
    let _sum = SumGuard::new();
    unsafe {
        (*tv) = TimeVal::now();
    }
    Ok(0)
}

// TODO：暂时不具体实现，而是直接返回固定的几个值
pub fn sys_times(buf: *mut Tms) -> SyscallResult {
    // TODO: 检查地址
    let _sum = SumGuard::new();
    let tms = Tms {
        stime: 1,
        utime: 1,
        cutime: 1,
        cstime: 1,
    };
    unsafe {
        buf.write_volatile(tms);
    }
    Ok(0)
}

// TODO: 实现的很丑陋，但是先这样做！
pub fn sys_nanosleep(req: *const TimeSpec, _rem: *mut TimeSpec) -> SyscallResult {
    let _sum = SumGuard::new();
    let end_time = unsafe { current_time() + (*req).sec() };
    while current_time() < end_time {
        suspend_current_task();
    }
    Ok(0)
}
