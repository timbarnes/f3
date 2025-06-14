// General-purpose builtin words

use crate::kernel::{DATA_SIZE, STACK_START};
use crate::runtime::{ForthRuntime, FALSE, TRUE};
use std::time::{Instant, Duration};
use std::thread;


/// u_is_integer determines whether a string parses correctly as an integer
///
pub fn u_is_integer(s: &str) -> bool {
    s.parse::<i64>().is_ok()
}

impl ForthRuntime {
    /// Basic Forth operations on the stack.
    ///
    pub fn f_plus(&mut self) {
        if self.kernel.stack_check(2, "+") {
            let a = self.kernel.pop();
            let b = self.kernel.pop();
            self.kernel.push(a + b);
        };
    }

    pub fn f_minus(&mut self) {
        self.kernel.pop2_push1("-", |a, b| a - b);
    }

    pub fn f_times(&mut self) {
        self.kernel.pop2_push1("*", |a, b| a * b);
    }

    pub fn f_divide(&mut self) {
        self.kernel.pop2_push1("/", |a, b| a / b);
    }

    pub fn f_mod(&mut self) {
        self.kernel.pop2_push1("mod", |a, b| a % b);
    }

    pub fn f_less(&mut self) {
        self.kernel.pop2_push1("<", |a, b| if a < b { -1 } else { 0 });
    }

    pub fn f_true(&mut self) {
        self.kernel.push(TRUE);
    }

    pub fn f_false(&mut self) {
        self.kernel.push(FALSE);
    }

    pub fn f_equal(&mut self) {
        self.kernel.pop2_push1("=", |a, b| if a == b { -1 } else { 0 });
    }

    pub fn f_0equal(&mut self) {
        self.kernel.pop1_push1("0=", |a| if a == 0 { -1 } else { 0 });
    }

    pub fn f_0less(&mut self) {
        self.kernel.pop1_push1("0<", |a| if a < 0 { -1 } else { 0 });
    }

    pub fn f_dup(&mut self) {
        if self.kernel.stack_check(1, "dup") {
            let top = self.kernel.top();
            self.kernel.push(top);
        }
    }
    pub fn f_drop(&mut self) {
        if self.kernel.stack_check(1, "drop") {
            self.kernel.pop();
        }
    }
    pub fn f_swap(&mut self) {
        if self.kernel.stack_check(2, "swap") {
            let a = self.kernel.pop();
            let b = self.kernel.pop();
            self.kernel.push(a);
            self.kernel.push(b);
        }
    }
    pub fn f_over(&mut self) {
        if self.kernel.stack_check(2, "over") {
            let first = self.kernel.pop();
            let second = self.kernel.pop();
            self.kernel.push(second);
            self.kernel.push(first);
            self.kernel.push(second);
        }
    }
    pub fn f_rot(&mut self) {
        if self.kernel.stack_check(3, "rot") {
            let first = self.kernel.pop();
            let second = self.kernel.pop();
            let third = self.kernel.pop();
            self.kernel.push(second);
            self.kernel.push(first);
            self.kernel.push(third);
        }
    }
pub fn f_pick(&mut self) {
    if self.kernel.stack_check(1, "pick") {
        let n = self.kernel.pop() as usize;
        if self.kernel.stack_check(n, "pick") {
            let val = self.kernel.get(self.kernel.stack_ptr + n + 1);
            self.kernel.push(val);
        }
    }
}
pub fn f_roll(&mut self) {
    if self.kernel.stack_check(1, "roll") {
        let n = self.kernel.pop() as usize;
        if n == 0 { return }; // 0 roll is a no-op
        if self.kernel.stack_check(n + 1, "roll") {
            // save the nth value
            let new_top = self.kernel.get(self.kernel.stack_ptr + n);
            // iterate, moving elements down
            let mut i = self.kernel.stack_ptr + n - 1;
            while i >= self.kernel.stack_ptr {
                let val = self.kernel.get(i);
                self.kernel.set(i + 1, val);
                i -= 1;
            } 
            self.kernel.stack_ptr += 1; // because we removed an element
            self.kernel.push(new_top);
        }
    }
}
    pub fn f_and(&mut self) {
        if self.kernel.stack_check(2, "and") {
            let a = self.kernel.pop();
            let b = self.kernel.pop();
            self.kernel.push((a as usize & b as usize) as i64);
        }
    }

