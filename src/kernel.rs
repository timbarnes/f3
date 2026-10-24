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
pub const CELL: usize = 8; // bytes per cell: the heap is byte-addressed, and cells are 8-byte aligned
pub const DATA_SIZE: usize = 1 << 20; // 1 MB
pub const BUF_SIZE: usize = 256; // TIB, PAD, TMP. Counted strings hold at most 255 bytes.
pub const RET_SIZE: usize = 1024;
pub const STACK_SIZE: usize = 1024;
pub const RET_START: usize = DATA_SIZE - CELL; // return stack counts downwards
pub const STACK_START: usize = RET_START - RET_SIZE * CELL; // data stack counts downwards
pub const BUFFERS: usize = STACK_START - STACK_SIZE * CELL - 4 * BUF_SIZE; // TIB PAD TMP, fixed
pub const TIB_START: usize = BUFFERS; // Text input buffer, used by readers
pub const PAD_START: usize = TIB_START + BUF_SIZE; // Scratchpad buffer, used by PARSE and friends
pub const TMP_START: usize = PAD_START + BUF_SIZE; // Temporary buffer, used for string input
pub const DICT_LIMIT: usize = BUFFERS; // the dictionary grows up to here
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
pub const ALIGNMENT: i64 = -23;
pub const DIVISION_BY_ZERO: i64 = -10;
pub const OUT_OF_RANGE: i64 = -11;
pub const UNDEFINED_WORD: i64 = -13;
pub const COMPILE_ONLY: i64 = -14;
pub const NAME_EXPECTED: i64 = -16;
pub const FILE_ERROR: i64 = -37;
pub const NO_SUCH_FILE: i64 = -38;
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
    heap: Box<[u8; DATA_SIZE]>, // all of memory, in bytes: a fixed size, so bounds checks compare against a constant
    builtins: Vec<BuiltInFn>,   // the dictionary of builtins
    stack_ptr: usize,           // top of the linear space stack
    return_ptr: usize,          // top of the return stack
}

