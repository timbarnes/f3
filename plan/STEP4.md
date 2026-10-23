# Step 4: one byte-addressed memory

Plan date: 2026-10-23, against commit `d0ec76a`. This is step 4 of the roadmap in [ANS.md](ANS.md).

**Goal:** make f3's memory work the way the Forth books describe it.
* There is one byte-addressed space holding the dictionary, the buffers, and both stacks.
* A cell is 8 bytes there, and a character is 1 byte.
* A dictionary entry is laid out as link field, name field, code field, then parameter field, as in fig-Forth and eForth. Only the cell size differs: 8 bytes, not 2.

At the end, `dump`, `see`, and the source should read like the diagrams in *Starting Forth*, *Threaded Interpretive Languages*, or the eForth notes. The reference `core.fr` should pass in full (638 of 638, against 597 today).

**Performance:** none expected. On this M2 Max, a cell load from a byte array (aligned) takes 0.58 ns, against 0.57 ns from today's `i64` array, and f3 spends 1.5–2.5 ns per executed cell on dispatch. Each stage still re-runs `bench.fs` to confirm.

---

## Design decisions

These are fixed up front, so the stages don't re-decide them.

| # | Decision | Choice | Why |
|---|---|---|---|
| 1 | Address unit | **Bytes.** `cells` = `8 *`, `cell+` = `8 +`, `chars` and `char+` = 1. | ANS, and every Forth book. |
| 2 | Cell format | 8 bytes, little-endian, read and written with `i64::from_le_bytes` / `to_le_bytes`. | Safe Rust (no `unsafe`), and the native order on ARM and x86. |
| 3 | Alignment | Cells live at 8-byte-aligned addresses. `@` and `!` at a misaligned address raise ANS error **−23** (address alignment exception). `,` aligns HERE first, so `c,` followed by `,` is safe. `align` and `aligned` do real work. | The CPU doesn't need it. But during the port, an off-by-8 bug then shows up as an immediate error instead of silent garbage, for the cost of one AND per access. It could be relaxed later. |
| 4 | Memory size and map | 1 MB: `[ system variables │ dictionary → … DICT_LIMIT │ buffers: TIB PAD TMP tmp2 #end │ ← data stack │ ← return stack ]`. Each stack is 1024 cells (8 KB). | It keeps today's capacity, 64K cells of dictionary, with room to grow. The buffers sit at fixed addresses below the stacks, near the top, much as classic Forths put the TIB. |
| 5 | Header layout | Classic: **`[link][name: count byte + characters, padded to a cell][code field][parameter field]`**. The link field holds the previous word's link address, with the flags (`IMMEDIATE_FLAG`, `COMPILE_ONLY_FLAG`) in its top bits. | This is the layout the books draw. It replaces f3's trailing back pointer, which is the source of the F30 fix (`,` and `allot` moving the back pointer), the `here = dp - 1` quirk, and `word_closed`. With the link first, `,` and `allot` simply append. |
| 6 | Names | Stored inline in the header. A count byte limits a name to 255 characters. | The books, and it removes string space. |
| 7 | String literals | Inline in the compiled code: `op:strlit`, then the counted string, padded to a cell. The inner interpreter pushes the string's address and skips past it. | The classic `(."` / `(s")` scheme, as in fig-Forth. |
| 8 | Visibility while compiling | `last` is the newest header; `context` is the newest *finished* word, so `find` searches from there. A word becomes findable when `;` or `(close)` sets `context`. | This is the classic SMUDGE, done the way f3 already does it. |
| 9 | Branch and loop operands | An offset in **bytes**, from the operand cell, as now. | The Forth code computes `here -` differences, which are in bytes automatically, so `target(pc, offset)` needs no change. |
| 10 | Execution tokens | An xt is the byte address of the code field; the builtin and opcode flags are unchanged (bits 59–62). | Addresses stay far below 2^56. |
| 11 | Error rollback | `pending_def` saves `here`, `last`, and `context`; `s-here` goes away. | Simpler. |

## What goes away

