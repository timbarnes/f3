//////////////////////////////////////////////////////////////////
use crate::internals::builtin::BuiltInFn;
use crate::internals::files::FileHandle;
use crate::internals::messages::Msg;
use crate::internals::terminal;
/// runtime.rs
///
/// Forth Runtime Engine
///
/// This module defines the ForthRuntime struct, which contains the state of the Forth interpreter.
/// It includes the kernel, the addresses of system variables, and the input and file state.
/// It also provides methods for initializing the runtime (cold_start).
///
/// Words are written in Rust only if they need the host (I/O, files, time, terminal), are hot
/// primitives (arithmetic, comparison, basic stack and memory words), or are needed before Forth
/// exists or by error recovery (parsing, find, number conversion, : ; create , and the quit loop).
/// Everything else is defined in Forth, in corelib.fs.
///
use crate::kernel::{
    throw_code, ForthError, ABORT_CODE, ABORT_QUOTE, DICTIONARY_FULL, Kernel, BUF_SIZE, DICT_LIMIT, RET_START, STACK_START, WORD_START,
};
use std::any::Any;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::time::Instant;

// STRING AREA constants
pub const TIB_START: usize = 0; // Text input buffer, used by readers
pub const PAD_START: usize = TIB_START + BUF_SIZE; // Scratchpad buffer, used by PARSE and friends
pub const TMP_START: usize = PAD_START + BUF_SIZE; // Temporary buffer, used for string input
pub const STR_START: usize = TMP_START + BUF_SIZE; // Free space for additional strings

// Inner-interpreter opcodes, compiled into definitions. OPCODE_FLAG tells them apart from
// word addresses. They are also defined as Forth constants of the same names (insert_constants).
pub const OPCODE_FLAG: i64 = 1 << 59;
pub const VARIABLE: i64 = OPCODE_FLAG | 1;
pub const CONSTANT: i64 = OPCODE_FLAG | 2;
pub const LITERAL: i64 = OPCODE_FLAG | 3; // operand: the number
pub const STRLIT: i64 = OPCODE_FLAG | 4; // operand: the string address
pub const DEFINITION: i64 = OPCODE_FLAG | 5; // a colon definition
pub const BRANCH: i64 = OPCODE_FLAG | 6; // operand: offset from the operand cell
pub const BRANCH0: i64 = OPCODE_FLAG | 7; // operand: offset from the operand cell
pub const ABORT: i64 = OPCODE_FLAG | 8; // break and reset
pub const EXIT: i64 = OPCODE_FLAG | 9; // returns from a word
pub const BREAK: i64 = OPCODE_FLAG | 10; // breaks out of a word
pub const EXEC: i64 = OPCODE_FLAG | 11; // calls the word with address on the stack
pub const ARRAY: i64 = OPCODE_FLAG | 12;
pub const DO: i64 = OPCODE_FLAG | 13; // starts a DO loop; operand: offset to the loop exit
pub const QDO: i64 = OPCODE_FLAG | 14; // ?DO: like DO, but skips the loop if index = limit
pub const LOOP: i64 = OPCODE_FLAG | 15; // ends a DO loop; operand: offset back to the loop body
pub const PLOOP: i64 = OPCODE_FLAG | 16; // +LOOP
pub const LEAVE: i64 = OPCODE_FLAG | 17; // jumps to the exit of the innermost loop
pub const FOR: i64 = OPCODE_FLAG | 18; // starts a FOR loop; operand: offset to the loop exit
pub const NEXT: i64 = OPCODE_FLAG | 19; // ends a FOR loop; operand: offset back to the loop body

// GENERAL constants
pub const TRUE: i64 = -1; // forth convention for true and false
pub const FALSE: i64 = 0;
pub const IMMEDIATE_FLAG: usize = 0x4000000000000000; // the immediate flag bit
pub const BUILTIN_FLAG: usize = 0x2000000000000000; // the builtin flag bit
pub const COMPILE_ONLY_FLAG: usize = 0x1000000000000000; // the word is an error outside a definition
pub const ADDRESS_MASK: usize = 0x00FFFFFFFFFFFFFF; // to get rid of flags

type Builtin = fn(&mut ForthRuntime);

