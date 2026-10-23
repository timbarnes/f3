// Debugging help

use crate::internals::messages::DebugLevel;
use crate::kernel::CELL;
use crate::runtime::{
    ForthRuntime, ABORT, ADDRESS_MASK, BRANCH, BRANCH0, BREAK, BUILTIN_FLAG, CONSTANT, CREATED,
    DEFINITION, DOES,
    DO, EXEC, EXIT, FOR, LEAVE, LITERAL, LOOP, NEXT, OPCODE_FLAG, PLOOP, QDO, STRLIT, VARIABLE,
};

impl ForthRuntime {
    /// dbg ( n -- ) sets the current debug level used by the message module
    ///
    pub fn f_dbg(&mut self) {
        if self.kernel.stack_check(1, "dbg") {
            match self.kernel.pop() {
                0 => self.msg.set_level(DebugLevel::Error),
                1 => self.msg.set_level(DebugLevel::Warning),
                2 => self.msg.set_level(DebugLevel::Info),
                _ => self.msg.set_level(DebugLevel::Debug),
            }
        }
    }

    pub fn f_debuglevel(&mut self) {
        let level = self.msg.get_level();
        let result = match level {
            DebugLevel::Error => 0, // the same numbering as dbg
            DebugLevel::Warning => 1,
            DebugLevel::Info => 2,
            DebugLevel::Debug => 3,
        };
        self.kernel.push(result as i64);
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
    #[cold]
    pub fn debug_step(&mut self, pc: usize, call_depth: usize) {
        let stepper_mode = self.kernel.get(self.stepper_ptr);
        let stepper_depth = self.kernel.get(self.step_depth_ptr) as usize;
        if stepper_mode == 0 || call_depth > stepper_depth {
            return;
        };
        let code = self.kernel.get(pc);
        let mut c = 's';

        // Print the program counter address
        print!("{:>5}: ", pc);
        // Indent based on call depth and print the stack
        for _i in 1..call_depth {
            print!(" ");
        }
        self.run_untraced(".s");

        match code {
            VARIABLE | CONSTANT | DEFINITION | CREATED => {
                let val = self.kernel.get(pc.wrapping_sub(CELL)) as usize;
                println!(" {} ", self.kernel.string_get(val))
            }
            LITERAL => println!(" {} ", self.kernel.get(pc + CELL)),
            STRLIT => {
                let val = self.kernel.get(pc + CELL) as usize;
                println!(" {} ", self.kernel.string_get(val))
            }
            BRANCH => {
                let val = self.kernel.get(pc + CELL);
                println!(" BRANCH:{}", val)
            }
            BRANCH0 => println!(" BRANCH0:{}", self.kernel.get(pc + CELL)),
            DO | QDO | LOOP | PLOOP | FOR | NEXT => {
                println!(
                    " LOOP-OP {}:{}",
                    code & !OPCODE_FLAG,
                    self.kernel.get(pc + CELL)
                )
            }
            LEAVE => println!(" LEAVE "),
            DOES => println!(" DOES> "),
            ABORT => println!(" ABORT "),
            EXIT => println!(" EXIT "),
            BREAK => println!(" BREAK "),
            EXEC => println!(" -> EXEC"),
            _ if code as usize & BUILTIN_FLAG != 0 => {
                println!(
                    " {} ",
                    self.kernel.get_builtin(code as usize & ADDRESS_MASK).name
                );
            }
            _ => {
                // it's a word address: step-in about to occur
                let val = self.kernel.get((code as usize).wrapping_sub(CELL));
                println!(" ->{}", self.kernel.string_get(val as usize));
            }
        }
        if stepper_mode == -1 {
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
        match c {
            't' => self.set_stepper(1),
            'i' => self.kernel.incr(self.step_depth_ptr),
            'o' => self.kernel.decr(self.step_depth_ptr),
            'c' => self.set_stepper(0),
            'h' | '?' => println!(
                "Stepper: 's' for show, 't' for trace, 'c' for continue, 'o' for step-out."
            ),
            _ => {}
        }
    }
}
