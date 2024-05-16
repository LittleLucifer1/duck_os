use alloc::sync::{Arc, Weak};

use crate::{config::fs::MAX_PIPE_BUFFER, process::hart::{cpu::suspend_current_task, env::SumGuard}, sync::SpinLock, syscall::error::{Errno, OSResult}};

use super::{file::{File, FileMeta}, info::OpenFlags};

#[derive(PartialEq, Debug)]
pub enum PipeStatus {
    Readable,
    Writable,
}

pub struct Pipe {
    pub status: PipeStatus,
    pub pipe_buffer: Arc<SpinLock<PipeRingBuffer>>,
    // meta: FileMeta,
}

impl Pipe {
    pub fn new(flags: OpenFlags, pipe_buffer: Arc<SpinLock<PipeRingBuffer>>) -> Option<Self> {
        let status: PipeStatus; 
        // TODO: 这里居然使用contain会有不符合语义的东西！！！
        // if flags.contain(OpenFlags::O_RDONLY)
        if flags == OpenFlags::O_RDONLY {
            // println!("The flag is {:#?}", flags);
            // println!("reach here1");
            status = PipeStatus::Readable;
        } else if flags == OpenFlags::O_WRONLY {
            // println!("The flag is {:#?}", flags);
            // println!("reach here2");
            status = PipeStatus::Writable;
        } else {
            return None;
        }
        Some(Self {
            status,
            pipe_buffer: Arc::clone(&pipe_buffer),
        })
    }
}

pub fn make_pipes() -> OSResult<(Arc<Pipe>, Arc<Pipe>)> {
    let buf = Arc::new(SpinLock::new(PipeRingBuffer::new()));
    let read_end = Arc::new(Pipe::new(OpenFlags::O_RDONLY, buf.clone()).ok_or(Errno::EINVAL)?);
    let write_end = Arc::new(Pipe::new(OpenFlags::O_WRONLY, buf.clone()).ok_or(Errno::EINVAL)?);
    buf.lock().set_read_end(Arc::downgrade(&read_end));
    buf.lock().set_write_end(Arc::downgrade(&write_end));
    Ok((read_end, write_end))
}

impl File for Pipe {
    fn metadata(&self) -> &FileMeta {
        todo!()
    }

    // 如果管道中没有足够的字符，并且写端被关闭了，则返回；否则等待；
    // 如果读够了，也返回。
    fn read(&self, buf: &mut [u8], _flags: OpenFlags) -> OSResult<usize> {
        assert!(self.status == PipeStatus::Readable);
        let buf_len = buf.len();
        let mut buf_off = 0usize;
        let mut already_read = 0usize;
        loop {
            let mut buf_lock = self.pipe_buffer.lock();
            let free_read = buf_lock.available_read();
            if free_read == 0 {
                if buf_lock.is_write_end_closed() {
                    return Ok(already_read);
                } else {
                    drop(buf_lock);
                    suspend_current_task();
                    continue;
                }
            }
            let _sum = SumGuard::new();
            for _ in 0..free_read {
                buf[buf_off] = buf_lock.read_byte().unwrap();
                buf_off += 1;
                already_read += 1;
                if already_read == buf_len {
                    return Ok(already_read);
                }
            }
        }
    }

    fn write(&self, buf: &[u8], _flags: OpenFlags) -> OSResult<usize> {
        assert!(self.status == PipeStatus::Writable);
        let buf_len = buf.len();
        let mut buf_off = 0usize;
        let mut alread_write = 0usize;
        loop {
            let mut buf_lock = self.pipe_buffer.lock();
            let free_write = buf_lock.available_write();
            if free_write == 0 {
                if buf_lock.is_read_end_closed() {
                    return Ok(alread_write);
                } else {
                    drop(buf_lock);
                    suspend_current_task();
                    continue;
                }
            }
            let _sum = SumGuard::new();
            for _ in 0..free_write {
                buf_lock.write_byte(buf[buf_off]);
                buf_off += 1;
                alread_write += 1;
                if alread_write == buf_len {
                    return Ok(alread_write);
                }
            }
        }
    }
}

#[derive(PartialEq)]
pub enum RingBufferStatus {
    Full,
    Empty,
    HasSome,
}

pub struct PipeRingBuffer {
    buf: [u8; MAX_PIPE_BUFFER],
    read_end: Option<Weak<Pipe>>,
    write_end: Option<Weak<Pipe>>,
    status: RingBufferStatus,
    head: usize,
    tail: usize,
}

impl PipeRingBuffer {
    pub fn new() -> Self {
        Self {
            buf: [0; MAX_PIPE_BUFFER],
            read_end: None,
            write_end: None,
            status: RingBufferStatus::Empty,
            head: 0,
            tail: 0,
        }
    }

    pub fn read_byte(&mut self) -> Option<u8> {
        if self.status == RingBufferStatus::Empty {
            return None;
        }
        self.status = RingBufferStatus::HasSome;
        let c = self.buf[self.head];
        self.head = (self.head + 1) % MAX_PIPE_BUFFER;
        if self.head == self.tail {
            self.status = RingBufferStatus::Empty;
        }
        Some(c)
    }

    pub fn write_byte(&mut self, byte: u8) -> bool {
        if self.status == RingBufferStatus::Full {
            return false;
        }
        self.status = RingBufferStatus::HasSome;
        self.buf[self.tail] = byte;
        self.tail = (self.tail + 1) % MAX_PIPE_BUFFER;
        if self.tail == self.head {
            self.status  = RingBufferStatus::Full;
        }
        true
    }

    pub fn is_write_end_closed(&self) -> bool {
        self.write_end.as_ref().unwrap().upgrade().is_none()
    }

    pub fn is_read_end_closed(&self) -> bool {
        self.read_end.as_ref().unwrap().upgrade().is_none()
    }

    pub fn set_read_end(&mut self, read_end: Weak<Pipe>) {
        self.read_end = Some(read_end);
    }

    pub fn set_write_end(&mut self, write_end: Weak<Pipe>) {
        self.write_end = Some(write_end);
    }

    pub fn available_read(&self) -> usize {
        if self.status == RingBufferStatus::Empty {
            0
        } else {
            if self.tail > self.head {
                self.tail - self.head
            } else {
                self.tail + MAX_PIPE_BUFFER - self.head
            }
        }
    }

    pub fn available_write(&self) -> usize {
        if self.status == RingBufferStatus::Full {
            0
        } else {
            MAX_PIPE_BUFFER - self.available_read()
        }
    }
}

