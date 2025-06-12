// General-purpose builtin words

use crate::engine::{DATA_SIZE, FALSE, STACK_START, TF, TRUE};
use std::time::{Instant, Duration};
use std::thread;

macro_rules! stack_ok {
    ($self:ident, $n: expr, $caller: expr) => {
        if $self.stack_ptr <= STACK_START - $n {
            true
        } else {
            $self.msg.error($caller, "Stack underflow", None::<bool>);
            $self.f_abort();
            false
        }
    };
}
macro_rules! pop {
    ($self:ident) => {{
        let r = $self.heap[$self.stack_ptr];
        //$self.data[$self.stack_ptr] = 999999;
        $self.stack_ptr += 1;
        r
    }};
}
macro_rules! top {
    ($self:ident) => {{
        $self.heap[$self.stack_ptr]
    }};
}
macro_rules! push {
    ($self:ident, $val:expr) => {
        $self.stack_ptr -= 1;
        $self.heap[$self.stack_ptr] = $val;
    };
}

macro_rules! pop2_push1 {
    // Helper macro
    ($self:ident, $word:expr, $expression:expr) => {
        if stack_ok!($self, 2, $word) {
            let j = pop!($self);
            let k = pop!($self);
            push!($self, $expression(k, j));
        }
    };
}
macro_rules! pop1_push1 {
    // Helper macro
    ($self:ident, $word:expr, $expression:expr) => {
        if stack_ok!($self, 1, $word) {
            let x = pop!($self);
            push!($self, $expression(x));
        }
    };
}

/// u_is_integer determines whether a string parses correctly as an integer
///
pub fn u_is_integer(s: &str) -> bool {
    s.parse::<i64>().is_ok()
}

impl TF {
    /// Basic Forth operations on the stack.
    ///
    pub fn f_plus(&mut self) {
        if stack_ok!(self, 2, "+") {
            let a = pop!(self);
            let b = pop!(self);
            push!(self, a + b);
        };
    }

    pub fn f_minus(&mut self) {
        pop2_push1!(self, "-", |a, b| a - b);
    }

    pub fn f_times(&mut self) {
        pop2_push1!(self, "*", |a, b| a * b);
    }

    pub fn f_divide(&mut self) {
        pop2_push1!(self, "/", |a, b| a / b);
    }

    pub fn f_mod(&mut self) {
        pop2_push1!(self, "mod", |a, b| a % b);
    }

    pub fn f_less(&mut self) {
        pop2_push1!(self, "<", |a, b| if a < b { -1 } else { 0 });
    }

    pub fn f_true(&mut self) {
        push!(self, TRUE);
    }

    pub fn f_false(&mut self) {
        push!(self, FALSE);
    }

    pub fn f_equal(&mut self) {
        pop2_push1!(self, "=", |a, b| if a == b { -1 } else { 0 });
    }

    pub fn f_0equal(&mut self) {
        pop1_push1!(self, "0=", |a| if a == 0 { -1 } else { 0 });
    }

    pub fn f_0less(&mut self) {
        pop1_push1!(self, "0<", |a| if a < 0 { -1 } else { 0 });
    }

    pub fn f_dup(&mut self) {
        if stack_ok!(self, 1, "dup") {
            let top = top!(self);
            push!(self, top);
        }
    }
    pub fn f_drop(&mut self) {
        if stack_ok!(self, 1, "drop") {
            pop!(self);
        }
    }
    pub fn f_swap(&mut self) {
        if stack_ok!(self, 2, "swap") {
            let a = pop!(self);
            let b = pop!(self);
            push!(self, a);
            push!(self, b);
        }
    }
    pub fn f_over(&mut self) {
        if stack_ok!(self, 2, "over") {
            let first = pop!(self);
            let second = pop!(self);
            push!(self, second);
            push!(self, first);
            push!(self, second);
        }
    }
    pub fn f_rot(&mut self) {
        if stack_ok!(self, 3, "rot") {
            let first = pop!(self);
            let second = pop!(self);
            let third = pop!(self);
            push!(self, second);
            push!(self, first);
            push!(self, third);
        }
    }
pub fn f_pick(&mut self) {
    if stack_ok!(self, 1, "pick") {
        let n = pop!(self) as usize;
        if stack_ok!(self, n, "pick") {
            push!(self, self.heap[self.stack_ptr + n + 1]);
        }
    }
}
pub fn f_roll(&mut self) {
    if stack_ok!(self, 1, "roll") {
        let n = pop!(self) as usize;
        if n == 0 { return }; // 0 roll is a no-op
        if stack_ok!(self, n + 1, "roll") {
            // save the nth value
            let new_top = self.heap[self.stack_ptr + n];
            // iterate, moving elements down
            let mut i = self.stack_ptr + n - 1;
            while i >= self.stack_ptr {
                self.heap[i + 1] = self.heap[i];
                i -= 1;
            } 
            self.stack_ptr += 1; // because we removed an element
            push!(self, new_top);
        }
    }
}
    pub fn f_and(&mut self) {
        if stack_ok!(self, 2, "and") {
            let a = pop!(self);
            let b = pop!(self);
            push!(self, (a as usize & b as usize) as i64);
        }
    }

