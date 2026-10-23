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
//
//     0 ... HERE ->  DICT_LIMIT | <- data stack  STACK_START | <- return stack  RET_START
//
// Compiled code tells opcodes from word addresses by OPCODE_FLAG (see runtime.rs),
// so DATA_SIZE can grow freely up to the address mask.
pub const DATA_SIZE: usize = 65536;
pub const STRING_SIZE: usize = 10000;
pub const BUF_SIZE: usize = 256; // TIB, PAD, TMP. Counted strings hold at most 255 bytes.
pub const RET_SIZE: usize = 1024;
pub const STACK_SIZE: usize = 1024;
pub const RET_START: usize = DATA_SIZE - 1; // return stack counts downwards
pub const STACK_START: usize = RET_START - RET_SIZE; // data stack counts downwards
pub const DICT_LIMIT: usize = STACK_START - STACK_SIZE; // the dictionary grows up to here
pub const WORD_START: usize = 0; // data area counts up from the bottom (builtins, words, variables etc.)
pub const ADDRESS_MASK: usize = 0x00FFFFFFFFFFFFFF; // to get rid of flags

/// A Forth error. Raised by `throw`, caught by CATCH, or else once per input line by `quit`,
/// which reports it and resets the system (see ForthRuntime::recover).
/// `code` is an ANS throw code (-1 ABORT, -4 stack underflow, ...); -256 is any other engine error.
/// An empty message is silent (ABORT).
#[derive(Debug)]
pub struct ForthError {
    pub code: i64,
    pub msg: String,
}

// ANS throw codes used by the engine
pub const ABORT_CODE: i64 = -1;
pub const ABORT_QUOTE: i64 = -2;
pub const STACK_OVERFLOW: i64 = -3;
pub const STACK_UNDERFLOW: i64 = -4;
pub const RSTACK_OVERFLOW: i64 = -5;
pub const RSTACK_UNDERFLOW: i64 = -6;
pub const DICTIONARY_FULL: i64 = -8;
pub const INVALID_ADDRESS: i64 = -9;
pub const DIVISION_BY_ZERO: i64 = -10;
pub const UNDEFINED_WORD: i64 = -13;
pub const COMPILE_ONLY: i64 = -14;
pub const NAME_EXPECTED: i64 = -16;
pub const FILE_ERROR: i64 = -37;
pub const ENGINE_ERROR: i64 = -256;

// ponytail: errors unwind instead of returning Result: no cost on the hot path, no signature
// changes across ~100 builtins, and the same catch turns Rust panics (index, overflow) into
// recoverable errors too. Requires panic = "unwind" (the default) in every profile.
pub fn throw(msg: impl Into<String>) -> ! {
    throw_code(ENGINE_ERROR, msg)
}

pub fn throw_code(code: i64, msg: impl Into<String>) -> ! {
    std::panic::panic_any(ForthError {
        code,
        msg: msg.into(),
    })
}

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
    heap: Box<[i64; DATA_SIZE]>, // a fixed size, so bounds checks compare against a constant
    strings: [u8; STRING_SIZE], // storage for strings
    builtins: Vec<BuiltInFn>,   // the dictionary of builtins
    stack_ptr: usize,           // top of the linear space stack
    return_ptr: usize,          // top of the return stack
    string_ptr: usize,          // pointer to the next free string space
}

impl Kernel {
    pub fn new() -> Kernel {
        Kernel {
            heap: vec![0; DATA_SIZE].into_boxed_slice().try_into().unwrap(),
            strings: [b' '; STRING_SIZE],
            builtins: Vec::new(),
            stack_ptr: STACK_START,
            return_ptr: RET_START,
            string_ptr: 0,
        }
    }

    /// reset() clears both stacks.
    ///
    pub fn reset(&mut self) {
        self.clear_stack();
        self.return_ptr = RET_START;
    }

    /// clear_stack() empties the data stack only.
    pub fn clear_stack(&mut self) {
        self.stack_ptr = STACK_START;
    }

    #[inline(always)]
    fn cell(&mut self, addr: usize) -> &mut i64 {
        match self.heap.get_mut(addr) {
            Some(cell) => cell,
            None => throw_code(INVALID_ADDRESS, format!("invalid address {}", addr as i64)),
        }
    }

    /// get returns the value of a cell on the heap using its address
    ///      This is used to access variables, constants, and other data stored in the heap.
    ///
    #[inline(always)]
    pub fn get(&self, addr: usize) -> i64 {
        match self.heap.get(addr) {
            Some(val) => *val,
            None => throw_code(INVALID_ADDRESS, format!("invalid address {}", addr as i64)),
        }
    }

