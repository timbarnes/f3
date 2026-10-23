// General-purpose builtin words

use crate::internals::builtin::BuiltInFn;
use crate::kernel::{
    throw_code, CELL, DICTIONARY_FULL, DICT_LIMIT, DIVISION_BY_ZERO, INVALID_ADDRESS, OUT_OF_RANGE,
};
use crate::runtime::ForthRuntime;
use std::thread;
use std::time::{Duration, Instant};

impl ForthRuntime {
    /// Basic Forth operations on the stack.
    ///
    pub fn f_plus(&mut self) {
        self.kernel.pop2_push1("+", |a, b| a.wrapping_add(b));
    }

    // Arithmetic wraps on overflow, as Forth expects, and behaves the same in debug and release builds.
    pub fn f_minus(&mut self) {
        self.kernel.pop2_push1("-", |a, b| a.wrapping_sub(b));
    }

    pub fn f_times(&mut self) {
        self.kernel.pop2_push1("*", |a, b| a.wrapping_mul(b));
    }

    pub fn f_divide(&mut self) {
        self.kernel.pop2_push1("/", |a, b| match b {
            0 => throw_code(DIVISION_BY_ZERO, "division by zero"),
            _ => a.wrapping_div(b),
        });
    }

    pub fn f_lshift(&mut self) {
        self.kernel.pop2_push1(
            "lshift",
            |a, b| if (0..64).contains(&b) { a << b } else { 0 },
        );
    }

    pub fn f_rshift(&mut self) {
        // logical shift, as ANS requires: zeros come in from the left
        self.kernel.pop2_push1("rshift", |a, b| {
            if (0..64).contains(&b) {
                ((a as u64) >> b) as i64
            } else {
                0
            }
        });
    }

    /// 2/ ( n -- n/2 ) arithmetic shift right by one, keeping the sign
    pub fn f_2_slash(&mut self) {
        self.kernel.pop1_push1("2/", |a| a >> 1);
    }

    // Double-cell numbers: the low cell is below the high cell, which is on top.
    fn pop_double(&mut self, word: &str) -> i128 {
        self.kernel.stack_check(2, word);
        let high = self.kernel.pop();
        let low = self.kernel.pop();
        ((high as i128) << 64) | (low as u64 as i128)
    }

    fn push_double(&mut self, d: i128) {
        self.kernel.push(d as i64);
        self.kernel.push((d >> 64) as i64);
    }

    /// um* ( u1 u2 -- ud ) unsigned multiply to a double-cell result
    pub fn f_um_star(&mut self) {
        self.kernel.stack_check(2, "um*");
        let b = self.kernel.pop() as u64 as u128;
        let a = self.kernel.pop() as u64 as u128;
        self.push_double((a * b) as i128);
    }

    /// m* ( n1 n2 -- d ) signed multiply to a double-cell result
    pub fn f_m_star(&mut self) {
        self.kernel.stack_check(2, "m*");
        let b = self.kernel.pop() as i128;
        let a = self.kernel.pop() as i128;
        self.push_double(a * b);
    }

    /// um/mod ( ud u -- urem uquot ) unsigned divide of a double-cell number
    pub fn f_um_slash_mod(&mut self) {
        self.kernel.stack_check(3, "um/mod");
        let divisor = self.kernel.pop() as u64 as u128;
        let dividend = self.pop_double("um/mod") as u128;
        if divisor == 0 {
            throw_code(DIVISION_BY_ZERO, "division by zero");
        }
        let quot = dividend / divisor;
        if quot > u64::MAX as u128 {
            throw_code(OUT_OF_RANGE, "um/mod: result out of range");
        }
        self.kernel.push((dividend % divisor) as i64);
        self.kernel.push(quot as i64);
    }

    /// fm/mod ( d n -- rem quot ) floored division; sm/rem ( d n -- rem quot ) symmetric division
    pub fn f_fm_slash_mod(&mut self) {
        self.signed_divide("fm/mod", true);
    }

    pub fn f_sm_slash_rem(&mut self) {
        self.signed_divide("sm/rem", false);
    }

    fn signed_divide(&mut self, word: &str, floored: bool) {
        self.kernel.stack_check(3, word);
        let divisor = self.kernel.pop() as i128;
        let dividend = self.pop_double(word);
        if divisor == 0 {
            throw_code(DIVISION_BY_ZERO, "division by zero");
        }
        let (mut quot, mut rem) = (dividend / divisor, dividend % divisor);
        if floored && rem != 0 && (rem < 0) != (divisor < 0) {
            quot -= 1;
            rem += divisor;
        }
        if quot < i64::MIN as i128 || quot > i64::MAX as i128 {
            throw_code(OUT_OF_RANGE, format!("{word}: result out of range"));
        }
        self.kernel.push(rem as i64);
        self.kernel.push(quot as i64);
    }

    pub fn f_mod(&mut self) {
        self.kernel.pop2_push1("mod", |a, b| match b {
            0 => throw_code(DIVISION_BY_ZERO, "division by zero"),
            _ => a.wrapping_rem(b),
        });
    }

