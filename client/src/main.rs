mod app;
mod events;
mod logging;
mod net;
mod protocol;
mod run;
mod ui;

fn main() -> color_eyre::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let mut terminal = ratatui::init();
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default().into_hooks();
    eyre_hook.install().inspect_err(|_error| {
        ratatui::restore();
    })?;
    std::panic::set_hook(Box::new(move |panic_info| {
        ratatui::restore();
        eprintln!("{}", panic_hook.panic_report(panic_info));
    }));
    let _guard = logging::init();

    let mut app = app::App::default();
    let result = runtime.block_on(run::run(&mut terminal, &mut app));
    ratatui::restore();

    result.map_err(Into::into)
}
