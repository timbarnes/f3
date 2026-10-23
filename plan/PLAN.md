# f3 Improvement Plan

Companion to [ANALYSIS.md](ANALYSIS.md). Issue IDs (F1…) refer to that document.

Guiding rules:
* **Tests before fixes.** Each confirmed bug becomes a failing test first.
* **Safety before speed.** The engine is fast enough. It isn't safe enough.
* **Keep it small.** Prefer deleting and simplifying to adding layers. No new dependencies are needed for any phase.
* Each phase leaves `cargo test`, `cargo clippy`, and `regression.fs` green, and can ship on its own.

Size key: S = under a day, M = 1–3 days, L = 3+ days.

---

## Phase 0: A safety net (S–M)

Goal: changes can be made without fear.

1. **Fix the build signals.** Fix the 2 clippy deny errors, and the stale `test_compile_builtins` assertion. Mark `test_read_char` `#[ignore]`, because it blocks on stdin.
2. **Capture output.** Give `ForthRuntime` an `out: Box<dyn Write>` (stdout by default, `Vec<u8>` in tests) and route `print!`/`(emit)`/`.s`/`see`/messages through it. This is the only abstraction this phase adds.
3. **Add a test harness.** It's one helper: `fn run(src: &str) -> (Vec<i64> /*stack*/, String /*output*/, Result<..>)`. It boots with `corelib.fs`, evaluates `src` line by line, and returns the result. Put it in `tests/forth.rs` (or `#[cfg(test)]` in `runtime.rs`).
4. **Turn every [confirmed] repro in ANALYSIS.md into a test** (F1–F10, F14, the overflow and divide-by-zero cases). Each one is expected to fail, so mark it `#[should_panic]` or `#[ignore = "F1"]` until it's fixed.
5. **Make `regression.fs` run headless.** Have it exit non-zero on failure (`test-results` → `bye` with a status), and add it as a `cargo test` case. Skip the raw-mode tests when there's no TTY.
6. **Add a benchmark script** (`plan/bench.fs` or `benches/`): `27 fib`, `10000000 for next`, and the time to load the core. Record the baseline: 26 ms / 251 ms.

Exit: CI-able `cargo test && cargo clippy -- -D warnings`, with known bugs tracked as ignored tests.

**Status: done (2026-10-22).**
* Item 2 was replaced by black-box testing. `tests/forth.rs` runs the real `f3` binary on stdin with a 5 s timeout, so there are no engine changes. It also catches process death and hangs, which in-process tests can't, and it will survive the Phase 3 refactor unchanged. Add `out: Box<dyn Write>` later only if a test needs to sit below the process level.
* Item 5: `regression.fs` is checked by matching "All tests passed!" rather than by exit code, so no new word was needed. The raw-mode test skips itself when there's no TTY.
* Two new bugs turned up and are tracked: F24 (`clear` wipes the return stack) and F25 (`-f` runs before the core library).
* Benchmark: `src/forth/bench.fs`.

Commands:
* `cargo test`: unit tests and the baseline Forth tests.
* `cargo test -- --ignored`: the known-bug tests. All of them should fail until their fix lands.
* `cargo clippy --all-targets -- -D warnings`: clean.

---

## Phase 1: Stop crashing (M)

Goal: no Forth input can kill the process, and any error returns to a clean prompt.

1. **Introduce one error type and one recovery path.**
   * `enum ForthError { StackUnderflow, StackOverflow, ReturnStackUnderflow, ReturnStackOverflow, BadAddress(i64), DivideByZero, DictionaryFull, StringSpaceFull, UndefinedWord(String), CompileOnly, Io(String), Abort }`.
   * Change the builtin signature to `fn(&mut ForthRuntime) -> Result<(), ForthError>`. Make `push`/`pop`/`get`/`set`/`push_r`/`pop_r`/`byte_*`/`string_*` return `Result`, and propagate with `?`. Most of the change is mechanical across ~96 builtins, and it also deletes the `stack_check` + `if` wrappers.
   * A single `abort(&mut self)` is the **only** recovery routine. It clears the data, return, and control stacks; sets `STATE = 0`; rolls back a half-built definition (restores `HERE`/`s-here`/`LAST` saved by `:`); pops all include readers back to stdin; and drops raw mode.
   * `quit` catches `Err`, prints it once, calls `abort`, and continues. `eval` stops the line on the first `Err`. This fixes F3, F4, and F11.
   * Keep `catch_unwind` in `main.rs` as a last-resort backstop that recovers instead of calling `resume_unwind`. After Phase 1 it should never fire, so add a test that it doesn't.
   * **Cheaper interim option**, if the full refactor has to wait: change only `main.rs` to call `abort` and loop instead of `resume_unwind`, and install a quiet panic hook. That stops the process dying in an afternoon, but it leaves the corrupted-state bugs in place. Treat it as a stopgap, not the fix.
