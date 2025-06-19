////////////////////////////
/// File: src/files.rs
/// 
/// This module provides functionality for reading and writing files,
///      Read tokens from a file or stdin, one line at a time.
///      Return one space-delimited token at a time.
///      Cache the remainder of the line.

use std::fs::File;
use std::io::{self, BufReader, BufRead, Read};
use crossterm::event::{poll, read, Event, KeyEvent, KeyCode};
use std::time::Duration;

use crate::internals::messages::{DebugLevel, Msg};

#[derive(Debug, PartialEq)]
pub enum FileMode {
    RW,     // -1 => Read-write
    RO,     //  0 => Read-only
    WO,     //  1 => Write-only
}

pub enum FType {
    Stdin,                          // Standard input
    File(File),
    BReader(BufReader<File>),       // Buffered reader for file input
}

pub struct FileHandle {
    pub source: FType,               // Stdin, File, or BufReader
    pub file_mode: FileMode,
    pub file_size: usize,
    pub file_position: usize,
}

impl FileHandle {
    pub fn new_file(file_path: Option<&std::path::PathBuf>, msg_handler: Msg, mode: FileMode) -> Option<FileHandle> {
        // Initialize a tokenizer.
        let mut message_handler = Msg::new();
        message_handler.set_level(DebugLevel::Warning);
        match file_path {
            Some(file_path) => {
                let file = File::open(file_path);
                match file {
                    Ok(file) => {
                        match mode {
                            FileMode::RO => 
                                Some(FileHandle {
                                    source: FType::BReader(BufReader::new(file)),
                                    file_mode: FileMode::RO,
                                    file_size: 0,
                                    file_position: 0,        
                                }),
                            FileMode::RW | FileMode::WO => {
                                Some(FileHandle {
                                    source: FType::File(file),
                                    file_mode: mode,
                                    file_size: 0,
                                    file_position: 0,
                                })
                            }
                        }
                    }
                    Err(_) => {
                        msg_handler.error(
                            "Reader::new",
                            "Unable to open file",
                            Some(file_path),
                        );
                        return None;
                    }
                }
            }
            None => {
                Some(FileHandle {
                    source: FType::Stdin,
                    file_mode: FileMode::RO,
                    file_size: 0,
                    file_position: 0,
                })
            }
        }
    }

    pub fn get_line(&mut self) -> Option<String> {
        match &mut self.source {
            FType::Stdin => {
                let mut new_line = String::new();
                if std::io::stdin().read_line(&mut new_line).is_ok() {
                    Some(new_line.trim_end().to_string())
                } else {
                    None
                }
            }
            FType::BReader(ref mut br) => {
                let mut new_line = String::new();
                match br.read_line(&mut new_line) {
                    Ok(n) if n > 0 => Some(new_line.trim_end().to_string()),
                    _ => None,
                }
            }
            FType::File(_) => None, // Files don't support line reading
        }
    }

    pub fn read_char(&mut self) -> Option<char> {
        match &mut self.source {
            FType::Stdin => {
                // Check if we're in raw mode by trying to poll for events
                if poll(Duration::from_millis(0)).unwrap_or(false) {
                    // Raw mode - use crossterm event system
                    match read() {
                        Ok(Event::Key(KeyEvent { code, .. })) => {
                            match code {
                                KeyCode::Char(c) => Some(c),
                                KeyCode::Enter => Some('\n'),
                                KeyCode::Backspace => Some(8 as char), // ASCII backspace
                                KeyCode::Delete => Some(127 as char),  // ASCII delete
                                _ => None, // Ignore other keys
                            }
                        }
                        _ => None, // Ignore non-key events
                    }
                } else {
                    // Non-raw mode - use stdin
                    let mut buf = [0; 1];
                    let mut handle = io::stdin().lock();
                    let bytes_read = handle.read(&mut buf);
                    match bytes_read {
                        Ok(_size) => Some(buf[0] as char),
                        Err(_) => None,
                    }
                }
            }
            FType::File(_) => {
                // Files don't support character-by-character reading
                None
            }
            FType::BReader(ref mut br) => {
                // Read from file using buffered reader
                let mut buf = [0; 1];
                match br.read(&mut buf) {
                    Ok(1) => {
                        self.file_position += 1;
                        Some(buf[0] as char)
                    }
                    _ => None,
                }
            }
        }
    }

    pub fn file_position(&self) -> usize {
        // Returns the current file position
        self.file_position // Stdin has no position
    }

    pub fn file_size(&self) -> usize {
        // Returns the size of the file, or 0 for stdin
        self.file_size
    }

    #[allow(dead_code)]
    pub fn file_mode(&self) -> &FileMode {
        // Returns the file mode
        &self.file_mode
    }

    #[allow(dead_code)]
    pub fn set_file_mode(&mut self, mode: FileMode) {
        // Sets the file mode
        self.file_mode = mode;
    }
}

//////////////////////////////////////////
/// TESTS
/// 
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_file_handle_new() {
        let msg = Msg::new();
        let buf = &PathBuf::from("src/test.fs");
        let file_path = Some(buf);
        let handle = FileHandle::new_file(file_path, msg, FileMode::RO);
        assert!(handle.is_some());
    }

    #[test]
    fn test_get_line() {
        let buf = &PathBuf::from("src/test.fs");
        let file_path = Some(buf);
        let mut handle = FileHandle::new_file(file_path, Msg::new(), FileMode::RO).unwrap();
        let line = handle.get_line();
        assert!(line.is_some());
        println!("Read line: {:?}", line.unwrap());
    }

    #[test]
    fn test_read_char() { // This test requires interactive input
        let mut handle = FileHandle::new_file(None, Msg::new(), FileMode::RO).unwrap();
        println!("Please enter a character:");
        let ch = handle.read_char();
        assert!(ch.is_some());
        println!("Read character: {:?}", ch.unwrap());
    }

    #[test]
    fn test_file_position() {
        let handle = FileHandle::new_file(None, Msg::new(), FileMode::RO).unwrap();
        assert_eq!(handle.file_position(), 0);
    }

    #[test]
    fn test_file_size() {
        let handle = FileHandle::new_file(None, Msg::new(), FileMode::RO).unwrap();
        assert_eq!(handle.file_size(), 0);
    }

    #[test] 
    fn test_file_mode() {
        let mut handle = FileHandle::new_file(None, Msg::new(), FileMode::RO).unwrap();
        assert_eq!(handle.file_mode(), &FileMode::RO);
        handle.set_file_mode(FileMode::RW);
        assert_eq!(handle.file_mode(), &FileMode::RW);
    }
}