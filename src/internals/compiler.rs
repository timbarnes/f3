// Compiler and Interpreter
//
// The outer interpreter is plain Rust: parse_token, find_word, parse_number and eval work on
// Rust values. The Forth words of the same names (parse-to, find, number?, create, ...) are thin
// wrappers that move values between the Forth stack and these functions.

use crate::kernel::throw;
use crate::runtime::PendingDef;
use crate::runtime::{
    ForthRuntime, ADDRESS_MASK, ARRAY, BUILTIN_FLAG, COMPILE_ONLY_FLAG, CONSTANT, DEFINITION, EXIT,
    FALSE, IMMEDIATE_FLAG, LITERAL, TRUE, VARIABLE,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// parse_number converts a token to a number in the given base.
///     A prefix overrides the base: # decimal, $ hex, % binary. A leading - makes it negative.
///     Values up to 2^64-1 are accepted, and wrap to negative like any other cell.
///
pub fn parse_number(token: &[u8], base: i64) -> Option<i64> {
    let text = std::str::from_utf8(token).ok()?;
    let (radix, digits) = match token.first() {
        Some(b'#') => (10, &text[1..]),
        Some(b'$') => (16, &text[1..]),
        Some(b'%') => (2, &text[1..]),
        _ => (base, text),
    };
    if !(2..=36).contains(&radix) || digits.starts_with('+') {
        return None;
    }
    let radix = radix as u32;
    i64::from_str_radix(digits, radix)
        .ok()
        .or_else(|| u64::from_str_radix(digits, radix).ok().map(|n| n as i64))
}

impl ForthRuntime {
    /// quit is the main loop in Forth, reading from the input stream and dispatching for evaluation
    ///     quit also issues the prompt and checks for a shutdown (exit) condition
    ///     Any error raised while handling a line unwinds to here, and is handled by recover
    pub fn f_quit(&mut self) {
        self.run_lines(0);
    }

    /// run_lines reads and interprets lines until input drops back to `depth` readers,
    ///     or BYE is executed. Depth 0 means: until BYE (or the end of stdin).
    ///
    pub fn run_lines(&mut self, depth: usize) {
        while !self.should_exit() && self.reader.len() > depth {
            let console = self.reader.len() == 1; // prompt only for lines typed at the console
            let result = catch_unwind(AssertUnwindSafe(|| {
                self.f_query();
                self.f_eval(); // interpret the contents of the line
                if console {
                    if self.kernel.get(self.show_stack_ptr) != FALSE {
                        self.run_untraced(".s");
                    }
                    print!("ok ");
                }
                self.f_flush();
            }));
            if let Err(payload) = result {
                self.recover(payload);
            }
        }
    }

    /// run_untraced executes a word by name, if it is defined, with the stepper off.
    ///     Used to run Forth-defined helpers like .s from the engine (the prompt, the tracer).
    ///
    pub fn run_untraced(&mut self, name: &str) {
        if let Some(cfa) = self.find_word(name.as_bytes()) {
            let stepper = self.kernel.get(self.stepper_ptr);
            self.kernel.set(self.stepper_ptr, 0);
            self.kernel.push(cfa as i64);
            self.f_execute();
            self.kernel.set(self.stepper_ptr, stepper);
        }
    }

    /// EXECUTE ( xt -- ) runs the word whose execution token is on the stack:
    ///     a cfa, or a builtin index overlaid with BUILTIN_FLAG (as compiled into definitions)
    ///
    pub fn f_execute(&mut self) {
        self.kernel.stack_check(1, "execute");
        let xt = self.kernel.pop() as usize;
        if xt & BUILTIN_FLAG != 0 {
            return self.builtin(xt & ADDRESS_MASK);
        }
        let code = self.kernel.get(xt);
        match code {
            DEFINITION => self.i_definition(xt + 1),
            VARIABLE | ARRAY => self.kernel.push(xt as i64 + 1),
            CONSTANT => {
                let val = self.kernel.get(xt + 1);
                self.kernel.push(val);
            }
            _ if code as usize & BUILTIN_FLAG != 0 => self.builtin(code as usize & ADDRESS_MASK),
            _ => throw(format!("execute: {} is not an execution token", xt as i64)),
        }
    }

    /// EVAL ( -- ) Interprets the rest of the line in the Text Input Buffer (TIB):
    ///     each token is a word (executed, or compiled when STATE is true and the word isn't immediate),
    ///     or a number (pushed, or compiled as a literal). Anything else is an error.
    ///
    pub fn f_eval(&mut self) {
        loop {
            let (addr, len) = self.parse_token(b' ');
            if len == 0 {
                break; // end of line
            }
            let compiling = self.get_compile_mode();
            match self.find_word(self.kernel.string_slice(addr, len)) {
                Some(cfa) => {
                    let flags = self.kernel.get(cfa - 1) as usize;
                    if compiling && flags & IMMEDIATE_FLAG == 0 {
                        // builtins are compiled as their flagged index, other words as their cfa
                        let code = self.kernel.get(cfa);
                        let builtin = code as usize & BUILTIN_FLAG != 0;
                        self.emit_cell(if builtin { code } else { cfa as i64 });
                    } else {
                        if !compiling && flags & COMPILE_ONLY_FLAG != 0 {
                            throw(format!(
                                "{} is only valid inside a definition",
                                self.token(addr, len)
                            ));
                        }
                        self.kernel.push(cfa as i64);
                        self.f_execute();
                    }
                }
                None => {
                    let base = self.kernel.get(self.base_ptr);
                    match parse_number(self.kernel.string_slice(addr, len), base) {
                        Some(n) if compiling => {
                            self.emit_cell(LITERAL);
                            self.emit_cell(n);
                        }
                        Some(n) => self.kernel.push(n),
                        None => throw(format!("{} ?", self.token(addr, len))),
                    }
                }
            }
        }
    }

    /// token returns a token from string space as text, for messages
    fn token(&self, addr: usize, len: usize) -> String {
        String::from_utf8_lossy(self.kernel.string_slice(addr, len)).into_owned()
    }

    /// parse_token finds the next delim-delimited token in the TIB, skipping leading delimiters,
    ///     and advances >IN past it. Returns the token's string address and length;
    ///     the length is 0 at the end of the line.
    ///
    pub fn parse_token(&mut self, delim: u8) -> (usize, usize) {
        let tib = self.kernel.get(self.tib_ptr) as usize;
        let end = self.kernel.get(self.tib_size_ptr) as usize + 1; // >IN counts from 1: byte 0 is the count
        let mut i = self.kernel.get(self.tib_in_ptr) as usize;
        while i < end && self.kernel.byte_get(tib + i) == delim {
            i += 1;
        }
        let start = i;
        while i < end && self.kernel.byte_get(tib + i) != delim {
            i += 1;
        }
        self.kernel.set(self.tib_in_ptr, i as i64 + 1); // past the delimiter
        (tib + start, i.saturating_sub(start))
    }

    /// find_word searches the dictionary, newest first, and returns the cfa of the word with this name
    ///
    pub fn find_word(&self, name: &[u8]) -> Option<usize> {
        let mut link = (self.kernel.get(self.context_ptr) as usize).wrapping_sub(1);
        while link > 0 {
            // the name field is immediately after the link
            let nfa = self.kernel.get(link + 1) as usize & ADDRESS_MASK;
            if self.kernel.counted_bytes(nfa) == name {
                return Some(link + 2);
            }
            link = self.kernel.get(link) as usize;
        }
        None
    }

    /// FIND ( s -- cfa T | s F ) Search the dictionary for the counted string at s.
    ///     If not found, return the string address so NUMBER? can look at it
    ///
    pub fn f_find(&mut self) {
        self.kernel.stack_check(1, "find");
        let addr = self.kernel.pop() as usize;
        match self.find_word(self.kernel.counted_bytes(addr)) {
            Some(cfa) => {
                self.kernel.push(cfa as i64);
                self.kernel.push(TRUE);
            }
            None => {
                self.kernel.push(addr as i64);
                self.kernel.push(FALSE);
            }
        }
    }

    /// number? ( s -- n T | s F ) converts the counted string at s to a number, using BASE
    ///
    pub fn f_number_q(&mut self) {
        self.kernel.stack_check(1, "number?");
        let addr = self.kernel.pop();
        let base = self.kernel.get(self.base_ptr);
        match parse_number(self.kernel.counted_bytes(addr as usize), base) {
            Some(n) => {
                self.kernel.push(n);
                self.kernel.push(TRUE);
            }
            None => {
                self.kernel.push(addr);
                self.kernel.push(FALSE);
            }
        }
    }

    /// f_comma ( n -- ) compile a value into a definition
    ///     Takes the top of the stack and writes it to the next free location in data space
    pub fn f_comma(&mut self) {
        let val = self.kernel.pop();
        self.emit_cell(val);
    }

    /// (') (TICK) <name> ( -- cfa ) Searches for a (postfix) word, and places its cfa on the stack.
    ///     It is an error if the word is not found.
    ///
    pub fn f_tick_p(&mut self) {
        let (addr, len) = self.parse_token(b' ');
        match self.find_word(self.kernel.string_slice(addr, len)) {
            Some(cfa) => self.kernel.push(cfa as i64),
            None if len == 0 => throw("' : name expected"),
            None => throw(format!("{} ?", self.token(addr, len))),
        }
    }

    /// PARSE-TO ( b d -- b u ) Get a d-delimited token from the TIB, and copy it as a counted string to b
    ///     u is the token's length; 0 means the line is used up
    ///
    pub fn f_parse_to(&mut self) {
        self.kernel.stack_check(2, "parse-to");
        let delim = self.kernel.pop() as u8;
        let dest = self.kernel.pop() as usize;
        let (addr, len) = self.parse_token(delim);
        self.kernel.string_copy(addr, dest, len, false);
        self.kernel.push(dest as i64);
        self.kernel.push(len as i64);
    }

    /// : (colon) starts the creation of a compiled function
    ///     It sets compile mode, creates the name header, and writes the constant that determines how
    ///     the word is to be processed at run time.
    ///
    pub fn f_colon(&mut self) {
        self.set_compile_mode(true);
        self.f_create(); // gets the name and makes a new dictionary entry
        self.emit_cell(DEFINITION);
    }

    /// ; terminates a definition, writing the cfa for EXIT, and resetting to interpret mode
    ///     It has to write the exit code word, and add a back pointer
    ///     It also has to update HERE and CONTEXT.
    ///     Finally it switches out of compile mode
    ///
    pub fn f_semicolon(&mut self) {
        self.emit_cell(EXIT);
        let back = self.kernel.get(self.last_ptr); // get the current LAST pointer
        self.emit_cell(back - 1); // write the back pointer
        self.kernel.set(self.context_ptr, back); // adds the new definition to FIND
        self.set_compile_mode(false);
    }

    /// CREATE <name> ( -- ) makes a new dictionary entry, using a postfix name
    ///     References HERE, and assumes back pointer is in place already.
    ///     Sets LAST and HERE; the word becomes visible (CONTEXT) when it is closed by ; or (close).
    ///
    pub fn f_create(&mut self) {
        let (addr, len) = self.parse_token(b' ');
        if len == 0 {
            throw("create: name expected");
        }
        let name = self.kernel.string_slice(addr, len).to_vec(); // copied: string_new writes string space
        if self.find_word(&name).is_some() {
            let msg = format!("{} redefined", self.token(addr, len));
            self.msg.warning("create", &msg, None::<bool>);
        }
        self.pending_def = Some(PendingDef {
            here: self.kernel.get(self.here_ptr),
            s_here: self.kernel.get(self.kernel.get_string_ptr()),
            last: self.kernel.get(self.last_ptr),
            context: self.kernel.get(self.context_ptr),
        });
        let nfa = self.kernel.string_new(&name);
        let here = self.kernel.get(self.here_ptr);
        self.kernel.set(self.last_ptr, here); // save the last pointer
        self.emit_cell(nfa as i64); // the name field
    }
}

#[cfg(test)]
mod tests {
    use super::parse_number;

    #[test]
    fn test_parse_number() {
        assert_eq!(parse_number(b"42", 10), Some(42));
        assert_eq!(parse_number(b"-42", 10), Some(-42));
        assert_eq!(parse_number(b"ff", 16), Some(255));
        assert_eq!(parse_number(b"$ff", 10), Some(255));
        assert_eq!(parse_number(b"#10", 16), Some(10));
        assert_eq!(parse_number(b"%101", 10), Some(5));
        assert_eq!(parse_number(b"$-10", 10), Some(-16));
        assert_eq!(parse_number(b"$ffffffffffffffff", 10), Some(-1));
        assert_eq!(parse_number(b"ff", 10), None);
        assert_eq!(parse_number(b"+5", 10), None);
        assert_eq!(parse_number(b"", 10), None);
        assert_eq!(parse_number(b"$", 10), None);
    }
}