2. **Make arithmetic wrap, like Forth.** Use `wrapping_add`/`sub`/`mul`/`neg`. Return `DivideByZero` from `/` and `mod`, and handle `MIN / -1` with `wrapping_div`. Mask shifts to 0–63, or return 0 when the shift is ≥ 64. Debug and release must then give the same results.
3. **Bounds-check every write that grows memory.** `,`/`emit_cell`/`allot` check `HERE` against the heap limit (F7). `push_r` checks the return-stack limit (F8). `string_new`/`s-create` check `s-here` and reject lengths over 255.
4. **Fix the input-layer fatals.**
   * stdin EOF (`Ok(0)`) → `bye` (F5).
   * Work on bytes rather than `&str` slices: `read_until(b'\n')` into `Vec<u8>`, and copy bytes. This fixes F14 and stops the silent UTF-8 mangling.
   * Long lines: report an error rather than truncating silently, or raise `BUF_SIZE` to 256 (F6).
   * `(system)` and `write-line` return `Io` errors instead of using `expect`.
   * A failed stdout write (`EPIPE`) → exit cleanly.

Exit: a fuzz-style test (random tokens from the dictionary plus random numbers, 10k lines) runs with no panic, and the process is still responsive after each error.

**Status: done (2026-10-22).** Fixed: F3, F4, F5, F6, F7, F8, F11, F12, F14, F24, and the overflow, divide-by-zero, and underflow crashes. The fuzz test (`fuzz_random_input_never_crashes`) produces about 7k errors and 3k clean lines, with no engine panics. Benchmarks are unchanged (25 ms / 230 ms).

One design decision differs from item 1 above:
* **Errors unwind instead of returning `Result`.** `kernel::throw(msg)` calls `panic_any(ForthError)`. `quit` wraps each line in `catch_unwind`, and the single `ForthRuntime::recover` routine reports the error and resets the system.
* Why:
  * Builtin signatures stay the same, so the diff is a fraction of the `Result` refactor.
  * It adds no cost to the fast path, where `Result` would add a branch to every push and pop.
  * The same catch also makes Rust panics (indexing, overflow) recoverable.
  * The nested `execute` → `i_definition` calls need no error threading.
* Limits:
  * The build must keep `panic = "unwind"`. Never set `panic = "abort"` in `Cargo.toml`.
  * A Rust panic is an engine bug. The panic hook prints `(Rust panic at file:line)` for it, while Forth errors print only `ERROR: …`.
* Phase 5 `catch`/`throw` maps directly onto this: `catch` = `catch_unwind` around `execute`.

Other notes:
* Memory is now 64K cells. The dictionary runs up to `DICT_LIMIT`, and the data stack and return stack each have 1024 cells, all bounds-checked. Phase 3.1 keeps this layout and exposes the stacks to Forth through `sp@` and `rp@`.
* `BUF_SIZE` is now 256, so the largest input line is 255 bytes. A longer line is an error, not a silent truncation.
* An unknown token is now an error in interpret mode as well, and the rest of the line is skipped.
* `abort` is now silent (`throw("")`). The `abort?` variable is no longer used, and the per-instruction abort-flag check is gone.
* New bugs found and tracked: F26 (`abort"`), F27 (negative `allot`), F28 (`builtin-name`), F29 (`'` of an unknown word).

---

## Phase 2: Core correctness (M)

1. **Fix the re-entrant inner interpreter (F1, F19).** `i_definition` records the return-stack depth on entry, and returns when an `EXIT` brings the return stack back to that depth. Remove the `0` sentinel and the `set_return_ptr(RET_START)` reset. `call_depth` for the stepper follows from the same depth.
2. **Use one execution-token format (F2).** An xt is always a cfa (a heap address). A builtin's cfa cell holds `BUILTIN_FLAG|index`, as it does today. Then:
   * `'` and `[']` always return the cfa.
   * `execute` dispatches on `heap[xt]`.
   * The compiler may still compile the flagged token inline for builtins as an optimisation (keep `$compile` as it is). `execute` additionally accepts a flagged token directly.
