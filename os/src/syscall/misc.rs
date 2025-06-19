use log::info;

use crate::{process::hart::env::SumGuard, utils::random::RANDOM_GENERATOR};

use super::error::SyscallResult;

// Description: Obtain a series of random bytes
// TODO: 检查地址的有效性
pub fn sys_getrandom(buf: *mut u8, buflen: usize, _flags: usize) -> SyscallResult {
    info!("[sys_getrandom]: buf address is 0x{:x}, buflen is {}", buf as usize, buflen);
    let _sum = SumGuard::new();
    let buf: &mut [u8] = unsafe {
        core::slice::from_raw_parts_mut(buf, buflen)
    };
    RANDOM_GENERATOR.lock().write_to_buf(buf);
    Ok(buf.len())
}