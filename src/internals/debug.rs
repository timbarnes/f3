// Debugging help

use crate::runtime::{ForthRuntime, ADDRESS_MASK, EXEC, BUILTIN_FLAG,
    VARIABLE, CONSTANT, LITERAL, STRLIT, DEFINITION, BRANCH, BRANCH0, ABORT, EXIT, BREAK};
use crate::internals::messages::DebugLevel;

impl ForthRuntime {
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

    /// dbg ( n -- ) sets the current debug level used by the message module
    ///
    pub fn f_dbg(&mut self) {
        if self.kernel.stack_check( 1, "dbg") {
            match self.kernel.pop(){
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
    pub fn debug_step(&mut self, pc: usize, call_depth: usize) {
        let stepper_mode = self.kernel.get(self.stepper_ptr);
        let stepper_depth = self.kernel.get(self.step_depth_ptr) as usize;
        if stepper_mode == 0  || call_depth > stepper_depth { return };
        let mut contents = self.kernel.get(pc) as usize;
        let is_builtin = if contents & BUILTIN_FLAG != 0 { true } else { false };
        contents &= ADDRESS_MASK;
        let mut c = 's';

        for _i in 1..call_depth { print!(" "); }  
        self.f_dot_s();

        match contents as i64 {
            VARIABLE | CONSTANT | DEFINITION => {
                let val = self.kernel.get(pc - 1) as usize;
                println!(" {} ", self.kernel.string_get(val))
            },
            LITERAL => println!(" {} ", self.kernel.get(pc + 1)),
            STRLIT => {
                let val = self.kernel.get(pc + 1) as usize;
                println!(" {} ", self.kernel.string_get(val))
            },
            BRANCH => {
                let val = self.kernel.get(pc + 1);
                println!(" BRANCH:{}", val)
            },
            BRANCH0 => println!(" BRANCH0:{}", self.kernel.get(pc + 1)),
            ABORT => println!(" ABORT "),
            EXIT => println!(" EXIT "),
            BREAK => println!(" BREAK "),
            EXEC => println!(" -> EXEC"),
            _ => {
                if is_builtin {
                    println!(" {} ", &self.kernel.get_builtin(contents).name);
                } else { 
                    // it's a word address: step-in about to occur
                    let val = self.kernel.get(contents - 1);
                    println!(" ->{}", self.kernel.string_get(val as usize));
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
                    c = self.kernel.pop() as u8 as char;
                    if c != '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
        match c {
            't' => self.kernel.set(self.stepper_ptr, 1),
            'i' => self.kernel.incr(self.step_depth_ptr),
            'o' => self.kernel.decr(self.step_depth_ptr),
            'c' => self.kernel.set(self.stepper_ptr, 0),
            'h' | '?' => println!("Stepper: 's' for show, 't' for trace, 'c' for continue, 'o' for step-out."),
            _ =>{}, 
        }
    }
}
