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
pub const DATA_SIZE: usize = 10000;
pub const STRING_SIZE: usize = 10000;
pub const BUF_SIZE: usize = 132;
pub const ALLOC_START: usize = DATA_SIZE / 2;
pub const STACK_START: usize = ALLOC_START - 1; // stack counts up
pub const RET_START: usize = DATA_SIZE - 1; // return stack counts downwards
pub const WORD_START: usize = 0; // data area counts up from the bottom (builtins, words, variables etc.)
pub const ADDRESS_MASK: usize = 0x00FFFFFFFFFFFFFF; // to get rid of flags

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
    strings: [u8; STRING_SIZE], // storage for strings
    builtins: Vec<BuiltInFn>,   // the dictionary of builtins
    stack_ptr: usize,           // top of the linear space stack
    return_ptr: usize,          // top of the return stack
    string_ptr: usize,          // pointer to the next free string space
                                //pub return_stack: Vec<i64>,     // for do loops etc.
}

impl Kernel {
    pub fn new() -> Kernel {
        let kernel = Kernel {
            heap: [0; DATA_SIZE],
            strings: [b' '; STRING_SIZE],
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

    /// delta adds a delta value to a cell on the heap
    pub fn delta(&mut self, addr: usize, delta: i64) {
        self.heap[addr] += delta;
    }

    /// Safe stack accessors
    #[inline(always)]
    pub fn push(&mut self, val: i64) {
        if self.stack_ptr <= 0 {
            panic!(
                "Stack corruption detected: cannot push, stack_ptr ({}) <= 0",
                self.stack_ptr
            );
        }
        self.stack_ptr -= 1;
        self.heap[self.stack_ptr] = val;
    }

    #[inline(always)]
    pub fn pop(&mut self) -> i64 {
        if self.stack_ptr >= STACK_START {
            panic!(
                "Stack corruption detected: cannot pop, stack_ptr ({}) >= STACK_START ({})",
                self.stack_ptr, STACK_START
            );
        }
        let r = self.heap[self.stack_ptr];
        self.stack_ptr += 1;
        r
    }

    #[inline(always)]
    pub fn top(&self) -> i64 {
        self.heap[self.stack_ptr]
    }

    #[inline(always)]
    pub fn peek(&self, n: usize) -> i64 {
        self.heap[self.stack_ptr + n]
    }

    // #[inline(always)]
    // pub fn set_top(&mut self, val: i64) {
    //     self.heap[self.stack_ptr] = val;
    // }

    #[inline(always)]
    pub fn stack_len(&self) -> usize {
        if self.stack_ptr > STACK_START {
            panic!(
                "Stack corruption detected: stack_ptr ({}) > STACK_START ({})",
                self.stack_ptr, STACK_START
            );
        }
        STACK_START - self.stack_ptr
    }

    /// stack_check checks if there are enough items on the stack for an operation
    #[inline(always)]
    pub fn stack_check(&self, needed: usize, word: &str) -> bool {
        let available = STACK_START - self.stack_ptr;
        if available < needed {
            panic!(
                "{}: Stack underflow: need {}, have {}",
                word, needed, available
            );
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
        } else {
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

    /// Returns the number of builtins
    pub fn max_builtin(&mut self) -> usize {
        self.builtins.len() - 1
    }

    /// string_new writes a new string into the next empty space, updating the free space pointer
    /// /// This function assumes that the string is counted, i.e. the first byte is the length of the string.
    /// /// Returns the address of the new string in the string space.
    ///
    pub fn string_new(&mut self, string: &str) -> usize {
        // place a new str into string space and update the free pointer string_ptr
        let mut ptr = self.heap[self.string_ptr] as usize;
        let result_ptr = ptr;
        self.strings[ptr] = string.len() as u8;
        ptr += 1;
        for (i, c) in string.chars().enumerate() {
            self.strings[ptr + i] = c as u8;
        }
        self.heap[self.string_ptr] = (ptr + string.len()) as i64;
        result_ptr
    }

    /// copy a string slice into string space adding a count byte
    ///
    pub fn string_save(&mut self, from: &str, to: usize) {
        self.strings[to] = from.len() as u8; // count byte
        for (i, c) in from.chars().enumerate() {
            self.strings[to + i + 1] = c as u8;
        }
    }

    /// string_get returns a string from a Forth string address
    /// Assumes the source string is counted (i.e. has its length in the first byte)
    ///
    pub fn string_get(&mut self, addr: usize) -> String {
        let str_addr = (addr & ADDRESS_MASK) + 1; //
        let last = str_addr + self.strings[addr] as usize;
        let mut result = String::new();
        for i in str_addr..last {
            result.push(self.strings[i] as char);
        }
        result
    }

    /// string_set saves a counted string to a Forth string address
    ///
    pub fn string_set(&mut self, addr: usize, string: &str) {
        let str_addr = addr & ADDRESS_MASK;
        self.strings[str_addr] = string.len() as u8; // count byte
        for (i, c) in string.chars().enumerate() {
            self.strings[str_addr + i + 1] = c as u8;
        }
    }

    /// copy a string from a text buffer in string space to a counted string
    ///     Typically used to copy to PAD from TIB
    ///     Can work with source strings counted or uncounted
    ///
    pub fn string_copy(&mut self, from: usize, to: usize, length: usize, counted: bool) {
        self.strings[to] = length as u8; // write count byte
        let offset = if counted { 1 } else { 0 };
        for i in 0..length {
            self.strings[to + i + 1] = self.strings[from + i + offset];
        }
    }

    /// Compare two Forth (counted) strings
    /// First byte is the length, so we'll bail quickly if they don't match
    ///
    pub fn string_equal(&mut self, s_addr1: usize, s_addr2: usize) -> bool {
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

    /// Return the length of a counted string
    /// This is the first byte of the string, so it is very fast
    ///
    pub fn string_length(&self, addr: usize) -> usize {
        self.strings[addr] as usize
    }

    /// Get a read-only string slice. Assumes a non-counted string.
    /// Used for detailed parsing of strings
    ///
    pub fn string_slice(&self, addr: usize, len: usize) -> &[u8] {
        &self.strings[addr..addr + len]
    }

    /// byte_get returns a byte from a string address
    /// /// This is used to access individual characters in a string.
    ///
    pub fn byte_get(&self, addr: usize) -> u8 {
        if addr >= STRING_SIZE {
            panic!("byte_get: index out of bounds");
        }
        self.strings[addr]
    }

    /// byte_set sets a byte in a string address
    /// /// This is used to modify individual characters in string space.
    ///
    pub fn byte_set(&mut self, addr: usize, value: u8) {
        if addr >= STRING_SIZE {
            panic!("byte_set: index out of bounds");
        }
        self.strings[addr] = value;
    }

    /// add_builtin adds a new builtin function to the kernel's list
    /// /// Returns the index of the new builtin in the list
    ///
    pub fn add_builtin(&mut self, builtin: BuiltInFn) -> usize {
        self.builtins.push(builtin);
        self.builtins.len() - 1
    }

    /// get_builtin returns a reference to a builtin function by its index
    ///
    pub fn get_builtin(&self, index: usize) -> &BuiltInFn {
        &self.builtins[index]
    }

    pub fn get_return_ptr(&self) -> usize {
        self.return_ptr
    }
    pub fn set_return_ptr(&mut self, val: usize) {
        self.return_ptr = val;
    }
    pub fn get_string_ptr(&self) -> usize {
        self.string_ptr
    }
    pub fn set_string_ptr(&mut self, val: usize) {
        self.string_ptr = val;
    }

    pub fn get_stack_ptr(&self) -> usize {
        self.stack_ptr
    }
}

//////////////////////////////////////////////
/// TESTS
///
#[cfg(test)]
mod tests {
    use super::*;

    fn kernel_with_string_ptr(start: usize) -> Kernel {
        let mut k = Kernel::new();
        k.heap[k.string_ptr] = start as i64;
        k
    }

    #[test]
    fn test_string_new_and_get() {
        let mut k = kernel_with_string_ptr(100);
        let addr = k.string_new("hello");
        assert_eq!(addr, 100);
        assert_eq!(k.string_get(addr), "hello");
    }

    #[test]
    fn test_string_save_and_get() {
        let mut k = Kernel::new();
        let addr = 200;
        k.string_save("world", addr);
        assert_eq!(k.string_get(addr), "world");
    }

    #[test]
    fn test_string_set_and_get() {
        let mut k = Kernel::new();
        let addr = 300;
        k.string_set(addr, "rust");
        assert_eq!(k.string_get(addr), "rust");
    }

    #[test]
    fn test_string_copy_counted() {
        let mut k = Kernel::new();
        k.string_save("forth", 50); // counted string at 50
        k.string_copy(50, 60, 5, true); // copy to 60, counted
        assert_eq!(k.string_get(60), "forth");
    }

    #[test]
    fn test_string_copy_uncounted() {
        let mut k = Kernel::new();
        k.string_set(100, "abcde");
        k.string_copy(101, 200, 5, false); // copy raw content (skip count)
        assert_eq!(k.string_get(200), "abcde");
    }

    #[test]
    fn test_string_equal_matches() {
        let mut k = Kernel::new();
        k.string_save("match", 10);
        k.string_save("match", 30);
        assert!(k.string_equal(10, 30));
    }

    #[test]
    fn test_string_equal_mismatch() {
        let mut k = Kernel::new();
        k.string_save("abc", 10);
        k.string_save("xyz", 20);
        assert!(!k.string_equal(10, 20));
    }

    #[test]
    fn test_string_length() {
        let mut k = Kernel::new();
        k.string_save("short", 400);
        assert_eq!(k.string_length(400), 5);
    }

    #[test]
    fn test_byte_get_and_set() {
        let mut k = Kernel::new();
        k.byte_set(500, b'X');
        assert_eq!(k.byte_get(500), b'X');
    }

    #[test]
    #[should_panic(expected = "byte_get: index out of bounds")]
    fn test_byte_get_oob_panics() {
        let k = Kernel::new();
        let _ = k.byte_get(STRING_SIZE); // out of bounds
    }

    #[test]
    #[should_panic(expected = "byte_set: index out of bounds")]
    fn test_byte_set_oob_panics() {
        let mut k = Kernel::new();
        k.byte_set(STRING_SIZE + 1, b'!');
    }

    #[test]
    fn test_string_slice() {
        let mut k = Kernel::new();
        k.string_save("abcdef", 600);
        let slice = k.string_slice(601, 3); // skip count byte
        assert_eq!(slice, b"abc");
    }
}
