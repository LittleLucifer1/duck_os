use core::panic;

use alloc::{collections::btree_map::BTreeMap, string::String, sync::{Arc, Weak}, vec::Vec};
use strum::FromRepr;

use crate::{
    config::fs::SECTOR_SIZE, driver::CharDevice, fs::{
        dentry::{Dentry, DentryMeta}, 
        file::{File, FileMeta, FileMetaInner}, 
        info::{FileMode, InodeMode, OpenFlags, TimeSpec}, 
        inode::{Inode, InodeDev, InodeMeta}
    },
    sync::{SpinLock, SpinNoIrqLock}, 
    syscall::error::{Errno, OSResult}
};

pub struct TtyDentry {
    pub meta: DentryMeta,
}

impl TtyDentry {
    pub fn new(
        name: String,
        path: String,
        inode: Arc<dyn Inode>,
        parent: Option<Arc<dyn Dentry>>
    ) -> Self {
        Self { meta: DentryMeta::new(
            name, 
            path, 
            inode, 
            parent, 
            BTreeMap::new(),
        ) }
    }
}

impl Dentry for TtyDentry {
    fn open(&self, dentry: Arc<dyn Dentry>, _flags: OpenFlags) -> OSResult<Arc<dyn File>> {
        dentry.metadata().inner.lock().d_inode.metadata().inner.lock().i_open_count += 1;
        let file = TtyFile::new(
            Arc::clone(&dentry), 
            Arc::downgrade(&Arc::clone(&dentry.metadata().inner.lock().d_inode))
        );
        let file_arc: Arc<TtyFile> = Arc::new(file);
        Ok(file_arc)
    }
    
    fn create(&self, _this: Arc<dyn Dentry>, _name: &str, _mode: InodeMode) -> OSResult<Arc<dyn Dentry>> {
        Err(Errno::ENOTDIR)
    }

    fn metadata(&self) -> &DentryMeta {
        &self.meta
    }

    fn unlink(&self, _child: Arc<dyn Dentry>) -> OSResult<()> {
        Err(Errno::ENOTDIR)
    }
}

pub struct TtyInode {
    pub meta: InodeMeta,
    // TODO: 暂时先设置为Option类型，应该不存在没有device的情况
    pub device: Option<Arc<dyn CharDevice>>,
}

impl TtyInode {
    pub fn new(mode: InodeMode) -> Self {
        Self { 
            meta: InodeMeta::new(
                mode, 
                0, 
                InodeDev::Todo, 
                SECTOR_SIZE, 
                TimeSpec::new(), 
                TimeSpec::new(),
                TimeSpec::new()
            ),
            device: None,
        }
    }
}

impl Inode for TtyInode {
    fn metadata(&self) -> &InodeMeta {
        &self.meta
    }

    fn read(&self, _offset: usize, _buf: &mut [u8]) -> OSResult<usize> {
        todo!()
    }

    fn write(&self, _offset: usize, _buf: &mut [u8]) -> OSResult<usize> {
        todo!()
    }

    fn delete_data(&self) -> OSResult<()> {
        todo!()
    }

    fn read_all(&self) -> OSResult<Vec<u8>> {
        todo!()
    }
}

pub struct TtyFile {
    meta: FileMeta,
    pub inner: SpinNoIrqLock<TtyInner>,
}

impl TtyFile {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Weak<dyn Inode>) -> Self {
        Self {
            meta: FileMeta { 
                f_mode: FileMode::empty(), 
                page_cache: None,
                f_dentry: Some(dentry),
                f_inode: inode,
                inner: SpinLock::new(FileMetaInner {
                    f_pos: 0,
                    dirent_index: 0,
                }),
            },
            inner: SpinNoIrqLock::new(TtyInner::new()),
        }
    }
}

impl File for TtyFile {
    fn metadata(&self) -> &FileMeta {
        &self.meta
    }

    // TODO: unimplemented
    fn read(&self, buf: &mut [u8], _flags: OpenFlags) -> OSResult<usize> {
        Ok(0)
    }

    // TODO: unimplemented
    fn write(&self, buf: &[u8], _flags: OpenFlags) -> OSResult<usize> {
        Ok(buf.len())
    }

    fn ioctl(&self, cmd: usize, arg: usize) -> OSResult<usize> {
        let cmd = TtyIoctlCmd::from_repr(cmd);
        if cmd.is_none() {
            panic!("Unknown cmd in tty");
        }
        match cmd.unwrap() {
            TtyIoctlCmd::TCGETS | TtyIoctlCmd::TCGETA => {
                let inner_lock = self.inner.lock();
                unsafe {
                    *(arg as *mut Termios) = inner_lock.termios;
                }
                Ok(0)
            }
            TtyIoctlCmd::TCSETS | TtyIoctlCmd::TCSETAF | TtyIoctlCmd::TCSETAW => {
                let mut inner_lock = self.inner.lock();
                unsafe {
                    inner_lock.termios = *(arg as *mut Termios);
                }
                Ok(0)
            }
            TtyIoctlCmd::TIOCGPGRP => {
                let pid_val = self.inner.lock().fg_pgid;
                unsafe {
                    *(arg as *mut usize) = pid_val;
                }
                Ok(0)
            }
            TtyIoctlCmd::TIOCSPGRP => {
                let pid_value: usize;
                unsafe {
                    pid_value = *(arg as *const usize);
                }
                self.inner.lock().fg_pgid = pid_value;
                Ok(0)
            }
            TtyIoctlCmd::TIOCGWINSZ => {
                let inner_lock = self.inner.lock();
                unsafe {
                    *(arg as *mut WinSize) = inner_lock.win_size;
                }
                Ok(0)
            }
            TtyIoctlCmd::TIOCSWINSZ => {
                let mut inner_lock = self.inner.lock();
                unsafe {
                    inner_lock.win_size = *(arg as *const WinSize);
                }
                Ok(0)
            }
            TtyIoctlCmd::TCSBRK => Ok(0),
            _ => todo!(),
        }
    }
}

