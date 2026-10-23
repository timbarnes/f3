/// Input-output words
use crate::internals::files::{FType, FileHandle, FileMode};
use crate::kernel::{throw, throw_code, BUF_SIZE, FILE_ERROR, NO_SUCH_FILE};
use crate::runtime::{ForthRuntime, FALSE, TRUE};
use std::io::{self, Write};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
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

    /// accept ( c-addr u -- u2 ) Read a line, storing up to u characters at c-addr; return the count.
    ///     If the read fails, we assume EOF, and pop the reader. Returned length will be 0.
    ///
    ///     Return the start of the string, and the number of characters read.
    ///     Longer lines are truncated to u characters (at most 255, the counted-string limit).
    ///
    pub fn f_accept(&mut self) {
        if self.kernel.stack_check(2, "accept") {
            let max_len = self.kernel.pop().max(0) as usize;
            let dest = self.kernel.pop() as usize;
            let line = self.read_line().unwrap_or_default();
            let length = line.len().min(max_len);
            self.kernel.bytes_write(&line[..length], dest);
            self.kernel.push(length as i64);
        }
    }

    /// read_line returns the next line from the current input source, or None at its end.
    ///     At the end of an included file, input returns to the previous source.
    ///     At the end of stdin, the interpreter exits.
    ///
    fn read_line(&mut self) -> Option<Vec<u8>> {
        let line = self.reader.last_mut().and_then(|reader| reader.get_line());
        if line.is_none() {
            if self.reader.len() > 1 {
                self.reader.pop(); // file goes out of scope and is closed automatically
            } else {
                self.exit_flag = true; // reader 0 is stdin
            }
        }
        line
    }

    /// evaluate ( c-addr u -- ) interprets the characters at c-addr as input, then carries on with
    ///     the rest of the current line. An error propagates (to CATCH, or to QUIT) after the
    ///     current input line has been restored.
    ///
    pub fn f_evaluate(&mut self) {
        self.kernel.stack_check(2, "evaluate");
        let len = self.kernel.pop();
        let addr = self.kernel.pop();
        self.kernel.string_slice(addr as usize, len as usize); // throws if out of range
        // interpret the string in place: SOURCE is the string itself (ANS). The input buffer
        // variables point at it ('tib is one below the text, where TIB's count byte would be).
        let saved = (
            self.kernel.get(self.tib_ptr),
            self.kernel.get(self.tib_size_ptr),
            self.kernel.get(self.tib_in_ptr),
        );
        self.kernel.set(self.tib_ptr, addr - 1);
        self.kernel.set(self.tib_size_ptr, len);
        self.kernel.set(self.tib_in_ptr, 0);
        let result = catch_unwind(AssertUnwindSafe(|| self.f_eval()));
        self.kernel.set(self.tib_ptr, saved.0);
        self.kernel.set(self.tib_size_ptr, saved.1);
        self.kernel.set(self.tib_in_ptr, saved.2);
        if let Err(payload) = result {
            resume_unwind(payload);
        }
    }

    /// interpret_source interprets an input source (a string, or a file) to its end, then
    ///     restores the current input line, so the rest of that line still runs.
    ///     An error propagates (to CATCH, or to QUIT) after the input line has been restored.
    ///
    fn interpret_source(&mut self, source: FileHandle) {
        // save the current input line, to restore afterwards
        let tib = self.kernel.get(self.tib_ptr) as usize;
        let line = self.kernel.counted_bytes(tib).to_vec();
        let size = self.kernel.get(self.tib_size_ptr);
        let offset = self.kernel.get(self.tib_in_ptr);
        let depth = self.reader.len();
        self.reader.push(source);
        let result = catch_unwind(AssertUnwindSafe(|| {
            while self.reader.len() > depth && !self.should_exit() {
                self.f_query();
                self.f_eval();
            }
        }));
        self.reader.truncate(depth);
        self.kernel.bytes_save(&line, tib);
        self.kernel.set(self.tib_size_ptr, size);
        self.kernel.set(self.tib_in_ptr, offset);
        if let Err(payload) = result {
            resume_unwind(payload);
        }
    }

    /// QUERY ( -- ) Load a new line of text into the TIB
    ///
    pub fn f_query(&mut self) {
        self.fill_tib();
    }

    /// refill ( -- flag ) reads the next line of the current input into the TIB.
    ///     False at the end of the input (an included file, or stdin).
    ///
    pub fn f_refill(&mut self) {
        let ok = self.fill_tib();
        self.kernel.push(if ok { TRUE } else { FALSE });
    }

    fn fill_tib(&mut self) -> bool {
        let tib = self.kernel.get(self.tib_ptr) as usize;
        let line = self.read_line();
        let ok = line.is_some();
        let line = line.unwrap_or_default();
        if line.len() >= BUF_SIZE {
            throw(format!(
                "input line longer than {} characters",
                BUF_SIZE - 1
            ));
        }
        self.kernel.bytes_save(&line, tib);
        self.kernel.set(self.tib_size_ptr, line.len() as i64); // update the TIB size pointer
        self.kernel.set(self.tib_in_ptr, 0); // >IN: 0-based offset into the line
        ok
    }

    // output functions

    /// emit ( c -- ) takes a character from the stack and prints it as a raw byte.
    ///
    pub fn f_emit_p(&mut self) {
        if self.kernel.stack_check(1, "emit") {
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

    /// pop_text pops ( c-addr u ) and returns the characters as text (a file name, for example)
    ///
    fn pop_text(&mut self, word: &str) -> String {
        self.kernel.stack_check(2, word);
        let len = self.kernel.pop() as usize;
        let addr = self.kernel.pop() as usize;
        String::from_utf8_lossy(self.kernel.string_slice(addr, len)).into_owned()
    }

    /// included ( c-addr u -- ) interprets the file named by the characters at c-addr, then
    ///     carries on with the rest of the current line (includes can nest).
    ///     A relative path is looked up next to the including file first, then in the current directory.
    ///
    pub fn f_included(&mut self) {
        let name = self.pop_text("included");
        match FileHandle::open(&self.resolve_path(&name), FileMode::RO) {
            Ok(handle) => self.interpret_source(handle),
            Err(e) => throw_code(NO_SUCH_FILE, format!("{name}: {e}")),
        }
    }

    /// include-file ( file-id -- ) interprets the rest of an open file, then closes it
    ///
    pub fn f_include_file(&mut self) {
        self.kernel.stack_check(1, "include-file");
        let file_id = self.kernel.pop();
        self.file(file_id); // throws if not open
        let handle = self.files[file_id as usize].take().unwrap();
        if !matches!(handle.source, FType::BReader(_)) {
            throw_code(FILE_ERROR, "include-file: the file is not open for reading (R/O)");
        }
        self.interpret_source(handle);
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
            None => throw_code(FILE_ERROR, format!("invalid file-id {file_id}")),
        }
    }

    /// open-file ( c-addr u fam -- file-id ior ) Open the file named by the characters at c-addr,
    ///     with file access mode fam (R/O, W/O, R/W). W/O and R/W create the file if necessary;
    ///     W/O truncates it. ior is 0 on success, -1 on failure.
    pub fn f_open_file(&mut self) {
        self.kernel.stack_check(3, "open-file");
        let mode = match self.kernel.pop() {
            -1 => FileMode::RW,
            1 => FileMode::WO,
            _ => FileMode::RO,
        };
        let name = self.pop_text("open-file");
        let handle = FileHandle::open(&self.resolve_path(&name), mode);
        self.push_file(&name, handle);
    }

    /// create-file ( c-addr u fam -- file-id ior ) Create a file, or truncate an existing one
    ///
    pub fn f_create_file(&mut self) {
        self.kernel.stack_check(3, "create-file");
        let readable = self.kernel.pop() == -1; // R/W
        let name = self.pop_text("create-file");
        let handle = FileHandle::create(&self.resolve_path(&name), readable);
        self.push_file(&name, handle);
    }

    /// delete-file ( c-addr u -- ior )
    ///
    pub fn f_delete_file(&mut self) {
        let name = self.pop_text("delete-file");
        let ior = match std::fs::remove_file(self.resolve_path(&name)) {
            Ok(()) => 0,
            Err(_) => -1,
        };
        self.kernel.push(ior);
    }

    fn push_file(&mut self, name: &str, handle: std::io::Result<FileHandle>) {
        match handle {
            Ok(handle) => {
                self.files.push(Some(handle));
                self.kernel.push(self.files.len() as i64 - 1); // the index is the file-id
                self.kernel.push(0);
            }
            Err(e) => {
                self.msg.warning("open-file", &format!("{name}: {e}"), None::<bool>);
                self.kernel.push(0);
                self.kernel.push(-1);
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

    /// read-line ( c-addr u file-id -- u2 flag ior ) Read a line of up to u characters into the
    ///     buffer at c-addr (in string space; s-allot makes one). u2 is the number of characters
    ///     read, without the newline. A longer line is returned in pieces by successive calls.
    ///     At end of file, u2 is 0 and flag is FALSE. ior is 0 on success, -1 on a read error.
    pub fn f_read_line(&mut self) {
        if self.kernel.stack_check(3, "read-line") {
            let file_id = self.kernel.pop();
            let max = self.kernel.pop().max(0) as usize;
            let addr = self.kernel.pop() as usize;
            match self.file(file_id).read_line(max) {
                Ok(Some(line)) => {
                    self.kernel.bytes_write(&line, addr);
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

    ///  write-line ( c-addr u file-id -- ior ) Write u characters at c-addr, and a newline.
    ///     ior is 0 on success, -1 on a write error (or if the file isn't open for writing).
    pub fn f_write_line(&mut self) {
        if self.kernel.stack_check(3, "write-line") {
            let file_id = self.kernel.pop();
            let chars = self.kernel.pop() as usize;
            let addr = self.kernel.pop() as usize;
            let bytes = self.kernel.string_slice(addr, chars).to_vec();
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