impl Kernel {
    pub fn new() -> Kernel {
        Kernel {
            heap: vec![0; DATA_SIZE].into_boxed_slice().try_into().unwrap(),
            builtins: Vec::new(),
            stack_ptr: STACK_START,
            return_ptr: RET_START,
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

    // Cells are 8 bytes at aligned byte addresses. The engine's own accesses (get, set) derive
    // from aligned addresses, so only @ and ! (get_checked, set_checked) test alignment: that is
    // where a wrong address from Forth code arrives. A misaligned engine access would read the
    // cell containing the address: wrong, but still bounds-checked.
    // A cell is 8 bytes, little-endian.
    #[inline(always)]
    fn read(&self, addr: usize) -> i64 {
        if addr > DATA_SIZE - CELL {
            Self::bad_address(addr);
        }
        // the check above lets the compiler drop the slice's own bounds checks
        i64::from_le_bytes(self.heap[addr..addr + CELL].try_into().unwrap())
    }

    #[inline(always)]
    fn write(&mut self, addr: usize, val: i64) {
        if addr > DATA_SIZE - CELL {
            Self::bad_address(addr);
        }
        self.heap[addr..addr + CELL].copy_from_slice(&val.to_le_bytes());
    }

    #[cold]
    #[inline(never)]
    fn bad_address(addr: usize) -> ! {
        throw_code(INVALID_ADDRESS, format!("invalid address {}", addr as i64))
    }

    #[inline(always)]
    fn check_aligned(addr: usize) {
        if !addr.is_multiple_of(CELL) {
            Self::misaligned(addr);
        }
    }

    // The error paths are out of line, so the checks above stay a test and a branch.
    #[cold]
    #[inline(never)]
    fn misaligned(addr: usize) -> ! {
        throw_code(ALIGNMENT, format!("address {} is not cell-aligned", addr as i64))
    }

    /// get returns the value of a cell on the heap using its address
    ///      This is used to access variables, constants, and other data stored in the heap.
    ///
    #[inline(always)]
    pub fn get(&self, addr: usize) -> i64 {
        self.read(addr)
    }

    /// set_var stores a new value to a cell on the heap using its address
    ///     This is used to set variables, constants, and other data stored in the heap.
    ///
    #[inline(always)]
    pub fn set(&mut self, addr: usize, val: i64) {
        self.write(addr, val);
    }

    /// get_checked and set_checked are @ and !: they also raise -23 for a misaligned address
    ///
    pub fn get_checked(&self, addr: usize) -> i64 {
        Self::check_aligned(addr);
        self.read(addr)
    }

    pub fn set_checked(&mut self, addr: usize, val: i64) {
        Self::check_aligned(addr);
        self.write(addr, val);
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
        let val = self.get(addr).wrapping_add(delta);
        self.set(addr, val);
    }

    /// Safe stack accessors
    #[inline(always)]
    pub fn push(&mut self, val: i64) {
        if self.stack_ptr <= DICT_LIMIT {
            throw_code(STACK_OVERFLOW, "stack overflow");
        }
        self.stack_ptr -= CELL;
        self.write(self.stack_ptr, val);
    }

    #[inline(always)]
    pub fn pop(&mut self) -> i64 {
        if self.stack_ptr >= STACK_START {
            throw_code(STACK_UNDERFLOW, "stack underflow");
        }
        let r = self.read(self.stack_ptr);
        self.stack_ptr += CELL;
        r
    }

    #[inline(always)]
    pub fn stack_len(&self) -> usize {
        (STACK_START - self.stack_ptr) / CELL
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
        self.return_ptr -= CELL;
        self.write(self.return_ptr, val);
    }

    #[inline(always)]
    pub fn pop_r(&mut self) -> i64 {
        if self.return_ptr >= RET_START {
            throw_code(RSTACK_UNDERFLOW, "return stack underflow");
        }
        let val = self.read(self.return_ptr);
        self.return_ptr += CELL;
        val
    }

    #[inline(always)]
    pub fn stack_check_r(&self, needed: usize, word: &str) -> bool {
        if (RET_START - self.return_ptr) / CELL < needed {
            throw_code(RSTACK_UNDERFLOW, format!("{word}: return stack underflow"));
        }
        true
    }

    /// nth and set_nth read and write the cell n deep in the data stack (0 is the top).
    ///     Primitives check the depth once with stack_check, then work on the cells in place.
    #[inline(always)]
    pub fn nth(&self, n: usize) -> i64 {
        self.read(self.stack_ptr + n * CELL)
    }

    #[inline(always)]
    pub fn set_nth(&mut self, n: usize, val: i64) {
        self.write(self.stack_ptr + n * CELL, val);
    }

    /// drop_cells removes n cells from the data stack (the caller has checked the depth)
    #[inline(always)]
    pub fn drop_cells(&mut self, n: usize) {
        self.stack_ptr += n * CELL;
    }

    #[inline(always)]
    pub fn pop2_push1<F>(&mut self, word: &str, f: F)
    where
        F: Fn(i64, i64) -> i64,
    {
        self.stack_check(2, word);
        let result = f(self.nth(1), self.nth(0));
        self.set_nth(1, result);
        self.drop_cells(1);
    }

    #[inline(always)]
    pub fn pop1_push1<F>(&mut self, word: &str, f: F)
    where
        F: Fn(i64) -> i64,
    {
        self.stack_check(1, word);
        let result = f(self.nth(0));
        self.set_nth(0, result);
    }

    // Bytes and strings. Characters are bytes in the same memory as cells; counted strings
    // have a count byte, then the characters (at most 255).

    /// range returns addr..addr+len if it is inside memory, or throws
    ///
    fn range(addr: usize, len: usize) -> std::ops::Range<usize> {
        match addr.checked_add(len) {
            Some(end) if end <= DATA_SIZE => addr..end,
            _ => throw_code(INVALID_ADDRESS, format!("invalid address {} (length {len})", addr as i64)),
        }
    }

    /// bytes_save writes bytes at `to` as a counted string
    ///
    pub fn bytes_save(&mut self, from: &[u8], to: usize) {
        if from.len() > 255 {
            throw("string longer than 255 characters");
        }
        let r = Self::range(to, 1 + from.len());
        self.heap[r.start] = from.len() as u8; // count byte
        self.heap[r.start + 1..r.end].copy_from_slice(from);
    }

    /// bytes_write copies raw bytes (no count byte) to `to`
    ///
    pub fn bytes_write(&mut self, from: &[u8], to: usize) {
        let r = Self::range(to, from.len());
        self.heap[r].copy_from_slice(from);
    }

    /// string_get returns the counted string at addr as text (flags in the address are ignored)
    ///
    pub fn string_get(&self, addr: usize) -> String {
        let bytes = self.counted_bytes(addr & ADDRESS_MASK);
        bytes.iter().map(|&b| b as char).collect()
    }

    /// string_set saves a counted string at addr
    ///
    pub fn string_set(&mut self, addr: usize, string: &str) {
        self.bytes_save(string.as_bytes(), addr & ADDRESS_MASK);
    }

    /// string_copy copies length characters from `from` to a counted string at `to`.
    ///     Typically used to copy a token from the TIB to PAD.
    ///     With `counted`, the source is a counted string and its count byte is skipped.
    ///
    pub fn string_copy(&mut self, from: usize, to: usize, length: usize, counted: bool) {
        let offset = if counted { 1 } else { 0 };
        if length > 255 {
            throw("string longer than 255 characters");
        }
        let src = Self::range(from + offset, length);
        Self::range(to, 1 + length);
        self.heap.copy_within(src, to + 1);
        self.heap[to] = length as u8; // write count byte
    }

    /// counted_bytes returns the characters of the counted string at addr (without the count byte)
    ///
    pub fn counted_bytes(&self, addr: usize) -> &[u8] {
        let len = self.byte_get(addr) as usize;
        self.string_slice(addr + 1, len)
    }

    /// cmove copies n bytes (overlapping ranges are fine)
    ///
    pub fn cmove(&mut self, from: usize, to: usize, n: usize) {
        let src = Self::range(from, n);
        Self::range(to, n);
        self.heap.copy_within(src, to);
    }

    /// string_slice returns len bytes at addr
    ///
    pub fn string_slice(&self, addr: usize, len: usize) -> &[u8] {
        &self.heap[Self::range(addr, len)]
    }

    /// byte_get and byte_set read and write one byte (character)
    ///
    pub fn byte_get(&self, addr: usize) -> u8 {
        self.heap[Self::range(addr, 1).start]
    }

    pub fn byte_set(&mut self, addr: usize, value: u8) {
        self.heap[Self::range(addr, 1).start] = value;
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
        let _ = k.byte_get(DATA_SIZE); // out of bounds
    }

    #[test]
    #[should_panic]
    fn test_byte_set_oob_panics() {
        let mut k = Kernel::new();
        k.byte_set(DATA_SIZE + 1, b'!');
    }

    #[test]
    fn test_string_slice() {
        let mut k = Kernel::new();
        k.string_set(600, "abcdef");
        let slice = k.string_slice(601, 3); // skip count byte
        assert_eq!(slice, b"abc");
    }
}
