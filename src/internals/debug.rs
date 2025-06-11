// Debugging help

use crate::engine::{ADDRESS_MASK, BUILTIN_MASK, STACK_START, TF, EXEC,
    VARIABLE, CONSTANT, LITERAL, STRLIT, DEFINITION, BRANCH, BRANCH0, ABORT, EXIT, BREAK};
use crate::messages::DebugLevel;

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
        let r = $self.data[$self.stack_ptr];
        //$self.data[$self.stack_ptr] = 999999;
        $self.stack_ptr += 1;
        r
    }};
}

macro_rules! push {
    ($self:ident, $val:expr) => {
        $self.stack_ptr -= 1;
        $self.data[$self.stack_ptr] = $val;
    };
}

impl TF {
    /// show-stack ( -- ) turns on stack printing at the time the prompt is issued
    ///
    pub fn f_show_stack(&mut self) {
        self.show_stack = true;
    }

    /// hide-stack ( -- ) turns off stack printing at the time the prompt is issued
    ///
    pub fn f_hide_stack(&mut self) {
        self.show_stack = false;
    }

    /// DEPTH - print the number of items on the stack
    ///
    pub fn f_stack_depth(&mut self) {
        let depth = STACK_START - self.stack_ptr;
        push!(self, depth as i64);
    }

    /// dbg ( n -- ) sets the current debug level used by the message module
    ///
    pub fn f_dbg(&mut self) {
        if stack_ok!(self, 1, "dbg") {
            match pop!(self) {
                0 => self.msg.set_level(DebugLevel::Error),
                1 => self.msg.set_level(DebugLevel::Warning),
                2 => self.msg.set_level(DebugLevel::Info),
                _ => self.msg.set_level(DebugLevel::Debug),
            }
        }
    }

    pub fn f_debuglevel(&mut self) {
        println!("DebugLevel is {:?}", self.msg.get_level());
    }

    /// u_step provides the step / trace functionality
    ///     called from inside the definition interpreter
    ///     it is driven by the STEPPER and STEPPER-DEPTH variables:
    ///     STEPPER = 0  => stepping is off
    ///     STEPPER = -1 => single step
    ///     STEPPER = 1  => trace mode, printing the stack and current word before each operation.
    ///             
    ///     STEPPER-DEPTH indicates how many levels of the return stack should be stepped or traced
    /// 
    ///     pc is the program counter, which represents the address of the cell being executed.
    ///
    pub fn u_step(&mut self, pc: usize, call_depth: usize) {
        let stepper_mode = self.data[self.stepper_ptr];
        let stepper_depth = self.data[self.step_depth_ptr] as usize;
        if stepper_mode == 0  || call_depth > stepper_depth { return };
        let mut contents = self.data[pc] as usize;
        let is_builtin = if contents & BUILTIN_MASK != 0 { true } else { false };
        contents &= ADDRESS_MASK;
        let mut c = 's';

        for _i in 1..call_depth { print!(" "); }  
        self.f_dot_s();

        match contents as i64 {
            VARIABLE | CONSTANT | DEFINITION => println!(" {} ", self.u_get_string(self.data[pc - 1] as usize)),
            LITERAL => println!(" {} ", self.data[pc + 1]),
            STRLIT => println!(" {} ", self.u_get_string(self.data[pc + 1] as usize)),
            BRANCH => println!(" BRANCH:{}", self.data[pc + 1]),
            BRANCH0 => println!(" BRANCH0:{}", self.data[pc + 1]),
            ABORT => println!(" ABORT "),
            EXIT => println!(" EXIT "),
            BREAK => println!(" BREAK "),
            EXEC => println!(" -> EXEC"),
            _ => {
                if is_builtin {
                    println!(" {} ", &self.builtins[contents].name);
                } else { 
                    // it's a word address: step-in about to occur
                    println!(" ->{}", self.u_get_string(self.data[contents - 1] as usize));
                }
            }
        } 
        match stepper_mode {
            -1 => {
                // step mode: get a character
                    print!("Step> ");
                    self.f_flush();
                    loop {
                    self.f_key();
                    c = pop!(self) as u8 as char;
                    if c != '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
        match c {
            't' => self.data[self.stepper_ptr] = 1,
            'i' => self.data[self.step_depth_ptr] += 1,
            'o' => self.data[self.step_depth_ptr] -= 1,
            'c' => self.data[self.stepper_ptr] = 0,
            'h' | '?' => println!("Stepper: 's' for show, 't' for trace, 'c' for continue, 'o' for step-out."),
            _ =>{}, 
        }
    }
}