3. **Make `number?` honour `BASE`** with `i64::from_str_radix`, plus the standard `#` (decimal), `$` (hex), and `%` (binary) prefixes and a leading `-` (F9).
4. **Finish `do … loop` (F10).** Add `+loop`, `leave`, `unloop`, `i`, and `j`. Getting this right is far easier with native `(do)`/`(loop)` opcodes (see Phase 4.3) than with compiled `2r@ < BRANCH0 …` sequences, so do 4.3's loop opcodes here.
5. **Make compile-only words error when interpreted.** Give them a `COMPILE_ONLY` flag bit next to `IMMEDIATE_FLAG`, and have `$interpret` refuse them (F21).
6. **Fix the file words (F13).**
   * Open with `OpenOptions` per mode, including create for W/O and R/W.
   * Drop the `canonicalize` requirement.
   * Store files as `Vec<Option<FileHandle>>` so ids stay stable when a file is closed.
   * Fill in `file-size` from metadata, and keep `file-position` up to date.
   * Bounds-check `read-line` and `write-line`.
7. **Boot fixes (F15).** Load the first core file that's found, and expand `~`. Make `include` paths relative to the including file, so `corelib.fs` doesn't depend on the working directory.
8. **Small fixes.** F16 (`see` constant name), F17 (`dbg`/`debuglevel` symmetry), F18 (delete `'eval`), F20 (remove the debug print in `(system)`). F12 was fixed in Phase 1.
9. **Fix the bugs found during Phases 0–1.**
   * F25: load the `-f` file after the core library. Read each boot file to EOF in order, instead of stacking all the readers.
   * F29: `'`, `[']`, and `(')` raise an error for an unknown word.
   * F26: move `abort"` below `type` in `corelib.fs`, and drop its stray `.s`.
   * F27: correct negative `allot`, and check it against `DICT_LIMIT`.
   * F28: `builtin-name` writes to `tmp @`.

Exit: all Phase 0 bug tests are un-ignored and passing.

**Status: done (2026-10-22).** F1–F29 are all fixed. No test is ignored (apart from one unit test that needs keyboard input), and the fuzz test still passes. Benchmarks: `fib(27)` 21 ms, the empty 10M `for` loop 27 ms (it was 251 ms; native `NEXT`).

How each item was done:
* **Inner interpreter:**
  * `i_definition(start)` returns when an EXIT brings the return stack back to its depth on entry.
  * Calls push the return address straight onto the return stack, which brings Phase 4.1 forward.
  * The empty `i_*` helpers are gone.
  * `execute` dispatches on the xt itself, and throws for anything that isn't an execution token.
* **One xt format:** `'` and `[']` return the cfa for every word. `i_definition` also runs the cfa of a builtin, so `['] dup ,` compiles working code. `$compile` still compiles builtins as their flagged index.
* **Loops:**
  * `do ?do loop +loop leave unloop` and `for next` are now native opcodes (DO, QDO, LOOP, PLOOP, LEAVE, FOR, NEXT; 100013–100019), so Phase 4.3 is done too.
  * Every loop keeps the same three-cell return-stack frame: exit address, limit, index. `i` and `j` therefore work the same for both loop kinds, and `leave` works in `for` loops.
  * `+loop` uses the standard boundary-crossing rule.
  * `0 for` and negative counts now skip the loop. They used to run about 2^64 times.
* **Compile-only words:** a `COMPILE_ONLY_FLAG` (bit 60) and a `compile-only` word mark them. All control-flow words, `;`, `exit`, `[']`, `[char]`, `recurse`, and `abort"` are marked.
* **Number parsing:** `parse_number` honours BASE and the `#`, `$`, and `%` prefixes. Values up to 2^64−1 wrap.
* **File words:**
  * Files open with `OpenOptions` per mode. W/O and R/W create the file.
  * File ids stay stable after `close-file`.
  * `file-size` and `file-position` return `u ior`.
  * `read-line` honours `u` and returns lines longer than `u` in pieces. At end of file it returns `0 FALSE 0`.
  * `write-line` writes exactly `u` characters plus a newline, and returns `ior`.
* **Boot:**
  * Each file is interpreted to the end in order: core first, then the `-f` file.
  * Only the first core file found is loaded, and `~` is expanded.
  * `include` looks next to the including file first, so `corelib.fs` now includes `terminal.fs` and friends by bare name.
  * The startup check that the stack is empty is gone; a user file may leave values.