pub struct ForthRuntime {
    pub kernel: Kernel,     // the kernel that contains the Forth runtime
    pub here_ptr: usize,    // first free cell at top of dictionary
    pub context_ptr: usize, // nfa of most recent word
    pub base_ptr: usize,    // for numeric I/O
    pub pad_ptr: usize,     // string buffer for parser
    pub tmp_ptr: usize,     // temporary string buffer
    pub last_ptr: usize,    // points to name of top word
    pub state_ptr: usize,   // true if compiling a word
    pub tib_ptr: usize,     // TIB
    pub tib_size_ptr: usize,
    pub tib_in_ptr: usize,
    pub show_stack_ptr: usize, // true: show the stack (with .s) at the prompt
    pub exit_flag: bool,       // set when the "bye" word is executed.
    pub msg: Msg,
    pub reader: Vec<FileHandle>, // allows for nested file processing
    pub files: Vec<Option<FileHandle>>, // open files, indexed by file-id; None once closed
    pub stepper_ptr: usize,      // indicates trace, step, or continuous execution
    pub tracing: bool,           // stepper != 0, cached for the inner interpreter (see set_stepper)
    pub step_depth_ptr: usize,   // number of levels deep to step or trace
    pub timer: Instant,          // for timing things
    pub pending_def: Option<PendingDef>, // dictionary state before the latest CREATE
    pub caught: Option<ForthError>,      // the error most recently caught by CATCH, for THROW to re-raise
}

/// Dictionary pointers saved by CREATE, so an error before the word is closed can roll it back.
pub struct PendingDef {
    pub here: i64,
    pub s_here: i64,
    pub last: i64,
    pub context: i64,
}

impl ForthRuntime {
    pub fn new() -> ForthRuntime {
        let mut runtime = ForthRuntime {
            kernel: Kernel::new(),
            here_ptr: WORD_START,
            context_ptr: 0,
            base_ptr: 0,
            pad_ptr: 0,
            tmp_ptr: 0,
            last_ptr: 0,
            state_ptr: 0,
            tib_ptr: 0,
            tib_size_ptr: 0,
            tib_in_ptr: 0,
            show_stack_ptr: 0,
            exit_flag: false,
            msg: Msg::new(),
            reader: Vec::new(),
            files: Vec::new(),
            stepper_ptr: 0,
            tracing: false,
            step_depth_ptr: 1,
            timer: Instant::now(),
            pending_def: None,
            caught: None,
        };
        runtime.reader.push(FileHandle::stdin()); // stdin is always the bottom reader
        runtime
    }

    /// Return the current value of the HERE pointer.
    ///
    pub fn here(&mut self) -> usize {
        self.kernel.get(self.here_ptr) as usize // Get the current HERE pointer
    }

    /// Emit a value into the current definition (at HERE) and increment HERE.
    /// Functionally equivalent to push() and comma().
    ///     After a finished word, HERE-1 holds its back pointer, which the next definition links
    ///     to: the value goes there and the back pointer moves up, so `variable x 1 , 2 ,`
    ///     gives x three contiguous cells and the dictionary stays intact.
    ///
    pub fn emit_cell(&mut self, value: i64) {
        let addr = self.here();
        if addr >= DICT_LIMIT {
            throw_code(DICTIONARY_FULL, "dictionary full");
        }
        if self.word_closed() {
            let back = self.kernel.get(addr - 1);
            self.kernel.set(addr - 1, value);
            self.kernel.set(addr, back);
        } else {
            self.kernel.set(addr, value);
        }
        self.kernel.incr(self.here_ptr);
    }

    /// word_closed is true when the latest word is finished (LAST = CONTEXT), so the cell at
    ///     HERE-1 is its back pointer; false while a definition is being built.
    ///
    pub fn word_closed(&self) -> bool {
        self.kernel.get(self.last_ptr) == self.kernel.get(self.context_ptr)
    }

    /// cold_start is where the interpreter begins, installing some variables and the builtin functions.
    pub fn cold_start(&mut self) {
        self.insert_variables();
        self.insert_constants();
        self.compile_builtins();
        self.kernel.set(self.state_ptr, FALSE);
        // LAST = CONTEXT means the latest word is finished (allot relies on this)
        let latest = self.kernel.get(self.context_ptr);
        self.kernel.set(self.last_ptr, latest);
    }

