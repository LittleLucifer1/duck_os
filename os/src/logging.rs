//! 基于外部库log 自定义log输出的内容，详见 fn log()函数
//！具体的使用方法参考log crate

use log::{Level, LevelFilter, Log, Metadata, Record};

/// a simple logger
struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    // TODO： 这里的log可以根据后续的情况进行修改。
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let color = level_to_color_code(record);
        println!(
            "\u{1B}[{}m[{:>5}] {}:{} {}\u{1B}[0m",
            color,
            record.level(),
            record.file().unwrap(),
            record.line().unwrap(),
            record.args(),
        );
    }
    fn flush(&self) {}
}

// ANSI Escape Code: https://gist.github.com/fnky/458719343aabd01cfb17a3a4f7296797
pub fn level_to_color_code(record: &Record) -> usize {
    let color: usize = match record.level() {
        Level::Error => 31, // Red
        Level::Warn => 93, // Grey
        Level::Trace => 1, // Bold white
        Level::Debug => 32, //Green
        Level::Info => 34, // Blue
    };
    color
}

/// initiate logger
pub fn init() {
    static LOGGER: SimpleLogger = SimpleLogger;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("ERROR") => LevelFilter::Error,
        Some("WARN") => LevelFilter::Warn,
        Some("INFO") => LevelFilter::Info,
        Some("DEBUG") => LevelFilter::Debug,
        Some("TRACE") => LevelFilter::Trace,
        _ => LevelFilter::Off,
    });
}
