# f3 Engine Review — Analysis

Review date: 2026-10-22, against commit `2ca8f01` (main).
Scope: the Rust engine (`src/*.rs`, `src/internals/*.rs`) and the bootstrap Forth in `src/forth/corelib.fs`.
Applications built on top (`editor.fs`, `ed2.fs`, `sequences.fs`, …) were only checked for dependencies on engine internals.

Every item marked **[confirmed]** was reproduced against the debug or release binary. The repro is given so it can become a regression test (see PLAN.md, Phase 0). Items without the tag come from reading the code.

---

## 1. Architecture as it stands

### 1.1 Components

| Module | Role |
|---|---|
| `kernel.rs` | `Kernel`: `heap: [i64; 10000]`, `strings: [u8; 10000]`, builtin table `Vec<BuiltInFn>`, stack pointers. Push/pop/get/set and counted-string helpers. |
| `runtime.rs` | `ForthRuntime`: owns `Kernel`, the control stack, cached addresses of system variables (`here_ptr`, `state_ptr`, …), the reader stack, open files. Builds the dictionary (`insert_variables`, `compile_builtins`). |
| `internals/inner.rs` | `i_definition`, the inner interpreter (address interpreter) for colon definitions. |
| `internals/compiler.rs` | Outer interpreter: `quit`, `eval`, `$compile`, `$interpret`, `find`, `number?`, parsing, `:`/`;`/`create`, `see`. |
| `internals/general.rs` | Arithmetic, stack, memory, string, and timer primitives. |
| `internals/console.rs`, `files.rs`, `terminal.rs` | I/O: line reader stack (stdin + nested includes), file words, raw mode via crossterm. |
| `internals/debug.rs`, `messages.rs` | Stepper/tracer, message levels. |
| `main.rs`, `config.rs` | Boot (load core files), REPL loop wrapped in `catch_unwind`. |
| `utility.rs` | **Not part of the build.** `main.rs` never declares `mod utility`, so it and its tests are dead. |

### 1.2 Memory map (cells in `heap`)

```
0 ........................ HERE ->         <- SP  4999 | 5000 .......... <- RP  9999
[ system vars | dictionary grows up ]  [ data stack grows DOWN ] [ return stack grows down ]
```

* `STACK_START = 4999`, and the data stack grows **down**, toward the dictionary. The comment in `kernel.rs:16` says "stack counts up", which is wrong.
* After `corelib.fs` and its includes load, `HERE = 4328`. That leaves **~670 cells** for all further definitions and the data stack combined. Nothing checks for a collision.
* The return stack gets 5000 cells, which is far more than it needs. `ALLOC_START` is declared but never used.
* String space is a separate `[u8; 10000]`. After boot, `s-here = 4762`. Strings are counted with a `u8`, so the maximum length is 255. Nothing checks for that either.
* There are two address spaces. `@`/`!` index `heap` and `c@`/`c!` index `strings`, so the same number means different memory depending on the word. This is non-standard, and it means a byte buffer cannot be made with `here`/`allot`.

### 1.3 Threading model

A colon definition looks like this: `[back-ptr][nfa][DEFINITION][xt…][EXIT][back-ptr]`. Each compiled cell is one of:
* an opcode: a magic number `100000..100012` (`LITERAL`, `BRANCH`, …), possibly followed by an operand;
* a builtin token: `BUILTIN_FLAG (1<<61) | index` into the builtin table;
* a cfa: the heap address of another word.

Opcodes and addresses can only be told apart because `DATA_SIZE (10000) < 100000`. If the heap ever grows past 100000 cells, they become ambiguous.

`i_definition` is a loop with a local `pc`. A call pushes `pc+1` onto the data stack and then moves it to the return stack with `f_to_r`. A sentinel `0` on the return stack marks the outermost return. When that sentinel is popped, the function **resets the return pointer to `RET_START`**. That reset is the root cause of the `execute` bug in F1.

### 1.4 Error model

