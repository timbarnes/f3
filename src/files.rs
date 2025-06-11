// Read tokens from a file or stdin, one line at a time.
// Return one space-delimited token at a time.
// Cache the remainder of the line.

use std::fs::File;
use std::io::{self, BufReader, BufRead, Read, Write};

use crate::messages::{DebugLevel, Msg};

#[derive(Debug)]
pub enum FileMode {
    RW,     // -1 => Read-write
    RO,     //  0 => Read-only
    WO,     //  1 => Write-only
}

#[derive(Debug)]
pub enum FType {
    Stdin,
    File(File),
    BReader(BufReader<File>),
}

#[derive(Debug)]
pub struct FileHandle {
    pub source: FType,               // Stdin, File, or BufReader
    pub file_mode: FileMode,
    pub file_size: usize,
    pub file_position: usize,
    msg: Msg,
}

/// Reader handles input, from stdin or files
/// 
///     A populated FileHandle is always for a specific file.
///     Stdin has None in the source field, and the other fields are not used in this case.
impl FileHandle {

    pub fn new(file_path: Option<&std::path::PathBuf>, msg_handler: Msg, mode: FileMode) -> Option<FileHandle> {
        // Initialize a tokenizer.
        let mut message_handler = Msg::new();
        message_handler.set_level(DebugLevel::Error);
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
                                    file_mode: FileMode::RO,
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

    /// get_line returns a line of text from the input stream, or an error if unable to do so
    ///
    pub fn get_line(&mut self) -> Option<String> {
        // Read a line, storing it if there is one
        // In interactive (stdin) mode, blocks until the user provides a line.
        // Returns Option(line text). None indicates the read failed.
        let mut new_line = String::new();
        let result;
        match self.source {
            FType::Stdin => {
                io::stdout().flush().unwrap();
                result = io::stdin().read_line(&mut new_line);
            }
            FType::BReader(ref mut br) => result = br.read_line(&mut new_line),
            _ => { return None }
        }
        match result {
            Ok(chars) => {
                if chars > 0 {
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
}
