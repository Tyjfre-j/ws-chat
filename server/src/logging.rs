use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

pub fn init() -> WorkerGuard {
    let log_dir = std::env::var_os("WS_CHAT_LOG_DIR").unwrap_or_else(|| "logs".into());
    let file_appender = tracing_appender::rolling::daily(log_dir, "server.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,ws_chat_server=debug"));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true);

    let console_layer = tracing_subscriber::fmt::layer()
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(console_layer)
        .init();

    guard
}