* Primitives either `panic!` (stack underflow, string bounds, `>c` with an unknown tag, stdin EOF), or print a message and call `f_abort`, which sets a heap flag.
* `main.rs` wraps the REPL in `catch_unwind`, prints "Resetting interpreter to prompt", and then calls **`resume_unwind`**, which kills the process. So any panic is fatal, despite the message. Clippy also flags this loop as "never loops".
* The abort flag is checked only inside `i_definition`. `eval` does not check it, so the rest of the input line keeps executing after an abort.
* `f_abort` clears the stacks. It does **not** reset `STATE`, the control stack, `HERE`, `s-here`, or the reader stack.

### 1.5 Architectural problems, ranked

1. **Failure is not modelled.** Panics, a heap flag, and printed messages are mixed together, and no single path restores a consistent state. That one decision causes most of the crashes and the corrupted-state bugs.
2. **Rust internals pass values through the Forth data stack.** `find`, `parse-to`, `(parse)`, `number?`, `immediate?`, `create`, and `query` push and pop arguments on the user's stack, even when they call each other from Rust. Every internal call pays for bounds checks. A mistake in the middle leaves junk on the user stack. The code is also hard to read, because a "function call" is a sequence of pushes and pops. These functions should be plain Rust functions (`find(&[u8]) -> Option<usize>`) with thin Forth wrappers around them.
3. **The stacks live in the dictionary's heap with no guard bands.** Dictionary growth, data stack, and return stack all overwrite each other silently (F7, F8). Neither stack needs to be addressable from Forth: no `.fs` file uses `sp@` or `rp@` against the heap. They can become separate bounded arrays.
4. **The inner interpreter is not re-entrant.** `execute` calls `i_definition` recursively, and that call clobbers the outer return stack (F1).
5. **The execution token format is inconsistent.** `'` returns a flagged builtin token for builtins and a cfa for colon words. `execute` handles neither builtin form correctly (F2). `[']` and `,` depend on the flagged form.
6. **The kernel's encapsulation leaks.** `kernel` is `pub`, and `main.rs` boots by writing strings into `tmp` and pushing pointers. Opcode values are duplicated as literals in `corelib.fs`.
7. **I/O and the engine are coupled.** `print!` and `io::stdout()` are called directly throughout, so engine output cannot be captured in tests.

---

## 2. Functional issues

