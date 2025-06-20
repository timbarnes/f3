//////////////////////////////////////////////////////////////////
/// runtime.rs
/// 
/// Forth Runtime Engine
/// 
/// This module defines the ForthRuntime struct, which contains the state of the Forth interpreter.
/// // It includes the kernel, stack pointers, and various other state variables.
/// // It also provides methods for initializing the runtime (cold_start).
///

use crate::kernel::{Kernel, WORD_START, BUF_SIZE};
use crate::internals::builtin::BuiltInFn;
use crate::internals::messages::Msg;
use crate::internals::files::{FileHandle, FType, FileMode}; // Import FileHandle and FType for file handling
use std::time::Instant;
use crate::internals::terminal;

// STRING AREA constants
pub const TIB_START: usize = 0; // Text input buffer, used by readers
pub const PAD_START: usize = TIB_START + BUF_SIZE; // Scratchpad buffer, used by PARSE and friends
pub const TMP_START: usize = PAD_START + BUF_SIZE; // Temporary buffer, used for string input
pub const STR_START: usize = TMP_START + BUF_SIZE; // Free space for additional strings

// Indices into builtins to drive execution of each data type
pub const BUILTIN: i64    = 100000;
pub const VARIABLE: i64   = 100001;
pub const CONSTANT: i64   = 100002;
pub const LITERAL: i64    = 100003;
pub const STRLIT: i64     = 100004;
pub const DEFINITION: i64 = 100005; // a Forth word
pub const BRANCH: i64     = 100006;
pub const BRANCH0: i64    = 100007;
pub const ABORT: i64      = 100008; // break and reset
pub const EXIT: i64       = 100009; // returns from a word
pub const BREAK: i64      = 100010; // breaks out of a word
pub const EXEC: i64       = 100011; // calls the word with address on the stack

pub const MARK_BEGIN: i64 = 200000; // marks the beginning of a control structure
pub const MARK_WHILE: i64 = 200001; // marks the beginning of a WHILE control structure
pub const MARK_FOR: i64   = 200002; // marks the beginning of a FOR control structure
pub const MARK_CASE: i64  = 200003; // marks the beginning of a CASE control structure
pub const MARK_OF: i64    = 200004; // marks the beginning of an OF control structure

// GENERAL constants
pub const TRUE: i64 = -1; // forth convention for true and false
pub const FALSE: i64 = 0;
pub const IMMEDIATE_FLAG: usize = 0x4000000000000000;  // the immediate flag bit
pub const BUILTIN_FLAG: usize   = 0x2000000000000000;  // the builtin flag bit
pub const ADDRESS_MASK: usize   = 0x00FFFFFFFFFFFFFF;  // to get rid of flags
pub const FILEMODE_RO: i64 = 0; // Read-only file mode

#[derive(Debug)]
pub enum ControlMarker {
    Begin(usize),       // address of begin
    While(usize),       // unresolved BRANCH0 location
    For(usize),         // address of FOR loop
    Case(usize),        // address of CASE
    Of(usize),          // address of OF
}

pub struct ForthRuntime {
    pub kernel: Kernel,               // the kernel that contains the Forth runtime
    pub control_stack: Vec<ControlMarker>, // stack for control structures like IF, BEGIN, WHILE
    pub here_ptr: usize,              // first free cell at top of dictionary
    pub context_ptr: usize,           // nfa of most recent word
    pub base_ptr: usize,              // for numeric I/O
    pub pad_ptr: usize,               // string buffer for parser
    pub tmp_ptr: usize,               // temporary string buffer
    pub last_ptr: usize,              // points to name of top word
    pub hld_ptr: usize,               // for numeric string work
    pub state_ptr: usize,             // true if compiling a word
    pub abort_ptr: usize,             // true if abort has been called
    pub tib_ptr: usize,               // TIB
    pub tib_size_ptr: usize,
    pub tib_in_ptr: usize,
    pub exit_flag: bool,              // set when the "bye" word is executed.
    pub msg: Msg,
    pub reader: Vec<FileHandle>,      // allows for nested file processing
    pub files: Vec<FileHandle>,       // keeps track of open files
    pub show_stack: bool,             // show the stack at the completion of a line of interaction
    pub stepper_ptr: usize,           // indicates trace, step, or continuous execution
    pub step_depth_ptr: usize,        // number of levels deep to step or trace
    pub timer: Instant,               // for timing things
}


