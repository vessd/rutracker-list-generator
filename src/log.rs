use chrono::Local;
use crate::config::{Log, LogDestination};
use slog::{self, Drain, Level};
use slog_async::{Async, OverflowStrategy};
use slog_term::{self, FullFormat, PlainDecorator, PlainSyncDecorator, TermDecorator};
use std::fs::{File, OpenOptions};
use std::io;

pub fn pre_init() -> slog::Logger {
    let decorator = PlainSyncDecorator::new(io::stdout());
    let drain = FullFormat::new(decorator)
        .use_custom_timestamp(move |io: &mut dyn io::Write| {
            write!(io, "{}", Local::now().format("%T"))
        })
        .build()
        .fuse();
    slog::Logger::root(drain, o!())
}

pub fn init(config: &Log) -> io::Result<slog::Logger> {
    let level = match config.level {
        0 => Level::Critical,
        1 => Level::Error,
        2 => Level::Warning,
        3 => Level::Info,
        4 => Level::Debug,
        _ => Level::Trace,
    };

    let decorator = match &config.destination {
        LogDestination::Stdout => TermDecorator::new()
            .stdout()
            .try_build()
            .map(Decorator::Term)
            .unwrap_or_else(|| Decorator::PlainStdout(PlainDecorator::new(io::stdout()))),
        LogDestination::Stderr => TermDecorator::new()
            .stderr()
            .try_build()
            .map(Decorator::Term)
            .unwrap_or_else(|| Decorator::PlainStderr(PlainDecorator::new(io::stderr()))),
        LogDestination::File(path) => Decorator::File(PlainDecorator::new(
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)?,
        )),
    };

    let drain = FullFormat::new(decorator)
        .use_custom_timestamp(move |io: &mut dyn io::Write| {
            write!(io, "{}", Local::now().format("%T"))
        })
        .build()
        .fuse();
    let drain = drain.filter_level(level).fuse();
    let drain = Async::new(drain)
        .overflow_strategy(OverflowStrategy::Block)
        .build()
        .fuse();
    Ok(slog::Logger::root(drain, o!()))
}

enum Decorator {
    Term(TermDecorator),
    PlainStdout(PlainDecorator<io::Stdout>),
    PlainStderr(PlainDecorator<io::Stderr>),
    File(PlainDecorator<File>),
}

impl slog_term::Decorator for Decorator {
    fn with_record<F>(
        &self, record: &slog::Record<'_>, logger_values: &slog::OwnedKVList, f: F,
    ) -> io::Result<()>
    where
        F: FnOnce(&mut dyn slog_term::RecordDecorator) -> io::Result<()>,
    {
        match *self {
            Decorator::Term(ref d) => d.with_record(record, logger_values, f),
            Decorator::PlainStdout(ref d) => d.with_record(record, logger_values, f),
            Decorator::PlainStderr(ref d) => d.with_record(record, logger_values, f),
            Decorator::File(ref d) => d.with_record(record, logger_values, f),
        }
    }
}

macro_rules! log_try {
    ($level:ident, $expr:expr, $retexpr:expr, $fmt:expr $(, $args:expr)*) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                $level!($fmt, err $(, $args)*);
                $retexpr;
            }
        }
    }
}

macro_rules! crit_try {
    ($expr:expr, $fmt:expr $(, $args:expr)*) => {
        log_try!(crit, $expr, return 1, $fmt $(, $args)*)
    }
}

macro_rules! error_try {
    ($expr:expr, $retexpr:expr, $fmt:expr $(, $args:expr)*) => {
        log_try!(error, $expr, $retexpr, $fmt $(, $args)*)
    }
}
