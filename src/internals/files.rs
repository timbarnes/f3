////////////////////////////
/// File: src/files.rs
/// 
/// This module provides functionality for reading and writing files,
///      Read tokens from a file or stdin, one line at a time.
///      Return one space-delimited token at a time.
///      Cache the remainder of the line.

use std::fs::File;
use std::io::{self, BufReader, BufRead, Read, Write, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Mutex;
use std::collections::HashMap;

use crate::internals::tui::ForthTui; 
use crate::internals::messages::{DebugLevel, Msg};

#[derive(Debug, PartialEq)]
pub enum FileMode {
    RW,     // -1 => Read-write
    RO,     //  0 => Read-only
    WO,     //  1 => Write-only
}
#[derive(Debug)]
pub enum FType {
    Stdin,                          // Standard input (deprecated, use Tui)
    File(File),
    BReader(BufReader<File>),       // Buffered reader for file input
    Tui(ForthTui),                  // ratatui terminal input
}

//#[derive(Debug)]
pub struct FileHandle {
    pub source: FType,               // Stdin, File, BufReader, or Tui
    file_mode: FileMode,
    file_size: usize,
    file_position: usize,
    msg: Msg,
}

/// Reader handles input, from stdin or files
/// 
///     A populated FileHandle is always for a specific file.
///     Stdin has None in the source field, and the other fields are not used in this case.
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
                                    msg: msg_handler,
                                }),
                            FileMode::RW | FileMode::WO => {
                                Some(FileHandle {
                                    source: FType::File(file),
                                    file_mode: mode,
                                    file_size: 0,
                                    file_position: 0,
                                    msg: msg_handler,
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
            None => Some(FileHandle { // Stdin
                source: FType::Stdin,
                file_mode: FileMode::RO,
                file_size: 0,
                file_position: 0,
                msg: message_handler,
            }),
        }
    }

    pub fn new_tui(msg_handler: Msg) -> FileHandle {
        let tui = ForthTui::new();
        FileHandle {
            source: FType::Tui(tui.unwrap()),
            file_mode: FileMode::RO,
            file_size: 0,
            file_position: 0,
            msg: msg_handler, // replace with your main message handler if needed
        }
    }

    /// get_line returns a line of text from the input stream, or an error if unable to do so
    ///
    pub fn get_line(&mut self) -> Option<String> {
        // Read a line, storing it if there is one
        // In interactive (stdin) mode, blocks until the user provides a line.
        // Returns Option(line text). None indicates the read failed.
        let mut new_line = String::new();
        let result;
 
        match &mut self.source {
            FType::Stdin => {
                io::stdout().flush().unwrap();
                result = io::stdin().read_line(&mut new_line);
            }
            FType::BReader(ref mut br) => {
                if self.file_mode == FileMode::WO {
                    println!("Error: Cannot read from a write-only file");
                    return None
                } else {
                    result = br.read_line(&mut new_line)
                }
            },
             FType::Tui(tui) => {
            // Delegate to your tui's get_line method
            return tui.get_line();
        }
            _ => { return None }
        }
        match result {
            Ok(chars) => {
                if chars > 0 {
                    let new_line = new_line.trim_end().to_string(); // Remove trailing newline
                    Some(new_line)
                } else {
                    None
                }
            }
            Err(e) => {
                self.msg
                    .error("get_line", "read_line error", Some(e.to_string()));
                None
            }
        }
    }

    /// read_char gets a single character from the input stream
    ///     Unfortunately it blocks until the user types return, so it can't be used
    ///     for truly interactive operations without a more complex implementation
    ///
    pub fn read_char(&self) -> Option<char> {
        let mut buf = [0; 1];
        let mut handle = io::stdin().lock();
        let bytes_read = handle.read(&mut buf);
        match bytes_read {
            Ok(_size) => Some(buf[0] as char),
            Err(_) => None,
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
        let handle = FileHandle::new_file(None, Msg::new(), FileMode::RO).unwrap();
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