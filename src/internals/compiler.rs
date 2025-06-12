// Compiler and Interpreter

use crate::kernel::{
    ABORT, ADDRESS_MASK, BRANCH, BRANCH0, BUILTIN, BUILTIN_MASK, CONSTANT, DEFINITION, EXIT, FALSE,
    EXEC, IMMEDIATE_MASK, LITERAL, BREAK, STACK_START, STRLIT, TF, TRUE, VARIABLE,
};
use crate::internals::general::u_is_integer;

macro_rules! stack_ok {
    ($self:ident, $n: expr, $caller: expr) => {
        if $self.stack_ptr <= STACK_START - $n {
            // $self.f_dot_s();
            true
        } else {
            $self.msg.error($caller, "Stack underflow", None::<bool>);
            $self.f_abort();
            false
        }
    };
}
macro_rules! pop {
    ($self:ident) => {{
        let r = $self.heap[$self.stack_ptr];
        //$self.data[$self.stack_ptr] = 999999;
        $self.stack_ptr += 1;
        r
    }};
}
macro_rules! top {
    ($self:ident) => {{
        $self.heap[$self.stack_ptr]
    }};
}
macro_rules! push {
    ($self:ident, $val:expr) => {
        $self.stack_ptr -= 1;
        $self.heap[$self.stack_ptr] = $val;
    };
}

impl TF {
    /// immediate ( -- ) sets the immediate flag on the most recently defined word
    ///     Context pointer links to the most recent name field
    ///
    pub fn f_immediate(&mut self) {
        let mut str_addr = self.heap[self.heap[self.context_ptr] as usize] as usize;
        str_addr |= IMMEDIATE_MASK;
        self.heap[self.heap[self.context_ptr] as usize] = str_addr as i64;
    }

    /// immediate? ( cfa -- T | F ) Determines if a word is immediate or not
    ///
    pub fn f_immediate_q(&mut self) {
        if stack_ok!(self, 1, "immediate?") {
            let cfa = pop!(self) as usize;
            let name_ptr = self.heap[cfa - 1] as usize;
            let immed = name_ptr & IMMEDIATE_MASK;
            let result = if immed == 0 { FALSE } else { TRUE };
            push!(self, result);
        }
    }

    /// quit is the main loop in Forth, reading from the input stream and dispatching for evaluation
    ///     quit also issues the prompt and checks for a shutdown (exit) condition
    pub fn f_quit(&mut self) {
        self.f_abort();
        loop {
            if self.should_exit() {
                break;
            } else {
                self.set_abort_flag(false);
                self.f_query();
                self.f_eval(); // interpret the contents of the line
                if self.reader.len() == 1 {
                    if self.show_stack {
                        self.f_dot_s();
                    }
                    print!(" ok ");
                }
                self.f_flush();
            }
        }
    }

    /// EXECUTE ( cfa -- ) interpret a word with addr on the stack
    /// stack value is the address of an inner interpreter
    pub fn f_execute(&mut self) {
        if stack_ok!(self, 1, "execute") {
            // call the appropriate inner interpreter
            let xt = pop!(self);
            push!(self, xt + 1);
            match self.heap[xt as usize] {
                BUILTIN    => self.msg.error("f_execute", "BUILTIN found", Some(xt)), //self.i_builtin(),
                VARIABLE   => self.i_variable(),
                CONSTANT   => self.i_constant(),
                LITERAL    => self.i_literal(),
                STRLIT     => self.i_strlit(),
                DEFINITION => self.i_definition(),
                BRANCH     => self.i_branch(),
                BRANCH0    => self.i_branch0(),
                ABORT      => self.i_abort(),
                EXIT       => self.i_exit(),
                BREAK      => self.i_exit(),
                _ => {
                    pop!(self);
                    let cfa = self.heap[xt as usize] as usize & ADDRESS_MASK;
                    push!(self, cfa as i64);
                    self.i_builtin();
                }
            }
        }
    }

