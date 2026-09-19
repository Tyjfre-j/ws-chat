use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;

pub fn init() -> WorkerGuard {
    let file_appender = tracing_appender::rolling::daily("logs", "server.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true);

    let console_layer = tracing_subscriber::fmt::layer()
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .init();

    guard
}