| # | Issue | Repro / location |
|---|---|---|
| F1 | **[confirmed]** Running `execute` on a colon word inside a definition destroys the caller's return stack, and the process dies. | `: inc 1 + ; : t ['] inc execute 100 + ; 1 t .` → panic `r>: Return stack underflow`. Cause: `inner.rs:82` resets `return_ptr = RET_START` when the nested call finishes. The same bug is likely behind the panic after `regression.fs` finishes. |
| F2 | **[confirmed]** `' dup execute` doesn't execute `dup`. Instead it pushes garbage. | `3 ' dup execute .s` → `[ 3 2305843009213693972 ]`. `f_execute` reads `heap[index]` instead of dispatching on the builtin flag. |
| F3 | **[confirmed]** An error inside `: … ;` installs a truncated word. | `: bad 1 nosuch 2 ;` then `see bad` → `: bad 1 2 ;`. `eval` ignores the abort flag, so the `;` that follows still runs. |
| F4 | **[confirmed]** An unterminated definition swallows every later line, including `bye`. With piped input it then spins forever. | Input `: aaaa…(260 chars) 1 ;`, then `bye`: the line is truncated, `;` is lost, `bye` is compiled, and the process never exits. `STATE` is not reset on abort. |
| F5 | **[confirmed]** EOF on stdin loops forever. | `printf '1 .\n' \| f3` hangs. `read_line` returns `Ok(0)`, which is treated as an empty line. The `panic!("EOF in stdin")` branch can't be reached. |
| F6 | **[confirmed]** Input lines are silently truncated to 131 bytes. | `accept` with `BUF_SIZE-1`. `corelib.fs:230` is already 135 chars (only a comment is lost for now). |
| F7 | **[confirmed]** Dictionary overflow runs through the data stack and the return stack, and then panics. It can also corrupt silently: `: fill 1000000 for 1 , next ; fill` ends with no error and HERE = 9998, because `,` overwrote the loop counter on the return stack. The next definition then panics. | 800 lines of `1 , …` → `index out of bounds: the len is 10000`. `f_comma` and `emit_cell` never check. |
| F8 | **[confirmed]** Unbounded recursion corrupts the whole heap, because `push_r` has no check. | `: deep recurse ; deep` hangs instead of reporting an overflow. |
| F9 | **[confirmed]** `number?` ignores `BASE`. | `hex ff .` → `ff` is not recognised, and the following `.` underflows and kills the process. `hex`/`binary` affect output only. |
| F10 | **[confirmed]** `do … loop` is broken (still in progress). | `corelib.fs:260,265`: `['] <` and `['] >r` are missing their `,`. `: lp 3 0 do i . loop ; lp` leaves junk on the stack. |
| F11 | The control stack is not cleared on abort. Stale markers from an earlier error break the next definition. | `runtime.rs` `control_stack`, `f_abort`. |
| F12 | `>c` panics on an unknown tag. On underflow, `c>` prints an error and pushes nothing, so the caller's next `drop` underflows. | `runtime.rs:146,179` |
| F13 | File words: | `files.rs`, `console.rs` |
| | • R/W and W/O modes open with `File::open`, which is read-only. Any write fails, and the `expect` panics. | `files.rs:51`, `console.rs:301` |
| | • `canonicalize` requires the file to exist, so new files can't be created. | `console.rs:210` |
| | • `close-file` uses `Vec::remove`, which shifts the ids of every file opened later. | `console.rs:243` |
| | • `file-size` is always 0. `file-position` is not updated by `read-line`. | |
| | • `write-line` slices `[0..chars-1]`, which panics for 0 or for a length beyond the string. It also drops the last char. | `console.rs:296` |
| | • `read-line` ignores `u`. Lines longer than 255 chars wrap the count byte and overflow `TMP` into the dictionary's name strings. | `console.rs:270`, `kernel.rs:246` |
| F14 | **[confirmed]** Non-ASCII input can kill the process: `&line[..131]` slices inside a UTF-8 char. Otherwise, `c as u8` silently corrupts every non-ASCII char. | 130×`a` + `é` → panic at `console.rs:92`. |
| F15 | Boot loads **every** path in `DEFAULT_CORE` instead of stopping at the first one found. With both `./corelib.fs` and `src/forth/corelib.fs` present, the core is defined twice. `~/.f2/…` is never expanded, so it never matches. | `main.rs:54`, `config.rs:6` |
| F16 | `see` on a constant prints the name's string address instead of the name. `?unique` prints a heap value labelled as an address. | `compiler.rs:520`, `compiler.rs:258` |
| F17 | `dbg` takes 0–3, but `debuglevel` returns 1–4. | `debug.rs:24-45` |
| F18 | `'eval` is created and then orphaned, because `state_ptr` is assigned twice. | `runtime.rs:317-319` |
| F19 | `execute` on `BRANCH`, `BRANCH0`, or `ABORT` calls the empty `i_branch`, `i_branch0`, or `i_abort` and does nothing. `EXIT`/`BREAK` through `execute` pops the return stack onto the data stack. | `compiler.rs:80-84`, `inner.rs:192-208` |
| F20 | `(system)` prints a debug line ("Adding …") for each argument, and panics (`expect`) if the command isn't found. | `console.rs:39,42` |
| F21 | Compile-only words (`if`, `for`, `next`, …) used while interpreting do odd things without any warning. | `5 for i . next` at the prompt prints `0` and leaves `5` on the stack. |
| F22 | `cargo test` has one failing test (`test_compile_builtins` expects `<` at index 5). `files::tests::test_read_char` blocks waiting for interactive stdin. | |
| F23 | `cargo clippy` fails with 2 deny-level errors (`stack_ptr <= 0`, and the loop in `main.rs` that never loops). | Fixed in Phase 0, along with F22. |
| F24 | **[confirmed]** `clear` resets the return stack as well as the data stack. Used inside a definition, it destroys the return address, and the process dies. This was the panic at the end of `regression.fs`; it only showed up because test 82 fails when no terminal is attached. | `: z 1 clear 2 ; z` → `r>: Return stack underflow`. `f_clear` → `kernel.reset()`. |
| F26 | **[confirmed]** `abort"` neither prints its message nor aborts. It is defined in `corelib.fs` before `type`, so its `['] type` compiles 0, and a compiled 0 silently ends the word. It also contains a stray `.s` that prints the stack while compiling. | `: t abort" oops" ; 1 2 t` → no message, stack `1 2 <addr>`. `corelib.fs:327`. |
| F27 | `allot` with a negative count *grows* the dictionary: `here @ swap - here !` subtracts a negative number. | `corelib.fs:195-204` |
| F28 | `builtin-name` writes the name to string address `tmp_ptr`, which is the heap address of the `tmp` variable, rather than to the TMP buffer. It lands inside the TIB. | `general.rs` `f_builtin_name` |
| F29 | `'` and `[']` on an unknown word push 0 rather than raising an error. `['] nosuch` inside a definition compiles a 0, which silently ends the word at run time. This is the root cause of F26. | `compiler.rs` `f_tick_p`, `corelib.fs:150` |
| F30 | **[confirmed]** `allot` or `,` after a finished word moves HERE without moving the back pointer at HERE−1, so the next definition links to a data cell and cuts the dictionary chain: every earlier word disappears. This breaks the classic `variable buf 10 allot` and `variable x 1 , 2 ,` idioms. Found in Phase 5. | `variable buf 10 allot : foo 7 ;` → `+ ?`, `bye ?`. Fixed: both carry the back pointer up when the latest word is finished (LAST = CONTEXT). |
| F31 | `case` kept its key on the return stack, so the default clause couldn't see the key. That differs from ANS, and from what `editor.fs` was written for: its escape-sequence and default handling were wrong. Found in Phase 5. | Fixed: ANS `case` (`of` = `over = if drop`, `endcase` drops the key). |
| F25 | **[confirmed]** A file given with `-f` runs **before** the core library. Boot pushes both readers onto the reader stack, and the last one pushed is read first. | A file containing `7 .` → `token not recognized: "."`. `main.rs` `boot_forth`. |