    /// EVAL ( -- ) Interprets a line of tokens from the Text Input Buffer (TIB
    pub fn f_eval(&mut self) {
        loop {
            push!(self, self.heap[self.pad_ptr]);
            push!(self, ' ' as i64);
            self.f_parse_to(); //  ( -- b u ) get a token
            let len = pop!(self);
            if len == FALSE {
                // Forth FALSE is zero, which here indicates end of line
                pop!(self); // lose the text pointer from parse-to
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
        if stack_ok!(self, 1, "$compile") {
            self.f_find();
            if pop!(self) == TRUE {
                let cfa = top!(self);
                // we found a word
                // if it's immediate, we need to execute it; otherwise continue compiling
                self.f_immediate_q();
                if pop!(self) == TRUE {
                    // call the interpreter for this word
                    push!(self, self.heap[self.pad_ptr] as i64);
                    self.f_d_interpret();
                } else {
                    // check if it's a builtin, and compile appropriately
                    let indirect = self.heap[cfa as usize] as usize;
                    if indirect & BUILTIN_MASK != 0 {
                        push!(self, indirect as i64);
                    } else {
                        push!(self, cfa);
                    }
                    self.f_comma(); // uses the cfa on the stack
                }
            } else {
                self.f_number_q();
                if pop!(self) == TRUE {
                    self.f_literal(); // compile the literal
                } else {
                    pop!(self); // lose the failed number
                    let word = &self.u_get_string(self.heap[self.pad_ptr] as usize);
                    self.msg
                        .warning("$interpret", "token not recognized", Some(word));
                    self.f_abort();
                }
            }
        }
    }

    /// $INTERPRET ( s -- ) executes a token whose string address is on the stack.
    ///            If not a word, try to convert to a number
    ///            If not a number, ABORT.
    ///
    pub fn f_d_interpret(&mut self) {
        if stack_ok!(self, 1, "$interpret") {
            let token_addr = top!(self);
            self.f_find(); // (s -- nfa, cfa, T | s F )
            if pop!(self) == TRUE {
                // we have a definition
                self.f_execute();
            } else {
                // try number?
                self.f_number_q(); // ( s -- n T | a F )
                if pop!(self) == TRUE {
                    // leave the converted number on the stack
                } else {
                    pop!(self); // lose the failed number
                    let word = &self.u_get_string(token_addr as usize);
                    self.msg
                        .warning("$interpret", "token not recognized", Some(word));
                }
            }
        }
    }

    /// FIND (s -- cfa T | s F ) Search the dictionary for the token indexed through s.
    ///     If not found, return the string address so NUMBER? can look at it
    ///
    pub fn f_find(&mut self) {
        if stack_ok!(self, 1, "find") {
            let mut result = false;
            let source_addr = pop!(self) as usize;
            let mut link = self.heap[self.context_ptr] as usize - 1;
            // link = self.data[link] as usize; // go back to the beginning of the top word
            while link > 0 {
                // name field is immediately after the link
                let nfa_val = self.heap[link + 1];
                let str_addr = nfa_val as usize & ADDRESS_MASK;
                if self.strings[str_addr] as u8 == self.strings[source_addr] as u8 {
                    if self.u_str_equal(source_addr, str_addr as usize) {
                        result = true;
                        break;
                    }
                }
                link = self.heap[link] as usize;
            }
            if result {
                push!(self, link as i64 + 2);
                push!(self, TRUE);
            } else {
                push!(self, source_addr as i64);
                push!(self, FALSE);
            }
        } else {
            // stack error
        }
    }

    /// number? ( s -- n T | a F ) tests a string to see if it's a number;
    /// leaves n and flag on the stack: true if number is ok.
    ///
    pub fn f_number_q(&mut self) {
        let buf_addr = pop!(self);
        let numtext = self.u_get_string(buf_addr as usize);
        if u_is_integer(&numtext.as_str()) {
            let result = numtext.parse().unwrap();
            push!(self, result);
            push!(self, TRUE);
        } else {
            push!(self, buf_addr);
            push!(self, FALSE);
        }
    }

    /// f_comma ( n -- ) compile a value into a definition
    ///     Takes the top of the stack and writes it to the next free location in data space
    pub fn f_comma(&mut self) {
        self.heap[self.heap[self.here_ptr] as usize] = pop!(self);
        self.heap[self.here_ptr] += 1;
    }

    /// f_literal ( n -- ) compile a literal number with it's inner interpreter code pointer
    ///     Numbers are represented in compiled functions with two words: the LITERAL constant, and the value
    ///     The value comes from the stack.
    ///
    pub fn f_literal(&mut self) {
        push!(self, LITERAL);
        self.f_comma();
        self.f_comma();
    }

    /// UNIQUE? (s -- s )
    ///     Checks the dictionary to see if the word pointed to is defined.
    ///     No stack impact - it's just offering a warning.
    pub fn f_q_unique(&mut self) {
        self.f_dup();
        self.f_find();
        let result = pop!(self);
        pop!(self);
        if result == TRUE {
            self.msg
                .warning("unique?", "Overwriting existing definition", None::<bool>);
        }
    }

    /// (') (TICK) <name> ( -- a | FALSE ) Searches for a word, places cfa on stack if found; otherwise FALSE
    ///     Looks for a (postfix) word in the dictionary
    ///     places it's execution token / address on the stack
    ///     Pushes 0 if not found
    ///
    pub fn f_tick_p(&mut self) {
        push!(self, self.heap[self.pad_ptr]);
        push!(self, ' ' as i64);
        self.f_parse_to(); // ( -- b u )
        pop!(self); // don't need the delim
        self.f_find(); // look for the token
        if pop!(self) == FALSE {
            // write an error message
            let mut msg = self.u_get_string(self.heap[self.pad_ptr] as usize);
            msg = format!("Word not found: {} ", msg);
            self.u_set_string(self.heap[self.pad_ptr] as usize, &msg);
            pop!(self);
            push!(self, FALSE);
        }
        // pop!(self);
    }

    /// (parse) - ( b u c -- b u delta )
    ///     Find a c-delimited token in the string buffer at b, buffer len u.
    ///     This is the heart of the parsing engine.
    ///     Return the pointer to the buffer, the length of the token,
    ///     and the offset from the start of the buffer to the start of the token.
    ///
    pub fn f_parse_p(&mut self) {
        if stack_ok!(self, 3, "(parse)") {
            let delim = pop!(self) as u8 as char;
            let buf_len = pop!(self);
            let in_p = pop!(self);
            // traverse the string, dropping leading delim characters
            // in_p points *into* a string, so no count field
            if buf_len > 0 {
                let start = in_p as usize;
                let end = start + buf_len as usize;
                let mut i = start as usize;
                let mut j;
                while self.strings[i] == delim && i < end {
                    i += 1;
                }
                j = i;
                while j < end && self.strings[j] != delim {
                    j += 1;
                }
                push!(self, in_p);
                push!(self, (j - i) as i64);
                push!(self, i as i64 - in_p);
            } else {
                // nothing left to read
                push!(self, in_p);
                push!(self, 0);
                push!(self, 0);
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
        if stack_ok!(self, 2, "parse") {
            let delim: i64 = pop!(self);
            let dest = pop!(self);
            if delim == 1 {
                self.heap[self.tib_in_ptr] = 1;
                self.heap[self.tib_size_ptr] = 0;
                push!(self, self.heap[self.tib_in_ptr]);
                push!(self, 0); // indicates nothing found, TIB is empty
                return;
            } else {
                push!(
                    // starting address in the string
                    self,
                    self.heap[self.tib_ptr] + self.heap[self.tib_in_ptr]
                );
                push!(
                    // bytes available (length of input string)
                    self,
                    self.heap[self.tib_size_ptr] - self.heap[self.tib_in_ptr] + 1
                );
                push!(self, delim);
                self.f_parse_p();
                // check length, and copy to PAD if a token was found
                let delta = pop!(self);
                let length = pop!(self);
                let addr = pop!(self);
                if length > 0 {
                    // copy to pad
                    self.u_str_copy(
                        (addr + delta) as usize,
                        dest as usize,
                        length as usize,
                        false,
                    );
                }
                self.heap[self.tib_in_ptr] += delta + length + 1;
                //pop!(self);
                push!(self, dest);
                push!(self, length);
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
        push!(self, DEFINITION);
        self.f_comma();
    }

    /// ; terminates a definition, writing the cfa for EXIT, and resetting to interpret mode
    ///     It has to write the exit code word, and add a back pointer
    ///     It also has to update HERE and CONTEXT.
    ///     Finally it switches out of compile mode
    ///
    pub fn f_semicolon(&mut self) {
        push!(self, EXIT);
        self.f_comma();
        self.heap[self.heap[self.here_ptr] as usize] = self.heap[self.last_ptr] - 1; // write the back pointer
        self.heap[self.here_ptr] += 1; // over EXIT and back pointer
        self.heap[self.context_ptr] = self.heap[self.last_ptr]; // adds the new definition to FIND
        self.set_compile_mode(false);
    }

    /// CREATE <name> ( -- ) makes a new dictionary entry, using a postfix name
    ///     References HERE, and assumes back pointer is in place already
    ///     create updates the three definition-related pointers: HERE, CONTEXT and LAST
    pub fn f_create(&mut self) {
        push!(self, self.heap[self.pad_ptr]);
        push!(self, ' ' as i64);
        self.f_parse_to(); // get the word's name
        pop!(self); // throw away the length, keep the text pointer
        self.f_q_unique(); // issue a warning if it's already defined
        let length = self.strings[self.heap[self.pad_ptr] as usize] as u8 as i64;
        push!(self, length);
        push!(self, self.heap[self.string_ptr]);
        self.f_smove(); // make a new string with the name from PAD
        self.heap[self.heap[self.here_ptr] as usize] = pop!(self); // the string header
        self.heap[self.string_ptr] += length + 1; // update the free string pointer
        self.heap[self.last_ptr] = self.heap[self.here_ptr];
        self.heap[self.here_ptr] += 1;
    }

    /*     /// variable <name> ( -- ) Creates a new variable in the dictionary
       ///     This is a good candidate for shifting to Forth
       ///     Variables use three words: a name pointer, the VARIABLE token, and the value
       ///
       pub fn f_variable(&mut self) {
           self.f_create(); // gets a name and makes a name field in the dictionary
           push!(self, VARIABLE);
           self.f_comma(); // ( n -- )
           push!(self, 0); // default initial value
           self.f_comma();
           self.data[self.data[self.here_ptr] as usize] = self.data[self.last_ptr] - 1; // write the back pointer
           self.data[self.here_ptr] += 1; // over EXIT and back pointer
           self.data[self.context_ptr] = self.data[self.last_ptr]; // adds the new definition to FIND
       }
    */
    /*     /// constant <name> ( n -- ) Creates and initializez a new constant in the dictionary
       ///     Very similar to variables, except that their value is not intended to be changed
       ///
       pub fn f_constant(&mut self) {
           if stack_ok!(self, 1, "constant") {
               self.f_create();
               push!(self, CONSTANT);
               self.f_comma();
               self.f_comma(); // write the value from the stack
               self.data[self.data[self.here_ptr] as usize] = self.data[self.last_ptr] - 1; // write the back pointer
               self.data[self.here_ptr] += 1; // over EXIT and back pointer
               self.data[self.context_ptr] = self.data[self.last_ptr]; // adds the new definition to FIND
           }
       }
    */
    /// f_pack_d ( source len dest -- dest ) builds a new counted string from an existing counted string.
    ///     Used by CREATE
    ///
    pub fn f_smove(&mut self) {
        let dest = pop!(self) as usize;
        let length = pop!(self) as usize;
        let source = pop!(self) as usize;
        // assuming both are counted, we begin with the count byte. Length should match the source count byte
        for i in 0..=length {
            self.strings[dest + i] = self.strings[source + i];
        }
        push!(self, dest as i64);
    }

    /// see <name> ( -- ) prints the definition of a word
    ///     Taking a postfix word name (normally used interactively), this is the Forth decompiler.
    ///
    pub fn f_see(&mut self) {
        self.f_tick_p(); // finds the address of the word
        let cfa = pop!(self);
        if cfa == FALSE {
            self.msg.warning("see", "Word not found", None::<bool>);
        } else {
            let mut nfa = self.heap[cfa as usize - 1] as usize;
            let is_immed = nfa & IMMEDIATE_MASK;
            let xt = self.heap[cfa as usize] as usize;
            let is_builtin = xt & BUILTIN_MASK;
            if is_builtin != 0 {
                println!(
                    "Builtin: {}",
                    self.builtins[xt as usize & !BUILTIN_MASK].doc
                );
            } else {
                // It's a definition of some kind
                nfa &= ADDRESS_MASK; // get rid of any special bits
                match xt as i64 {
                    DEFINITION => {
                        print!(": ");
                        let name = self.u_get_string(nfa);
                        print!("{name} ");
                        let mut index = cfa as usize + 1; // skip the inner interpreter
                        loop {
                            let xt = self.heap[index];
                            match xt {
                                LITERAL => {
                                    print!("{} ", self.heap[index as usize + 1]);
                                    index += 1;
                                }
                                STRLIT => {
                                    let s_addr = self.heap[index as usize + 1] as usize;
                                    print!("\" {}\" ", self.u_get_string(s_addr));
                                    index += 1;
                                }
                                BRANCH => {
                                    print!("branch:{} ", self.heap[index as usize + 1]);
                                    index += 1;
                                }
                                BRANCH0 => {
                                    print!("branch0:{} ", self.heap[index as usize + 1]);
                                    index += 1;
                                }
                                ABORT => println!("abort "),
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
                                    let mut cfa = self.heap[index] as usize;
                                    let mut mask = cfa & BUILTIN_MASK;
                                    if mask == 0 {
                                        let word = ADDRESS_MASK & self.heap[self.heap[index] as usize - 1] as usize; // nfa address
                                        let name = self.u_get_string(word);
                                        print!("{name} ");
                                    } else {
                                        mask = !BUILTIN_MASK;
                                        cfa &= mask;
                                        let name = &self.builtins[cfa].name;
                                        print!("{name} ");
                                    }
                                }
                            }
                            index += 1;
                        }
                    }
                    CONSTANT => println!(
                        "Constant: {} = {}",
                        self.u_get_string(self.heap[cfa as usize - 1] as usize),
                        self.heap[cfa as usize + 1]
                    ),
                    VARIABLE => println!(
                        "Variable: {} = {}",
                        self.u_get_string(self.heap[cfa as usize - 1] as usize),
                        self.heap[cfa as usize + 1]
                    ),
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

    /// u_get_string returns a string from a Forth string address
    ///     Assumes the source string is counted (i.e. has its length in the first byte)
    ///
    pub fn u_get_string(&mut self, addr: usize) -> String {
        let str_addr = (addr & ADDRESS_MASK) + 1; //
        let last = str_addr + self.strings[addr] as usize;
        let mut result = String::new();
        for i in str_addr..last {
            result.push(self.strings[i]);
        }
        result
    }

    /// u_set_string saves a counted string to a Forth string address
    ///
    pub fn u_set_string(&mut self, addr: usize, string: &str) {
        let str_addr = addr & ADDRESS_MASK;
        self.strings[str_addr] = string.len() as u8 as char; // count byte
        for (i, c) in string.chars().enumerate() {
            self.strings[str_addr + i + 1] = c;
        }
    }

    /// copy a string from a text buffer to a counted string
    ///     Typically used to copy to PAD from TIB
    ///     Can work with source strings counted or uncounted
    ///
    pub fn u_str_copy(&mut self, from: usize, to: usize, length: usize, counted: bool) {
        self.strings[to] = length as u8 as char; // write count byte
        let offset = if counted { 1 } else { 0 };
        for i in 0..length {
            self.strings[to + i + 1] = self.strings[from + i + offset];
        }
    }

    /// Compare two Forth (counted) strings
    /// First byte is the length, so we'll bail quickly if they don't match
    ///
    pub fn u_str_equal(&mut self, s_addr1: usize, s_addr2: usize) -> bool {
        if self.strings[s_addr1] != self.strings[s_addr2] {
            return false;
        }
        for i in 0..=self.strings[s_addr1] as usize {
            if self.strings[s_addr1 + i] != self.strings[s_addr2 + i] {
                return false;
            }
        }
        true
    }

    /// copy a string slice into string space
    ///    
    pub fn u_save_string(&mut self, from: &str, to: usize) {
        self.strings[to] = from.len() as u8 as char; // count byte
        for (i, c) in from.chars().enumerate() {
            self.strings[to + i + 1] = c;
        }
    }
}
