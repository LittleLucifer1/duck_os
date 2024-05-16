use crate::{process::hart::{cpu::suspend_current_task, env::SumGuard}, sbi, syscall::error::OSResult};

use super::{file::File, info::OpenFlags};

pub const STDIN: usize = 0;
pub const STDOUT: usize = 1;
pub const STDERR: usize = 2;

pub struct Stdin;
pub struct Stdout;
pub struct Stderr;


impl File for Stdout {
    fn read(&self, _buf: &mut [u8], _flags: OpenFlags) -> OSResult<usize>{
        todo!()
    }

    fn write(&self, buf: &[u8], flags: OpenFlags) -> OSResult<usize>{
        assert!(flags.is_writable());
        let _sum = SumGuard::new();
        if let Ok(data) = core::str::from_utf8(buf) {
            print!("{}", data);
            Ok(buf.len())
        } else {
            Ok(0)
        }
    }
}

impl File for Stdin {
    // TODO: 这一块的代码不是很清楚！
    fn read(&self, buf: &mut [u8], flags: OpenFlags) -> OSResult<usize> {
        assert!(flags.is_readable());
        if buf.len() == 0 {
            return Ok(0);
        }
        buf[0] = loop {
            let c = self.getchar();
            if c == 0 || c == 255 {
                suspend_current_task();
                continue;
            } else {
                break c;
            }
        };
        Ok(1)
    }

    fn write(&self, _buf: &[u8], _flags: OpenFlags) -> OSResult<usize> {
        todo!()
    }
}

impl File for Stderr {
    fn read(&self, _buf: &mut [u8], _flags: OpenFlags) -> OSResult<usize> {
        todo!()
    }

    fn write(&self, _buf: &[u8], _flags: OpenFlags) -> OSResult<usize> {
        todo!()
    }
}

impl Stdin {
    #[inline]
    #[allow(deprecated)]
    pub fn getchar(&self) -> u8 {
        sbi::console_getchar()
    }
}