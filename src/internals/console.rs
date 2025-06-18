/// Input-output words
use crate::kernel::{BUF_SIZE, STACK_START};
use crate::internals::messages::Msg;
use crate::internals::files::{FileHandle, FType, FileMode};
use crate::runtime::{ForthRuntime, FALSE, TRUE, FILEMODE_RO};
use std::cmp::min;
use std::io::{self, Write, BufRead};
use std::process::Command;


    /// file I/O and system call
    /// 
    /// Most activity uses STDIN and STDOUT, but the system can also process source code
    /// from files (typically ending in .fs.
    /// 
    /// Additionally basic general file I/O is supported with file-open, file-close, read-line, write-line
    /// and utilities file-size and file-position capturing file size and position within the file.
    /// 
    /// File modes can be r/w or r/o, and are set with constants passed to open-file.
    /// 
    /// Forth needs an i64 / usize as a file reference. This is achieved by creating a vector of file handles.
    /// Forth accesses files via an index into the vector.

impl ForthRuntime {
    /// (system) ( s -- ) Execute a shell command from the string on the stack (Unix-like operating systems)
    /// 
pub fn f_system_p(&mut self) {
    if self.kernel.stack_check(1, "(system)") {
        let addr = self.kernel.pop() as usize;
        let cmd_string = self.kernel.string_get(addr);
        let mut args = cmd_string.split_ascii_whitespace();
        //println!("args: {:?}", args);
        let mut cmd: Command;
        let c = args.next();
        match c {
            Some(c) =>  cmd = Command::new(c),
            None => return,
        }
        for arg in args {
            println!("Adding {}", arg);
            cmd.arg(arg);
        }
        let output = cmd.output().expect("(system) failed to execute command");
        io::stdout().write_all(&output.stdout).unwrap();
        io::stderr().write_all(&output.stderr).unwrap();
   }
}

