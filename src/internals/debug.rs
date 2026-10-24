// Debugging help

use crate::internals::messages::DebugLevel;
use crate::runtime::ForthRuntime;

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

    /// debug_step runs the Forth word (step) ( pc depth mode -- mode' ) before each cell when
    ///     STEPPER is non-zero: -1 single step, 1 trace. (step), in debug.fs, prints the cell
    ///     and reads the stepper commands; it returns the new mode, and can change STEPPER-DEPTH.
    ///     It runs with the stepper off, so it isn't traced itself.
    ///
    #[cold]
    pub fn debug_step(&mut self, pc: usize, call_depth: usize) {
        let Some(step) = self.find_word(b"(step)") else {
            return; // no core loaded
        };
        let mode = self.kernel.get(self.stepper_ptr);
        self.set_stepper(0);
        self.kernel.push(pc as i64);
        self.kernel.push(call_depth as i64);
        self.kernel.push(mode);
        self.kernel.push(step as i64);
        self.f_execute();
        let mode = self.kernel.pop();
        self.set_stepper(mode);
    }
}