* **The string space:** `Kernel::strings`, `s-here`, `s-allot` (use `here` and `allot`), `s-save` and `s-create` (replaced by `,"` and `place`), and the string-space rules in `doc.md`.
* **The trailing back pointer:** the F30 machinery in `emit_cell` and `f_allot`, `word_closed`, `(close)`'s back-pointer writing, and `here = dp @ 1-`.
* **The TIB's count byte:** the `'tib @ 1+` offsets, and the "TIB is a counted string" special cases. `source` returns the buffer and `#tib`.
* **Two caveats in `doc.md`:** "character buffers must be in string space", and "a heap address is not a string address".

---

## Stages

Each stage leaves `cargo test` (the 333 core tests, the regression suite, the fuzz test, and the black-box tests) and `cargo clippy` clean. Each is committed separately and benchmarked.

### Stage 0: make the code cell-size-agnostic, in today's model (M, about 2 days)

This is the de-risking step. Every address calculation is rewritten to say what it means, while a cell is still 1 unit, so the tests prove each rewrite is a no-op before anything changes size.

* **Rust:** add `pub const CELL: usize = 1;` (which becomes 8 in stage 1), and route every cell offset through it.
  * `pc += 1` becomes `pc += CELL`; `get(cfa - 1)` becomes `get(cfa - CELL)`; `xt + 2` becomes `xt + 2 * CELL`; return-stack and loop-frame offsets likewise.
  * That's about 70 sites, in `inner.rs`, `compiler.rs`, `runtime.rs`, `general.rs`, `debug.rs`, and the stack accessors in `kernel.rs`.
  * Byte offsets in string space stay as they are.
* **Forth:** in the address arithmetic, `1+` and `1-` become `cell+` and `cell-` (add `cell-`). `2 +` on a cfa becomes `2 cells +`. Counts of cells get `cells`; counts of characters get `chars`.
  * This covers about 170 candidate lines: `corelib` 58, `debug` 26, `core-tests` 65, `sequences` 7, `editor` 7, `regression` 6, `tester` 2, `terminal` 1.
  * Each one is reviewed by hand; plenty of `1+` are arithmetic, not addresses.
  * The test files change too (for example `buf 10 + !` becomes `buf 10 cells + !`).
* **`cells`, `cell+`, and `cell-`** become proper definitions (`CELL *`, `CELL +`, `CELL -`), using a Forth constant `cell` exported from Rust.
* **Checkpoint:** everything passes with `CELL = 1`, and nothing visible has changed.

Then, as an experiment only (not committed), set `CELL = 2` with a 2-wide cell index. It flushes out any arithmetic stage 0 missed, before the real switch.

**Status: done (2026-10-23).**

What changed:
* **Rust:** `kernel::CELL` (1) is used for every cell step and offset: the stacks, the inner interpreter's `pc`, the code-field and link offsets, the hand-built system variables, and the memory-map constants (`DATA_SIZE`, `STACK_START`, `DICT_LIMIT`). It is exported to Forth as the constant `cell`.
* **`top_cells`**, which handed out a slice of the cell array, is replaced by `nth` and `set_nth` (the cell `n` deep), so the primitives no longer depend on the heap being an `i64` array. Same speed.
* **Forth:** `cells`, `cell+`, and `cell-` are defined on `cell`. Every heap-address calculation in `corelib.fs`, `debug.fs`, `sequences.fs`, `tester.fs`, the core tests, and the black-box tests uses them.
  * String-space arithmetic (bytes) and plain numbers were left alone, after a line-by-line review.
  * `editor.fs`, `terminal.fs`, and `regression.fs` needed no changes; their `1+` and `1-` are character positions and numbers.