impl ForthRuntime {
    pub fn new() -> ForthRuntime {
        let mut runtime = ForthRuntime {
            kernel: Kernel::new(),
            control_stack: Vec::new(),
            here_ptr: WORD_START,
            context_ptr: 0,
            base_ptr: 0,
            pad_ptr: 0,
            tmp_ptr: 0,
            last_ptr: 0,
            hld_ptr: 0,
            state_ptr: 0,
            abort_ptr: 0,
            tib_ptr: 0,
            tib_size_ptr: 0,
            tib_in_ptr: 0,
            exit_flag: false,
            msg: Msg::new(),
            reader: Vec::new(),
            files: Vec::new(),
            show_stack: true,
            stepper_ptr: 0,
            step_depth_ptr: 1,
            timer: Instant::now(),
        };
        let fh = FileHandle {
            source: FType::Stdin, // Use standard input
            file_mode: FileMode::RO,
            file_size: 0,
            file_position: 0,
        }; 
        runtime.reader.push(fh); // Set fh as the active reader
        runtime
    }

    /// Return the current value of the HERE pointer.
    /// 
    pub fn here(&mut self) -> usize {
        self.kernel.get(self.here_ptr) as usize // Get the current HERE pointer
    } 

    /// Emit a value into the current definition (at HERE) and increment HERE.
    /// Functionally equivalent to push() and comma().
    /// 
    pub fn emit_cell(&mut self, value: i64) {
        let addr = self.here();
        self.kernel.set(addr, value);
        self.kernel.incr(self.here_ptr);
    }

    fn f_to_c(&mut self) {
        let tag = self.kernel.pop();      // e.g. 1 = Begin, 2 = While, etc.
        let addr = self.kernel.pop() as usize;      // Optionally, could take another item from stack
        let marker = match tag {
            MARK_BEGIN => ControlMarker::Begin(addr),
            MARK_WHILE => ControlMarker::While(addr),
            MARK_FOR   => ControlMarker::For(addr),
            MARK_CASE  => ControlMarker::Case(addr), 
            MARK_OF    => ControlMarker::Of(addr), 
            _          => panic!(">c: unknown control tag {}", tag),
        };
        //println!(">c pushing {:?}", marker);
        self.control_stack.push(marker);
    }

    fn f_from_c(&mut self) {
        match self.control_stack.pop() {
            Some(ControlMarker::Begin(addr)) |
            Some(ControlMarker::While(addr)) |
            Some(ControlMarker::For(addr))   |
            Some(ControlMarker::Case(addr))  |
            Some(ControlMarker::Of(addr)) => {
                //println!("c> popping {:?}", addr);
                self.kernel.push(addr as i64)
            },
            None => self.msg.error("c>", "control stack underflow", None::<()>),
        }
    }

    /// cold_start is where the interpreter begins, installing some variables and the builtin functions.
    pub fn cold_start(&mut self) {
        self.insert_variables();
        self.compile_builtins();
        self.kernel.set(self.state_ptr, FALSE);
        self.insert_code(); // allows forth code to be run prior to presenting a prompt.
    }

    /// get_compile_mode determines whether or not compile mode is active
    ///     Traditionally, a variable called 'EVAL stores the compile or the interpret functions
    ///     In this version, the STATE variable is used directly.
    ///
    pub fn get_compile_mode(&mut self) -> bool {
        if self.kernel.get(self.state_ptr) == FALSE {
            false
        } else {
            true
        }
    }

    /// set_compile_mode turns on compilation mode
    ///
    pub fn set_compile_mode(&mut self, value: bool) {
        let val = if value { -1 } else { 0 };
        // println!("Setting compile mode to {}", value);
        self.kernel.set(self.state_ptr, val);
    }

    /// abort empties the stack, resets any pending operations, and returns to the prompt
    ///     There is a version called abort" implemented in Forth, which prints an error message
    ///
    pub fn f_abort(&mut self) {
        // empty the stack, reset any pending operations, and return to the prompt
        self.f_raw_mode_off();
        self.msg
            .warning("ABORT", "Terminating execution", None::<bool>);
        self.f_clear();
        self.set_abort_flag(true);
    }

    /// f_clear resets the stack and return stack pointers to their initial values
    ///
     pub fn f_clear(&mut self) {
        // println!("Clearing interpreter state");
        self.kernel.reset(); // Reset the kernel state
   }

