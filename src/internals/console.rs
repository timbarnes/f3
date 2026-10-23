/// Input-output words
use crate::internals::files::{FileHandle, FileMode};
use crate::kernel::{throw, BUF_SIZE};
use crate::runtime::{ForthRuntime, FALSE, TRUE};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

/// Write to stdout; if stdout is gone (e.g. a closed pipe) there is no one left to talk to, so exit.
fn out(bytes: &[u8]) {
    if io::stdout().write_all(bytes).is_err() {
        std::process::exit(0);
    }
}

fn flush() {
    if io::stdout().flush().is_err() {
        std::process::exit(0);
    }
}

// file I/O and system call
//
// Most activity uses STDIN and STDOUT, but the system can also process source code
// from files (typically ending in .fs.
//
// Additionally basic general file I/O is supported with file-open, file-close, read-line, write-line
// and utilities file-size and file-position capturing file size and position within the file.
//
// File modes can be r/w or r/o, and are set with constants passed to open-file.
//
// Forth needs an i64 / usize as a file reference. This is achieved by creating a vector of file handles.
// Forth accesses files via an index into the vector.

impl ForthRuntime {
    /// (system) ( s -- ) Execute a shell command from the string on the stack (Unix-like operating systems)
    ///
    pub fn f_system_p(&mut self) {
        if self.kernel.stack_check(1, "(system)") {
            let addr = self.kernel.pop() as usize;
            let cmd_string = self.kernel.string_get(addr);
            let mut args = cmd_string.split_ascii_whitespace();
            let mut cmd: Command;
            let c = args.next();
            match c {
                Some(c) => cmd = Command::new(c),
                None => return,
            }
            cmd.args(args);
            let output = match cmd.output() {
                Ok(output) => output,
                Err(e) => throw(format!("(system): {e}")),
            };
            out(&output.stdout);
            let _ = io::stderr().write_all(&output.stderr);
        }
    }

    /// key ( -- c | 0 ) get a character and push on the stack, or zero if none available
    pub fn f_key(&mut self) {
        if let Some(reader) = self.reader.last_mut() {
            let c = reader.read_char();
            match c {
                Some(c) => {
                    self.kernel.push(c as u8 as i64);
                }
                None => {
                    self.kernel.push(0);
                }
            }
        }
    }

    /// key? ( -- b ) Checks to see if a character is ready to be read.
    ///     Only works properly in raw mode. In cooked mode it waits until EOL.
    ///
    pub fn f_key_q(&mut self) {
        let result = crate::internals::files::key_available();
        if result {
            self.kernel.push(TRUE);
        } else {
            self.kernel.push(FALSE);
        }
    }

    /// accept ( b u -- b u ) Read up to u characters, storing them at string address b and returning the actual length.
    ///     If the read fails, we assume EOF, and pop the reader. Returned length will be 0.
    ///
    ///     Return the start of the string, and the number of characters read.
    ///     Longer lines are truncated to u characters (at most 255, the counted-string limit).
    ///
    pub fn f_accept(&mut self) {
        if self.kernel.stack_check(2, "accept") {
            let max_len = self.kernel.pop().clamp(0, BUF_SIZE as i64 - 1) as usize;
            let dest = self.kernel.top() as usize;
            let line = self.read_line();
            let length = line.len().min(max_len);
            self.kernel.bytes_save(&line[..length], dest); // write a counted string
            self.kernel.push(length as i64);
        }
    }

    /// read_line returns the next line from the current input source.
    ///     At the end of an included file, input returns to the previous source and the line is empty.
    ///     At the end of stdin, the interpreter exits.
    ///
    fn read_line(&mut self) -> Vec<u8> {
        match self.reader.last_mut().and_then(|reader| reader.get_line()) {
            Some(line) => line,
            None => {
                if self.reader.len() > 1 {
                    self.reader.pop(); // file goes out of scope and is closed automatically
                } else {
                    self.exit_flag = true; // reader 0 is stdin
                }
                Vec::new()
            }
        }
    }

    /// QUERY ( -- ) Load a new line of text into the TIB
    ///
    pub fn f_query(&mut self) {
        let tib = self.kernel.get(self.tib_ptr) as usize;
        let line = self.read_line();
        if line.len() >= BUF_SIZE {
            throw(format!(
                "input line longer than {} characters",
                BUF_SIZE - 1
            ));
        }
        self.kernel.bytes_save(&line, tib);
        self.kernel.set(self.tib_size_ptr, line.len() as i64); // update the TIB size pointer
        self.kernel.set(self.tib_in_ptr, 1); // set the starting point in the TIB
    }

    // output functions

    /// (emit) ( c -- ) takes a character from the stack and prints it as a raw byte.
    ///
    pub fn f_emit_p(&mut self) {
        if self.kernel.stack_check(1, "(emit)") {
            let c = self.kernel.pop() as u8;
            out(&[c]);
        }
    }

    /// flush ( -- ) Push any characters in Rust's output buffer out.
    ///     By default printed characters are buffered until a newline.
    ///     This forces them out sooner
    ///
    pub fn f_flush(&mut self) {
        flush();
    }

    /// include-file (s -- T | F ) Pushes a new reader for the file named at s, returning TRUE,
    ///     or FALSE if the file can't be opened.
    ///     The standard loop then continues, reading lines from the file.
    ///     At the end of the file, the reader is popped off the stack. This allows for nested file reads.
    ///     A relative path is looked up next to the including file first, then in the current directory.
    ///
    pub fn f_include_file(&mut self) {
        if self.kernel.stack_check(1, "include-file") {
            let addr = self.kernel.pop() as usize;
            let name = self.kernel.string_get(addr);
            match FileHandle::open(&self.resolve_path(&name), FileMode::RO) {
                Ok(handle) => {
                    self.reader.push(handle);
                    self.kernel.push(TRUE);
                }
                Err(e) => {
                    self.msg
                        .warning("include", &format!("{name}: {e}"), None::<bool>);
                    self.kernel.push(FALSE);
                }
            }
        }
    }

