/// Inner Interpreter
///
/// Executes compiled definitions
///
use crate::kernel::{throw, CELL};
use crate::runtime::{
    ForthRuntime, ABORT, ADDRESS_MASK, ARRAY, BRANCH, BRANCH0, BREAK, BUILTIN_FLAG, CONSTANT,
    CREATED, DEFINITION, DO, DOES, EXEC, EXIT, FOR, LEAVE, LITERAL, LOOP, NEXT, PLOOP, QDO, STRLIT,
    VARIABLE,
};

/// The address a branch-style operand at `pc` points to (operands are offsets from their own cell)
fn target(pc: usize, offset: i64) -> usize {
    (pc as i64).wrapping_add(offset) as usize // a bad offset gives a bad address, which get() rejects
}

#[cold]
#[inline(never)]
fn not_an_xt(xt: usize) -> ! {
    throw(format!("{} is not an execution token", xt as i64))
}

impl ForthRuntime {
    /// Executes the builtin with the given index
    ///
    pub fn builtin(&mut self, code: usize) {
        let func = &self.kernel.get_builtin(code);
        (func.code)(self); // call the function pointer directly
    }

    /// enter runs the word whose code field is at xt, as far as Rust can: a builtin or a data
    ///     word is finished on return (None); a colon definition, or a created word with DOES> code,
    ///     returns the address of the code to run next. EXECUTE and the inner interpreter share it.
    ///
    #[inline(always)]
    pub fn enter(&mut self, xt: usize) -> Option<usize> {
        let code = self.kernel.get(xt);
        match code {
            DEFINITION => Some(xt + CELL),
            VARIABLE | ARRAY => {
                self.kernel.push((xt + CELL) as i64);
                None
            }
            CONSTANT => {
                let val = self.kernel.get(xt + CELL);
                self.kernel.push(val);
                None
            }
            CREATED => {
                self.kernel.push((xt + 2 * CELL) as i64); // the data field
                match self.kernel.get(xt + CELL) as usize {
                    0 => None,
                    does => Some(does),
                }
            }
            _ if code as usize & BUILTIN_FLAG != 0 => {
                // the cfa of a builtin word (e.g. compiled by ['] ... ,)
                self.builtin(code as usize & ADDRESS_MASK);
                None
            }
            _ => not_an_xt(xt),
        }
    }

    /// set_does makes `code` the DOES> code of the latest word, which must have been made by CREATE
    ///
    fn set_does(&mut self, code: usize) {
        let cfa = self.cfa_of(self.kernel.get(self.context_ptr) as usize - CELL);
        if self.kernel.get(cfa) != CREATED {
            throw("does>: the latest word was not made by create");
        }
        self.kernel.set(cfa + CELL, code as i64);
    }

    /// unloop ( -- ) ( R: exit limit index -- ) discards the innermost loop's return-stack frame
    ///
    pub fn f_unloop(&mut self) {
        self.kernel.stack_check_r(3, "unloop");
        for _ in 0..3 {
            self.kernel.pop_r();
        }
    }