       /// make-variable creates a variable, returning the address of the variable's value
    fn make_variable(&mut self, name: &str) -> usize {
        let code_ptr = self.make_word(&name, &[VARIABLE, 0]); // install the name
        code_ptr + 1 // the location of the variable's value
    }

    /* fn u_make_constant(&mut self, name: &str, val: i64) -> usize {
           // Create a constant
           let code_ptr = self.kernel.make_word(name, &[val]); // install the name
           code_ptr + 1
       }
    */
    /// make_word installs a new word with provided name and arguments
    ///     back link is already in place
    ///     place it HERE
    ///     update HERE and LAST
    ///     return pointer to first parameter field - the code field pointer or cfa
    ///     This is used for making headers for words, variables, and constants.
    ///

    fn make_word(&mut self, name: &str, args: &[i64]) -> usize {
        // println!("Making word: {}", name);
        let back = self.kernel.get(self.here_ptr) as usize - 1; // the top-of-stack back pointer's location
        let mut ptr = back + 1;
        let val = self.kernel.string_new(name) as i64;
        self.kernel.set(ptr, val as i64);
        for val in args {
            ptr += 1;
            self.kernel.set(ptr, *val);
        }
        ptr += 1;
        self.kernel.set(ptr, back as i64); // the new back pointer
        self.kernel.set(self.here_ptr, ptr as i64 + 1); // start of free space = HERE
        self.kernel.set(self.context_ptr, back as i64 + 1); // context is the name_pointer field of this word
        back + 2 // address of first parameter field
    }

    pub fn insert_variables(&mut self) {
        // install system variables in data area
        // hand craft S-HERE (free string pointer) so write_string() can work
        self.kernel.set(0, 0);
        self.kernel.set(1, 0); // the first two cells are reserved for the stack pointer and return pointer
        self.kernel.set(2, STR_START as i64); //

        self.kernel.string_set(STR_START, "s-here");

        // self.kernel.strings[STR_START] = 6 as u8; // length of "s-here"
        // for (i, c) in "s-here".as_bytes().iter().enumerate() {
        //     self.kernel.strings[i + STR_START + 1] = *c;
        // }

        self.kernel.set_string_ptr(4);
        self.kernel.set(3, VARIABLE);
        self.kernel.set(4, (STR_START + 7) as i64); // update the value of S-HERE
        self.kernel.set(5, 1); // back pointer
                          // hand craft HERE, because it's needed by make_word
        let name_pointer = self.kernel.string_new("here");
        self.kernel.set(6,name_pointer as i64);
        self.kernel.set(7, VARIABLE);
        self.kernel.set(8, 10); // the value of HERE
        self.kernel.set(9, 5); // back pointer
        self.here_ptr = 8; // the address of the HERE variable

        // hand craft CONTEXT, because it's needed by make_word
        let str_addr = self.kernel.string_new("context");
        self.kernel.set(10,  str_addr as i64);
        self.kernel.set(11, VARIABLE);
        self.kernel.set(12, 10);
        self.kernel.set(13, 9); // back pointer
        self.context_ptr = 12;
        self.kernel.set(self.here_ptr, 14);

        self.pad_ptr = self.make_variable("pad");
        self.kernel.set(self.pad_ptr,PAD_START as i64);
        self.base_ptr = self.make_variable("base");
        self.kernel.set(self.base_ptr, 10); // decimal
        self.tmp_ptr = self.make_variable("tmp");
        self.kernel.set(self.tmp_ptr, TMP_START as i64);
        self.tib_ptr = self.make_variable("'tib");
        self.kernel.set(self.tib_ptr,TIB_START as i64);
        self.tib_size_ptr = self.make_variable("#tib");
        self.kernel.set(self.tib_size_ptr, 0); // means there's nothing in the TIB yet
        self.tib_in_ptr = self.make_variable(">in");
        self.kernel.set(self.tib_in_ptr, TIB_START as i64 + 1);
        self.hld_ptr = self.make_variable("hld");
        self.last_ptr = self.make_variable("last"); // points to nfa of new definition
        self.state_ptr = self.make_variable("'eval");
        self.abort_ptr = self.make_variable("abort?");
        self.state_ptr = self.make_variable("state");
        self.stepper_ptr = self.make_variable("stepper"); // turns the stepper on or off
        self.step_depth_ptr = self.make_variable("stepper-depth"); // turns the stepper on or off
        self.kernel.set(self.abort_ptr, FALSE);
    }