    pub fn f_or(&mut self) {
        if stack_ok!(self, 2, "or") {
            let a = pop!(self);
            let b = pop!(self);
            push!(self, (a as usize | b as usize) as i64);
        }
    }

    /// @ (get) ( a -- n ) loads the value at address a onto the stack
    pub fn f_get(&mut self) {
        if stack_ok!(self, 1, "@") {
            let addr = pop!(self) as usize;
            if addr < DATA_SIZE {
                push!(self, self.heap[addr]);
            } else {
                self.msg.error("@", "Address out of range", Some(addr));
                self.f_abort();
            }
        }
    }

    /// ! (store) ( n a -- ) stores n at address a. Generally used with variables
    ///
    pub fn f_store(&mut self) {
        if stack_ok!(self, 2, "!") {
            let addr = pop!(self) as usize;
            let value = pop!(self);
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
        if stack_ok!(self, 1, ">r") {
            let value = pop!(self);
            self.return_ptr -= 1;
            self.heap[self.return_ptr] = value;
        }
    }

    /// r> ( -- n ) Pops the return stack, pushing the value to the calculation stack
    ///
    pub fn f_r_from(&mut self) {
        push!(self, self.heap[self.return_ptr]);
        self.return_ptr += 1;
    }

    /// r@ ( -- n ) Gets the top value from the return stack, pushing the value to the calculation stack
    ///
    pub fn f_r_get(&mut self) {
        push!(self, self.heap[self.return_ptr]);
    }

    /// i ( -- n ) Pushes the current loop index to the calculation stack
    ///
    pub fn f_i(&mut self) {
        push!(self, self.heap[self.return_ptr]);
    }

    /// j ( -- n ) Pushes the second level (outer) loop index to the calculation stack
    ///
    pub fn f_j(&mut self) {
        push!(self, self.heap[self.return_ptr + 1]);
    }

    /// c@ - ( s -- c ) read a character from a string and place on the stack
    ///
    pub fn f_c_get(&mut self) {
        if stack_ok!(self, 1, "c@") {
            let s_address = pop!(self) as usize;
            push!(self, self.strings[s_address] as u8 as i64);
        }
    }

    /// c! - ( c s -- ) read a character from a string and place on the stack
    pub fn f_c_store(&mut self) {
        if stack_ok!(self, 2, "c!") {
            let s_address = pop!(self) as usize;
            self.strings[s_address] = pop!(self) as u8 as char;
        }
    }

    /// s-copy (s-from s-to -- s-to )
    pub fn f_s_copy(&mut self) {
        if stack_ok!(self, 2, "s-copy") {
            let dest = pop!(self) as usize;
            let result_ptr = dest as i64;
            let source = pop!(self) as usize;
            let length = self.strings[source] as u8 as usize + 1;
            let mut i = 0;
            while i < length {
                self.strings[dest + i] = self.strings[source + i];
                i += 1;
            }
            self.heap[self.string_ptr] += length as i64;
            push!(self, result_ptr);
        }
    }
    /// s-create ( s-from -- s-to ) copies a counted string into the next empty space, updating the free space pointer
    pub fn f_s_create(&mut self) {
        if stack_ok!(self, 1, "s-create") {
            let source = top!(self) as usize;
            let length = self.strings[source] as usize;
            let dest = self.heap[self.string_ptr];
            push!(self, dest); // destination
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
        push!(self, duration.as_micros() as i64);
    }

    /// millis ( -- n ) returns the number of milliseconds since NOW was called
    pub fn f_millis(&mut self) {
        let duration = self.timer.elapsed();
        push!(self, duration.as_millis() as i64);
    }

    /// ms ( ms -- ) Sleep for ms milliseconds
    pub fn f_ms(&mut self) {
        if stack_ok!(self, 1, "sleep") {
            let delay = pop!(self) as u64;
            let millis = Duration::from_millis(delay);
            thread::sleep(millis);
        }
   }
}
