use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    DefaultTerminal, Frame,
};

#[derive(Default)]
struct App {
    input: String,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut app = App::default();
    ratatui::run(|terminal| run(terminal, &mut app))?;
    Ok(())
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| render(frame, app))?;
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => break Ok(()),
                KeyCode::Char(c) => app.input.push(c),
                KeyCode::Backspace => { app.input.pop(); }
                _ => {}
            }
        }
    }
}

fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)])
        .split(frame.area());

    frame.render_widget(
        Paragraph::new("Status: Disconnected")
            .block(Block::default().borders(Borders::ALL).title("ws-chat")),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new("Alice: hey\nBob: hi there")
            .block(Block::default().borders(Borders::ALL).title("Messages")),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(app.input.as_str())
            .block(Block::default().borders(Borders::ALL).title("Input")),
        chunks[2],
    );
}