pub struct TtyInner {
    fg_pgid: usize,
    win_size: WinSize,
    termios: Termios,
}

impl TtyInner {
    pub fn new() -> Self {
        Self { 
        // TODO: Warning: 这个tty中的pid值设置为root所在的值1,不知道会不会有什么重要影响？
            fg_pgid: 1, 
            win_size: WinSize::new(), 
            termios: Termios::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct WinSize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

impl WinSize {
    fn new() -> Self {
        Self { 
            ws_row: 67, 
            ws_col: 120, 
            ws_xpixel: 0, 
            ws_ypixel: 0 
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Termios {
    pub iflag: u32, // input mode flags
    pub oflag: u32, // output mode flags
    pub cflag: u32, // control mode flags
    pub lflag: u32, // local mode flags
    pub line: u8, // Line description
    pub cc: [u8; 19], // control characters
}

impl Termios {
    fn new() -> Self {
        Self {
            // iflag: 输入模式标志
            // BRKINT | ICRNL | IXON | IUTF8 | IMAXBEL | IXANY
            iflag: 0o000002 | 0o000200 | 0o004000 | 0o040000 | 0o100000 | 0o010000,

            // oflag: 输出模式标志
            // OPOST | ONLCR
            oflag: 0o000001 | 0o000004,

            // cflag: 控制模式标志
            // CS8 | CREAD | HUPCL
            // 注意：EXTB 并不是一个标准标志，移除。
            cflag: 0o000060 | 0o000200 | 0o000400,
            // HUPCL | CREAD | CSIZE | EXTB
            // cflag: 0o2277,

            // lflag: 本地模式标志
            // ISIG | ICANON | ECHO | ECHOE | ECHOK | ECHOCTL | ECHOKE | IEXTEN
            lflag: 0o000001 | 0o000002 | 0o000010 | 0o000020 | 0o000040 |
                    0o004000 | 0o020000 | 0o001000,

            // 默认线路号，几乎总为 0
            line: 0,
            cc: [
                3,   // VINTR:     Ctrl-C
                28,  // VQUIT:     Ctrl-\
                127, // VERASE:    Backspace
                21,  // VKILL:     Ctrl-U
                4,   // VEOF:      Ctrl-D
                0,   // VTIME:     非规范模式下的超时（秒）
                1,   // VMIN:      非规范模式下的最小字节数
                0,   // VSWTC:     Linux 特有，通常不使用
                17,  // VSTART:    Ctrl-Q
                19,  // VSTOP:     Ctrl-S
                26,  // VSUSP:     Ctrl-Z
                255, // VEOL:      额外行结束符（无效）
                18,  // VREPRINT:  Ctrl-R
                15,  // VDISCARD:  Ctrl-O
                23,  // VWERASE:   Ctrl-W
                22,  // VLNEXT:    Ctrl-V
                255, // VEOL2:     第二个行结束符（无效）
                0,   // 保留位
                0,   // 保留位
            ],
        }
    }
}

/// TTY 相关的 ioctl 命令编号枚举，定义见 <asm-generic/ioctls.h>
#[derive(FromRepr, Debug)]
#[repr(usize)]
enum TtyIoctlCmd {
    // --- termios 结构相关 ioctl（现代接口） ---
    /// 获取当前终端的 termios 设置（用于现代接口 struct termios）。
    TCGETS = 0x5401,
    /// 立即设置终端 termios 参数（不等待缓冲区处理完成）。
    TCSETS = 0x5402,
    /// 等待输入输出缓冲区处理完后再设置 termios 参数。
    TCSETSW = 0x5403,
    /// 清空输入输出缓冲区后设置 termios 参数。
    TCSETSF = 0x5404,

    // --- termio 结构相关 ioctl（已过时，旧接口） ---
    /// 获取 termio 设置（用于旧接口 struct termio，已不推荐使用）。
    TCGETA = 0x5405,
    /// 立即设置 termio 参数。
    TCSETA = 0x5406,
    /// 等待缓冲区处理完后设置 termio 参数。
    TCSETAW = 0x5407,
    /// 清空缓冲区后设置 termio 参数。
    TCSETAF = 0x5408,

    // --- 其他控制命令 ---
    /// 向终端发送一个“中断信号”（Break）：
    /// 如果参数为 0，则发送一个 0.25 到 0.5 秒的中断比特流；
    /// 如果为非 0，则不发送中断。
    TCSBRK = 0x5409,
    /// 获取当前终端的前台进程组 ID（进程控制）。
    TIOCGPGRP = 0x540F,
    /// 设置当前终端的前台进程组 ID（进程控制）。
    TIOCSPGRP = 0x5410,
    /// 获取终端窗口大小（struct winsize）。
    TIOCGWINSZ = 0x5413,
    /// 设置终端窗口大小。
    TIOCSWINSZ = 0x5414,
}