    /// i_definition runs the compiled code starting at pc (the cell after a DEFINITION opcode)
    ///
    ///    A program counter is used to step through the entries in the definition.
    ///    Each entry is one or two cells, and may be an inner interpreter code (opcode), with or without an argument,
    ///    or a defined word. For space efficiency, builtin words are compiled as their builtin index,
    ///    overlaid with BUILTIN_FLAG; other words are compiled as their cfa.
    ///    Calling a word pushes the return address on the return stack; EXIT pops it.
    ///    The call returns when an EXIT brings the return stack back to where it was on entry,
    ///    so i_definition can be re-entered safely (e.g. by EXECUTE inside a definition).
    ///
    ///    Loops (DO, ?DO, FOR) keep a three-cell frame on the return stack: exit address, limit, index.
    ///    The index is on top, so I reads it directly, and J reads the index three cells further down.
    ///
    ///    cfa means the code field address (the address in data space of the opcode to be executed)
    ///    nfa means the name field address (a pointer to the string naming the word)
    ///    xt  means the execution token: a cfa, or a builtin index overlaid with BUILTIN_FLAG
    ///
    pub fn i_definition(&mut self, start: usize) {
        let entry_rp = self.kernel.get_return_ptr();
        let mut pc = start;
        let mut call_depth: usize = 1;
        loop {
            let code = self.kernel.get(pc); // throws if pc has left the heap
            if self.tracing {
                self.debug_step(pc, call_depth); // only when stepping or tracing
            }
            if code as usize & BUILTIN_FLAG != 0 {
                // the most common cell: a builtin, compiled as its flagged index
                self.builtin(code as usize & ADDRESS_MASK);
                pc += CELL;
                continue;
            }
            match code {
                LITERAL => {
                    pc += CELL;
                    let val = self.kernel.get(pc);
                    self.kernel.push(val);
                    pc += CELL;
                }
                STRLIT => {
                    // the counted string follows inline, padded to a cell
                    pc += CELL;
                    self.kernel.push(pc as i64);
                    pc += (1 + self.kernel.byte_get(pc) as usize).next_multiple_of(CELL);
                }
                BRANCH => {
                    pc += CELL;
                    pc = target(pc, self.kernel.get(pc));
                }
                BRANCH0 => {
                    pc += CELL;
                    if self.kernel.pop() == 0 {
                        pc = target(pc, self.kernel.get(pc));
                    } else {
                        pc += CELL; // skip over the offset
                    }
                }
                DO | QDO | FOR => {
                    // operand: offset to the first cell after the loop
                    pc += CELL;
                    let exit = target(pc, self.kernel.get(pc));
                    let (limit, index) = if code == FOR {
                        (0, self.kernel.pop())
                    } else {
                        let index = self.kernel.pop();
                        (self.kernel.pop(), index)
                    };
                    let skip = match code {
                        QDO => index == limit,
                        FOR => index <= 0,
                        _ => false,
                    };
                    if skip {
                        pc = exit;
                    } else {
                        self.kernel.push_r(exit as i64);
                        self.kernel.push_r(limit);
                        self.kernel.push_r(index);
                        pc += CELL;
                    }
                }
                LOOP | PLOOP | NEXT => {
                    // operand: offset back to the first cell of the loop body
                    pc += CELL;
                    self.kernel.stack_check_r(3, "loop");
                    let rp = self.kernel.get_return_ptr();
                    let index = self.kernel.get(rp);
                    let limit = self.kernel.get(rp + CELL);
                    let (next, done) = if code == NEXT {
                        (index - 1, index <= 1) // FOR counts down from n to 1
                    } else {
                        let step = if code == PLOOP { self.kernel.pop() } else { 1 };
                        // finished when the index crosses the boundary between limit-1 and limit
                        let diff = index.wrapping_sub(limit);
                        let crossed = (diff ^ diff.wrapping_add(step)) < 0 && (diff ^ step) < 0;
                        (index.wrapping_add(step), crossed)
                    };
                    if done {
                        self.f_unloop();
                        pc += CELL;
                    } else {
                        self.kernel.set(rp, next);
                        pc = target(pc, self.kernel.get(pc));
                    }
                }
                LEAVE => {
                    self.kernel.stack_check_r(3, "leave");
                    self.kernel.pop_r(); // index
                    self.kernel.pop_r(); // limit
                    pc = self.kernel.pop_r() as usize; // exit address
                }
                EXIT | BREAK | DOES => {
                    if code == DOES {
                        self.set_does(pc + CELL); // the code after DOES> belongs to the new word
                    }
                    if self.kernel.get_return_ptr() == entry_rp {
                        return; // the word we were called for is finished
                    }
                    pc = self.kernel.pop_r() as usize;
                    call_depth -= 1;
                }
                ABORT => self.f_abort(),
                EXEC => {
                    self.f_execute();
                    pc += CELL;
                }
                // code fields say what kind a word is; they are never compiled as cells
                DEFINITION | VARIABLE | CONSTANT | ARRAY | CREATED => not_an_xt(code as usize),
                _ => {
                    // a word's cfa: run it, entering its body if it has one
                    match self.enter(code as usize) {
                        Some(body) => {
                            call_depth += 1;
                            self.kernel.push_r((pc + CELL) as i64);
                            pc = body;
                        }
                        None => pc += CELL,
                    }
                }
            }
        }
    }
}