    /// resolve_path expands a leading ~, and tries a relative path next to the file being read
    ///
    fn resolve_path(&self, name: &str) -> PathBuf {
        let path = match name.strip_prefix("~/") {
            Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
            None => PathBuf::from(name),
        };
        if path.is_relative() {
            if let Some(dir) = self.reader.last().and_then(|r| r.dir.as_ref()) {
                let beside = dir.join(&path);
                if beside.exists() {
                    return beside;
                }
            }
        }
        path
    }

    /// load_file interprets a whole source file before returning. Used at startup.
    ///     Returns false if the file can't be opened.
    ///
    pub fn load_file(&mut self, name: &str) -> bool {
        match FileHandle::open(&self.resolve_path(name), FileMode::RO) {
            Ok(handle) => {
                let depth = self.reader.len();
                self.reader.push(handle);
                self.run_lines(depth);
                true
            }
            Err(_) => false,
        }
    }

    /// file returns the open file with the given file-id, or throws
    ///
    fn file(&mut self, file_id: i64) -> &mut FileHandle {
        match self
            .files
            .get_mut(file_id as usize)
            .and_then(Option::as_mut)
        {
            Some(file) => file,
            None => throw(format!("invalid file-id {file_id}")),
        }
    }

    /// open-file ( s fam -- file-id ior ) Open the file named by the counted string at s,
    ///     with file access mode fam (R/O, W/O, R/W). W/O and R/W create the file if necessary;
    ///     W/O truncates it. ior is 0 on success, -1 on failure.
    pub fn f_open_file(&mut self) {
        if self.kernel.stack_check(2, "open-file") {
            let mode = match self.kernel.pop() {
                -1 => FileMode::RW,
                1 => FileMode::WO,
                _ => FileMode::RO,
            };
            let addr = self.kernel.pop() as usize;
            let name = self.kernel.string_get(addr);
            match FileHandle::open(&self.resolve_path(&name), mode) {
                Ok(handle) => {
                    self.files.push(Some(handle));
                    self.kernel.push(self.files.len() as i64 - 1); // the index is the file-id
                    self.kernel.push(0);
                }
                Err(e) => {
                    self.msg
                        .warning("open-file", &format!("{name}: {e}"), None::<bool>);
                    self.kernel.push(0);
                    self.kernel.push(-1);
                }
            }
        }
    }

    ///  close-file ( file-id -- ior ) Close a file, returning the I/O status code.
    ///     Dropping the handle closes the file; other file-ids are unaffected.
    pub fn f_close_file(&mut self) {
        if self.kernel.stack_check(1, "close-file") {
            let file_id = self.kernel.pop();
            self.file(file_id); // throws if not open
            self.files[file_id as usize] = None;
            self.kernel.push(0);
        }
    }

    /// read-line ( u file-id -- u2 flag ior ) Read a line of up to u characters (at most 255) into TMP.
    ///     u2 is the number of characters read, without the newline. A longer line is
    ///     returned in pieces by successive calls. At end of file, u2 is 0 and flag is FALSE.
    ///     ior is 0 on success, -1 on a read error.
    pub fn f_read_line(&mut self) {
        if self.kernel.stack_check(2, "read-line") {
            let file_id = self.kernel.pop();
            let max = self.kernel.pop().clamp(0, BUF_SIZE as i64 - 1) as usize;
            match self.file(file_id).read_line(max) {
                Ok(Some(line)) => {
                    let addr = self.kernel.get(self.tmp_ptr) as usize;
                    self.kernel.bytes_save(&line, addr);
                    self.kernel.push(line.len() as i64);
                    self.kernel.push(TRUE);
                    self.kernel.push(0);
                }
                Ok(None) => {
                    self.kernel.push(0);
                    self.kernel.push(FALSE);
                    self.kernel.push(0);
                }
                Err(_) => {
                    self.kernel.push(0);
                    self.kernel.push(FALSE);
                    self.kernel.push(-1);
                }
            }
        }
    }

    ///  write-line ( s u file-id -- ior ) Write u characters of the counted string at s, and a newline.
    ///     ior is 0 on success, -1 on a write error (or if the file isn't open for writing).
    pub fn f_write_line(&mut self) {
        if self.kernel.stack_check(3, "write-line") {
            let file_id = self.kernel.pop();
            let chars = self.kernel.pop() as usize;
            let addr = self.kernel.pop() as usize;
            let bytes = self.kernel.string_slice(addr + 1, chars).to_vec();
            let ior = match self.file(file_id).write_line(&bytes) {
                Ok(()) => 0,
                Err(_) => -1,
            };
            self.kernel.push(ior);
        }
    }

    ///  file-size ( file-id -- u ior ) Returns the size in characters of the file, plus an i/o result code
    pub fn f_file_size(&mut self) {
        if self.kernel.stack_check(1, "file-size") {
            let file_id = self.kernel.pop();
            let size = self.file(file_id).file_size();
            self.kernel.push(size as i64);
            self.kernel.push(0);
        }
    }

    /// file-position ( file-id -- u ior ) Returns the current file position and an i/o result
    pub fn f_file_position(&mut self) {
        if self.kernel.stack_check(1, "file-position") {
            let file_id = self.kernel.pop();
            let position = self.file(file_id).file_position();
            self.kernel.push(position as i64);
            self.kernel.push(0);
        }
    }
}
