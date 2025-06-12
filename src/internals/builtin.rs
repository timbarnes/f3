/// Interpreter for builtins
///
/// Set up a table of builtin functions, with names and code

#[allow(dead_code)]
use crate::kernel::{BUILTIN_MASK, FALSE, STR_START, TF, TIB_START, VARIABLE};
use crate::kernel::{PAD_START, TMP_START};

// The mechanism for storing and calling function pointers
pub trait BuiltinCall {
    fn call(&mut self);
}

// The internal format for builtins: a name, code pointer, and documentation string for use by SEE
pub struct BuiltInFn {
    pub name: String,
    pub code: for<'a> fn(&'a mut TF),
    pub doc: String,
}

impl BuiltinCall for BuiltInFn {
    fn call(&mut self) {}
}

impl BuiltInFn {
    pub fn new(name: String, code: for<'a> fn(&'a mut TF), doc: String) -> BuiltInFn {
        BuiltInFn { name, code, doc }
    }
}

impl TF {
    /// u_insert_variables builds the initial set of variables that are visible in Forth and Rust
    ///
    /// They use pointers stored in the main interpreter struct, but the values are stored in Forth user space
    ///
    pub fn u_insert_variables(&mut self) {
        // install system variables in data area
        // hand craft S-HERE (free string pointer) so write_string() can work
        self.heap[0] = 0;
        self.heap[1] = 0;
        self.heap[2] = STR_START as i64; //
        self.strings[STR_START] = 6 as char; // length of "s-here"
        for (i, c) in "s-here".chars().enumerate() {
            self.strings[i + STR_START + 1] = c;
        }
        self.string_ptr = 4;
        self.heap[3] = VARIABLE;
        self.heap[4] = (STR_START + 7) as i64; // update the value of S-HERE
        self.heap[5] = 1; // back pointer
                          // hand craft HERE, because it's needed by make_word
        let name_pointer = self.u_new_string("here");
        self.heap[6] = name_pointer as i64;
        self.heap[7] = VARIABLE;
        self.heap[8] = 10; // the value of HERE
        self.heap[9] = 5; // back pointer
        self.here_ptr = 8; // the address of the HERE variable

        // hand craft CONTEXT, because it's needed by make_word
        self.heap[10] = self.u_new_string("context") as i64;
        self.heap[11] = VARIABLE;
        self.heap[12] = 10;
        self.heap[13] = 9; // back pointer
        self.context_ptr = 12;
        self.heap[self.here_ptr] = 14;

        self.pad_ptr = self.u_make_variable("pad");
        self.heap[self.pad_ptr] = PAD_START as i64;
        self.base_ptr = self.u_make_variable("base");
        self.heap[self.base_ptr] = 10; // decimal
        self.tmp_ptr = self.u_make_variable("tmp");
        self.heap[self.tmp_ptr] = TMP_START as i64;
        self.tib_ptr = self.u_make_variable("'tib");
        self.heap[self.tib_ptr] = TIB_START as i64;
        self.tib_size_ptr = self.u_make_variable("#tib");
        self.heap[self.tib_size_ptr] = 0; // means there's nothing in the TIB yet
        self.tib_in_ptr = self.u_make_variable(">in");
        self.heap[self.tib_in_ptr] = TIB_START as i64 + 1;
        self.hld_ptr = self.u_make_variable("hld");
        self.last_ptr = self.u_make_variable("last"); // points to nfa of new definition
        self.state_ptr = self.u_make_variable("'eval");
        self.abort_ptr = self.u_make_variable("abort?");
        self.state_ptr = self.u_make_variable("state");
        self.stepper_ptr = self.u_make_variable("stepper"); // turns the stepper on or off
        self.step_depth_ptr = self.u_make_variable("stepper-depth"); // turns the stepper on or off
        self.heap[self.abort_ptr] = FALSE;
    }

    /// Insert Forth code into the dictionary by causing the reader to interpret a string
    ///
    pub fn u_insert_code(&mut self) {
        // self.u_interpret("2 2 + .");
    }

    /// u_write_string writes a new string into the next empty space, updating the free space pointer
    fn u_new_string(&mut self, string: &str) -> usize {
        // place a new str into string space and update the free pointer string_ptr
        let mut ptr = self.heap[self.string_ptr] as usize;
        let result_ptr = ptr;
        self.strings[ptr] = string.len() as u8 as char;
        ptr += 1;
        for (i, c) in string.chars().enumerate() {
            self.strings[ptr + i] = c;
        }
        self.heap[self.string_ptr] = (ptr + string.len()) as i64;
        result_ptr
    }

