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
* Memory is now 64K cells. The dictionary runs up to `DICT_LIMIT`, and the data stack and return stack each have 1024 cells, all bounds-checked. This is the interim layout for Phase 3.1.
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

---

## Phase 3: Architecture cleanup (M–L)

Do these in order. Each step is a refactor guarded by the Phase 0 tests.

1. **Take the stacks out of the heap.**
   * `data: [i64; N]` + `sp` and `ret: [i64; M]` + `rp`, as `Kernel` fields with one bounds check each.
   * The dictionary then gets the whole heap. Size it at 64K cells and let the return stack be about 1K.
   * Nothing in the `.fs` sources addresses the stacks through the heap (checked), so the only users are `.s`, `depth`, `pick`, `roll`, `i`, `j`, `r@`, and `.rs`.
   * This removes the dictionary/stack collision as a class of bug, and the `STACK_START` arithmetic with it.
2. **Make opcodes self-describing.** Replace the `100000+` magic numbers with a flag-bit encoding (for example `OPCODE_FLAG = 1<<60 | n`) so the heap can grow past 100000 cells without ambiguity. Export the values to Forth from Rust (`compile_builtins` defines the constants) instead of duplicating literals in `corelib.fs:33-56`.
3. **Make the internals plain Rust functions.** Rewrite `find`, `parse`/`(parse)`, `number?`, `immediate?`, `create`, `query`/`accept`, and `include` as Rust functions with Rust arguments and returns (`fn find(&self, name: &[u8]) -> Option<Cfa>`, `fn parse_word(&mut self, delim: u8) -> Option<Range<usize>>`). The builtin of the same name becomes a 3-line wrapper that moves values between Forth and Rust. The outer interpreter calls the Rust versions directly. This removes most of the internal pushing and popping, and the class of "junk left on the stack" bugs.
4. **Tighten the kernel API.**
   * Make `kernel` private, and have `ForthRuntime` expose `include(path)` and `eval(line)` so that `main.rs` stops building strings in `tmp` by hand.
   * Cache only the system-variable addresses that are really needed. Several `*_ptr` fields (`hld_ptr`, `tib_ptr`) could become constants, because their addresses never change.
5. **Settle the string model.** Keep the separate byte space, which is simpler than packing bytes into cells, but write it down:
   * counted strings up to 255;
   * `c@`/`c!` address string space and `@`/`!` address the heap;
   * provide `s-allot` for byte buffers.

   The alternative is a single byte-addressed space with cells overlaid (`i64::from_le_bytes`). That's more standard, but it's a rewrite of every string word and `corelib.fs`. **Recommendation:** don't do it unless ANS compatibility becomes a goal.
6. **Delete the dead code.** Remove `utility.rs`, the empty `i_*` functions, the `BUILTIN` opcode, `Config::run`, `insert_code`, `ALLOC_START`, the commented-out blocks, and the stray files (`test.js`, `.DS_Store`, `out.txt`, `*~`, `#*#`), and add them to `.gitignore`.

Exit: `kernel.rs` + `inner.rs` + `compiler.rs` are smaller than today, and no `pub` fields are left on `Kernel`.

---

## Phase 4: Performance (M)

Re-measure against the Phase 0 baseline after each step, and keep only the changes that pay off.

1. **Use a direct call path.** With separate stacks (3.1), a call becomes `ret.push(pc+1); pc = cfa+1` and a return becomes `pc = ret.pop()`. Don't round-trip through the data stack. Expect the biggest single gain on `fib`.
2. **Take the per-instruction work out of the inner loop.**
   * Errors now propagate by `Result`, so the per-cell `get_abort_flag()` load goes away.
   * Gate `debug_step` behind one `bool` field (`self.tracing`) that is set when `stepper` is written through `!` and by `step-on`/`trace-on`. Alternatively, keep two copies of the loop, one plain and one that traces.
   * Drop the `pc < DATA_SIZE` branch, because `get` is checked anyway.
3. **Add loop opcodes.** `(next)` (decrement the top of the return stack and branch if it's still non-zero), `(do)`, `(loop)`, and `(+loop)`. `for…next` drops from 7 dispatches per iteration to 1.
4. **Make hot words primitives.** Write `.`, `u.`, `.r`, `type`, `tell`, `spaces`, `cr`, `1+`, `1-`, `2dup`, `nip`, `tuck`, `>`, `<>`, `0>`, `negate`, `abs`, `min`, and `max` in Rust. Output words write whole strings rather than one char at a time.
5. **Buffer the output.** Wrap `out` (from 0.2) in a `BufWriter`, and flush it at the prompt, on `key`/`key?`, on `flush`, and before `(system)`/`ms`.
6. **Remove allocations from the outer interpreter.** `number?` parses from `&[u8]` with no `String`. `$compile` reuses the cfa it already found for immediate words, instead of looking it up again.
7. **Low priority.** Builtin dispatch through `get_unchecked` once the flag is validated at compile time. `[profile.release] lto = true, codegen-units = 1`. A hash index for `find`, but only if loading the core ever becomes noticeably slow (it isn't today).

Target: `27 fib` ≤ 12 ms and the empty `for` loop ≤ 80 ms, about 2–3× faster. Beyond that, a closure-compiled or JIT design would be a different project.

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
| 3 Architecture | 2 | Stacks out of the heap, internals in plain Rust, dead code gone |
| 4 Performance | 3 (4.1 needs 3.1) | 2–3× on the benchmarks |
| 5 Standards | 2 | ANS core tests |

If time is short, **Phase 0 + Phase 1.1 + Phase 2.1/2.2** fix the worst user-visible problems: process death, corrupted state after errors, and `execute` inside a definition.