---

## 3. Safety: everything that can kill the process

Any panic is fatal today (§1.4). The reachable panic sources, grouped:

**Explicit `panic!`.** Stack underflow and overflow (`kernel.rs:103,115,143,156,190,207`), `byte_get`/`byte_set` out of range, `>c` with a bad tag, stdin EOF (unreachable, see F5).

**Unchecked indexing (reachable from Forth with ordinary values):**
* `heap[addr]` in `get`/`set`/`incr`/`delta`/`top`/`peek`/`push_r`/`pop_r`. `@` and `!` check their address, but every internal user of a stored pointer (`here`, `context`, `last`, `pad`, `tmp`, `'tib`, `>in`, `#tib`, `stepper-depth`, and the branch targets) does not. Example: `-1 here !` breaks the next definition.
* `strings[…]` in `string_new`/`save`/`set`/`get`/`copy`/`equal`/`slice`, in `(parse)` (`buffer[i]` is tested before `i < end`), and in `s-copy`/`s-create`/`s-move`, all with user-supplied addresses.
* `get_builtin(index)` in `builtin-name` with any integer.
* `i_definition` sends a compiled cell that is neither an opcode nor a valid address to `pc = code as usize`. A negative value becomes a huge `pc`, and the loop then treats `pc` itself as the code.

