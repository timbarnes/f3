### A tiny Forth in Rust and Forth, based on eForth

This implementation attempts to create a reasonable minimum system in Rust, with as much as possible implemented in Forth.
This is a rewrite of f2, which was a rewrite of tforth.

The goal of this rewrite is improved modularity and testability. Cleaner structure and fewer public functions

Memory is one byte-addressed array of 1 MB, as in the Forth books, with 8-byte cells at aligned addresses (`1 cells` is 8). It holds:
- the dictionary: words, variables, constants, and builtins (each builtin has a dictionary entry pointing at its Rust function), with names and string literals inline, plus space reserved with `allot` or `c,`;
- the text buffers `TIB`, `PAD`, and `TMP`;
- the data stack and the return stack, at the top of memory.

Each word has a classic header: link field, name (a count byte and the characters), code field, and parameter field. Compiled code is indirect-threaded: cells are opcodes, flagged builtin indexes, or the code field addresses of other words. `doc.md` walks through real memory dumps of each kind of word, compared with fig-Forth and F83.

Every memory access is checked, and every error, whether a Forth error or an engine error, is reported and returns the interpreter to a clean prompt. No Forth input can crash it. Errors carry ANS throw codes and can be handled with `catch` and `throw`.

Rust implements only what needs the host, the hot primitives, and the interpreter core; everything else is written in Forth (`src/forth/corelib.fs`). f3 follows ANS Forth where it can, and lists its deliberate deviations in `doc.md`.

`cargo test` runs unit tests, a regression suite, Hayes-style core tests, the reference ANS `core.fr` (all 638 tests pass), and a fuzz test against the real binary. `plan/` holds the engine review and the improvement plan that produced the current design.

Additional documentation is available in [doc.md](https://github.com/timbarnes/f3/tree/main/src/doc.md).
