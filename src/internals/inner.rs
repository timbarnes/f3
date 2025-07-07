use crate::kernel::{DATA_SIZE, RET_START};
/// Inner Interpreters
///
/// Core functions to execute specific types of objects
///
use crate::runtime::{
    ForthRuntime, ABORT, ADDRESS_MASK, ARRAY, BRANCH, BRANCH0, BREAK, BUILTIN, BUILTIN_FLAG,
    CONSTANT, DEFINITION, EXEC, EXIT, LITERAL, STRLIT, VARIABLE,
};

impl ForthRuntime {
    /// Executes the builtin at the next address in DATA
    ///
    ///    [ index of i_builtin ] [ index of builtin ] in a compiled word
    ///
    pub fn builtin(&mut self, code: usize) {
        let func = &self.kernel.get_builtin(code);
        (func.code)(self); // call the function pointer directly
    }

    /// Places the address of the adjacent variable on the stack
    ///
    ///    [ index of i_variable ] [ index of builtin ] in a compiled word
    ///
    pub fn i_variable(&mut self) {
        let val = self.kernel.pop();
        self.kernel.push(val); // address of the value
    }

    /// Places the value of the adjacent constant on the stack
    ///
    ///    [ index of i_constant ] [ constant value ] in a compiled word
    ///
    pub fn i_constant(&mut self) {
        let val = self.kernel.pop();
        let val = self.kernel.get(val as usize);
        self.kernel.push(val);
    }

    pub fn i_array(&mut self) {
        self.i_variable();
    }

    /// Places the number in data[d] on the stack
    ///
    ///    [ index of i_literal ] [ number ] in a compiled word
    ///
    pub fn i_literal(&mut self) {} // Number is already on the stack

    /// Places the address (in string space) of the adjacent string on the stack
    ///
    ///    [ i_string ] [ index into string space ] in a compiled word
    ///
    pub fn i_strlit(&mut self) {} // Address is already on the stack

    /// i_definition ( cfa -- ) Loops through the adjacent definition, running their inner interpreters
    ///
    ///    [ index of i_definition ] [ sequence of compiled words ]
    ///
    ///    A program counter is used to step through the entries in the definition.
    ///    Each entry is one or two cells, and may be an inner interpreter code (opcode), with or without an argument,
    ///    or a defined word. For space efficiency, builtin words and user defined (colon) words are
    ///    represented by the cfa of their definition, overlaid with a flag. The interpreter calls the builtin code.
    ///    For nested definitions, the inner interpreter pushes the program counter (PC) and continues.
    ///    When the end of a definition is found, the PC is restored from the previous caller.
    ///
    ///    Most data is represented by an address, so self.data[pc] is the cfa of the word referenced.
    ///    Each operation advances the pc to the next token.
    ///
    ///    cfa means the code field address (the address in data space of the opcode to be executed)
    ///    nfa means the name field address (a pointer to the string naming the word)
    ///    xt  means the execution token - a value that tells the engine what to do
    ///
    pub fn i_definition(&mut self) {
        let mut pc = self.kernel.pop() as usize; // This is the start of the definition: first word after the inner interpreter opcode
        let mut call_depth: usize = 1;
        self.kernel.push(0); // this is how we know when we're done
        self.f_to_r();
        loop {
            // each time round the loop should be one word
            if pc == 0 || self.get_abort_flag() {
                self.kernel.set_return_ptr(RET_START); // clear the return stack
                return; // we've completed the last exit or encountered an error
            }
            let code = if pc < DATA_SIZE {
                self.kernel.get(pc)
            } else {
                pc as i64
            };
            self.debug_step(pc, call_depth);
            match code {
                BUILTIN => {
                    self.msg
                        .error("i_definition", "Found BUILTIN???", Some(code));
                    self.f_r_from();
                    pc = self.kernel.pop() as usize;
                }
                VARIABLE | ARRAY => {
                    // this means we've pushed into a variable or an array reference
                    pc += 1;
                    self.kernel.push(pc as i64); // the address of the variable's data
                    self.f_r_from();
                    pc = self.kernel.pop() as usize;
                }
                CONSTANT => {
                    pc += 1;
                    let val = self.kernel.get(pc);
                    self.kernel.push(val); // the value of the constant
                    self.f_r_from();
                    pc = self.kernel.pop() as usize;
                }
                LITERAL => {
                    pc += 1;
                    let val = self.kernel.get(pc);
                    self.kernel.push(val); // the data stored in the current definition
                    pc += 1;
                }
                STRLIT => {
                    pc += 1;
                    let val = self.kernel.get(pc) as i64;
                    self.kernel.push(val); // the string address of the data
                    pc += 1;
                }
                DEFINITION => {
                    pc += 1;
                    // Continue to work through the definition
                    // at the end, EXIT will pop back to the previous definition
                }
                BRANCH => {
                    // Unconditional jump based on self.data[pc + 1]
                    pc += 1;
                    let offset = self.kernel.get(pc);
                    if offset < 0 {
                        pc -= offset.abs() as usize;
                    } else {
                        pc += offset as usize;
                    }
                }
                BRANCH0 => {
                    pc += 1;
                    if self.kernel.pop() == 0 {
                        let offset = self.kernel.get(pc);
                        if offset < 0 {
                            pc -= offset.abs() as usize;
                        } else {
                            pc += offset as usize;
                        }
                    } else {
                        pc += 1; // skip over the offset
                    }
                }
                ABORT => {
                    self.f_abort();
                    break;
                }
                EXIT => {
                    // Current definition is finished, so pop the PC from the return stack
                    self.f_r_from();
                    pc = self.kernel.pop() as usize;
                    call_depth -= 1;
                }
                BREAK => {
                    // Breaks out of a word by popping the PC from the return stack
                    self.f_r_from();
                    pc = self.kernel.pop() as usize;
                }
                EXEC => {
                    self.f_execute();
                    pc += 1;
                }
                _ => {
                    // we have a word address
                    // see if it's a builtin:
                    let builtin_flag = code as usize & BUILTIN_FLAG;
                    let address = code as usize & ADDRESS_MASK;
                    if builtin_flag != 0 && (address <= self.kernel.max_builtin()) {
                        self.builtin(address);
                        pc += 1;
                    } else {
                        call_depth += 1;
                        self.kernel.push(pc as i64 + 1); // the return address is the next object in the list
                        self.f_to_r(); // save it on the return stack
                        pc = code as usize;
                    }
                }
            }
        }
    }

    /// Unconditional branch, used by condition and loop structures
    ///
    pub fn i_branch(&mut self) {}

    /// Branch if zero, used by condition and loop structures
    ///
    pub fn i_branch0(&mut self) {}

    /// Force an abort
    ///
    pub fn i_abort(&mut self) {}

    /// Leave the current word
    ///     *** doesn't work, because there's no way to reset the program counter from here
    ///
    pub fn i_exit(&mut self) {
        self.f_r_from();
        // pc = self.kernel.pop() as usize;
    }
}