    /// Insert Forth code into the dictionary by causing the reader to interpret a string
    ///
    pub fn insert_code(&mut self) {
        // self.kernel.u_interpret("2 2 + .");
    }

    /// add_builtin creates a builtin record with function pointer etc. and links it to a word in the dictionary
    ///     The dual representation is used because of strong typing - it would be great to store the
    ///     function pointer directly in user space, but it would require ugly casting (if it's even possible).
    ///     Also, calling a function via an address in data space is going to cause a crash if the data space
    ///     pointer is incorrect.
    /// 
    ///     The function returns a pointer to the cfa of the builtin, which is the index of the function 
    ///     pointer for the code, combined with the BUILTIN_MASK to indicate that this is a builtin function.
    ///
    fn add_builtin(&mut self, name: &str, code: fn(&mut ForthRuntime), doc: &str) -> usize{
        let index = self.kernel.add_builtin(BuiltInFn::new(name.to_string(), code, doc.to_string()));
        let cfa = index | BUILTIN_FLAG;
        self.make_word(name, &[cfa as i64])
}
    /// Set up all the words that are implemented in Rust
    ///     Each one gets a standard dictionary reference, and a slot in the builtins data structure.
    fn compile_builtins(&mut self) {
        self.add_builtin("+", ForthRuntime::f_plus, "+ ( j k -- j+k ) Push j+k on the stack");
        self.add_builtin("-", ForthRuntime::f_minus, "- ( j k -- j+k ) Push j-k on the stack");
        self.add_builtin("*", ForthRuntime::f_times, "* ( j k -- j-k ) Push  -k on the stack");
        self.add_builtin("/", ForthRuntime::f_divide, "/ ( j k -- j/k ) Push j/k on the stack");
        self.add_builtin("mod", ForthRuntime::f_mod, "mod ( j k -- j/k ) Push j%k on the stack");
        self.add_builtin(
            "<",
            ForthRuntime::f_less,
            "( j k -- j/k ) If j < k push true else false",
        );
        self.add_builtin(
            "true",
            ForthRuntime::f_true,
            "true ( -- -1 ) Push the canonical true value on the stack.",
        );
        self.add_builtin(
            "false",
            ForthRuntime::f_false,
            "false ( -- 0 ) Push the canonical false value on the stack",
        );
        self.add_builtin(
            "=",
            ForthRuntime::f_equal,
            "= ( j k -- b ) If j == k push true else false",
        );
        self.add_builtin(
            "0=",
            ForthRuntime::f_0equal,
            "0= ( j -- b ) If j == 0 push true else false",
        );
        self.add_builtin(
            "0<",
            ForthRuntime::f_0less,
            "( j k -- j/k ) If j < 0 push true else false",
        );
        self.add_builtin(
            ".s",
            ForthRuntime::f_dot_s,
            ".s ( -- ) Print the contents of the calculation stack",
        );
        self.add_builtin(
            "show-stack",
            ForthRuntime::f_show_stack,
            "show-stack ( -- ) Display the stack at the end of each line of console input",
        );
        self.add_builtin(
            "hide-stack",
            ForthRuntime::f_hide_stack,
            "hide-stack ( -- ) Turn off automatic stack display",
        );
        self.add_builtin(
            "(emit)",
            ForthRuntime::f_emit_p,
            "(emit): ( c -- ) sends character c to the terminal",
        );
        self.add_builtin(
            "flush",
            ForthRuntime::f_flush,
            "flush: forces pending output to appear on the terminal",
        );
        self.add_builtin("clear", ForthRuntime::f_clear, "clear: resets the stack to empty");
        self.add_builtin(":", ForthRuntime::f_colon, ": starts a new definition");
        self.add_builtin("bye", ForthRuntime::f_bye, "bye: exits to the operating system");
        self.add_builtin(
            "dup",
            ForthRuntime::f_dup,
            "dup ( n -- n n ) Push a second copy of the top of stack",
        );
        self.add_builtin(
            "drop",
            ForthRuntime::f_drop,
            "drop ( n --  ) Pop the top element off the stack",
        );
        self.add_builtin(
            "swap",
            ForthRuntime::f_swap,
            "swap ( m n -- n m ) Reverse the order of the top two stack elements",
        );
        self.add_builtin(
            "over",
            ForthRuntime::f_over,
            "over ( m n -- m n m ) Push a copy of the second item on the stack on to",
        );
        self.add_builtin(
            "rot",
            ForthRuntime::f_rot,
            "rot ( i j k -- j k i ) Move the third stack item to the top",
        );
        self.add_builtin(
            "pick",
            ForthRuntime::f_pick,
            "pick ( .. n -- .. v ) Push a copy of the nth item on the stack (after removing n) on top",
        );
        self.add_builtin(
            "roll",
            ForthRuntime::f_roll,
            "roll ( .. n -- .. v ) Rotate the nth item on the stack (after removing n) to the top",
        );
        self.add_builtin(
            "and",
            ForthRuntime::f_and,
            "and ( a b -- a & b ) Pop a and b, returning the logical and",
        );
        self.add_builtin(
            "or",
            ForthRuntime::f_or,
            "or ( a b -- a | b ) Pop a and b, returning the logical or",
        );
        self.add_builtin("@", ForthRuntime::f_get, "@: ( a -- v ) Pushes variable a's value");
        self.add_builtin("!", ForthRuntime::f_store, "!: ( v a -- ) stores v at address a");
        self.add_builtin("i", ForthRuntime::f_i, "Pushes the current FOR - NEXT loop index");
        self.add_builtin("j", ForthRuntime::f_j, "Pushes the second-level (outer) loop index");
        self.add_builtin(
            "abort",
            ForthRuntime::f_abort,
            "abort ( -- ) Ends execution of the current word and clears the stack",
        );
        self.add_builtin(
            "depth",
            ForthRuntime::f_stack_depth,
            "depth: Pushes the current stack depth",
        );
        self.add_builtin(
            "key",
            ForthRuntime::f_key,
            "key ( -- c | 0 ) get a character and push on the stack, or zero if none available",
        );
       self.add_builtin(
            "include-file",
            ForthRuntime::f_include_file,
            "include-file ( a -- ) Taking the TOS as a pointer to 
        a filename (string), load a file of source code",
        );
        self.add_builtin("dbg", ForthRuntime::f_dbg, "");
        self.add_builtin(
            "debuglevel",
            ForthRuntime::f_debuglevel,
            "debuglevel ( -- ) Displays the current debug level",
        );
        self.add_builtin(
            ">r",
            ForthRuntime::f_to_r,
            ">r ( n -- ) Pop stack and push value to return stack",
        );
        self.add_builtin(
            "r>",
            ForthRuntime::f_r_from,
            "r> ( -- n ) Pop return stack and push value to calculation stack",
        );
        self.add_builtin(
            "r@",
            ForthRuntime::f_r_get,
            "r@ ( -- n ) Push the value on the top of the return stack to the calculation stack",
        );
        self.add_builtin(
            "immediate",
            ForthRuntime::f_immediate,
            "immediate sets the immediate flag on the most recently defined word",
        );
        self.add_builtin(
            "quit",
            ForthRuntime::f_quit,
            "quit ( -- ) Outer interpreter that repeatedly reads input lines and runs them",
        );
        self.add_builtin(
            "execute",
            ForthRuntime::f_execute,
            "execute: interpret the word whose address is on the stack",
        );
        self.add_builtin(
            "interpret",
            ForthRuntime::f_eval,
            "interpret: Interprets one line of Forth",
        );
        self.add_builtin(
            "number?",
            ForthRuntime::f_number_q,
            "number? ( a -- n T | a F ) tests a string to see if it's a number;
            leaves n and flag on the stack: true if number is ok.",
        );
        self.add_builtin(
            "?unique",
            ForthRuntime::f_q_unique,
            "?unique ( a -- b ) tests to see if the name TOS points to is in the dictionary",
        );
        self.add_builtin(
            "find",
            ForthRuntime::f_find,
            "FIND (s -- a | F ) Search the dictionary for the token indexed through s. 
        Return it's address or FALSE if not found",
        );
        self.add_builtin(
            "(')",
            ForthRuntime::f_tick_p,
            "(') <name> ( -- a ) searches the dictionary for a (postfix) word, returning its address",
        );
        self.add_builtin(
            "query",
            ForthRuntime::f_query,
            "query ( -- ) Read a line from the console into TIB",
        );
        self.add_builtin(
            "accept",
            ForthRuntime::f_accept,
            "accept ( b l1 -- b l2 ) Read up to l1 characters into the buffer at b.
        Return the pointer to the buffer and the actual number of characters read.",
        );
        self.add_builtin(
            "parse-to",
            ForthRuntime::f_parse_to,
            "parse-to ( b c -- b u ) Get a c-delimited token from TIB, and return counted string in string buffer b",
        );
        self.add_builtin(
            "(parse)",
            ForthRuntime::f_parse_p,
            "(parse) - b u c -- b u delta ) return the location of a delimited token in string space",
        );
        self.add_builtin(
            "create",
            ForthRuntime::f_create,
            "create <name> ( -- ) creates a name field in the dictionary",
        );
        self.add_builtin(
            "s-move",
            ForthRuntime::f_smove,
            "pack$ ( src n dest -- ) copies a counted string to a new location",
        );
        self.add_builtin(
            "eval",
            ForthRuntime::f_eval,
            "eval ( dest -- ) interprets a line of tokens from the TIB",
        );
        self.add_builtin(
            ",",
            ForthRuntime::f_comma,
            ", ( n -- ) copies the top of the stack to the top of the dictionary",
        ); 
        self.add_builtin(
            ";",
            ForthRuntime::f_semicolon,
            "; ( -- ) terminate a definition, resetting to interpret mode",
        );
        self.f_immediate(); // set the immediate flag on the most recent word

        self.add_builtin(
            "immed?",
            ForthRuntime::f_immediate_q,
            "immed? ( cfa -- T | F ) Determines if a word is immediate",
        );
        self.add_builtin("see", ForthRuntime::f_see, "see <name> decompiles and prints a word");
        self.add_builtin(
            "s-create",
            ForthRuntime::f_s_create,
            "s-create ( s1 -- s2 ) Copy a string to the head of free space and return its address",
        );
        self.add_builtin(
            "s-copy",
            ForthRuntime::f_s_copy,
            "s-copy ( source dest -- ) Copy a counted string from source to dest",
        );
        self.add_builtin(
            "c@",
            ForthRuntime::f_c_get,
            "c@ ( s -- c ) Copy a character from string address s to the stack",
        );
        self.add_builtin(
            "c!",
            ForthRuntime::f_c_store,
            "c! ( c s -- ) Copy character c to string address s",
        );
        self.add_builtin("now", ForthRuntime::f_now, "c! ( -- ) Start a timers");
        self.add_builtin(
            "micros",
            ForthRuntime::f_micros,
            "elapsed ( -- n ) Microseconds since NOW was called",
        );
        self.add_builtin(
            "millis",
            ForthRuntime::f_millis,
            "millis ( -- n ) Milliseconds since NOW was called",
        );
        self.add_builtin("open-file", ForthRuntime::f_open_file, "open-file ( s u fam -- file-id ior ) Open the file named at s, length u, with file access mode fam.
        Returns a file handle and 0 if successful.");
        self.add_builtin("close-file", ForthRuntime::f_close_file, "close-file ( file-id -- ior ) Close a file, returning the I/O status code.");
        self.add_builtin("read-line", ForthRuntime::f_read_line, "read-line ( s u file-id -- u flag ior ) Read up to u characters from a file.
        Returns the number of characters read, a flag indicating success or failure, and an i/o result code.
        Starts from FILE_POSITION, and updates FILE_POSITION on completion.");
        self.add_builtin("write-line", ForthRuntime::f_write_line, "write-line ( s u file-id -- ior ) Write u characters from s to a file, returning an i/o result code.");
        self.add_builtin("file-position", ForthRuntime::f_file_position, "file-position ( file-id -- u ior ) Returns the current file position and an i/o result");
        self.add_builtin("file-size", ForthRuntime::f_file_size, "file-size ( file-id -- u ior ) Returns the size in characters of the file, plus an i/o result code");
        self.add_builtin("(system)", ForthRuntime::f_system_p, "(system) ( s -- ) Execute a shell command, using string s.
        Output is channeled to stdout");
        self.add_builtin("ms", ForthRuntime::f_ms, "sleep ( ms -- ) Puts the current thread to sleep for ms milliseconds");
        self.add_builtin("raw-mode-on", ForthRuntime::f_raw_mode_on, "raw-mode-on ( -- ) Enable raw terminal mode");
        self.add_builtin("raw-mode-off", ForthRuntime::f_raw_mode_off, "raw-mode-off ( -- ) Disable raw terminal mode");
        self.add_builtin("raw-mode?", ForthRuntime::f_raw_mode_q, "raw-mode? ( -- f ) Returns true if in raw mode");
        //self.add_builtin("mark-begin", ForthRuntime::f_mark_begin, "Mark BEGIN");
        //self.add_builtin("mark-while", ForthRuntime::f_mark_while, "Mark WHILE");
        //self.add_builtin("mark-for", ForthRuntime::f_mark_for, "Mark FOR");
        //self.add_builtin("patch-repeat", ForthRuntime::f_patch_repeat, "Patch REPEAT");
        //self.add_builtin("patch-until", ForthRuntime::f_patch_until, "Patch UNTIL");
        //self.add_builtin("patch-again", ForthRuntime::f_patch_again, "Patch AGAIN");
        //self.add_builtin("patch-next", ForthRuntime::f_patch_next, "Patch NEXT");
        self.add_builtin(">c", ForthRuntime::f_to_c,
         ">c ( tag -- ) Push a control marker onto the control stack");
        self.add_builtin("c>", ForthRuntime::f_from_c,
         ">c ( -- tag ) Pop a control marker from the control stack and push its address");
    }

    /// set_abort_flag allows the abort condition to be made globally visible
    ///
    pub fn set_abort_flag(&mut self, v: bool) {
        self.kernel.set(self.abort_ptr, if v { -1 } else { 0 });
    }

    /// get_abort_flag returns the current value of the flag
    ///
    pub fn get_abort_flag(&mut self) -> bool {
        let val = self.kernel.get(self.abort_ptr);
        if val == FALSE {
            false
        } else {
            true
        }
    }

    /// should_exit determines whether or not the user has executed BYE
    ///
    pub fn should_exit(&self) -> bool {
        // Method to determine if we should exit
        self.exit_flag
    }

    pub fn f_bye(&mut self) {
        self.exit_flag = true;
    }

    pub fn f_raw_mode_on(&mut self) {
        if let Err(e) = terminal::enable_raw() {
            self.msg.error("raw-mode-on", &e.to_string(), None::<bool>);
        }
    }

    pub fn f_raw_mode_off(&mut self) {
        if let Err(e) = terminal::disable_raw() {
            self.msg.error("raw-mode-off", &e.to_string(), None::<bool>);
        }
    }

    pub fn f_raw_mode_q(&mut self) {
        match terminal::get_raw_mode() {
            Ok(enabled) => self.kernel.push(if enabled { TRUE } else { FALSE }),
            Err(e) => self.msg.error("raw-mode?", &e.to_string(), None::<bool>),
        }
    }
}

/////////////////////////
/// TESTS
/// 

#[cfg(test)]
mod tests {
    use super::*;
    //use crate::kernel::{RET_START, STACK_START};

    // Access the kernel directly for testing purposes
    #[test]
    fn test_stack_push_and_pop() {
        let mut rt = ForthRuntime::new();
        rt.cold_start();
        rt.kernel.push(42);
        assert_eq!(rt.kernel.pop(), 42);
    }

    #[test]
    fn test_new_runtime() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        assert_eq!(runtime.kernel.get(0), 0); // stack pointer
        assert_eq!(runtime.kernel.get(1), 0); // return pointer
        // assert_eq!(runtime.here_ptr, WORD_START);
        // assert_eq!(runtime.context_ptr, 0);
        assert!(!runtime.exit_flag);
    }

    #[test]
    fn test_cold_start() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();
        assert_eq!(runtime.kernel.get(runtime.state_ptr), FALSE);
    }


    #[test]
    fn test_make_word() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        let code_ptr = runtime.make_word("test", &[1, 2, 3]);
        println!("Code pointer: {}", code_ptr);
        let s1 = runtime.kernel.get(code_ptr - 1) as usize;
        let s2 = runtime.kernel.string_new("test");
        assert!(runtime.kernel.string_equal(s1, s2));
        assert_eq!(runtime.kernel.get(code_ptr), 1);
        assert_eq!(runtime.kernel.get(code_ptr + 1), 2);
        assert_eq!(runtime.kernel.get(code_ptr + 2), 3);
    }