* **Behaviour changes users may notice:**
  * `'` of a builtin now gives its cfa. Use `' x @` for the flagged index.
  * `'` of an unknown word is an error, and so is `forget` of one.
  * `debuglevel` now returns 0–3, the same numbering `dbg` takes.
  * `allot` is now a builtin.

---

## Phase 3: Architecture cleanup and the Forth/Rust split (M–L)

**The split ("pragmatic minimal", decided 2026-10-22).** A word is written in Rust only if:
1. it needs the host (I/O, files, time, terminal, shell);
2. it's a hot primitive, whose Forth version costs about 5× more per call (arithmetic, comparison, basic stack and memory words); or
3. it's needed before Forth exists, or by error recovery (parsing, `find`, number conversion, `:` `;` `create` `,`, the `quit`/`eval` loop).

Everything else is written in Forth. Measured (release build): a builtin call costs about 3 ns, and a colon-word call adds about 15–20 ns. `rot` as `>r swap r> swap` costs 20 ns against 3.4 ns as a builtin.

Do these in order. Each step is a refactor guarded by the tests.

1. **Keep the stacks in the heap, and expose them to Forth.** This reverses the earlier plan to move them into Rust arrays. Phase 1 already gave each stack a bounded, checked region, so the original reason (collisions) is gone, and addressable stacks let Forth define the stack-inspection words.
   * Add the primitives `sp@` and `rp@`, and the constants `sp0` and `rp0` (the empty-stack positions).
   * Remember the data stack grows down, so the item `n` deep is at `sp@ + n`.
   * Then move these words to Forth: `depth`, `pick`, `roll`, `.s`, `clear`, `.rs`.
   * `i`, `j`, and `r@` stay primitives, because they are hot.
   * The prompt's stack display calls the Forth `.s` by name. `show-stack`/`hide-stack` become a Forth variable that `quit` reads.
2. **Drop the Rust control stack.** Compile-time markers go on the data stack as `addr tag` pairs, which is how `if`/`then` already work. `begin`, `while`, `repeat`, `until`, `again`, `case`, `of`, `endof`, `endcase`, and the loop words change accordingly in `corelib.fs`. Delete `control_stack`, `>c`, `c>`, and their part of `recover`; recovery already clears the data stack.
3. **Move the remaining cold words to Forth.**
   * `see`: the decompiler is about 110 lines of Rust, and `debug.fs` already has the dump machinery.
   * `immediate` and `compile-only` become `last @ dup @ FLAG or swap !`. They are defined at the top of `corelib.fs`, before their first use. Rust keeps a private helper for `;` at cold start.
   * `immed?` and `?unique`. `create` calls the Forth `?unique` by name, or the warning moves into `:`.
   * `s-copy` and `s-create`, on top of a new `cmove` primitive.
   * Delete `interpret`, which is a duplicate name for `eval`, and make `s-move` an internal Rust function rather than a word.
