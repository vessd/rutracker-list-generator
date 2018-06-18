use chrono::Local;
use config::{LogConfig, LogDestination};
use slog::{self, Drain, Level};
use slog_async::{Async, OverflowStrategy};
use slog_term::{self, FullFormat, PlainDecorator, TermDecorator};
use std::fs::{File, OpenOptions};
use std::io;

pub fn init(config: &LogConfig) -> io::Result<slog::Logger> {
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
        .use_custom_timestamp(move |io: &mut io::Write| write!(io, "{}", Local::now().format("%T")))
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
        &self,
        record: &slog::Record,
        logger_values: &slog::OwnedKVList,
        f: F,
    ) -> io::Result<()>
    where
        F: FnOnce(&mut slog_term::RecordDecorator) -> io::Result<()>,
    {
        match *self {
            Decorator::Term(ref d) => d.with_record(record, logger_values, f),
            Decorator::PlainStdout(ref d) => d.with_record(record, logger_values, f),
            Decorator::PlainStderr(ref d) => d.with_record(record, logger_values, f),
            Decorator::File(ref d) => d.with_record(record, logger_values, f),
        }
    }
}
