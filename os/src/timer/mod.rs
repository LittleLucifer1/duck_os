//! RISC-V timer-related functionality

use riscv::register::time;

use crate::config::timer::{CLOCK_FREQUENCY, MSEC_PER_SEC, USEC_PER_SEC};

fn get_time() -> usize {
    time::read()
}

pub fn current_time_ms() -> usize {
    get_time() / (CLOCK_FREQUENCY / MSEC_PER_SEC)
}

pub fn current_time_us() -> usize {
    get_time() / (CLOCK_FREQUENCY / USEC_PER_SEC)
}

pub fn current_time_ns() -> usize {
    get_time() / 12
}

pub fn current_time() -> usize {
    get_time() / CLOCK_FREQUENCY
}
