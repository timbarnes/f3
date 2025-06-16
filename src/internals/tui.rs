use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{enable_raw_mode, disable_raw_mode},
};
use std::{
    io::{self, Write, stdout},
};

#[derive(Debug)]
pub struct ForthLineEditor {
    buffer: Vec<char>,
    cursor: usize,
    previous_line: Option<String>,
}

impl ForthLineEditor {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            cursor: 0,
            previous_line: None,
        }
    }

    pub fn run(&mut self) -> Option<String> {
        // Reset state
        self.buffer.clear();
        self.cursor = 0;
        
        enable_raw_mode().ok()?;
        stdout().flush().ok()?;
        
        // Print initial prompt
        print!("ok> ");
        stdout().flush().ok()?;
        
        let result = loop {
            if let Ok(Event::Key(key)) = event::read() {
                match key.code {
                    KeyCode::Char(c) if key.modifiers.is_empty() => {
                        self.buffer.insert(self.cursor, c);
                        self.cursor += 1;
                        print!("{}", c);
                        stdout().flush().ok()?;
                    }
                    KeyCode::Backspace => {
                        if self.cursor > 0 {
                            self.cursor -= 1;
                            self.buffer.remove(self.cursor);
                            print!("\x08 \x08"); // Backspace, space, backspace
                            stdout().flush().ok()?;
                        }
                    }
                    KeyCode::Left => {
                        if self.cursor > 0 {
                            self.cursor -= 1;
                            print!("\x1b[D"); // Move left
                            stdout().flush().ok()?;
                        }
                    }
                    KeyCode::Right => {
                        if self.cursor < self.buffer.len() {
                            self.cursor += 1;
                            print!("\x1b[C"); // Move right
                            stdout().flush().ok()?;
                        }
                    }
                    KeyCode::Up => {
                        if let Some(prev) = &self.previous_line {
                            // Clear current line
                            print!("\r\x1b[K"); // Carriage return and clear line
                            self.buffer = prev.chars().collect();
                            self.cursor = self.buffer.len();
                            print!("ok> {}", prev);
                            stdout().flush().ok()?;
                        }
                    },
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        while self.cursor > 0 {
                            self.cursor -= 1;
                            print!("\x1b[D"); // Move left
                        }
                        stdout().flush().ok()?;
                    }
                    KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        while self.cursor < self.buffer.len() {
                            self.cursor += 1;
                            print!("\x1b[C"); // Move right
                        }
                        stdout().flush().ok()?;
                    }
                    KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Clear from cursor to end of line
                        print!("\x1b[K"); // Clear from cursor to end
                        self.buffer.truncate(self.cursor);
                        stdout().flush().ok()?;
                    }
                    KeyCode::Enter => {
                        let line: String = self.buffer.iter().collect();
                        self.previous_line = Some(line.clone());
                        print!("\r\n"); // Move to next line
                        stdout().flush().ok()?;
                        break Some(line);
                    }                    
                    KeyCode::Esc => {
                        print!("\r\n"); // Move to next line
                        stdout().flush().ok()?;
                        break None;
                    }
                    _ => {}
                }
            }
        };

        // Clean up terminal state
        disable_raw_mode().ok()?;
        result
    }
}

#[derive(Debug)]
pub struct ForthTui {
    editor: ForthLineEditor,
}

impl ForthTui {
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self {
            editor: ForthLineEditor::new(),
        })
    }

    pub fn get_line(&mut self) -> Option<String> {
        self.editor.run()
    }

    pub fn cleanup(&mut self) -> Result<(), io::Error> {
        // Ensure we're not in raw mode
        disable_raw_mode()?;
        Ok(())
    }
}