    pub fn f_or(&mut self) {
        if self.kernel.stack_check(2, "or") {
            let a = self.kernel.pop();
            let b = self.kernel.pop();
            self.kernel.push((a as usize | b as usize) as i64);
        }
    }

    /// @ (get) ( a -- n ) loads the value at address a onto the stack
    pub fn f_get(&mut self) {
        if self.kernel.stack_check(1, "@") {
            let addr = self.kernel.pop() as usize;
            if addr < DATA_SIZE {
                let val = self.kernel.get(addr);
                self.kernel.push(val);
            } else {
                self.msg.error("@", "Address out of range", Some(addr));
                self.f_abort();
            }
        }
    }

    /// ! (store) ( n a -- ) stores n at address a. Generally used with variables
    ///
    pub fn f_store(&mut self) {
        if self.kernel.stack_check(2, "!") {
            let addr = self.kernel.pop() as usize;
            let value = self.kernel.pop();
            if addr < DATA_SIZE {
                self.kernel.set(addr, value);
            } else {
                self.msg.error("@", "Address out of range", Some(addr));
                self.f_abort();
            }
        }
    }

    /// >r ( n -- ) Pops the stack, placing the value on the return stack
    ///
    pub fn f_to_r(&mut self) {
        if self.kernel.stack_check(1, ">r") {
            let val = self.kernel.pop();
            self.kernel.return_ptr -= 1;
            self.kernel.set(self.kernel.return_ptr, val);
        }
    }

    /// r> ( -- n ) Pops the return stack, pushing the value to the calculation stack
    ///
    pub fn f_r_from(&mut self) {
        let val = self.kernel.get(self.kernel.return_ptr);
        self.kernel.push(val);
        self.kernel.return_ptr += 1;
    }

    /// r@ ( -- n ) Gets the top value from the return stack, pushing the value to the calculation stack
    ///
    pub fn f_r_get(&mut self) {
        let val = self.kernel.get(self.kernel.return_ptr);
        self.kernel.push(val);
    }

    /// i ( -- n ) Pushes the current loop index to the calculation stack
    ///
    pub fn f_i(&mut self) {
        let val = self.kernel.get(self.kernel.return_ptr);
        self.kernel.push(val);
    }

    /// j ( -- n ) Pushes the second level (outer) loop index to the calculation stack
    ///
    pub fn f_j(&mut self) {
        let val = self.kernel.get(self.kernel.return_ptr + 1);
        self.kernel.push(val);
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

    /// s-copy (s-from s-to -- s-to ) copies a counted string from one address to another.
    /// /// This does NOT update the free space pointer - it's intended for use in pre-allocated buffers.
    /// /// The source string is expected to be a counted string, with the first byte being the length.
    pub fn f_s_copy(&mut self) {
        if self.kernel.stack_check(2, "s-copy") {
            let dest = self.kernel.pop() as usize;
            let source = self.kernel.pop() as usize;
            let length = self.kernel.byte_get(source) + 1; // +1 for the length byte
            self.kernel.string_copy(source, dest, length as usize, true);
        }
    }

    /// s-create ( s-from -- s-to ) copies a counted string into the next empty space, updating the free space pointer
    pub fn f_s_create(&mut self) {
        if self.kernel.stack_check(1, "s-create") {
            let source = self.kernel.pop() as usize;
            let length = self.kernel.byte_get(source) as usize;
            let dest = self.kernel.get(self.kernel.string_ptr);
            self.kernel.string_copy(source, dest as usize, length, true);
            self.kernel.delta(self.kernel.string_ptr, length as i64 + 1);
            self.kernel.push(dest); // pointer to the beginning of the new string
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
            let delay = self.kernel.pop() as u64;
            let millis = Duration::from_millis(delay);
            thread::sleep(millis);
        }
    }
    /// DEPTH - print the number of items on the stack
    ///
    pub fn f_stack_depth(&mut self) {
        let depth = STACK_START - self.kernel.stack_ptr;
        self.kernel.push(depth as i64);
    }

}