    /// make-variable creates a variable, returning the address of the variable's value
    fn u_make_variable(&mut self, name: &str) -> usize {
        let code_ptr = self.u_make_word(&name, &[VARIABLE, 0]); // install the name
        code_ptr + 1 // the location of the variable's value
    }

    /* fn u_make_constant(&mut self, name: &str, val: i64) -> usize {
           // Create a constant
           let code_ptr = self.u_make_word(name, &[val]); // install the name
           code_ptr + 1
       }
    */
    /// u_make_word Install a new word with provided name and arguments
    ///     back link is already in place
    ///     place it HERE
    ///     update HERE and LAST
    ///     return pointer to first parameter field - the code field pointer or cfa
    ///
    fn u_make_word(&mut self, name: &str, args: &[i64]) -> usize {
        let back = self.heap[self.here_ptr] as usize - 1; // the top-of-stack back pointer's location
        let mut ptr = back + 1;
        self.heap[ptr] = self.u_new_string(name) as i64;
        for val in args {
            ptr += 1;
            self.heap[ptr] = *val;
        }
        ptr += 1;
        self.heap[ptr] = back as i64; // the new back pointer
        self.heap[self.here_ptr] = ptr as i64 + 1; // start of free space = HERE
        self.heap[self.context_ptr] = back as i64 + 1; // context is the name_pointer field of this word
        back + 2 // address of first parameter field
    }