Results:
* **The `CELL = 2` experiment** found two misses: `find_word`'s start offset (a `wrapping_sub(1)` the search pattern didn't match), and the offsets in the `test_make_word` unit test. With both fixed, every test passes at `CELL = 2`. The reference core.fr then stops only at `1 A-ADDR C!`, which uses a heap address as character memory. That's the two-space limitation stage 2 removes. (At `CELL = 1` that test "passes" by writing to an unrelated string-space address.)
* **F34**, found during the review: `sequences.fs`'s stack words `>s` and `s>` had never worked (`!: stack underflow`). They are fixed, using a shared `(elem)` helper, and tested along with arrays.
* **`debug.fs`:** `dump` now loops over cells with `?do` (it used `incr-for` on raw addresses), and `dump-stringspace` used the stale buffer size 132; it is now 256.

Tests: 31 unit, 51 black-box, and 333 core tests, all passing at `CELL = 1`. The benchmarks are unchanged.

### Stage 1: byte-addressed heap, string space still separate (M, about 2 days)

* **`Kernel::heap` becomes `Vec<u8>`** (1 MB), with `cell(addr) -> i64` and `set_cell(addr, v)` using `from_le_bytes` and `to_le_bytes`, and checks for bounds and alignment (−23).
  * `push`, `pop`, `push_r`, and `pop_r` step by 8.
  * `CELL = 8`.
  * `DICT_LIMIT`, `sp0`, `rp0`, and the system-variable addresses become byte addresses.
  * The hand-built cells in `insert_variables` are laid out on 8-byte boundaries.
* **The inner interpreter** advances `pc` by `CELL`. `target()` is unchanged (decision 9).
* **`,` and `emit_cell`** align HERE first (decision 3). `align` and `aligned` become real; `chars` is still 1.
* **The header** keeps today's trailing-back-pointer layout for now. Only the addresses change size.
* **Checkpoint:** all tests pass, the benchmarks are unchanged, and `core.fr` is still 597 of 638. (`C,` needs stage 2.)

**Status: done (2026-10-23), with two changes to the plan.**

1. **The classic header layout moved here from stage 3** (as stage 1a, still at `CELL = 1`).
   * The reason: with byte addresses, `allot` can leave HERE unaligned. The trailing back pointer, which `,` and `allot` had to drag along, would then have to sit at a misaligned address, or `allot` would have to round, breaking ANS `here 1 allot`.
   * Link-first headers remove the problem. And in f3's memory image the link cell was already where the previous word's back pointer had been; only the writer changed (`create` instead of `;`). So `find` didn't change at all.
   * Gone: `word_closed`, the F30 shuffling in `emit_cell` and `allot`, `(close)`'s back-pointer write, and `here = dp @ 1-`. `forget`, `forget-last`, and `words` walk the links.
   * Names are still pointers into string space; stage 3 makes them inline.
2. **Alignment is checked only by `@` and `!`** (decision 3, narrowed).
   * The engine's own accesses (pc, loop frames, stacks, header fields) always derive from aligned addresses, and checking them cost about 0.5 ns per loop iteration.
   * `@` and `!` are where a wrong address from Forth code arrives, which is what the check is for. Misaligned engine accesses would still be bounds-checked.

Stage 1b as built:
* **The heap** is 1 MB, byte-addressed, with `CELL = 8`, stored as 8-byte words (an aligned cell is one load).
* **`,`** aligns HERE first, `allot` counts bytes, and `align`/`aligned` are real.
* **`RET_START`** had to become `DATA_SIZE - CELL` (it was `- 1`). The alignment check caught this immediately.