**Arithmetic.** These panic in debug builds and wrap or misbehave in release:
* `+ - *` overflow **[confirmed]**;
* `/` and `mod` by zero **[confirmed, both builds]**;
* `i64::MIN / -1`;
* `lshift`/`rshift` with a shift ≥ 64 or negative;
* `usize` subtraction in `find` (`context - 1`), `immediate?` (`cfa - 1`), `see`, `?unique`, `BRANCH` offsets, and `stack_check_r`.

Forth semantics call for wrapping arithmetic, so the results should be identical in debug and release.

**`unwrap`/`expect`.** `(system)` when the command isn't found, `write-line` I/O errors, and `stdout` write or flush failures, such as a broken pipe when piping into `head`.

**Integer to `u8` truncation.** String lengths over 255 (`string_new`/`save`/`set`/`copy`) write a wrapped count byte and then keep copying past the buffer.

Panics are not the only safety problem. **Silent corruption** is worse, and it has no detection at all: dictionary into stack (F7), return stack into everything (F8), string buffers into each other (`PAD`→`TMP`→names, for example the error message in `(')` that grows `PAD`), and `allot` with no bound.

---

## 4. Performance

Baseline, release build on the dev machine:

| Benchmark | Time | Per unit |
|---|---|---|
| `27 fib` (recursive, ~630k calls) | 26 ms | ~40 ns / call |
| `10000000 for next` (empty loop, 6 primitives per iteration) | 251 ms | ~25 ns / iteration, ~4 ns / primitive |

That is roughly 3–5× slower than a straightforward Rust token-threaded interpreter. It's acceptable for interactive use, so performance comes **after** safety. Where the time goes:

1. **The call path goes through the data stack.** A call does a `push` and then `f_to_r` (`stack_check` + `pop` + `push_r`). A return does `f_r_from` (`stack_check_r` + `pop_r` + `push`) and then `pop`. That's six checked operations where one would do.
2. **There are per-instruction loads in the inner loop.** `get_abort_flag()` (a heap load) and `debug_step()` (two heap loads plus a compare) run on every cell executed, and `pc < DATA_SIZE` is a branch.
3. **Every primitive is double-checked.** `stack_check(n)` runs first, then each `pop`/`push` re-checks with its own panic branch. `pop2_push1` checks three times.
4. **Loops compile to many primitives.** `for…next` compiles to `r> 1 - dup 0= BRANCH0 … >r`, which is seven dispatches per iteration. A single `(next)` opcode would do the same work in one.
5. **Output goes through Forth one character at a time.** `.` is recursive Forth (`u.` → `/mod` → `.d` → `emit`), and `type` runs `for … c@ emit next`. Each `(emit)` locks stdout and writes one UTF-8-encoded char.
6. **The outer interpreter allocates.** `number?` makes a `String` for every token (`string_get`). Immediate words are looked up twice (`$compile` → `$interpret` runs `find` again). `find` is a linear scan, which is fine at today's dictionary size.
7. **Builtin dispatch** does `&self.kernel.get_builtin(code)` (a `Vec` bounds check), after an `address <= max_builtin()` check that does `len()-1` each time.

---

## 5. Housekeeping

* Dead code: `utility.rs` (not compiled), `i_branch`/`i_branch0`/`i_abort`/`i_exit`/`i_literal`/`i_strlit`, the `BUILTIN` opcode, `Config::run`, `insert_code`, `ALLOC_START`, and large blocks of commented-out code.
* Stray tracked files: `.DS_Store`, `test.js`, `src/forth/out.txt`. Untracked editor droppings in `src/forth/`: `#wrk.fs#`, `*.fs~`.
* `FileHandle::new_file` builds a `message_handler` it never uses, and takes a `msg_handler` parameter that is only used on error.
* Doc strings contain errors: `*` is documented as `j-k`, `-` as `j+k`, `now` as `c!`, and the `stack counts up` comment is wrong.
