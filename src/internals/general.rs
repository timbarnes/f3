// General-purpose builtin words

use crate::kernel::{DATA_SIZE, FALSE, TF, TRUE};
use std::time::{Instant, Duration};
use std::thread;


/// u_is_integer determines whether a string parses correctly as an integer
///
pub fn u_is_integer(s: &str) -> bool {
    s.parse::<i64>().is_ok()
}

impl TF {
    /// Basic Forth operations on the stack.
    ///
    pub fn f_plus(&mut self) {
        if self.stack_check(2, "+") {
            let a = self.pop();
            let b = self.pop();
            self.push(a + b);
        };
    }

    pub fn f_minus(&mut self) {
        self.pop2_push1("-", |a, b| a - b);
    }

    pub fn f_times(&mut self) {
        self.pop2_push1("*", |a, b| a * b);
    }

    pub fn f_divide(&mut self) {
        self.pop2_push1("/", |a, b| a / b);
    }

    pub fn f_mod(&mut self) {
        self.pop2_push1("mod", |a, b| a % b);
    }

    pub fn f_less(&mut self) {
        self.pop2_push1("<", |a, b| if a < b { -1 } else { 0 });
    }

    pub fn f_true(&mut self) {
        self.push(TRUE);
    }

    pub fn f_false(&mut self) {
        self.push(FALSE);
    }

    pub fn f_equal(&mut self) {
        self.pop2_push1("=", |a, b| if a == b { -1 } else { 0 });
    }

    pub fn f_0equal(&mut self) {
        self.pop1_push1("0=", |a| if a == 0 { -1 } else { 0 });
    }

    pub fn f_0less(&mut self) {
        self.pop1_push1("0<", |a| if a < 0 { -1 } else { 0 });
    }

    pub fn f_dup(&mut self) {
        if self.stack_check(1, "dup") {
            let top = self.top();
            self.push(top);
        }
    }
    pub fn f_drop(&mut self) {
        if self.stack_check(1, "drop") {
            self.pop();
        }
    }
    pub fn f_swap(&mut self) {
        if self.stack_check(2, "swap") {
            let a = self.pop();
            let b = self.pop();
            self.push(a);
            self.push(b);
        }
    }
    pub fn f_over(&mut self) {
        if self.stack_check(2, "over") {
            let first = self.pop();
            let second = self.pop();
            self.push(second);
            self.push(first);
            self.push(second);
        }
    }
    pub fn f_rot(&mut self) {
        if self.stack_check(3, "rot") {
            let first = self.pop();
            let second = self.pop();
            let third = self.pop();
            self.push(second);
            self.push(first);
            self.push(third);
        }
    }
pub fn f_pick(&mut self) {
    if self.stack_check(1, "pick") {
        let n = self.pop() as usize;
        if self.stack_check(n, "pick") {
            self.push(self.heap[self.stack_ptr + n + 1]);
        }
    }
}
pub fn f_roll(&mut self) {
    if self.stack_check(1, "roll") {
        let n = self.pop() as usize;
        if n == 0 { return }; // 0 roll is a no-op
        if self.stack_check(n + 1, "roll") {
            // save the nth value
            let new_top = self.heap[self.stack_ptr + n];
            // iterate, moving elements down
            let mut i = self.stack_ptr + n - 1;
            while i >= self.stack_ptr {
                self.heap[i + 1] = self.heap[i];
                i -= 1;
            } 
            self.stack_ptr += 1; // because we removed an element
            self.push(new_top);
        }
    }
}
    pub fn f_and(&mut self) {
        if self.stack_check(2, "and") {
            let a = self.pop();
            let b = self.pop();
            self.push((a as usize & b as usize) as i64);
        }
    }

    pub fn f_or(&mut self) {
        if self.stack_check(2, "or") {
            let a = self.pop();
            let b = self.pop();
            self.push((a as usize | b as usize) as i64);
        }
    }

    /// @ (get) ( a -- n ) loads the value at address a onto the stack
    pub fn f_get(&mut self) {
        if self.stack_check(1, "@") {
            let addr = self.pop() as usize;
            if addr < DATA_SIZE {
                self.push(self.heap[addr]);
            } else {
                self.msg.error("@", "Address out of range", Some(addr));
                self.f_abort();
            }
        }
    }