    /// set_var stores a new value to a cell on the heap using its address
    ///     This is used to set variables, constants, and other data stored in the heap.
    ///
    pub fn set(&mut self, addr: usize, val: i64) {
        *self.cell(addr) = val;
    }
    /// incr and decr are used to increment or decrement a cell on the heap
    pub fn incr(&mut self, addr: usize) {
        self.delta(addr, 1);
    }

    #[inline(always)]
    pub fn decr(&mut self, addr: usize) {
        self.delta(addr, -1);
    }

    /// delta adds a delta value to a cell on the heap
    pub fn delta(&mut self, addr: usize, delta: i64) {
        let cell = self.cell(addr);
        *cell = cell.wrapping_add(delta);
    }

    /// Safe stack accessors
    #[inline(always)]
    pub fn push(&mut self, val: i64) {
        if self.stack_ptr <= DICT_LIMIT {
            throw_code(STACK_OVERFLOW, "stack overflow");
        }
        self.stack_ptr -= 1;
        self.heap[self.stack_ptr] = val;
    }

    #[inline(always)]
    pub fn pop(&mut self) -> i64 {
        if self.stack_ptr >= STACK_START {
            throw_code(STACK_UNDERFLOW, "stack underflow");
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
    pub fn stack_len(&self) -> usize {
        STACK_START - self.stack_ptr
    }

    /// stack_check checks if there are enough items on the stack for an operation
    #[inline(always)]
    pub fn stack_check(&self, needed: usize, word: &str) -> bool {
        if self.stack_len() < needed {
            throw_code(STACK_UNDERFLOW, format!("{word}: stack underflow"));
        }
        true
    }

    #[inline(always)]
    pub fn push_r(&mut self, val: i64) {
        if self.return_ptr <= STACK_START {
            throw_code(RSTACK_OVERFLOW, "return stack overflow");
        }
        self.return_ptr -= 1;
        self.heap[self.return_ptr] = val;
    }

    #[inline(always)]
    pub fn pop_r(&mut self) -> i64 {
        if self.return_ptr >= RET_START {
            throw_code(RSTACK_UNDERFLOW, "return stack underflow");
        }
        let val = self.heap[self.return_ptr];
        self.return_ptr += 1;
        val
    }

    #[inline(always)]
    pub fn stack_check_r(&self, needed: usize, word: &str) -> bool {
        if RET_START - self.return_ptr < needed {
            throw_code(RSTACK_UNDERFLOW, format!("{word}: return stack underflow"));
        }
        true
    }

    /// top returns the top n cells of the data stack, top first, after one depth check.
    ///     Primitives work on this slice directly instead of popping and pushing.
    #[inline(always)]
    pub fn top_cells(&mut self, n: usize, word: &str) -> &mut [i64] {
        self.stack_check(n, word);
        &mut self.heap[self.stack_ptr..self.stack_ptr + n]
    }

    /// drop_cells removes n cells from the data stack (the caller has checked the depth)
    #[inline(always)]
    pub fn drop_cells(&mut self, n: usize) {
        self.stack_ptr += n;
    }

    #[inline(always)]
    pub fn pop2_push1<F>(&mut self, word: &str, f: F)
    where
        F: Fn(i64, i64) -> i64,
    {
        let cells = self.top_cells(2, word);
        cells[1] = f(cells[1], cells[0]);
        self.drop_cells(1);
    }

    #[inline(always)]
    pub fn pop1_push1<F>(&mut self, word: &str, f: F)
    where
        F: Fn(i64) -> i64,
    {
        let cells = self.top_cells(1, word);
        cells[0] = f(cells[0]);
    }

    /// string_new writes a new string into the next empty space, updating the free space pointer
    /// /// This function assumes that the string is counted, i.e. the first byte is the length of the string.
    /// /// Returns the address of the new string in the string space.
    ///
    pub fn string_new(&mut self, string: &[u8]) -> usize {
        // place a new string into string space and update the free pointer string_ptr
        let ptr = self.get(self.string_ptr) as usize;
        self.bytes_save(string, ptr);
        self.set(self.string_ptr, (ptr + 1 + string.len()) as i64);
        ptr
    }

    /// copy bytes into string space as a counted string
    ///
    pub fn bytes_save(&mut self, from: &[u8], to: usize) {
        if from.len() > 255 {
            throw("string longer than 255 characters");
        }
        if to + 1 + from.len() > STRING_SIZE {
            throw_code(DICTIONARY_FULL, "string space full");
        }
        self.strings[to] = from.len() as u8; // count byte
        self.strings[to + 1..to + 1 + from.len()].copy_from_slice(from);
    }

    /// string_get returns a string from a Forth string address
    /// Assumes the source string is counted (i.e. has its length in the first byte)
    ///
    pub fn string_get(&self, addr: usize) -> String {
        let bytes = self.counted_bytes(addr & ADDRESS_MASK);
        bytes.iter().map(|&b| b as char).collect()
    }

    /// string_set saves a counted string to a Forth string address
    ///
    pub fn string_set(&mut self, addr: usize, string: &str) {
        self.bytes_save(string.as_bytes(), addr & ADDRESS_MASK);
    }

    /// copy a string from a text buffer in string space to a counted string
    ///     Typically used to copy to PAD from TIB
    ///     Can work with source strings counted or uncounted
    ///
    pub fn string_copy(&mut self, from: usize, to: usize, length: usize, counted: bool) {
        let offset = if counted { 1 } else { 0 };
        if length > 255 || to + 1 + length > STRING_SIZE || from + offset + length > STRING_SIZE {
            throw_code(INVALID_ADDRESS, "invalid string address or length");
        }
        self.strings[to] = length as u8; // write count byte
        for i in 0..length {
            self.strings[to + i + 1] = self.strings[from + i + offset];
        }
    }

    /// counted_bytes returns the characters of the counted string at addr (without the count byte)
    ///
    pub fn counted_bytes(&self, addr: usize) -> &[u8] {
        let len = self.byte_get(addr) as usize;
        self.string_slice(addr + 1, len)
    }

    /// cmove copies n bytes within string space (overlapping ranges are fine)
    ///
    pub fn cmove(&mut self, from: usize, to: usize, n: usize) {
        let fits = |a: usize| a.checked_add(n).is_some_and(|end| end <= STRING_SIZE);
        if !fits(from) || !fits(to) {
            throw_code(INVALID_ADDRESS, "cmove: invalid string address or length");
        }
        self.strings.copy_within(from..from + n, to);
    }

    /// Get a read-only string slice. Assumes a non-counted string.
    /// Used for detailed parsing of strings
    ///
    pub fn string_slice(&self, addr: usize, len: usize) -> &[u8] {
        match self.strings.get(addr..addr.saturating_add(len)) {
            Some(slice) => slice,
            None => throw_code(INVALID_ADDRESS, "invalid string address or length"),
        }
    }

    /// byte_get returns a byte from a string address
    /// /// This is used to access individual characters in a string.
    ///
    pub fn byte_get(&self, addr: usize) -> u8 {
        if addr >= STRING_SIZE {
            throw_code(INVALID_ADDRESS, format!("invalid string address {}", addr as i64));
        }
        self.strings[addr]
    }

    /// byte_set sets a byte in a string address
    /// /// This is used to modify individual characters in string space.
    ///
    pub fn byte_set(&mut self, addr: usize, value: u8) {
        if addr >= STRING_SIZE {
            throw_code(INVALID_ADDRESS, format!("invalid string address {}", addr as i64));
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
        match self.builtins.get(index) {
            Some(builtin) => builtin,
            None => throw(format!("invalid builtin index {}", index as i64)),
        }
    }

    pub fn get_return_ptr(&self) -> usize {
        self.return_ptr
    }
    /// set_return_ptr and set_stack_ptr restore depths saved earlier (CATCH)
    pub fn set_return_ptr(&mut self, rp: usize) {
        self.return_ptr = rp;
    }
    pub fn set_stack_ptr(&mut self, sp: usize) {
        self.stack_ptr = sp;
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
        let addr = k.string_new(b"hello");
        assert_eq!(addr, 100);
        assert_eq!(k.string_get(addr), "hello");
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
        k.string_set(50, "forth"); // counted string at 50
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
    fn test_counted_bytes_and_cmove() {
        let mut k = Kernel::new();
        k.string_set(10, "abc");
        assert_eq!(k.counted_bytes(10), b"abc");
        k.cmove(10, 20, 4); // count byte and characters
        assert_eq!(k.counted_bytes(20), b"abc");
    }

    #[test]
    fn test_byte_get_and_set() {
        let mut k = Kernel::new();
        k.byte_set(500, b'X');
        assert_eq!(k.byte_get(500), b'X');
    }

    #[test]
    #[should_panic]
    fn test_byte_get_oob_panics() {
        let k = Kernel::new();
        let _ = k.byte_get(STRING_SIZE); // out of bounds
    }

    #[test]
    #[should_panic]
    fn test_byte_set_oob_panics() {
        let mut k = Kernel::new();
        k.byte_set(STRING_SIZE + 1, b'!');
    }

    #[test]
    fn test_string_slice() {
        let mut k = Kernel::new();
        k.string_set(600, "abcdef");
        let slice = k.string_slice(601, 3); // skip count byte
        assert_eq!(slice, b"abc");
    }
}