    /// get_compile_mode determines whether or not compile mode is active
    ///     The STATE variable is used directly.
    ///
    pub fn get_compile_mode(&mut self) -> bool {
        self.kernel.get(self.state_ptr) != FALSE
    }

    /// set_compile_mode turns on compilation mode
    ///
    pub fn set_compile_mode(&mut self, value: bool) {
        let val = if value { -1 } else { 0 };
        self.kernel.set(self.state_ptr, val);
    }

    /// set_stepper writes the STEPPER variable. Every write goes through here (! included),
    ///     so the inner interpreter can test a plain bool instead of reading the heap per cell.
    ///
    pub fn set_stepper(&mut self, mode: i64) {
        self.kernel.set(self.stepper_ptr, mode);
        self.tracing = mode != 0;
    }

    /// abort empties the stack, resets any pending operations, and returns to the prompt
    ///     There is a version called abort" implemented in Forth, which prints an error message
    ///
    pub fn f_abort(&mut self) {
        throw_code(ABORT_CODE, ""); // silent: quit catches it and calls recover
    }

    /// catch ( ... xt -- ... 0 | n ) runs xt. If it completes, push 0. If it throws (THROW, ABORT,
    ///     or any engine error), restore the data and return stack depths to what they were before
    ///     xt ran, and push the throw code.
    ///
    pub fn f_catch(&mut self) {
        self.kernel.stack_check(1, "catch");
        let xt = self.kernel.pop();
        let (sp, rp) = (self.kernel.get_stack_ptr(), self.kernel.get_return_ptr());
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.kernel.push(xt);
            self.f_execute();
        }));
        match result {
            Ok(()) => self.kernel.push(0),
            Err(payload) => match payload.downcast::<ForthError>() {
                Ok(error) => {
                    self.kernel.set_stack_ptr(sp);
                    self.kernel.set_return_ptr(rp);
                    self.kernel.push(error.code);
                    self.caught = Some(*error);
                }
                Err(other) => resume_unwind(other), // a Rust panic is an engine bug: let quit report it
            },
        }
    }

    /// throw ( n -- ) does nothing if n is 0; otherwise raises an error with code n.
    ///     Re-throwing the code CATCH just returned keeps that error's message.
    ///
    pub fn f_throw(&mut self) {
        self.kernel.stack_check(1, "throw");
        let code = self.kernel.pop();
        if code == 0 {
            return;
        }
        let msg = match self.caught.take() {
            Some(error) if error.code == code => error.msg,
            _ if code == ABORT_CODE => String::new(),
            _ => format!("uncaught exception {code}"),
        };
        throw_code(code, msg);
    }

    /// (error) ( s -- ) raises an error whose message is the counted string at s. Used by abort"
    ///
    pub fn f_error(&mut self) {
        self.kernel.stack_check(1, "(error)");
        let addr = self.kernel.pop() as usize & ADDRESS_MASK;
        let msg = String::from_utf8_lossy(self.kernel.counted_bytes(addr)).into_owned();
        throw_code(ABORT_QUOTE, msg);
    }

    /// recover reports an error caught by quit, and returns the system to a clean state:
    ///     empty stacks, interpret mode, no half-built definition, input back to the console.
    ///
    pub fn recover(&mut self, payload: Box<dyn Any + Send>) {
        let msg = if let Some(ForthError { msg, .. }) = payload.downcast_ref::<ForthError>() {
            msg.clone()
        } else if let Some(msg) = payload.downcast_ref::<&str>() {
            format!("internal: {msg}")
        } else if let Some(msg) = payload.downcast_ref::<String>() {
            format!("internal: {msg}")
        } else {
            "internal error".to_string()
        };
        if msg.starts_with("internal: failed printing to stdout") {
            std::process::exit(0); // stdout is gone (e.g. a closed pipe): nothing left to talk to
        }
        if !msg.is_empty() {
            println!("ERROR: {msg}");
        }

        self.kernel.reset();
        self.set_compile_mode(false);
        if let Some(def) = self.pending_def.take() {
            // CREATE ran but the word was never closed (CONTEXT unchanged): remove it
            if self.kernel.get(self.context_ptr) == def.context {
                self.kernel.set(self.here_ptr, def.here);
                self.kernel.set(self.kernel.get_string_ptr(), def.s_here);
                self.kernel.set(self.last_ptr, def.last);
            }
        }
        self.reader.truncate(1); // abandon any files being included
        if terminal::get_raw_mode().unwrap_or(false) {
            let _ = terminal::disable_raw();
        }
        self.f_flush();
    }

    /// make-variable creates a variable, returning the address of the variable's value
    fn make_variable(&mut self, name: &str) -> usize {
        let code_ptr = self.make_word(name, &[VARIABLE, 0]); // install the name
        code_ptr + 1 // the location of the variable's value
    }

    /// make_word installs a new word with provided name and arguments
    ///     back link is already in place
    ///     place it HERE
    ///     update HERE and LAST
    ///     return pointer to first parameter field - the code field pointer or cfa
    ///     This is used for making headers for words, variables, and constants.
    fn make_word(&mut self, name: &str, args: &[i64]) -> usize {
        let back = self.kernel.get(self.here_ptr) as usize - 1; // the top-of-stack back pointer's location
        let mut ptr = back + 1;
        let val = self.kernel.string_new(name.as_bytes()) as i64;
        self.kernel.set(ptr, val);
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
        self.kernel.set(1, 0);
        self.kernel.set(2, STR_START as i64); //

        self.kernel.string_set(STR_START, "s-here");

        self.kernel.set_string_ptr(4);
        self.kernel.set(3, VARIABLE);
        self.kernel.set(4, (STR_START + 7) as i64); // update the value of S-HERE
        self.kernel.set(5, 1); // back pointer
                               // hand craft HERE, because it's needed by make_word
        let name_pointer = self.kernel.string_new(b"dp"); // the dictionary pointer; HERE is `dp @`
        self.kernel.set(6, name_pointer as i64);
        self.kernel.set(7, VARIABLE);
        self.kernel.set(8, 10); // the value of HERE
        self.kernel.set(9, 5); // back pointer
        self.here_ptr = 8; // the address of the HERE variable

        // hand craft CONTEXT, because it's needed by make_word
        let str_addr = self.kernel.string_new(b"context");
        self.kernel.set(10, str_addr as i64);
        self.kernel.set(11, VARIABLE);
        self.kernel.set(12, 10);
        self.kernel.set(13, 9); // back pointer
        self.context_ptr = 12;
        self.kernel.set(self.here_ptr, 14);

        self.pad_ptr = self.make_variable("'pad"); // PAD is `'pad @`
        self.kernel.set(self.pad_ptr, PAD_START as i64);
        self.base_ptr = self.make_variable("base");
        self.kernel.set(self.base_ptr, 10); // decimal
        self.tmp_ptr = self.make_variable("tmp");
        self.kernel.set(self.tmp_ptr, TMP_START as i64);
        self.tib_ptr = self.make_variable("'tib");
        self.kernel.set(self.tib_ptr, TIB_START as i64);
        self.tib_size_ptr = self.make_variable("#tib");
        self.kernel.set(self.tib_size_ptr, 0); // means there's nothing in the TIB yet
        self.tib_in_ptr = self.make_variable(">in");
        self.kernel.set(self.tib_in_ptr, 0);
        self.make_variable("hld");
        self.last_ptr = self.make_variable("last"); // points to nfa of new definition
        self.state_ptr = self.make_variable("state");
        self.stepper_ptr = self.make_variable("stepper"); // turns the stepper on or off
        self.step_depth_ptr = self.make_variable("stepper-depth"); // how many levels to step
        self.show_stack_ptr = self.make_variable("show-stack?");
        self.kernel.set(self.show_stack_ptr, TRUE);
    }

    /// insert_constants defines the engine's opcodes (op:literal etc.), flags and stack bases as Forth constants
    ///
    fn insert_constants(&mut self) {
        let constants: &[(&str, i64)] = &[
            ("op:variable", VARIABLE),
            ("op:constant", CONSTANT),
            ("op:literal", LITERAL),
            ("op:strlit", STRLIT),
            ("op:definition", DEFINITION),
            ("op:branch", BRANCH),
            ("op:branch0", BRANCH0),
            ("op:abort", ABORT),
            ("op:exit", EXIT),
            ("op:break", BREAK),
            ("op:exec", EXEC),
            ("op:array", ARRAY),
            ("op:do", DO),
            ("op:qdo", QDO),
            ("op:loop", LOOP),
            ("op:ploop", PLOOP),
            ("op:leave", LEAVE),
            ("op:for", FOR),
            ("op:next", NEXT),
            ("OPCODE_FLAG", OPCODE_FLAG),
            ("IMMEDIATE_FLAG", IMMEDIATE_FLAG as i64),
            ("BUILTIN_FLAG", BUILTIN_FLAG as i64),
            ("COMPILE_ONLY_FLAG", COMPILE_ONLY_FLAG as i64),
            ("ADDRESS_MASK", ADDRESS_MASK as i64),
            ("DICT_LIMIT", DICT_LIMIT as i64), // the dictionary can grow up to here
            ("sp0", STACK_START as i64), // sp@ when the data stack is empty
            ("rp0", RET_START as i64),   // rp@ when the return stack is empty
        ];
        for (name, value) in constants {
            self.make_word(name, &[CONSTANT, *value]);
        }
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
    fn add_builtin(&mut self, name: &str, code: Builtin, doc: &str) -> usize {
        let index =
            self.kernel
                .add_builtin(BuiltInFn::new(name.to_string(), code, doc.to_string()));
        let cfa = index | BUILTIN_FLAG;
        self.make_word(name, &[cfa as i64])
    }

    /// Set up all the words that are implemented in Rust
    ///     Each one gets a standard dictionary reference, and a slot in the builtins data structure.
    fn compile_builtins(&mut self) {
        let builtins: &[(&str, Builtin, &str)] = &[
            // arithmetic and logic
            ("+", Self::f_plus, "+ ( j k -- j+k ) Push j+k on the stack"),
            ("-", Self::f_minus, "- ( j k -- j-k ) Push j-k on the stack"),
            ("*", Self::f_times, "* ( j k -- j*k ) Push j*k on the stack"),
            ("/", Self::f_divide, "/ ( j k -- j/k ) Push j/k on the stack"),
            ("mod", Self::f_mod, "mod ( j k -- r ) Push the remainder of j/k"),
            ("lshift", Self::f_lshift, "lshift ( j k -- j<<k ) Shift j left by k bits"),
            ("rshift", Self::f_rshift, "rshift ( u k -- u>>k ) Logical shift right by k bits"),
            ("2/", Self::f_2_slash, "2/ ( n -- n/2 ) Arithmetic shift right by one bit, keeping the sign"),
            ("<", Self::f_less, "< ( j k -- b ) If j < k push true else false"),
            ("=", Self::f_equal, "= ( j k -- b ) If j == k push true else false"),
            ("0=", Self::f_0equal, "0= ( j -- b ) If j == 0 push true else false"),
            ("0<", Self::f_0less, "0< ( j -- b ) If j < 0 push true else false"),
            ("u<", Self::f_u_less, "u< ( u1 u2 -- b ) Unsigned comparison: true if u1 < u2"),
            ("and", Self::f_and, "and ( a b -- a&b ) Bitwise and"),
            ("or", Self::f_or, "or ( a b -- a|b ) Bitwise or"),
            ("xor", Self::f_xor, "xor ( a b -- a^b ) Bitwise exclusive or"),
            ("um*", Self::f_um_star, "um* ( u1 u2 -- ud ) Unsigned multiply, double-cell result (high cell on top)"),
            ("m*", Self::f_m_star, "m* ( n1 n2 -- d ) Signed multiply, double-cell result"),
            ("um/mod", Self::f_um_slash_mod, "um/mod ( ud u -- urem uquot ) Unsigned divide of a double-cell number"),
            ("fm/mod", Self::f_fm_slash_mod, "fm/mod ( d n -- rem quot ) Floored divide of a double-cell number"),
            ("sm/rem", Self::f_sm_slash_rem, "sm/rem ( d n -- rem quot ) Symmetric divide of a double-cell number"),
            // stacks
            ("dup", Self::f_dup, "dup ( n -- n n ) Push a second copy of the top of stack"),
            ("drop", Self::f_drop, "drop ( n -- ) Pop the top element off the stack"),
            ("swap", Self::f_swap, "swap ( m n -- n m ) Reverse the order of the top two stack elements"),
            ("over", Self::f_over, "over ( m n -- m n m ) Push a copy of the second item on the stack"),
            ("rot", Self::f_rot, "rot ( i j k -- j k i ) Move the third stack item to the top"),
            ("sp@", Self::f_sp_fetch, "sp@ ( -- a ) Push the address of the top of the data stack (sp0 when empty)"),
            ("rp@", Self::f_rp_fetch, "rp@ ( -- a ) Push the address of the top of the return stack (rp0 when empty)"),
            (">r", Self::f_to_r, ">r ( n -- ) Pop stack and push value to return stack"),
            ("r>", Self::f_r_from, "r> ( -- n ) Pop return stack and push value to calculation stack"),
            ("r@", Self::f_r_get, "r@ ( -- n ) Push a copy of the top of the return stack"),
            ("i", Self::f_i, "i ( -- n ) Push the index of the innermost loop"),
            ("j", Self::f_j, "j ( -- n ) Push the index of the next outer loop"),
            ("rdrop", Self::f_rdrop, "rdrop ( -- ) ( R: x -- ) Discard the top of the return stack"),
            ("2>r", Self::f_2_to_r, "2>r ( x1 x2 -- ) ( R: -- x1 x2 ) Move a pair to the return stack"),
            ("2r>", Self::f_2_r_from, "2r> ( -- x1 x2 ) ( R: x1 x2 -- ) Move a pair from the return stack"),
            ("2r@", Self::f_2_r_get, "2r@ ( -- x1 x2 ) ( R: x1 x2 -- x1 x2 ) Copy a pair from the return stack"),
            ("2rdrop", Self::f_2rdrop, "2rdrop ( -- ) ( R: x1 x2 -- ) Discard a pair from the return stack"),
            ("unloop", Self::f_unloop, "unloop ( -- ) Discard the innermost loop's parameters, e.g. before EXIT inside a loop"),
            // memory
            ("@", Self::f_get, "@ ( a -- v ) Push the value at heap address a"),
            ("!", Self::f_store, "! ( v a -- ) Store v at heap address a"),
            ("c@", Self::f_c_get, "c@ ( s -- c ) Push the byte at string address s"),
            ("c!", Self::f_c_store, "c! ( c s -- ) Store byte c at string address s"),
            ("cmove", Self::f_cmove, "cmove ( from to n -- ) Copy n bytes within string space"),
            (",", Self::f_comma, ", ( n -- ) Copy the top of the stack to the top of the dictionary"),
            ("allot", Self::f_allot, "allot ( n -- ) Reserve n cells of data space, or release them if n is negative"),
            // compiler and interpreter
            (":", Self::f_colon, ": <name> ( -- ) Start a new definition"),
            (":noname", Self::f_noname, ":noname ( -- xt ) Start a definition without a name; ; leaves its xt"),
            (";", Self::f_semicolon, "; ( -- ) End a definition, returning to interpret mode"),
            ("create", Self::f_create, "create <name> ( -- ) Create a name field in the dictionary"),
            ("execute", Self::f_execute, "execute ( xt -- ) Run the word whose execution token is on the stack"),
            ("eval", Self::f_eval, "eval ( -- ) Interpret the rest of the line in the TIB"),
            ("quit", Self::f_quit, "quit ( -- ) Outer interpreter that repeatedly reads input lines and runs them"),
            ("abort", Self::f_abort, "abort ( -- ) Stop, clear the stacks, and return to the prompt"),
            ("catch", Self::f_catch, "catch ( ... xt -- ... 0 | n ) Run xt, returning 0, or the throw code if it throws"),
            ("throw", Self::f_throw, "throw ( n -- ) If n is not 0, raise an error with throw code n"),
            ("(error)", Self::f_error, "(error) ( s -- ) Raise an error whose message is the counted string at s"),
            ("find", Self::f_find, "find ( s -- xt 1 | xt -1 | s 0 ) Search for the counted string at s: 1 if immediate, -1 if not"),
            ("number?", Self::f_number_q, "number? ( s -- n T | s F ) Convert the counted string at s to a number, using BASE"),
            ("(')", Self::f_tick_p, "(') <name> ( -- cfa ) Push the cfa of the next word in the input"),
            ("parse-to", Self::f_parse_to, "parse-to ( b c -- b u ) Copy the next c-delimited token from the TIB to b as a counted string"),
            ("query", Self::f_query, "query ( -- ) Read a line from the input into the TIB"),
            ("refill", Self::f_refill, "refill ( -- flag ) Read the next line of the current input into the TIB; false at its end"),
            ("accept", Self::f_accept, "accept ( b u1 -- b u2 ) Read a line of up to u1 characters into b"),
            ("include-file", Self::f_include_file, "include-file ( s -- b ) Interpret the file named by the counted string at s"),
            // terminal and host
            ("emit", Self::f_emit_p, "emit ( c -- ) Send byte c to the terminal"),
            ("flush", Self::f_flush, "flush ( -- ) Force pending output to appear on the terminal"),
            ("key", Self::f_key, "key ( -- c | 0 ) Get a character, or zero if none available"),
            ("key?", Self::f_key_q, "key? ( -- b ) True if a character is available"),
            ("bye", Self::f_bye, "bye ( -- ) Exit to the operating system"),
            ("ms", Self::f_ms, "ms ( n -- ) Sleep for n milliseconds"),
            ("now", Self::f_now, "now ( -- ) Start a timer"),
            ("micros", Self::f_micros, "micros ( -- n ) Microseconds since NOW was called"),
            ("millis", Self::f_millis, "millis ( -- n ) Milliseconds since NOW was called"),
            ("open-file", Self::f_open_file, "open-file ( s fam -- file-id ior ) Open the file named by the counted string at s"),
            ("close-file", Self::f_close_file, "close-file ( file-id -- ior ) Close a file"),
            ("read-line", Self::f_read_line, "read-line ( u file-id -- u2 flag ior ) Read a line of up to u characters into TMP"),
            ("write-line", Self::f_write_line, "write-line ( s u file-id -- ior ) Write u characters of the string at s, and a newline"),
            ("file-position", Self::f_file_position, "file-position ( file-id -- u ior ) The current position in the file"),
            ("file-size", Self::f_file_size, "file-size ( file-id -- u ior ) The size of the file in characters"),
            ("(system)", Self::f_system_p, "(system) ( s -- ) Run the shell command in the counted string at s"),
            ("raw-mode-on", Self::f_raw_mode_on, "raw-mode-on ( -- ) Enable raw terminal mode"),
            ("raw-mode-off", Self::f_raw_mode_off, "raw-mode-off ( -- ) Disable raw terminal mode"),
            ("raw-mode?", Self::f_raw_mode_q, "raw-mode? ( -- f ) True if in raw mode"),
            // engine introspection
            ("dbg", Self::f_dbg, "dbg ( n -- ) Set the message level: 0 errors, 1 warnings, 2 info, 3 debug"),
            ("debuglevel", Self::f_debuglevel, "debuglevel ( -- n ) The message level set by dbg"),
            ("builtin-name", Self::f_builtin_name, "builtin-name ( index -- s ) The name of a builtin, in TMP"),
            ("builtin-doc", Self::f_builtin_doc, "builtin-doc ( index -- s ) The documentation of a builtin, in TMP"),
        ];
        for (name, code, doc) in builtins {
            self.add_builtin(name, *code, doc);
        }
        // ; ends definitions, so it must run while compiling, and only then
        let semicolon = self.find_word(b";").expect("; is a builtin");
        let nfa = self.kernel.get(semicolon - 1) as usize | IMMEDIATE_FLAG | COMPILE_ONLY_FLAG;
        self.kernel.set(semicolon - 1, nfa as i64);
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
// TESTS
//

#[cfg(test)]
mod tests {
    use super::*;

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
        let name = runtime.kernel.get(code_ptr - 1) as usize;
        assert_eq!(runtime.kernel.counted_bytes(name), b"test");
        assert_eq!(runtime.find_word(b"test"), Some(code_ptr));
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
        let cfa = rt.kernel.get(addr) as usize & ADDRESS_MASK;
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
        assert_eq!(runtime.kernel.get_builtin(8).name, "<".to_string());
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
    fn test_f_abort() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();
        runtime.kernel.push(42);
        runtime.kernel.push_r(99);
        runtime.set_compile_mode(true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.f_abort()));
        runtime.recover(result.unwrap_err());
        assert_eq!(runtime.kernel.stack_len(), 0); // stack should be cleared
        assert_eq!(runtime.kernel.get_return_ptr(), crate::kernel::RET_START);
        assert!(!runtime.get_compile_mode());
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
    fn test_f_should_exit() {
        let mut runtime = ForthRuntime::new();
        runtime.cold_start();

        assert!(!runtime.should_exit());
    }
}