    pub fn f_less(&mut self) {
        self.kernel
            .pop2_push1("<", |a, b| if a < b { -1 } else { 0 });
    }

    pub fn f_equal(&mut self) {
        self.kernel
            .pop2_push1("=", |a, b| if a == b { -1 } else { 0 });
    }

    pub fn f_0equal(&mut self) {
        self.kernel
            .pop1_push1("0=", |a| if a == 0 { -1 } else { 0 });
    }

    pub fn f_0less(&mut self) {
        self.kernel.pop1_push1("0<", |a| if a < 0 { -1 } else { 0 });
    }

    // Stack primitives check the depth once, then work on the cells in place (nth 0 is the top).
    pub fn f_dup(&mut self) {
        self.kernel.stack_check(1, "dup");
        self.kernel.push(self.kernel.nth(0));
    }
    pub fn f_drop(&mut self) {
        self.kernel.stack_check(1, "drop");
        self.kernel.drop_cells(1);
    }
    pub fn f_swap(&mut self) {
        self.kernel.stack_check(2, "swap");
        let (a, b) = (self.kernel.nth(0), self.kernel.nth(1));
        self.kernel.set_nth(0, b);
        self.kernel.set_nth(1, a);
    }
    pub fn f_over(&mut self) {
        self.kernel.stack_check(2, "over");
        self.kernel.push(self.kernel.nth(1));
    }
    pub fn f_rot(&mut self) {
        // ( i j k -- j k i ): i is 2 deep, and goes to the top
        self.kernel.stack_check(3, "rot");
        let (k, j, i) = (self.kernel.nth(0), self.kernel.nth(1), self.kernel.nth(2));
        self.kernel.set_nth(2, j);
        self.kernel.set_nth(1, k);
        self.kernel.set_nth(0, i);
    }
    pub fn f_and(&mut self) {
        self.kernel.pop2_push1("and", |a, b| a & b);
    }

    pub fn f_xor(&mut self) {
        self.kernel.pop2_push1("xor", |a, b| a ^ b);
    }

    /// u< ( u1 u2 -- b ) unsigned comparison
    pub fn f_u_less(&mut self) {
        self.kernel
            .pop2_push1("u<", |a, b| if (a as u64) < (b as u64) { -1 } else { 0 });
    }

    /// sp@ ( -- a ) the address of the top of the data stack, before sp@ pushes it.
    ///     The data stack grows down, so the item n deep is at sp@ + n.
    pub fn f_sp_fetch(&mut self) {
        let sp = self.kernel.get_stack_ptr();
        self.kernel.push(sp as i64);
    }

    /// rp@ ( -- a ) the address of the top of the return stack (it also grows down)
    pub fn f_rp_fetch(&mut self) {
        let rp = self.kernel.get_return_ptr();
        self.kernel.push(rp as i64);
    }

    /// cmove ( from to n -- ) copies n bytes within string space
    pub fn f_cmove(&mut self) {
        self.kernel.stack_check(3, "cmove");
        let n = self.kernel.pop();
        let to = self.kernel.pop() as usize;
        let from = self.kernel.pop() as usize;
        if n > 0 {
            self.kernel.cmove(from, to, n as usize);
        }
    }

    pub fn f_or(&mut self) {
        self.kernel.pop2_push1("or", |a, b| a | b);
    }

    /// @ (get) ( a -- n ) loads the value at address a onto the stack
    pub fn f_get(&mut self) {
        if self.kernel.stack_check(1, "@") {
            let addr = self.kernel.pop() as usize;
            let val = self.kernel.get_checked(addr); // throws on a bad or misaligned address
            self.kernel.push(val);
        }
    }

    /// ! (store) ( n a -- ) stores n at address a. Generally used with variables
    ///
    pub fn f_store(&mut self) {
        if self.kernel.stack_check(2, "!") {
            let addr = self.kernel.pop() as usize;
            let value = self.kernel.pop();
            if addr == self.stepper_ptr {
                self.set_stepper(value); // keeps the tracing flag in step
            } else {
                self.kernel.set_checked(addr, value); // throws on a bad or misaligned address
            }
        }
    }

    /// >r ( n -- ) Pops the stack, placing the value on the return stack
    ///
    pub fn f_to_r(&mut self) {
        if self.kernel.stack_check(1, ">r") {
            let val = self.kernel.pop();
            self.kernel.push_r(val);
        }
    }

    /// r> ( -- n ) Pops the return stack, pushing the value to the calculation stack
    ///
    pub fn f_r_from(&mut self) {
        if self.kernel.stack_check_r(1, "r>") {
            let val = self.kernel.pop_r();
            self.kernel.push(val);
        }
    }

    /// rdrop 2>r 2r> 2r@ 2rdrop: return-stack words. They must be builtins: a colon definition
    ///     would find its own return address on top of the return stack.
    pub fn f_rdrop(&mut self) {
        self.kernel.stack_check_r(1, "rdrop");
        self.kernel.pop_r();
    }

