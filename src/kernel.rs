//////////////////////////////////////////////////////////////
/// Forth Interpreter Kernel
/// 
/// This module contains the core data structures and functions for the Forth interpreter.
/// The intention is that this module handles lower level functions directly related to the data structures.
/// 
/// Specifically it manages the main data area (heap) in an FK (Forth Kernel) struct.
/// 


use crate::internals::builtin::BuiltInFn;

// DATA AREA constants
pub const DATA_SIZE: usize    = 10000;
pub const STRING_SIZE: usize  = 5000;
pub const BUF_SIZE: usize     = 132;
pub const ALLOC_START: usize  = DATA_SIZE / 2;
pub const STACK_START: usize  = ALLOC_START - 1; // stack counts up
pub const RET_START: usize    = DATA_SIZE - 1; // return stack counts downwards
pub const WORD_START: usize   = 0; // data area counts up from the bottom (builtins, words, variables etc.)
pub const ADDRESS_MASK: usize   = 0x00FFFFFFFFFFFFFF;  // to get rid of flags

/// The primary data structure for the Forth engine
///
///     Forth's main data structure is a fixed array of integers (overloaded with characters and unsigned values).
///     This holds all the program data - words, variables, constants, stack etc. used by Forth
///     Strings are kept in a separate array, which is simpler than packing ASCII characters into 64 bit words
///     The Rust side of the engine keeps track of some variables with names following a *_ptr pattern.
///     This allows these values to be easily used by both Rust and Forth.
///     A small reader module manages input from files and stdin. Unfortunatly there is no easy way to provide
///     unbuffered keystroke input without a special library.
///     A simple messaging system provides warnings and errors. Ultimately these should be restricted to Rust error conditions,
///     while Forth should use its own methods to display and process errors and warnings.
///
//#[derive(Debug)]
pub struct Kernel {
    heap: [i64; DATA_SIZE],
    pub strings: [char; STRING_SIZE], // storage for strings
    pub builtins: Vec<BuiltInFn>,     // the dictionary of builtins
    pub stack_ptr: usize,             // top of the linear space stack
    pub return_ptr: usize,            // top of the return stack
    pub string_ptr: usize,            // pointer to the next free string space
 
    //pub return_stack: Vec<i64>,     // for do loops etc.
}


impl Kernel {
    pub fn new() -> Kernel {
        let kernel = Kernel {
            heap: [0; DATA_SIZE],
            strings: [' '; STRING_SIZE],
            builtins: Vec::new(),
            stack_ptr: STACK_START,
            return_ptr: RET_START,
            string_ptr: 0,

        };
        kernel
    }

    /// reset() clears the stacks.
    /// 
    pub fn reset(&mut self) {
        // Reset the stack pointers
        self.stack_ptr = STACK_START;
        self.return_ptr = RET_START;
    }


    /// get returns the value of a cell on the heap using its address
    ///      This is used to access variables, constants, and other data stored in the heap.
    ///
    pub fn get(&mut self, addr: usize) -> i64 {
        self.heap[addr]
    }

    /// set_var stores a new value to a cell on the heap using its address
    ///     This is used to set variables, constants, and other data stored in the heap.
    /// 
    pub fn set(&mut self, addr: usize, val: i64) {
        self.heap[addr] = val;
    }
    /// incr and decr are used to increment or decrement a cell on the heap
    pub fn incr(&mut self, addr: usize) {
        self.heap[addr] += 1;
    }

    pub fn decr(&mut self, addr: usize) {
        self.heap[addr] -= 1;
    }

    pub fn delta(&mut self, addr: usize, delta: i64) {
        self.heap[addr] += delta;
    }

    #[inline(always)]
    pub fn top(&mut self) -> i64 {
        self.heap[self.stack_ptr]
    }

     #[inline(always)]
    pub fn push(&mut self, val: i64) {
        self.stack_ptr -= 1;
        self.heap[self.stack_ptr] = val;
    }

    #[inline(always)]
    pub fn pop(&mut self) -> i64 {
        let r = self.heap[self.stack_ptr];
        self.stack_ptr += 1;
        r
    }

        #[inline(always)]
    pub fn stack_check(&self, needed: usize, word: &str) -> bool{
        if self.stack_ptr < needed {
            panic!("{}: Stack underflow: need {}, have {}", word, needed, self.stack_ptr);
        }
        true
    }

    /* #[inline(always)]
    pub fn push_r(&mut self, val: i64) {
        self.heap[self.return_ptr] = val;
        self.return_ptr += 1;
    }

    #[inline(always)]
    pub fn pop_r(&mut self) -> i64 {
        self.return_ptr -= 1;
        self.heap[self.return_ptr]
    }

   #[inline(always)]
    pub fn stack_check_r(&self, needed: usize, word: &str) -> bool{
        if self.return_ptr < needed {
            panic!("{}: Return stack underflow: need {}, have {}", word, needed, self.return_ptr);
        }
        true
    } */

    pub fn pop2_push1<F>(&mut self, word: &str, f: F)
    where
        F: Fn(i64, i64) -> i64,
    {
        if self.stack_check(2, "pop2_push1") {
            let j = self.pop();
            let k = self.pop();
            self.push(f(k, j));
        }
        else {
            panic!("{}: Stack underflow in pop2_push1", word);
        }
    }

    pub fn pop1_push1<F>(&mut self, word: &str, f: F)
    where
        F: Fn(i64) -> i64,
    {
        if self.stack_check(1, word) {
            let x = self.pop();
            self.push(f(x) as i64);
        }
    }

    /// new_string writes a new string into the next empty space, updating the free space pointer
    pub fn new_string(&mut self, string: &str) -> usize {
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

    /// copy a string slice into string space
    ///    
    pub fn save_string(&mut self, from: &str, to: usize) {
        self.strings[to] = from.len() as u8 as char; // count byte
        for (i, c) in from.chars().enumerate() {
            self.strings[to + i + 1] = c;
        }
    }

        /// u_get_string returns a string from a Forth string address
    ///     Assumes the source string is counted (i.e. has its length in the first byte)
    ///
    pub fn get_string(&mut self, addr: usize) -> String {
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
    pub fn set_string(&mut self, addr: usize, string: &str) {
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
    pub fn str_copy(&mut self, from: usize, to: usize, length: usize, counted: bool) {
        self.strings[to] = length as u8 as char; // write count byte
        let offset = if counted { 1 } else { 0 };
        for i in 0..length {
            self.strings[to + i + 1] = self.strings[from + i + offset];
        }
    }

    /// Compare two Forth (counted) strings
    /// First byte is the length, so we'll bail quickly if they don't match
    ///
    pub fn str_equal(&mut self, s_addr1: usize, s_addr2: usize) -> bool {
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

    pub fn add_builtin(&mut self, builtin: BuiltInFn) -> usize {
        self.builtins.push(builtin);
        self.builtins.len() - 1
    }

    pub fn get_builtin(&self, index: usize) -> &BuiltInFn {
        &self.builtins[index]
    }
}
