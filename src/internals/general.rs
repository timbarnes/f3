// General-purpose builtin words

use crate::kernel::DATA_SIZE;
use crate::runtime::{ForthRuntime};
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
            if self.kernel.stack_check(n + 1, "pick") {
                let val = self.kernel.peek(n);
                self.kernel.push(val);
            }
        }
    }
    pub fn f_roll(&mut self) {
        if self.kernel.stack_check(1, "roll") {
            let n = self.kernel.pop() as usize;
            if n == 0 { return; } // 0 roll is a no-op
            if self.kernel.stack_check(n + 1, "roll") {
                // Save the nth value from the top
                let val = self.kernel.peek(n);
                // Shift all items above it down by one
                for i in (1..=n).rev() {
                    let tmp = self.kernel.peek(i - 1);
                    let idx = self.kernel.get_stack_ptr() + i;
                    self.kernel.set(idx, tmp);
                }
                // Place the saved value on top
                self.kernel.set(self.kernel.get_stack_ptr(), val);
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
            self.kernel.set_return_ptr(self.kernel.get_return_ptr() - 1);
            self.kernel.set(self.kernel.get_return_ptr(), val);
        }
    }

    /// r> ( -- n ) Pops the return stack, pushing the value to the calculation stack
    ///
    pub fn f_r_from(&mut self) {
        let val = self.kernel.get(self.kernel.get_return_ptr());
        self.kernel.push(val);
        self.kernel.set_return_ptr(self.kernel.get_return_ptr() + 1);
    }

    /// r@ ( -- n ) Gets the top value from the return stack, pushing the value to the calculation stack
    ///
    pub fn f_r_get(&mut self) {
        let val = self.kernel.get(self.kernel.get_return_ptr());
        self.kernel.push(val);
    }

    /// i ( -- n ) Pushes the current loop index to the calculation stack
    ///
    pub fn f_i(&mut self) {
        let val = self.kernel.get(self.kernel.get_return_ptr());
        self.kernel.push(val);
    }

    /// j ( -- n ) Pushes the second level (outer) loop index to the calculation stack
    ///
    pub fn f_j(&mut self) {
        let val = self.kernel.get(self.kernel.get_return_ptr() + 1);
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
            let length = self.kernel.byte_get(source);
            self.kernel.string_copy(source, dest, length as usize, true);
        }
    }

    /// s-create ( s-from -- s-to ) copies a counted string into the next empty space, updating the free space pointer
    pub fn f_s_create(&mut self) {
        if self.kernel.stack_check(1, "s-create") {
            let source = self.kernel.pop() as usize;
            let length = self.kernel.byte_get(source) as usize;
            let dest = self.kernel.get(self.kernel.get_string_ptr());
            self.kernel.string_copy(source, dest as usize, length, true);
            self.kernel.delta(self.kernel.get_string_ptr(), length as i64 + 1);
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
        let depth = self.kernel.stack_len();
        self.kernel.push(depth as i64);
    }

    pub fn f_builtin_name(&mut self) {
        if self.kernel.stack_check(1, "builtin-name") {
            let index = self.kernel.pop() as usize;
            let builtin = self.kernel.get_builtin(index).name.clone();
            let name = builtin.as_str();
            let destination = self.tmp_ptr;
            self.kernel.string_save(name, destination);
            self.kernel.push(destination as i64);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::ForthRuntime;

    fn setup_stack(rt: &mut ForthRuntime, vals: &[i64]) {
        for &v in vals.iter() {
            rt.kernel.push(v);
        }
    }

    #[test]
    fn test_roll_basic() {
        let mut rt = ForthRuntime::new();
        rt.cold_start();
        setup_stack(&mut rt, &[1, 2, 3, 4, 5]);
        // Stack: 1 2 3 4 5 (5 is top)
        rt.kernel.push(2); // n = 2
        rt.f_roll();
        // Should move 3 to top: 1 2 4 5 3
        let mut result = vec![];
        for i in (0..rt.kernel.stack_len()).rev() {
            result.push(rt.kernel.peek(i));
        }
        assert_eq!(result, vec![1, 2, 4, 5, 3]);
    }

    #[test]
    fn test_roll_zero() {
        let mut rt = ForthRuntime::new();
        setup_stack(&mut rt, &[1, 2, 3]);
        rt.kernel.push(0); // n = 0
        rt.f_roll();
        // Should be unchanged
        let mut result = vec![];
        for i in (0..rt.kernel.stack_len()).rev() {
            result.push(rt.kernel.peek(i));
        }
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_roll_top() {
        let mut rt = ForthRuntime::new();
        setup_stack(&mut rt, &[7, 8, 9]);
        rt.kernel.push(2); // n = 2 (bottom)
        rt.f_roll();
        // Should move 7 to top: 8 9 7
        let mut result = vec![];
        for i in (0..rt.kernel.stack_len()).rev() {
            result.push(rt.kernel.peek(i));
        }
        assert_eq!(result, vec![8, 9, 7]);
    }

    #[test]
    #[should_panic]
    fn test_roll_underflow() {
        let mut rt = ForthRuntime::new();
        setup_stack(&mut rt, &[1]);
        rt.kernel.push(2); // n = 2, not enough items
        rt.f_roll();
    }
}