Performance:
* `fib(27)` is unchanged at 13–14 ms.
* The tight loop microbenchmarks are slower by about 0.5 ns per iteration (the empty `for` loop is 19 → 23–24 ms), with code layout held constant using `-C llvm-args=-align-loops=64`.
* Code layout alone moves these microbenchmarks by up to 15% (stage 0's own empty loop went from 19 to 23 ms just from that flag), so they are a poor guide on their own. The cost was accepted, and stages 2 and 3 re-measure.
* Word storage, byte storage, and moving the error paths out of line were all tried; none recovered it.

Tests: 31 unit, 51 black-box, and 333 core tests, all passing. core.fr stops at its first `C,` (and at `1 A-ADDR C!`, a heap address used as character memory), as expected until stage 2.

### Stage 2: merge string space into the heap (M, about 3 days)

* **Remove `Kernel::strings`.** `c@`, `c!`, `cmove`, and every `string_*` and `bytes_*` helper address the heap.
* **The buffers** (TIB, PAD, TMP, tmp2, and the pictured-output buffer) move to their fixed region (decision 4). The TIB loses its count byte: `source` = `tib #tib @`, and `>in` indexes from `tib`.
* **Names** are written into the dictionary. For now the name field still points at them.
* **String literals** go inline in the code (decision 7): `op:strlit`, a count byte, the characters, and padding. The inner interpreter pushes `pc + CELL` and advances past the padded string. `see` and the stepper skip them the same way. `s"`, `c"`, `."`, and `abort"` compile this way.
* **Add the words:** `c,`, `,"` (compile a counted string), and `place`. `s-allot` becomes `here swap allot` for any callers that remain; `editor.fs` and `sequences.fs` are updated.
* **Checkpoint:** all tests pass, and `core.fr` should reach 638 of 638.

### Stage 3: the classic header layout (M, about 2 days)

* **Headers** become `[link | flags][count][name…][pad][code field][parameter field]` (decision 5).
  * `find_word` follows the links from `context`, masks the flags, and compares the inline name.
  * The cfa is `link + CELL + aligned(1 + count)`.
* **Add the Forth words** `>name`, `name>`, `>link`, and `>flags`. (Forth-2012 has `name>string`, `name>interpret`, and `name>compile`; adding them later is cheap.)
* **Delete the trailing-back-pointer machinery:**
  * the special cases in `emit_cell` and `f_allot`, and `word_closed`;
  * the back-pointer write in `(close)` (it now only sets `context`);
  * `here = dp @ 1-`, so `here` is just `dp @`.

  `;` no longer emits a back pointer.
* **`forget` and `forget-last`** follow the new links. `words` and `dump` in `debug.fs` walk the new layout.
* **Checkpoint:** all tests pass, `core.fr` passes 638 of 638, and a header in `dump` looks like the books' diagram.

### Stage 4: documentation for study (S–M, about 1 day)

* **`doc.md`:** a memory-map figure with real addresses, a header-layout figure, a worked `see` and `dump` of `: square dup * ;`, and how a colon definition, a variable, a `create … does>` word, and a string literal each look in memory. Every one is cross-referenced to the classic layouts (fig-Forth's link/name/code/parameter fields).
* **`README.md`:** describe the memory model, and drop the "two address spaces" caveat.
* **`ANS.md`:** remove deviation B1 (and its knock-ons), and record the `core.fr` result.
* **Reference suite in CI:** decide whether `core.fr` can be vendored and added to `cargo test`. The Hayes tester is public domain; check the forth2012-test-suite licence first. If it can't, keep the reproduction instructions.

---

## Risks and how they're handled

| Risk | Mitigation |
|---|---|
| Off-by-8 address bugs: the main risk | Stage 0 makes every offset explicit and testable while a cell is 1 unit. Strict alignment (−23) turns most misses into immediate, clear errors. The fuzz test and 330+ core tests run at every stage. |
| Library code (`editor.fs`, `debug.fs`) that isn't covered by tests | Review it by hand in stage 0, then exercise it interactively after stages 1 and 2. The editor needs a terminal, so that check falls to you. |
| Literal strings in code confuse `see`, the tracer, and `dump` | One helper, "the size of the cell at pc, including any inline string", shared by the inner interpreter, `see`, and `debug_step`. |
| Memory size: names and literals now share the dictionary | 1 MB is about 2× today's heap and string space combined. `unused` reports what's left. |
| Speed | Measured up front as negligible. `bench.fs` is checked at every stage, and a stage that slows down by more than 5% gets a look. |

## Effort

About 9–11 working days in total: stage 0 ~2, stage 1 ~2, stage 2 ~3, stage 3 ~2, stage 4 ~1. Each stage is a separate commit, and a working f3 can be released after any of them.