    #[test]
    fn test_add_builtin() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        let addr = runtime.add_builtin("test", ForthRuntime::f_plus, "Test function");
        let cfa = runtime.kernel.get(addr) as usize;
        println!("ADDR: {}, CFA: {}", addr, cfa);
        assert!(cfa > BUILTIN_FLAG);
    }

    #[test]
    fn test_add_and_call_builtin() {
        let mut rt = ForthRuntime::new();
        rt.cold_start();

        fn sample_add(rt: &mut ForthRuntime) {
            let b = rt.kernel.pop();
            let a = rt.kernel.pop();
            rt.kernel.push(a + b);
        }

        let addr = rt.add_builtin("add", sample_add, "Add two numbers");
        rt.kernel.push(10);
        rt.kernel.push(32);
        let cfa = rt.kernel.get(addr as usize) as usize & ADDRESS_MASK;
        rt.builtin(cfa);

        assert_eq!(rt.kernel.pop(), 42);
    }

    #[test]
    fn test_insert_variables() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        runtime.insert_variables();
        assert!(runtime.kernel.get(0) == 0); // stack pointer
        assert!(runtime.kernel.get(1) == 0); // return pointer
        assert!(runtime.kernel.get(runtime.here_ptr) > WORD_START as i64);
        assert!(runtime.kernel.get(runtime.pad_ptr) == PAD_START as i64);
        assert!(runtime.kernel.get(runtime.base_ptr) == 10); // decimal base
    }


    #[test]
    fn test_compile_builtins() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        runtime.compile_builtins();
        assert_eq!(runtime.kernel.get_builtin(6).name, "true".to_string());
        assert_eq!(runtime.kernel.get_builtin(5).name, "<".to_string());
        assert_eq!(runtime.kernel.get_builtin(0).name, "+".to_string());
    }


    #[test]
    fn test_get_compile_mode() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        runtime.set_compile_mode(true);
        assert!(runtime.get_compile_mode());
        runtime.set_compile_mode(false);
        assert!(!runtime.get_compile_mode());
    }

    #[test]
    fn test_set_compile_mode() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        runtime.set_compile_mode(true);
        assert_eq!(runtime.kernel.get(runtime.state_ptr), -1);
        runtime.set_compile_mode(false);
        assert_eq!(runtime.kernel.get(runtime.state_ptr), 0);
    }

    #[test]
    fn test_set_abort_flag() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        runtime.set_abort_flag(true);
        assert!(runtime.get_abort_flag());
        runtime.set_abort_flag(false);
        assert!(!runtime.get_abort_flag());
    }

    #[test]
    fn test_should_exit() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        assert!(!runtime.should_exit());
    }

    #[test]
    fn test_f_bye() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        runtime.f_bye();
        assert!(runtime.should_exit());
    }

    #[test]
    fn test_f_clear() {
        let mut runtime = ForthRuntime::new();
        runtime.kernel.push(42);
        runtime.kernel.push(99);
        assert_eq!(runtime.kernel.stack_len(), 2); // stack should have 2 items
        runtime.f_clear();
        assert_eq!(runtime.kernel.stack_len(), 0); // stack should be cleared
    }


    #[test]
    fn test_f_abort() {
        let mut runtime = ForthRuntime::new();
        runtime.kernel.push(42);
        runtime.kernel.push(99);
        assert_eq!(runtime.kernel.stack_len(), 2); // stack should have 2 items
        runtime.f_abort();
        assert_eq!(runtime.kernel.stack_len(), 0); // stack should be cleared
    }

    #[test]
    fn test_f_get_compile_mode() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        runtime.set_compile_mode(true);
        assert!(runtime.get_compile_mode());
        runtime.set_compile_mode(false);
        assert!(!runtime.get_compile_mode());
    }

    #[test]
    fn test_f_set_compile_mode() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        runtime.set_compile_mode(true);
        assert_eq!(runtime.kernel.get(runtime.state_ptr), -1);
        runtime.set_compile_mode(false);
        assert_eq!(runtime.kernel.get(runtime.state_ptr), 0);
    }

    #[test]
    fn test_f_set_abort_flag() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        runtime.set_abort_flag(true);
        assert!(runtime.get_abort_flag());
        runtime.set_abort_flag(false);
        assert!(!runtime.get_abort_flag());
    }

    #[test]
    fn test_f_should_exit() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        assert!(!runtime.should_exit());
    }
}