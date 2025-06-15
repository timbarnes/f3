use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute, terminal,
};
use std::{
    io::{self, Write, Stdout, stdout},
    time::Duration,
};

#[derive(Debug)]
pub struct ForthTui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    input_buffer: String,
    output_log: Vec<String>,
}

impl ForthTui {
    pub fn new() -> Result<Self, io::Error> {
        // terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, terminal::EnterAlternateScreen, DisableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            input_buffer: String::new(),
            output_log: vec!["Welcome to Forth TUI".into()],
        })
    }

    pub fn get_line(&mut self) -> Option<String> {
        let mut buffer = String::new();
        print!("→ ");
        stdout().flush().ok()?;
        loop {
            if let Ok(Event::Key(key_event)) = event::read() {
                match key_event.code {
                    KeyCode::Char(c) => {
                        buffer.push(c);
                        print!("{c}");
                        stdout().flush().ok()?;
                    }
                    KeyCode::Enter => {
                        println!();
                        return Some(buffer);
                    }
                    KeyCode::Backspace => {
                        if buffer.pop().is_some() {
                            print!("\u{8} \u{8}"); // move back, erase, move back
                            stdout().flush().ok()?;
                        }
                    }
                    KeyCode::Esc => {
                        return None; // or handle differently
                    }
                    _ => {}
                }
            }
        }
    }
    pub fn run(&mut self) -> Result<(), io::Error> {
        loop {
            self.terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([Constraint::Min(5), Constraint::Length(3)].as_ref())
                    .split(f.size());

                let log = Paragraph::new(self.output_log.join("\n"))
                    .block(Block::default().title("Output").borders(Borders::ALL));
                f.render_widget(log, chunks[0]);

                let input = Paragraph::new(self.input_buffer.as_str())
                    .block(Block::default().title("Input").borders(Borders::ALL));
                f.render_widget(input, chunks[1]);
            })?;

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char(c) => self.input_buffer.push(c),
                        KeyCode::Backspace => {
                            self.input_buffer.pop();
                        }
                        KeyCode::Enter => {
                            let line = self.input_buffer.trim().to_string();
                            self.output_log.push(format!("> {line}"));
                            self.input_buffer.clear();

                        }
                        _ => {}
                    }
                }
            }
        }

        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<(), io::Error> {
        terminal::disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            terminal::LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}