    /// u_add_builtin creates a builtin record with function pointer etc. and links it to a word in the dictionary
    ///     The dual representation is used because of strong typing - it would be great to store the
    ///     function pointer directly in user space, but it would require ugly casting (if it's even possible).
    ///     Also, calling a function via an address in data space is going to cause a crash if the data space
    ///     pointer is incorrect.
    ///
    fn u_add_builtin(&mut self, name: &str, code: for<'a> fn(&'a mut TF), doc: &str) {
        self.builtins
            .push(BuiltInFn::new(name.to_owned(), code, doc.to_string()));
        // now build the DATA space record
        let cfa = (self.builtins.len() - 1) | BUILTIN_MASK;
        self.u_make_word(name, &[cfa as i64]);
    }

    /// Set up all the words that are implemented in Rust
    ///     Each one gets a standard dictionary reference, and a slot in the builtins data structure.
    pub fn add_builtins(&mut self) {
        self.u_add_builtin("+", TF::f_plus, "+ ( j k -- j+k ) Push j+k on the stack");
        self.u_add_builtin("-", TF::f_minus, "- ( j k -- j+k ) Push j-k on the stack");
        self.u_add_builtin("*", TF::f_times, "* ( j k -- j-k ) Push  -k on the stack");
        self.u_add_builtin("/", TF::f_divide, "/ ( j k -- j/k ) Push j/k on the stack");
        self.u_add_builtin("mod", TF::f_mod, "mod ( j k -- j/k ) Push j%k on the stack");
        self.u_add_builtin(
            "<",
            TF::f_less,
            "( j k -- j/k ) If j < k push true else false",
        );
        self.u_add_builtin(
            "true",
            TF::f_true,
            "true ( -- -1 ) Push the canonical true value on the stack.",
        );
        self.u_add_builtin(
            "false",
            TF::f_false,
            "false ( -- 0 ) Push the canonical false value on the stack",
        );
        self.u_add_builtin(
            "=",
            TF::f_equal,
            "= ( j k -- b ) If j == k push true else false",
        );
        self.u_add_builtin(
            "0=",
            TF::f_0equal,
            "0= ( j -- b ) If j == 0 push true else false",
        );
        self.u_add_builtin(
            "0<",
            TF::f_0less,
            "( j k -- j/k ) If j < 0 push true else false",
        );
        self.u_add_builtin(
            ".s",
            TF::f_dot_s,
            ".s ( -- ) Print the contents of the calculation stack",
        );
        self.u_add_builtin(
            "show-stack",
            TF::f_show_stack,
            "show-stack ( -- ) Display the stack at the end of each line of console input",
        );
        self.u_add_builtin(
            "hide-stack",
            TF::f_hide_stack,
            "hide-stack ( -- ) Turn off automatic stack display",
        );
        self.u_add_builtin(
            "(emit)",
            TF::f_emit_p,
            "(emit): ( c -- ) sends character c to the terminal",
        );
        self.u_add_builtin(
            "flush",
            TF::f_flush,
            "flush: forces pending output to appear on the terminal",
        );
        self.u_add_builtin("clear", TF::f_clear, "clear: resets the stack to empty");
        self.u_add_builtin(":", TF::f_colon, ": starts a new definition");
        self.u_add_builtin("bye", TF::f_bye, "bye: exits to the operating system");
        self.u_add_builtin(
            "dup",
            TF::f_dup,
            "dup ( n -- n n ) Push a second copy of the top of stack",
        );
        self.u_add_builtin(
            "drop",
            TF::f_drop,
            "drop ( n --  ) Pop the top element off the stack",
        );
        self.u_add_builtin(
            "swap",
            TF::f_swap,
            "swap ( m n -- n m ) Reverse the order of the top two stack elements",
        );
        self.u_add_builtin(
            "over",
            TF::f_over,
            "over ( m n -- m n m ) Push a copy of the second item on the stack on to",
        );
        self.u_add_builtin(
            "rot",
            TF::f_rot,
            "rot ( i j k -- j k i ) Move the third stack item to the top",
        );
        self.u_add_builtin(
            "pick",
            TF::f_pick,
            "pick ( .. n -- .. v ) Push a copy of the nth item on the stack (after removing n) on top",
        );
        self.u_add_builtin(
            "roll",
            TF::f_roll,
            "roll ( .. n -- .. v ) Rotate the nth item on the stack (after removing n) to the top",
        );
        self.u_add_builtin(
            "and",
            TF::f_and,
            "and ( a b -- a & b ) Pop a and b, returning the logical and",
        );
        self.u_add_builtin(
            "or",
            TF::f_or,
            "or ( a b -- a | b ) Pop a and b, returning the logical or",
        );
        self.u_add_builtin("@", TF::f_get, "@: ( a -- v ) Pushes variable a's value");
        self.u_add_builtin("!", TF::f_store, "!: ( v a -- ) stores v at address a");
        self.u_add_builtin("i", TF::f_i, "Pushes the current FOR - NEXT loop index");
        self.u_add_builtin("j", TF::f_j, "Pushes the second-level (outer) loop index");
        self.u_add_builtin(
            "abort",
            TF::f_abort,
            "abort ( -- ) Ends execution of the current word and clears the stack",
        );
        self.u_add_builtin(
            "depth",
            TF::f_stack_depth,
            "depth: Pushes the current stack depth",
        );
        self.u_add_builtin(
            "key",
            TF::f_key,
            "key ( -- c | 0 ) get a character and push on the stack, or zero if none available",
        );
       self.u_add_builtin(
            "include-file",
            TF::f_include_file,
            "include-file ( a -- ) Taking the TOS as a pointer to 
        a filename (string), load a file of source code",
        );
        self.u_add_builtin("dbg", TF::f_dbg, "");
        self.u_add_builtin(
            "debuglevel",
            TF::f_debuglevel,
            "debuglevel ( -- ) Displays the current debug level",
        );
        self.u_add_builtin(
            ">r",
            TF::f_to_r,
            ">r ( n -- ) Pop stack and push value to return stack",
        );
        self.u_add_builtin(
            "r>",
            TF::f_r_from,
            "r> ( -- n ) Pop return stack and push value to calculation stack",
        );
        self.u_add_builtin(
            "r@",
            TF::f_r_get,
            "r@ ( -- n ) Push the value on the top of the return stack to the calculation stack",
        );
        self.u_add_builtin(
            "immediate",
            TF::f_immediate,
            "immediate sets the immediate flag on the most recently defined word",
        );
        self.u_add_builtin(
            "quit",
            TF::f_quit,
            "quit ( -- ) Outer interpreter that repeatedly reads input lines and runs them",
        );
        self.u_add_builtin(
            "execute",
            TF::f_execute,
            "execute: interpret the word whose address is on the stack",
        );
        self.u_add_builtin(
            "interpret",
            TF::f_eval,
            "interpret: Interprets one line of Forth",
        );
        self.u_add_builtin(
            "number?",
            TF::f_number_q,
            "number? ( a -- n T | a F ) tests a string to see if it's a number;
            leaves n and flag on the stack: true if number is ok.",
        );
        self.u_add_builtin(
            "?unique",
            TF::f_q_unique,
            "?unique ( a -- b ) tests to see if the name TOS points to is in the dictionary",
        );
        self.u_add_builtin(
            "find",
            TF::f_find,
            "FIND (s -- a | F ) Search the dictionary for the token indexed through s. 
        Return it's address or FALSE if not found",
        );
        self.u_add_builtin(
            "(')",
            TF::f_tick_p,
            "(') <name> ( -- a ) searches the dictionary for a (postfix) word, returning its address",
        );
        self.u_add_builtin(
            "query",
            TF::f_query,
            "query ( -- ) Read a line from the console into TIB",
        );
        self.u_add_builtin(
            "accept",
            TF::f_accept,
            "accept ( b l1 -- b l2 ) Read up to l1 characters into the buffer at b.
        Return the pointer to the buffer and the actual number of characters read.",
        );
        self.u_add_builtin(
            "parse-to",
            TF::f_parse_to,
            "parse-to ( b c -- b u ) Get a c-delimited token from TIB, and return counted string in string buffer b",
        );
        self.u_add_builtin(
            "(parse)",
            TF::f_parse_p,
            "(parse) - b u c -- b u delta ) return the location of a delimited token in string space",
        );
        self.u_add_builtin(
            "create",
            TF::f_create,
            "create <name> ( -- ) creates a name field in the dictionary",
        );
        self.u_add_builtin(
            "s-move",
            TF::f_smove,
            "pack$ ( src n dest -- ) copies a counted string to a new location",
        );
        self.u_add_builtin(
            "eval",
            TF::f_eval,
            "eval ( dest -- ) interprets a line of tokens from the TIB",
        );
        self.u_add_builtin(
            ",",
            TF::f_comma,
            ", ( n -- ) copies the top of the stack to the top of the dictionary",
        );
        self.u_add_builtin(
            ";",
            TF::f_semicolon,
            "; ( -- ) terminate a definition, resetting to interpret mode",
        );
        self.f_immediate();
        self.u_add_builtin(
            "immed?",
            TF::f_immediate_q,
            "immed? ( cfa -- T | F ) Determines if a word is immediate",
        );
        self.u_add_builtin("see", TF::f_see, "see <name> decompiles and prints a word");
        self.u_add_builtin(
            "s-create",
            TF::f_s_create,
            "s-create ( s1 -- s2 ) Copy a string to the head of free space and return its address",
        );
        self.u_add_builtin(
            "s-copy",
            TF::f_s_copy,
            "s-copy ( source dest -- ) Copy a counted string from source to dest",
        );
        self.u_add_builtin(
            "c@",
            TF::f_c_get,
            "c@ ( s -- c ) Copy a character from string address s to the stack",
        );
        self.u_add_builtin(
            "c!",
            TF::f_c_store,
            "c! ( c s -- ) Copy character c to string address s",
        );
        self.u_add_builtin("now", TF::f_now, "c! ( -- ) Start a timers");
        self.u_add_builtin(
            "micros",
            TF::f_micros,
            "elapsed ( -- n ) Microseconds since NOW was called",
        );
        self.u_add_builtin(
            "millis",
            TF::f_millis,
            "millis ( -- n ) Milliseconds since NOW was called",
        );
        self.u_add_builtin("open-file", TF::f_open_file, "open-file ( s u fam -- file-id ior ) Open the file named at s, length u, with file access mode fam.
        Returns a file handle and 0 if successful.");
        self.u_add_builtin("close-file", TF::f_close_file, "close-file ( file-id -- ior ) Close a file, returning the I/O status code.");
        self.u_add_builtin("read-line", TF::f_read_line, "read-line ( s u file-id -- u flag ior ) Read up to u characters from a file.
        Returns the number of characters read, a flag indicating success or failure, and an i/o result code.
        Starts from FILE_POSITION, and updates FILE_POSITION on completion.");
        self.u_add_builtin("write-line", TF::f_write_line, "write-line ( s u file-id -- ior ) Write u characters from s to a file, returning an i/o result code.");
        self.u_add_builtin("file-position", TF::f_file_position, "file-position ( file-id -- u ior ) Returns the current file position and an i/o result");
        self.u_add_builtin("file-size", TF::f_file_size, "file-size ( file-id -- u ior ) Returns the size in characters of the file, plus an i/o result code");
        self.u_add_builtin("(system)", TF::f_system_p, "(system) ( s -- ) Execute a shell command, using string s.
        Output is channeled to stdout");
        self.u_add_builtin("ms", TF::f_ms, "sleep ( ms -- ) Puts the current thread to sleep for ms milliseconds");
    }
}
