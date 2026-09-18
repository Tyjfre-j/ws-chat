mod app;
mod events;
mod net;
mod protocol;
mod run;
mod ui;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let runtime = tokio::runtime::Runtime::new()?;

    let mut app = app::App::default();
    let mut terminal = ratatui::init();
    let result = runtime.block_on(run::run(&mut terminal, &mut app));
    ratatui::restore();

    result.map_err(Into::into)
}
