////////////////////////////
use crossterm::event::{poll, read, Event, KeyCode, KeyEvent};
/// File: src/files.rs
///
/// This module provides functionality for reading and writing files:
///      source code is read from a file or stdin one line at a time,
///      and the file words (open-file etc.) read and write lines.
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

// Check if a key is available. Only works properly in raw mode
pub fn key_available() -> bool {
    poll(Duration::from_millis(0)).unwrap_or(false)
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FileMode {
    RW, // -1 => Read-write
    RO, //  0 => Read-only
    WO, //  1 => Write-only
}

pub enum FType {
    Stdin, // Standard input
    File(File),
    BReader(BufReader<File>), // Buffered reader for file input
    Text(io::Cursor<Vec<u8>>), // a string being interpreted by EVALUATE
}

pub struct FileHandle {
    pub source: FType, // Stdin, File, or BufReader
    pub file_position: usize,
    pub dir: Option<PathBuf>, // the file's directory, so includes can be relative to it
}

/// Read one line of at most `max` bytes, without the newline. None at end of file.
///     A longer line is returned in pieces by successive calls.
fn read_line_max(r: &mut impl Read, max: usize) -> io::Result<Option<(Vec<u8>, usize)>> {
    let mut line = Vec::new();
    let mut consumed = 0;
    let mut byte = [0u8; 1];
    while line.len() < max {
        if r.read(&mut byte)? == 0 {
            break; // end of file
        }
        consumed += 1;
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    Ok(if consumed == 0 && max > 0 {
        None
    } else {
        Some((line, consumed))
    })
}

impl FileHandle {
    /// text makes an input source from a string, for EVALUATE
    pub fn text(text: Vec<u8>) -> FileHandle {
        FileHandle {
            source: FType::Text(io::Cursor::new(text)),
            file_position: 0,
            dir: None,
        }
    }

    pub fn stdin() -> FileHandle {
        FileHandle {
            source: FType::Stdin,
            file_position: 0,
            dir: None,
        }
    }

    /// open opens a file: read-only files are buffered for line reading;
    ///     write-only files are created or truncated; read-write files are created if necessary.
    pub fn open(path: &Path, mode: FileMode) -> io::Result<FileHandle> {
        let source = match mode {
            FileMode::RO => FType::BReader(BufReader::new(File::open(path)?)),
            FileMode::WO => FType::File(
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(path)?,
            ),
            FileMode::RW => FType::File(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(path)?,
            ),
        };
        Ok(FileHandle {
            source,
            file_position: 0,
            dir: path.parent().map(Path::to_path_buf),
        })
    }

    /// get_line returns the next line of source as raw bytes, without trailing whitespace.
    ///     None means end of input (or a read error).
    pub fn get_line(&mut self) -> Option<Vec<u8>> {
        let mut line = Vec::new();
        let read = match &mut self.source {
            FType::Stdin => io::stdin().lock().read_until(b'\n', &mut line),
            FType::BReader(br) => br.read_until(b'\n', &mut line),
            FType::Text(text) => text.read_until(b'\n', &mut line),
            FType::File(_) => return None, // Files don't support line reading
        };
        match read {
            Ok(n) if n > 0 => {
                while line.last().is_some_and(|c| c.is_ascii_whitespace()) {
                    line.pop();
                }
                Some(line)
            }
            _ => None,
        }
    }

    /// read_line reads a line of at most `max` bytes for READ-LINE. None at end of file.
    pub fn read_line(&mut self, max: usize) -> io::Result<Option<Vec<u8>>> {
        let result = match &mut self.source {
            FType::BReader(br) => read_line_max(br, max)?,
            FType::File(f) => read_line_max(f, max)?,
            FType::Stdin => read_line_max(&mut io::stdin().lock(), max)?,
            FType::Text(text) => read_line_max(text, max)?,
        };
        Ok(result.map(|(line, consumed)| {
            self.file_position += consumed;
            line
        }))
    }

    /// write_line writes the bytes followed by a newline, for WRITE-LINE.
    pub fn write_line(&mut self, bytes: &[u8]) -> io::Result<()> {
        match &mut self.source {
            FType::File(f) => {
                f.write_all(bytes)?;
                f.write_all(b"\n")?;
                self.file_position += bytes.len() + 1;
                Ok(())
            }
            _ => Err(io::Error::other("file is not open for writing")),
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
                                _ => None,                             // Ignore other keys
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
            FType::File(_) | FType::Text(_) => {
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
        let metadata = match &self.source {
            FType::File(f) => f.metadata(),
            FType::BReader(br) => br.get_ref().metadata(),
            FType::Stdin | FType::Text(_) => return 0,
        };
        metadata.map(|m| m.len() as usize).unwrap_or(0)
    }
}

//////////////////////////////////////////
// TESTS
//
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_line() {
        let mut handle = FileHandle::open(Path::new("src/test.fs"), FileMode::RO).unwrap();
        assert!(handle.get_line().is_some());
    }

    #[test]
    #[ignore = "needs interactive stdin"]
    fn test_read_char() {
        // This test requires interactive input
        let mut handle = FileHandle::stdin();
        println!("Please enter a character:");
        assert!(handle.read_char().is_some());
    }

    #[test]
    fn test_stdin_handle() {
        let handle = FileHandle::stdin();
        assert_eq!(handle.file_position(), 0);
        assert_eq!(handle.file_size(), 0);
    }

    #[test]
    fn test_read_line_max() {
        let mut input: &[u8] = b"abcdef\r\nxy\n\nz";
        let mut lines = vec![];
        while let Some((line, _)) = read_line_max(&mut input, 4).unwrap() {
            lines.push(String::from_utf8(line).unwrap());
        }
        assert_eq!(lines, ["abcd", "ef", "xy", "", "z"]);
    }

    #[test]
    fn test_write_then_read() {
        let path = std::env::temp_dir().join("f3_files_test.txt");
        let mut w = FileHandle::open(&path, FileMode::WO).unwrap();
        w.write_line(b"hello").unwrap();
        drop(w);
        let mut r = FileHandle::open(&path, FileMode::RO).unwrap();
        assert_eq!(r.file_size(), 6);
        assert_eq!(r.read_line(100).unwrap().unwrap(), b"hello");
        assert_eq!(r.file_position(), 6);
        assert!(r.read_line(100).unwrap().is_none());
    }
}