    /// key ( -- c | 0 ) get a character and push on the stack, or zero if none available
    pub fn f_key(&mut self) {
        let reader = self.reader.last();
        match reader {
            Some(reader) => {
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
            None => {}
        }
    }

    /// accept ( b u -- b u ) Read up to u characters, storing them at string address b and returning the actual length.
    ///     If the read fails, we assume EOF, and pop the reader. Returned length will be 0.
    ///
    ///     Return the start of the string, and the number of characters read.
    ///     Typically writes a counted string to the TIB, in which case,
    ///     it needs TIB_START and BUF_SIZE - 1 on the stack.
    ///
    pub fn f_accept(&mut self) {
        if self.kernel.stack_check(2, "accept") {
            let max_len = self.kernel.pop();
            let dest = self.kernel.top() as usize;
            match self.reader.last_mut() {
                Some(reader) => {
                    let l = reader.get_line();
                    match l {
                        Some(line) => {
                            let length = min(line.len(), max_len as usize) as usize;
                            let line_str = &line[..length];
                            self.kernel.string_save(line_str, dest); // write a counted string
                            self.kernel.push(length as i64);
                        }
                        None => {
                            // EOF - there are no more lines to read
                            if self.reader.len() > 1 {
                                // Reader 0 is stdin
                                self.reader.pop(); // file goes out of scope and should be closed automatically
                                self.kernel.push(0);
                            } else {
                                panic!("Reader error - EOF in stdin");
                            }
                        }
                    }
                }
                None => self
                    .msg
                    .error("accept", "No input source available", None::<bool>),
            }
        }
    }

    /// QUERY ( -- ) Load a new line of text into the TIB
    ///     
    pub fn f_query(&mut self) {
        let addr = self.kernel.get(self.tib_ptr);
        self.kernel.push(addr);
        self.kernel.push(BUF_SIZE as i64 - 1);
        self.f_accept();
        let val = self.kernel.pop(); // this is the length of the string read
        self.kernel.set(self.tib_size_ptr, val); // update the TIB size pointer
        self.kernel.set(self.tib_in_ptr, 1); // set the starting point in the TIB
        self.kernel.pop(); // we don't need the address
    }

    // output functions

    /// (emit) ( c -- ) takes a character from the stack and prints it.
    ///     (emit) will output any ASCII value (mod 128).
    ///
    pub fn f_emit_p(&mut self) {
        if self.kernel.stack_check(1, "(emit)") {
            let c = self.kernel.pop() % 128;
            print!("{}", c as u8 as char);
        }
    }

    /// flush ( -- ) Push any characters in Rust's output buffer out.
    ///     By default printed characters are buffered until a newline.
    ///     This forces them out sooner
    ///
    pub fn f_flush(&mut self) {
        io::stdout().flush().unwrap();
    }

    /// .s ( -- ) prints a copy of the computation stack
    ///
    pub fn f_dot_s(&mut self) {
        // print!("State: {}", self.kernel.get(self.state_ptr));
        print!("[ ");
        for i in (self.kernel.stack_ptr..STACK_START).rev() {
            print!("{} ", self.kernel.get(i));
        }
        print!("] ");
        io::stdout().flush().unwrap();
    }

    /// include-file (s -- T | F ) Pushes a new reader, pointing to the file named at s, calling ABORT if unsuccessful
    ///     The intent is that the standard loop will continue, now reading lines from the file
    ///     At the end of the file, the reader will be popped off the stack.
    ///     This allows for nested file reads.
    ///
    pub fn f_include_file(&mut self) {
        if self.kernel.stack_check(1, "include-file") {
            let addr = self.kernel.pop() as usize;
            let file_name = self.kernel.string_get(addr);
            let mode = FILEMODE_RO; // Read-only mode for included files
            let handle = self.u_open_file( &file_name, mode as i64);
            match handle {
                Some(handle) => {
                    self.reader.push(handle);
                    self.kernel.push(TRUE);
                }
                None => {
                    self.kernel.push(FALSE);
                }
            }
        }
    }

    /// open-file ( s fam -- file-id ior ) Open the file named at s, length u, with file access mode fam.
    pub fn f_open_file(&mut self) {
        if self.kernel.stack_check(2, "open-file") {
            let mode = self.kernel.pop();
            let addr = self.kernel.pop() as usize;
            let name = self.kernel.string_get(addr);
            println!("open-file: name={}, mode={}", name, mode);
            let handle = self.u_open_file(&name, mode);
            match handle {
                Some(handle) => {
                    self.files.push(handle);
                    self.kernel.push(self.files.len() as i64 - 1); // Push the index as a file-id
                    self.kernel.push(0);                    // 0 means success in this case
                }
                None => {
                    self.kernel.push(0);
                    self.kernel.push(-1);        // Signals an error condition
                }
            }
        }

    }
    /// u_open-file  Open the named file with file access mode mode.
    ///    Returns a file handle and 0 if successful. 
    pub fn u_open_file(&mut self, name: &str, mode: i64) -> Option<FileHandle> {
        let full_path = std::fs::canonicalize(name);
        let mode = match mode {
            -1 => FileMode::RW,
             1 => FileMode::WO,
             _ => FileMode::RO
        };
        match full_path {
            Ok(full_path) => {
                let file_handle = FileHandle::new_file(Some(&full_path), Msg::new(), mode);
                match file_handle {
                    Some(fh) => {
                        // self.kernel.push(TRUE);
                        return Some(fh);
                    }
                    None => {
                        self.kernel.push(FALSE);
                        self.msg.error(
                            "open-file",
                            "Failed to create new reader",
                            None::<bool>,
                        );
                    }
                }
            }
            Err(error) => {
                self.kernel.push(FALSE);
                self.msg
                    .warning("open-file", error.to_string().as_str(), None::<bool>);
            }
        }
        None
   }

    ///  close-file ( file-id -- ior ) Close a file, returning the I/O status code.
    ///     In rust, we just need it to go out of scope, so delete it from the vector
    pub fn f_close_file(&mut self) {
        if self.kernel.stack_check(1, "close-file") {
            let file_id = self.kernel.pop() as usize;
            if file_id < self.files.len() { 
                self.files.remove(file_id);
                self.kernel.push(0);
            }
        }
    }

    /// read-line ( u file-id -- u flag ior ) Read up to u characters from a file.
    ///     Returns the number of characters read, a flag indicating success or failure, and an i/o result code.
    ///     Starts from FILE_POSITION, and updates FILE_POSITION on completion
    ///     Characters are read into TMP
    pub fn f_read_line(&mut self) {
        if self.kernel.stack_check(2, "read-line") {
            let file_id = self.kernel.pop() as usize;
            let _chars = self.kernel.pop() as usize;
            if file_id < self.files.len() {
                let mut result = String::new();
                match self.files[file_id].source {
                    FType::BReader(ref mut br) => {
                        match br.read_line(&mut result) {
                            Ok(r) => {
                                if r == 0 {
                                    // EOF
                                    self.kernel.push(0);
                                    self.kernel.push(FALSE);
                                    self.kernel.push(-1);
                                } else {
                                    let addr = self.kernel.get(self.tmp_ptr) as usize;
                                    self.kernel.string_save(&result, addr);
                                    self.kernel.push(r as i64);  // Chars read
                                    self.kernel.push(TRUE);
                                    self.kernel.push(0);
                                }
                            }
                            Err(e) => self.msg.error("read-line", e.to_string().as_str(), None::<bool>),
                        }
                    }
                    _ => {}//self.msg.error("read-line", "No source found", Some(&self.files[file_id].source)),
                }
            }
        }
    }

    ///  write-line ( s u file-id -- ior ) Write u characters from s to a file, returning an i/o result code.
    ///     Not intended to work with stdout
    pub fn f_write_line(&mut self) {
        if self.kernel.stack_check(3, "write-line") {
            let file_id = self.kernel.pop() as usize;
            let chars = self.kernel.pop() as usize;
            let addr = self.kernel.pop() as usize;
            if file_id < self.files.len() {
                let string = self.kernel.string_get(addr)[0..chars - 1].to_owned();
                // write the string to the file
                match self.files[file_id].source {
                    FType::File(ref mut f) => {
                        f.write_all(&string.as_bytes()).expect("Error writing to file");
                    }
                    _ => {}
                }
            }
        }
    }

    ///  file-size ( file-id -- u ior ) Returns the size in characters of the file, plus an i/o result code
    pub fn f_file_size(&mut self) {
        if self.kernel.stack_check(1, "file-size") {
            let file_id = self.kernel.pop() as usize;
            if file_id < self.files.len() {
                self.kernel.push(self.files[file_id].file_size() as i64);
            } else {
                self.msg.error("file-size", "No such file-id", Some(file_id));
            }
        }
    }

    /// file-position ( file-id -- u ior ) Returns the current file position and an i/o result
    pub fn f_file_position(&mut self) {
        if self.kernel.stack_check(1, "file-position") {
            let file_id = self.kernel.pop() as usize;
            if file_id < self.files.len() {
                self.kernel.push(self.files[file_id].file_position() as i64);
            } else {
                self.msg.error("file-position", "No such file-id", Some(file_id));
            }
        }

    }
    
}
