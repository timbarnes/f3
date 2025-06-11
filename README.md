### A tiny Forth in Rust and Forth, based on eForth

This implementation attempts to create a reasonable minimum system in Rust, with as much as possible implemented in Forth.
This is a rewrite of f2, which was a rewrite of tforth.

The goal of this rewrite is improved modularity and testability. Cleaner structure and fewer public functions

For convenience rather than efficiency, the data store is an array[i64], and it uses indirect threading.
Builtin functions are made visible in the data space, which also contains:
- The text input buffer `TIB`
- The text working buffer `PAD`
- A second text buffer `TMP`
- A general area for use by `ALLOT`
- The Forth calculation `STACK`
- The return stack `RET`
- `WORD`, `VARIABLE`, and `CONSTANT` storage

Despite being written in Rust, this program can crash from bad memory accesses, because Forth allows any value to be used as a memory address. It would be possible to check every reference before accessing it, but this is not always done in Forth engines, which are quick to restart.

Additional documentation is available in [doc.md](https://github.com/timbarnes/f3/tree/main/src/doc.md).
