extern crate render;
extern crate log;

use log::{Record, Level, Metadata};
use log::{SetLoggerError, LevelFilter};
use render::render::SimpleRenderer;

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
}

fn main() {
    init();
    let mut renderer = SimpleRenderer::create();
    loop {
        renderer.do_stuff();
    }
}

