//! Configuration in timer

pub const TICKS_PER_SEC: usize = 100;
pub const MSEC_PER_SEC: usize = 1_000;
pub const USEC_PER_SEC: usize = 1_000_000;
pub const NSEC_PER_SEC: usize = 1_000_000_000;
pub const NSEC_PER_MSEC: usize = NSEC_PER_SEC / MSEC_PER_SEC;
// QEMU的时钟频率
// qemu时钟频率12500000
pub const CLOCK_FREQUENCY: usize = 1250_0000; 