// Compiler and Interpreter

use crate::kernel::throw;
use crate::runtime::PendingDef;
use crate::runtime::{
    ForthRuntime, ABORT, ADDRESS_MASK, ARRAY, BRANCH, BRANCH0, BREAK, CONSTANT, DEFINITION, DO,
    EXEC, EXIT, FOR, LEAVE, LITERAL, LOOP, NEXT, PLOOP, QDO, STRLIT, VARIABLE,
};
use crate::runtime::{BUILTIN_FLAG, COMPILE_ONLY_FLAG, FALSE, IMMEDIATE_FLAG, TRUE};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// parse_number converts a token to a number in the given base.
///     A prefix overrides the base: # decimal, $ hex, % binary. A leading - makes it negative.
///     Values up to 2^64-1 are accepted, and wrap to negative like any other cell.
///
pub fn parse_number(text: &str, base: i64) -> Option<i64> {
    let (radix, digits) = match text.as_bytes().first() {
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
    /// set_flag sets a flag (IMMEDIATE_FLAG, COMPILE_ONLY_FLAG) on the most recently defined word
    ///     Context pointer links to the most recent name field
    ///
    fn set_flag(&mut self, flag: usize) {
        let addr = self.kernel.get(self.context_ptr) as usize;
        let name = self.kernel.get(addr) as usize | flag;
        self.kernel.set(addr, name as i64);
    }

    /// immediate ( -- ) sets the immediate flag on the most recently defined word
    ///
    pub fn f_immediate(&mut self) {
        self.set_flag(IMMEDIATE_FLAG);
    }

    /// compile-only ( -- ) marks the most recently defined word as an error outside a definition
    ///
    pub fn f_compile_only(&mut self) {
        self.set_flag(COMPILE_ONLY_FLAG);
    }

    /// immediate? ( cfa -- flag ) checks if the word at cfa is immediate
    ///
    pub fn f_immediate_q(&mut self) {
        if self.kernel.stack_check(1, "immediate?") {
            let cfa = self.kernel.pop() as usize;

            // Clear the BUILTIN_MASK to get the actual address for indexing
            let clean_cfa = cfa & ADDRESS_MASK;

            // Get the NFA (Name Field Address) which is always cfa - 1
            let name_ptr = self.kernel.get(clean_cfa.wrapping_sub(1)) as usize;
            let immed = name_ptr & IMMEDIATE_FLAG;

            let result = if immed == 0 { FALSE } else { TRUE };
            self.kernel.push(result);
        }
    }

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
                    if self.show_stack {
                        self.f_dot_s();
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

    /// EVAL ( -- ) Interprets a line of tokens from the Text Input Buffer (TIB
    pub fn f_eval(&mut self) {
        loop {
            let val = self.kernel.get(self.pad_ptr);
            self.kernel.push(val);
            self.kernel.push(' ' as i64);
            self.f_parse_to(); //  ( -- b u ) get a token
            let len = self.kernel.pop();
            if len == FALSE {
                // Forth FALSE is zero, which here indicates end of line
                self.kernel.pop(); // lose the text pointer from parse-to
                break;
            } else {
                // we have a token
                if self.get_compile_mode() {
                    self.f_d_compile();
                } else {
                    self.f_d_interpret();
                }
            }
        }
    }

    /// $COMPILE ( s -- ) compiles a token whose string address is on the stack
    ///            If not a word, try to convert to a number
    ///            If not a number, ABORT.
    pub fn f_d_compile(&mut self) {
        if self.kernel.stack_check(1, "$compile") {
            self.f_find();
            if self.kernel.pop() == TRUE {
                let cfa = self.kernel.top();
                // we found a word
                // if it's immediate, we need to execute it; otherwise continue compiling
                self.f_immediate_q();
                if self.kernel.pop() == TRUE {
                    // call the interpreter for this word
                    let val = self.kernel.get(self.pad_ptr);
                    self.kernel.push(val);
                    self.f_d_interpret();
                } else {
                    // check if it's a builtin, and compile appropriately
                    let indirect = self.kernel.get(cfa as usize) as usize;
                    if indirect & BUILTIN_FLAG != 0 {
                        self.kernel.push(indirect as i64);
                    } else {
                        self.kernel.push(cfa);
                    }
                    self.f_comma(); // uses the cfa on the stack
                }
            } else {
                self.f_number_q();
                if self.kernel.pop() == TRUE {
                    self.f_literal(); // compile the literal
                } else {
                    self.kernel.pop(); // lose the failed number
                    let addr = self.kernel.get(self.pad_ptr) as usize;
                    throw(format!("{} ?", self.kernel.string_get(addr)));
                }
            }
        }
    }

    /// $INTERPRET ( s -- ) executes a token whose string address is on the stack.
    ///            If not a word, try to convert to a number
    ///            If not a number, ABORT.
    ///
    pub fn f_d_interpret(&mut self) {
        if self.kernel.stack_check(1, "$interpret") {
            let token_addr = self.kernel.top();
            self.f_find(); // (s -- nfa, cfa, T | s F )
            if self.kernel.pop() == TRUE {
                // we have a definition
                let cfa = self.kernel.top() as usize;
                let name = self.kernel.get(cfa - 1) as usize;
                if name & COMPILE_ONLY_FLAG != 0 && !self.get_compile_mode() {
                    let name = self.kernel.string_get(name & ADDRESS_MASK);
                    throw(format!("{name} is only valid inside a definition"));
                }
                self.f_execute();
            } else {
                // try number?
                self.f_number_q(); // ( s -- n T | a F )
                if self.kernel.pop() == TRUE {
                    // leave the converted number on the stack
                } else {
                    self.kernel.pop(); // lose the failed number
                    throw(format!("{} ?", self.kernel.string_get(token_addr as usize)));
                }
            }
        }
    }

    /// FIND (s -- cfa T | s F ) Search the dictionary for the token indexed through s.
    ///     If not found, return the string address so NUMBER? can look at it
    ///
    pub fn f_find(&mut self) {
        if self.kernel.stack_check(1, "find") {
            let mut result = false;
            let source_addr = self.kernel.pop() as usize;
            let mut link = (self.kernel.get(self.context_ptr) as usize).wrapping_sub(1);
            // link = self.data[link] as usize; // go back to the beginning of the top word
            while link > 0 {
                // name field is immediately after the link
                let nfa_val = self.kernel.get(link + 1);
                let str_addr = nfa_val as usize & ADDRESS_MASK;
                if self.kernel.string_equal(source_addr, str_addr) {
                    result = true;
                    break;
                }
                link = self.kernel.get(link) as usize;
            }
            if result {
                self.kernel.push(link as i64 + 2);
                self.kernel.push(TRUE);
            } else {
                self.kernel.push(source_addr as i64);
                self.kernel.push(FALSE);
            }
        } else {
            // stack error
        }
    }

    /// number? ( s -- n T | a F ) tests a string to see if it's a number;
    /// leaves n and flag on the stack: true if number is ok.
    ///
    pub fn f_number_q(&mut self) {
        let buf_addr = self.kernel.pop();
        let text = self.kernel.string_get(buf_addr as usize);
        let base = self.kernel.get(self.base_ptr);
        if let Some(n) = parse_number(&text, base) {
            self.kernel.push(n);
            self.kernel.push(TRUE);
        } else {
            self.kernel.push(buf_addr);
            self.kernel.push(FALSE);
        }
    }

    /// f_comma ( n -- ) compile a value into a definition
    ///     Takes the top of the stack and writes it to the next free location in data space
    pub fn f_comma(&mut self) {
        let val = self.kernel.pop();
        self.emit_cell(val);
    }

    /// f_literal ( n -- ) compile a literal number with it's inner interpreter code pointer
    ///     Numbers are represented in compiled functions with two words: the LITERAL constant, and the value
    ///     The value comes from the stack.
    ///
    pub fn f_literal(&mut self) {
        self.emit_cell(LITERAL);
        self.f_comma(); // write the value passed in from the stack
    }

    /// UNIQUE? (s -- s )
    ///     Checks the dictionary to see if the word pointed to is defined.
    ///     No stack impact - it's just offering a warning.
    pub fn f_q_unique(&mut self) {
        self.f_dup();
        self.f_find();
        let result = self.kernel.pop();
        let s_addr = (self.kernel.pop() as usize).wrapping_sub(1) & ADDRESS_MASK;
        if result == TRUE {
            self.msg.warning(
                "unique?",
                "Overwriting existing definition name at ",
                Some(self.kernel.get(s_addr)),
            );
        }
    }

    /// (') (TICK) <name> ( -- cfa ) Searches for a (postfix) word, and places its cfa on the stack.
    ///     It is an error if the word is not found.
    ///
    pub fn f_tick_p(&mut self) {
        let val = self.kernel.get(self.pad_ptr);
        self.kernel.push(val);
        self.kernel.push(' ' as i64);
        self.f_parse_to(); // ( -- b u )
        self.kernel.pop(); // don't need the delim
        self.f_find(); // look for the token
        if self.kernel.pop() == FALSE {
            let addr = self.kernel.pop() as usize;
            throw(format!("{} ?", self.kernel.string_get(addr)));
        }
    }

    /// (parse) - ( b u c -- b u delta )
    ///     Find a c-delimited token in the string buffer at b, buffer len u.
    ///     This is the heart of the parsing engine.
    ///     Return the pointer to the buffer, the length of the token,
    ///     and the offset from the start of the buffer to the start of the token.
    ///
    pub fn f_parse_p(&mut self) {
        if self.kernel.stack_check(3, "(parse)") {
            let delim = self.kernel.pop() as u8;
            let buf_len = self.kernel.pop();
            let in_p = self.kernel.pop();
            // println!("f_parse_p: in_p = {in_p}, buf_len = {buf_len}");
            if buf_len > 0 {
                // get a read-only &slice from kernel.strings, starting at in_p
                // and ending at in_p + buf_len
                let buffer = self
                    .kernel
                    .string_slice(in_p as usize, buf_len as usize + 1);
                // println!("f_parse_p: buffer = {:?}", buffer);
                // traverse the string, dropping leading delim characters
                // in_p points *into* a string, so no count field
                let end = buf_len as usize;
                let mut i = 0;
                let mut j;
                while i < end && buffer[i] == delim {
                    i += 1;
                }
                j = i;
                while j < end && buffer[j] != delim {
                    j += 1;
                }
                self.kernel.push(in_p);
                self.kernel.push((j - i) as i64); // length of the token
                self.kernel.push(i as i64);
            } else {
                // nothing left to read
                self.kernel.push(in_p);
                self.kernel.push(0);
                self.kernel.push(0);
            }
        }
    }

    /// PARSE-TO ( b d -- b u ) Get a d-delimited token from TIB, and return counted string in string buffer at b
    /// need to check if TIB is empty
    /// if delimiter = 1, get the rest of the TIB
    /// Update >IN as required, and set #TIB to zero if the line has been consumed
    /// The main text parser that reads tokens from the TIB for interactive operations
    ///
    pub fn f_parse_to(&mut self) {
        if self.kernel.stack_check(2, "parse") {
            let delim: i64 = self.kernel.pop();
            let dest = self.kernel.pop();
            // println!("f_parse_to: delim = {delim}, dest = {dest}");
            if delim == 1 {
                self.kernel.set(self.tib_in_ptr, 1);
                self.kernel.set(self.tib_size_ptr, 0);
                let val = self.kernel.get(self.tib_ptr);
                self.kernel.push(val);
                self.kernel.push(0); // indicates nothing found, TIB is empty
            } else {
                let addr = self.kernel.get(self.tib_ptr) + self.kernel.get(self.tib_in_ptr);
                self.kernel.push(addr); // Starting address in the string
                let val = self.kernel.get(self.tib_size_ptr) - self.kernel.get(self.tib_in_ptr) + 1;
                self.kernel.push(val); // bytes available
                self.kernel.push(delim);
                self.f_parse_p();
                // check length, and copy to PAD if a token was found
                let delta = self.kernel.pop();
                let length = self.kernel.pop();
                let addr = self.kernel.pop();
                if length > 0 {
                    // copy to pad
                    self.kernel.string_copy(
                        (addr + delta) as usize,
                        dest as usize,
                        length as usize,
                        false,
                    );
                }
                let val = self.kernel.get(self.tib_in_ptr) + delta + length + 1;
                self.kernel.set(self.tib_in_ptr, val);
                self.kernel.push(dest);
                self.kernel.push(length);
            }
        }
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
        // println!("; (semicolon) - end of definition");
        self.emit_cell(EXIT);
        let back = self.kernel.get(self.last_ptr); // get the current LAST pointer
        self.emit_cell(back - 1); // write the back pointer
        self.kernel.set(self.context_ptr, back); // adds the new definition to FIND
        self.set_compile_mode(false);
    }

    /// CREATE <name> ( -- ) makes a new dictionary entry, using a postfix name
    ///     References HERE, and assumes back pointer is in place already
    ///     create updates the three definition-related pointers: HERE, CONTEXT and LAST
    pub fn f_create(&mut self) {
        self.pending_def = Some(PendingDef {
            here: self.kernel.get(self.here_ptr),
            s_here: self.kernel.get(self.kernel.get_string_ptr()),
            last: self.kernel.get(self.last_ptr),
            context: self.kernel.get(self.context_ptr),
        });
        let pad = self.kernel.get(self.pad_ptr);
        self.kernel.push(pad);
        self.kernel.push(' ' as i64);
        self.f_parse_to(); // get the word's name
        self.kernel.pop(); // throw away the length, keep the text pointer
        self.f_q_unique(); // issue a warning if it's already defined
        let str_addr = self.kernel.get(self.pad_ptr) as usize; // get the string address
        let length = self.kernel.string_length(str_addr) as u8 as i64;
        self.kernel.push(length);
        let val = self.kernel.get(self.kernel.get_string_ptr());
        self.kernel.push(val);
        self.f_smove(); // make a new string with the name from PAD
        let val = self.kernel.pop(); // get the string address
        self.kernel.delta(self.kernel.get_string_ptr(), length + 1); // update the free string pointer
        let here = self.kernel.get(self.here_ptr);
        self.kernel.set(self.last_ptr, here); // save the last pointer
        self.emit_cell(val); // the string header
    }

    /// see <name> ( -- ) prints the definition of a word
    ///     Taking a postfix word name (normally used interactively), this is the Forth decompiler.
    ///
    pub fn f_see(&mut self) {
        self.f_tick_p(); // finds the address of the word, or throws
        let cfa = self.kernel.pop();
        {
            let mut nfa = self.kernel.get(cfa as usize - 1) as usize;
            let is_immed = nfa & IMMEDIATE_FLAG;
            let xt = self.kernel.get(cfa as usize) as usize;
            let is_builtin = xt & BUILTIN_FLAG;
            if is_builtin != 0 {
                println!(
                    "Builtin: {}",
                    self.kernel.get_builtin(xt & !BUILTIN_FLAG).doc
                );
            } else {
                // It's a definition of some kind
                nfa &= ADDRESS_MASK; // get rid of any special bits
                match xt as i64 {
                    DEFINITION => {
                        print!(": ");
                        let name = self.kernel.string_get(nfa);
                        print!("{name} ");
                        let mut index = cfa as usize + 1; // skip the inner interpreter
                        loop {
                            let xt = self.kernel.get(index);
                            // print!("(xt is {}) ", xt);
                            match xt {
                                LITERAL => {
                                    print!("{} ", self.kernel.get(index + 1));
                                    index += 1;
                                }
                                ARRAY => {
                                    print!("ARRAY pointer ");
                                    index += 1;
                                }
                                STRLIT => {
                                    // println!("Found STRLIT {}:{}", xt, self.kernel.get(index as usize + 1));
                                    let s_addr = self.kernel.get(index + 1) as usize;
                                    print!("\" {}\" ", self.kernel.string_get(s_addr));
                                    index += 1;
                                }
                                BRANCH => {
                                    print!("branch:{} ", self.kernel.get(index + 1));
                                    index += 1;
                                }
                                BRANCH0 => {
                                    print!("branch0:{} ", self.kernel.get(index + 1));
                                    index += 1;
                                }
                                DO | QDO | LOOP | PLOOP | FOR | NEXT => {
                                    let name = match xt {
                                        DO => "do",
                                        QDO => "?do",
                                        LOOP => "loop",
                                        PLOOP => "+loop",
                                        FOR => "for",
                                        _ => "next",
                                    };
                                    print!("{name}:{} ", self.kernel.get(index + 1));
                                    index += 1;
                                }
                                LEAVE => print!("leave "),
                                ABORT => print!("abort "),
                                BREAK => print!("exit "),
                                EXIT => {
                                    print!("; ");
                                    if is_immed != 0 {
                                        println!("immediate");
                                    } else {
                                        println!();
                                    }
                                    break;
                                }
                                EXEC => print!("exec "),
                                _ => {
                                    // it's a definition or a builtin
                                    let mut cfa = self.kernel.get(index) as usize;
                                    let mut mask = cfa & BUILTIN_FLAG;
                                    if mask == 0 {
                                        let addr = self.kernel.get(index) as usize - 1;
                                        let word = ADDRESS_MASK & self.kernel.get(addr) as usize; // nfa address
                                        let name = self.kernel.string_get(word);
                                        print!("{name} ");
                                    } else {
                                        mask = !BUILTIN_FLAG;
                                        cfa &= mask;
                                        let name = &self.kernel.get_builtin(cfa).name;
                                        print!("{name} ");
                                    }
                                }
                            }
                            self.f_flush();
                            index += 1;
                        }
                    }
                    CONSTANT => {
                        println!(
                            "Constant: {} = {}",
                            self.kernel.string_get(nfa),
                            self.kernel.get(cfa as usize + 1),
                        );
                    }
                    VARIABLE => {
                        let addr = self.kernel.get(cfa as usize - 1) as usize;
                        println!(
                            "Variable: {} = {}",
                            self.kernel.string_get(addr),
                            self.kernel.get(cfa as usize + 1),
                        )
                    }
                    _ => self.msg.error("see", "Unrecognized type", None::<bool>),
                }
            }
        }
    }

    /*  fn f_d_pack(&mut self) {
        // pack the string in PAD and place it in the dictionary for a new word
        let data = self.f_string_at(addr);
        let packed = self.pack_string(&data);
        for c in packed {
            let here = self.data[self.here_ptr];
            self.data[]
        }
    }
    */

    // Execute a Forth word by name from Rust, with the same semantics as Forth's run
    // pub fn run_forth_word(&mut self, name: &str) {
    //     let tmp_addr = self.kernel.get(self.tmp_ptr) as usize;
    //     self.kernel.string_save(name, tmp_addr);
    //     self.kernel.push(tmp_addr as i64);
    //     self.f_find(); // ( s -- cfa T | s F )
    //     if self.kernel.pop() == TRUE {
    //         // Found: cfa is on top of stack
    //         self.f_execute();
    //     } else {
    //         // Not found: drop and abort
    //         self.kernel.pop(); // drop the string address
    //         self.f_abort();
    //     }
    // }
}

#[cfg(test)]
mod tests {
    // use super::*;

    //     #[test]
    //     fn test_run_forth_word_dup_and_mul() {
    //         let mut rt = ForthRuntime::new();
    //         rt.cold_start(); // Initialize the Forth system and builtins
    //         rt.kernel.push(7);
    //         rt.run_forth_word("dup");
    //         rt.run_forth_word("*");
    //         let result = rt.kernel.pop();
    //         assert_eq!(result, 49);
    //     }
}