    /// ! (store) ( n a -- ) stores n at address a. Generally used with variables
    ///
    pub fn f_store(&mut self) {
        if self.stack_check(2, "!") {
            let addr = self.pop() as usize;
            let value = self.pop();
            if addr < DATA_SIZE {
                self.heap[addr] = value;
            } else {
                self.msg.error("@", "Address out of range", Some(addr));
                self.f_abort();
            }
        }
    }

    /// >r ( n -- ) Pops the stack, placing the value on the return stack
    ///
    pub fn f_to_r(&mut self) {
        if self.stack_check(1, ">r") {
            let value = self.pop();
            self.return_ptr -= 1;
            self.heap[self.return_ptr] = value;
        }
    }

    /// r> ( -- n ) Pops the return stack, pushing the value to the calculation stack
    ///
    pub fn f_r_from(&mut self) {
        self.push(self.heap[self.return_ptr]);
        self.return_ptr += 1;
    }

    /// r@ ( -- n ) Gets the top value from the return stack, pushing the value to the calculation stack
    ///
    pub fn f_r_get(&mut self) {
        self.push(self.heap[self.return_ptr]);
    }

    /// i ( -- n ) Pushes the current loop index to the calculation stack
    ///
    pub fn f_i(&mut self) {
        self.push(self.heap[self.return_ptr]);
    }

    /// j ( -- n ) Pushes the second level (outer) loop index to the calculation stack
    ///
    pub fn f_j(&mut self) {
        self.push(self.heap[self.return_ptr + 1]);
    }

    /// c@ - ( s -- c ) read a character from a string and place on the stack
    ///
    pub fn f_c_get(&mut self) {
        if self.stack_check(1, "c@") {
            let s_address = self.pop() as usize;
            self.push(self.strings[s_address] as u8 as i64);
        }
    }

    /// c! - ( c s -- ) read a character from a string and place on the stack
    pub fn f_c_store(&mut self) {
        if self.stack_check(2, "c!") {
            let s_address = self.pop() as usize;
            self.strings[s_address] = self.pop() as u8 as char;
        }
    }

    /// s-copy (s-from s-to -- s-to )
    pub fn f_s_copy(&mut self) {
        if self.stack_check(2, "s-copy") {
            let dest = self.pop() as usize;
            let result_ptr = dest as i64;
            let source = self.pop() as usize;
            let length = self.strings[source] as u8 as usize + 1;
            let mut i = 0;
            while i < length {
                self.strings[dest + i] = self.strings[source + i];
                i += 1;
            }
            self.heap[self.string_ptr] += length as i64;
            self.push(result_ptr);
        }
    }
    /// s-create ( s-from -- s-to ) copies a counted string into the next empty space, updating the free space pointer
    pub fn f_s_create(&mut self) {
        if self.stack_check(1, "s-create") {
            let source = self.top() as usize;
            let length = self.strings[source] as usize;
            let dest = self.heap[self.string_ptr];
            self.push(dest); // destination
            self.f_s_copy();
            self.heap[self.string_ptr] += length as i64 + 1;
        }
    }

    /// f_now ( -- ) Start a timer
    pub fn f_now(&mut self) {
        self.timer = Instant::now();
    }

    /// micros ( -- n ) returns the number of microseconds since NOW was called
    pub fn f_micros(&mut self) {
        let duration = self.timer.elapsed();
        self.push(duration.as_micros() as i64);
    }

    /// millis ( -- n ) returns the number of milliseconds since NOW was called
    pub fn f_millis(&mut self) {
        let duration = self.timer.elapsed();
        self.push(duration.as_millis() as i64);
    }

    /// ms ( ms -- ) Sleep for ms milliseconds
    pub fn f_ms(&mut self) {
        if self.stack_check(1, "sleep") {
            let delay = self.pop() as u64;
            let millis = Duration::from_millis(delay);
            thread::sleep(millis);
        }
   }
}