4. **Add the primitives that make Forth versions practical.** `xor` (it's missing; `invert`, `negate`, and others need it), `cmove ( src dst n -- )` in string space, and `u<`. Then define `invert`, `negate`, `abs`, and friends in Forth on top of them.
5. **Make opcodes self-describing.**
   * Replace the `100000+` magic numbers with a flag-bit encoding (for example `OPCODE_FLAG = 1<<59 | n`), so the heap can grow past 100000 cells without ambiguity.
   * Export the opcode and flag values to Forth from Rust (`compile_builtins` defines them as constants), instead of duplicating literals in `corelib.fs`.
6. **Make the outer-interpreter internals plain Rust functions.**
   * `find`, `parse`/`(parse)`, `number?`, `create`, `query`/`accept`: Rust arguments and returns (`fn find(&self, name: &[u8]) -> Option<usize>`, `fn parse_word(&mut self, delim: u8) -> Option<Range<usize>>`).
   * The builtin of the same name becomes a thin wrapper.
   * The outer interpreter calls the Rust versions directly. That removes most of the internal pushing and popping, and the class of "junk left on the stack" bugs.
7. **Tighten the kernel API.** Make `kernel` private. Cache only the system-variable addresses that are really needed; several `*_ptr` fields (`hld_ptr`, `tib_ptr`) could be constants, because their addresses never change.
8. **Write down the string model.** Keep the separate byte space:
   * counted strings up to 255;
   * `c@`/`c!` address string space and `@`/`!` address the heap;
   * provide `s-allot` for byte buffers.

   A single byte-addressed space would be more standard, but it means rewriting every string word. Leave it unless ANS compatibility becomes a goal.
9. **Delete the dead code.** Remove `utility.rs`, `Config::run`, `insert_code`, the unused `abort?` variable, the commented-out blocks, and the stray files (`test.js`, `.DS_Store`, `out.txt`, `*~`, `#*#`), and add them to `.gitignore`. (The empty `i_*` functions and the `BUILTIN` opcode went in Phase 2.)

Exit:
* About 18 builtins have moved to Forth or been deleted.
* The `.rs` sources are smaller, and no `pub` fields are left on `Kernel`.
* All tests pass, and the benchmarks are within 5% of the Phase 2 numbers (21 ms / 27 ms). Moved words are all cold paths, so a measurable slowdown would mean something went wrong.

---

## Phase 4: Performance (M)

Goal: make **Forth-defined words** cheaper, which is the change that supports the minimal-engine split. Moving more words into Rust is not the goal. Re-measure after each step, and keep only the changes that pay off.

Already done in Phase 2: the direct call path (return addresses go straight to the return stack), the per-cell abort-flag check is gone, and native loop opcodes (the empty 10M `for` loop went from 251 ms to 27 ms).

1. **Add a call-overhead benchmark** to `bench.fs`: a colon `rot` (`>r swap r> swap`) against the builtin `rot`. It measures the cost of a colon call, about 17 ns today.
2. **Take the per-instruction work out of the inner loop.**
   * Gate `debug_step` behind one `bool` (`self.tracing`), set by the stepper words, rather than reading two heap cells on every cell executed.
   * Alternatively, keep two copies of the loop, one plain and one that traces.
3. **Cheapen calls.**
   * Classify a call target at compile time. Compile a builtin's cfa as its flagged index, so `['] x ,` code doesn't pay the extra "is this a builtin's cfa?" load on every call.
   * The EXIT check (`return_ptr == entry_rp`) stays, because it is what makes re-entry safe.
4. **Cheaper primitive bodies.** One bounds check per primitive instead of `stack_check` plus a per-`pop` check: check depth once, then use unchecked pops, or use helpers like `pop2()`.
5. **Buffer the output.** Use a `BufWriter` on stdout, flushed at the prompt, on `key`/`key?`, on `flush`, and before `(system)`/`ms`. Printing stays in Forth; this just makes each `(emit)` cheap.
6. **Remove allocations from the outer interpreter.** `number?` parses from `&[u8]` with no `String`. `$compile` reuses the cfa it already found for immediate words, instead of looking it up again.
7. **Low priority.** `[profile.release] lto = true, codegen-units = 1`. A hash index for `find`, but only if loading the core ever becomes noticeably slow (it takes about 6 ms today).

Dropped: the earlier item 4.4, which would have written `.`, `type`, `1+`, `2dup`, and similar words in Rust. It contradicts the split above. Printing a number in Forth takes about 2 µs.

Targets: colon-call overhead ≤ 8 ns (from ~17), and `27 fib` ≤ 12 ms (from 21). Beyond that, a closure-compiled or JIT design would be a different project.

---

## Phase 5: Standards and polish (optional, L)

* Run the Hayes/Gerry Jackson ANS core test suite (`tester.fs` + `core.fs`). Add words or fix semantics until it passes, or record the deliberate deviations (the two address spaces, for example).
* Add `catch`/`throw` on top of `ForthError`, which falls out almost free from Phase 1.
* Update `doc.md` and `README.md`: memory map, error model, xt format, and the remaining known deviations.

---

## Suggested order and checkpoints

| Phase | Depends on | Checkpoint |
|---|---|---|
| 0 Safety net | none | Tests and benchmark exist, and known bugs are captured |
| 1 Stop crashing | 0 | Fuzz run with no panics, and errors return to a clean prompt |
| 2 Correctness | 1 | All F-tests pass |
| 3 Architecture | 2 | Cold words in Forth, stacks addressable via `sp@`/`rp@`, internals in plain Rust, dead code gone |
| 4 Performance | 3 | Colon-call overhead ≤ 8 ns, `fib` ≤ 12 ms |
| 5 Standards | 2 | ANS core tests |

If time is short, **Phase 0 + Phase 1.1 + Phase 2.1/2.2** fix the worst user-visible problems: process death, corrupted state after errors, and `execute` inside a definition.
