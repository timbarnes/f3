### A tiny Forth in Rust and Forth, based on eForth

This implementation attempts to create a reasonable minimum system in Rust, with as much as possible implemented in Forth.
This is a rewrite of f2, which was a rewrite of tforth.

The goal of this rewrite is improved modularity and testability. Cleaner structure and fewer public functions

The data store is an array of i64 cells, and compiled code is token-threaded (cells are opcodes, flagged builtin indexes, or word addresses).
The cell heap holds:
- the dictionary: words, variables, constants, and builtins (each builtin has a dictionary entry pointing at its Rust function), plus space reserved with `allot`;
- the data stack and the return stack, at the top of the heap.

A separate byte space holds the text buffers `TIB`, `PAD`, and `TMP`, and all strings.

Every memory access is checked, and every error, whether a Forth error or an engine error, is reported and returns the interpreter to a clean prompt. No Forth input can crash it. Errors carry ANS throw codes and can be handled with `catch` and `throw`.

Rust implements only what needs the host, the hot primitives, and the interpreter core; everything else is written in Forth (`src/forth/corelib.fs`). f3 follows ANS Forth where it can, and lists its deliberate deviations in `doc.md`.

`cargo test` runs unit tests, a regression suite, Hayes-style core tests, and a fuzz test against the real binary. `plan/` holds the engine review and the improvement plan that produced the current design.

Additional documentation is available in [doc.md](https://github.com/timbarnes/f3/tree/main/src/doc.md).