    pub fn f_2_to_r(&mut self) {
        self.kernel.stack_check(2, "2>r");
        let x2 = self.kernel.pop();
        let x1 = self.kernel.pop();
        self.kernel.push_r(x1);
        self.kernel.push_r(x2);
    }

    pub fn f_2_r_from(&mut self) {
        self.kernel.stack_check_r(2, "2r>");
        let x2 = self.kernel.pop_r();
        let x1 = self.kernel.pop_r();
        self.kernel.push(x1);
        self.kernel.push(x2);
    }

    pub fn f_2_r_get(&mut self) {
        self.kernel.stack_check_r(2, "2r@");
        let rp = self.kernel.get_return_ptr();
        let (x2, x1) = (self.kernel.get(rp), self.kernel.get(rp + CELL));
        self.kernel.push(x1);
        self.kernel.push(x2);
    }

    pub fn f_2rdrop(&mut self) {
        self.kernel.stack_check_r(2, "2rdrop");
        self.kernel.pop_r();
        self.kernel.pop_r();
    }

    /// r@ ( -- n ) Gets the top value from the return stack, pushing the value to the calculation stack
    ///
    pub fn f_r_get(&mut self) {
        let val = self.kernel.get(self.kernel.get_return_ptr());
        self.kernel.push(val);
    }

    /// i ( -- n ) Pushes the current loop index to the calculation stack
    ///     Loops keep three cells on the return stack: exit address, limit, index (on top)
    ///
    pub fn f_i(&mut self) {
        self.kernel.stack_check_r(3, "i");
        let val = self.kernel.get(self.kernel.get_return_ptr());
        self.kernel.push(val);
    }

    /// j ( -- n ) Pushes the second level (outer) loop index to the calculation stack
    ///
    pub fn f_j(&mut self) {
        self.kernel.stack_check_r(6, "j");
        let val = self.kernel.get(self.kernel.get_return_ptr() + 3 * CELL);
        self.kernel.push(val);
    }

    /// allot ( n -- ) reserves n address units of data space at HERE, or releases them if n is
    ///     negative. Words start with their own link field, so data can simply be appended:
    ///     `variable buf 10 cells allot` gives buf 11 contiguous cells.
    ///
    pub fn f_allot(&mut self) {
        self.kernel.stack_check(1, "allot");
        let n = self.kernel.pop();
        let old = self.kernel.get(self.here_ptr);
        let here = old.wrapping_add(n);
        if here < 1 || here as usize > DICT_LIMIT {
            if n > 0 {
                throw_code(DICTIONARY_FULL, "dictionary full");
            }
            throw_code(INVALID_ADDRESS, "allot: below the start of memory");
        }
        self.kernel.set(self.here_ptr, here);
    }

    /// c@ - ( s -- c ) read a character from a string address and place on the stack
    ///
    pub fn f_c_get(&mut self) {
        if self.kernel.stack_check(1, "c@") {
            let s_address = self.kernel.pop() as usize;
            let c = self.kernel.byte_get(s_address);
            self.kernel.push(c as i64);
        }
    }

    /// c! - ( c s -- ) write a character to the string-space address on the stack
    ///
    pub fn f_c_store(&mut self) {
        if self.kernel.stack_check(2, "c!") {
            let s_address = self.kernel.pop() as usize;
            let c = self.kernel.pop() as u8;
            self.kernel.byte_set(s_address, c);
        }
    }

    /// f_now ( -- ) Start a timer
    pub fn f_now(&mut self) {
        self.timer = Instant::now();
    }

    /// micros ( -- n ) returns the number of microseconds since NOW was called
    pub fn f_micros(&mut self) {
        let duration = self.timer.elapsed();
        self.kernel.push(duration.as_micros() as i64);
    }

    /// millis ( -- n ) returns the number of milliseconds since NOW was called
    pub fn f_millis(&mut self) {
        let duration = self.timer.elapsed();
        self.kernel.push(duration.as_millis() as i64);
    }

    /// ms ( ms -- ) Sleep for ms milliseconds
    pub fn f_ms(&mut self) {
        if self.kernel.stack_check(1, "sleep") {
            let delay = self.kernel.pop().max(0) as u64; // a negative delay would sleep "forever"
            let millis = Duration::from_millis(delay);
            thread::sleep(millis);
        }
    }
    /// builtin-name ( index -- s ) copies the name of a builtin to TMP
    pub fn f_builtin_name(&mut self) {
        self.builtin_text(|b| &b.name);
    }

    /// builtin-doc ( index -- s ) copies the documentation of a builtin to TMP (at most 255 characters)
    pub fn f_builtin_doc(&mut self) {
        self.builtin_text(|b| &b.doc);
    }

    fn builtin_text(&mut self, field: fn(&BuiltInFn) -> &String) {
        self.kernel.stack_check(1, "builtin-name");
        let index = self.kernel.pop() as usize;
        let text = field(self.kernel.get_builtin(index)).as_bytes();
        let text = text[..text.len().min(255)].to_vec();
        let destination = self.kernel.get(self.tmp_ptr) as usize;
        self.kernel.bytes_save(&text, destination);
        self.kernel.push(destination as i64);
    }
}
