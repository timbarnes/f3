# f3 Documentation

f3 is written and tested on MacOS, but does not use any MacOS-specific calls, so it should compile on Linux and Windows.
The implementation is a combination of Rust and Forth - the core capabilities are implemented in Rust, including the memory model, the compiler and interpreter, and a range of basic operations like arithmetic and stack operations. The release binary is about 0.8 MB.

All of memory is one byte-addressed array: the dictionary (words, variables, names and string literals), three buffers, and the data and return stacks. Cells are 8 bytes (i64), at aligned addresses; characters are single bytes in the same memory. Builtin words are implemented through a separate jump table of function pointers.

Compilation results in the address of words being stored so the engine can simply jump to the code for any referenced word. For builtin functions, the index of the function pointer is stored, with a flag to indicate that it's a builtin function.

Names are counted strings (the first byte is the length), stored inline in each word's header. String literals are counted strings compiled inline, after `op:strlit`, padded to a cell.

Three string buffers, 256 bytes each, sit between the dictionary and the stacks:

- TIB - the text input buffer is the location where input is loaded for parsing, a line at a time. `source` is `'tib @ #tib @`.
- PAD - a working area used by `parse-to` (and words built on it, like `text` and `char`). The interpreter itself reads tokens in place in the TIB.
- TMP - a second working area where strings are staged. `s"` (interpreted), `read-line`, and `builtin-name` use it.

The dictionary is a linked list, implemented directly in memory, using link fields to string words together like this (each box is a cell, except the name, which takes as many cells as it needs). The order is F83's (Laxen and Perry); fig-Forth puts the name first (see [Memory, word by word](#memory-word-by-word)):

| lfa                | nfa                                   | cfa                 | pfa                          |
| ------------------ | ------------------------------------- | ------------------- | ---------------------------- |
| link field         | name field                            | code field          | parameter field ...          |
| the previous word's link field, with the IMMEDIATE and COMPILE_ONLY flags in its top bits (0 ends the chain) | a count byte, then the characters, padded to a cell | indicates word type | additional data for the word |

`context` and `last` hold name field addresses. `name> ( nfa -- xt )` steps over the name; going the other way, `>name ( xt -- nfa )` searches the dictionary, because names vary in length. `>link ( xt -- lfa )` and `>flags ( xt -- flags )` are built on it.

So for example, a word defined as follows: `: double 2 * ;` would be stored like this:

| n    | n+1        | n+2        | n+3     | n+4 | n+5            | n+6  |
| ---- | ---------- | ---------- | ------- | --- | -------------- | ---- |
| link | 6 "double" | DEFINITION | LITERAL | 2   | address of \*  | EXIT |
| the previous word's link field, and the "immediate" flag as required | the name (7 bytes, padded to 8) | indicates this is a `:` (colon) definition | indicates the next value should be pushed on the stack | the value to push on the stack | address includes a flag indicating `*` is a builtin | acts like a return, ending execution of this word |

The builtin flag disinguishes between a builtin function accessed through the jump table, and a word defined in Forth. This lets the interpreter know to look in the builtin array for a function pointer, rather than looking in data space for a definition.

This system I believe is roughly equivalent to indirect threading, which allows a simple state-machine like function to step through a definition, executing words in sequence on the basis of their code addresses. When a new word is entered, the interpreter pushes a return address on the return stack. At the end of the execution of a word (or when the `exit` word is called explicitly), the return stack is popped and the program counter updated accordingly.

The definitions include their names, because the compiler is incremental. When a new word is defined in terms of other words, the dictionary is searched back from the newest word. Once found, we compile the address of the word, rather than the name. So interpretation is a matter of following address links, and all name searching is done at compile time.

The inclusion of the names in the dictionary also supports the `see` operation, which decompiles user definitions, and provides basic documentation for builtin functions. Note that the decompiled version of a function is not identical to the original source code, because control structures (for example) generate branch code and insert that into the definition. This is one of Forth's superpowers. The engine only provides `BRANCH` and `BRANCH0` primitives. All the higher level control structures are implemented in Forth. The source code for these is in `corelib.fs`.

## Memory map

The heap is 1 MB, **byte-addressed**, with 8-byte cells (`1 cells` is 8) at aligned addresses. Every access is bounds-checked, and `@` or `!` at a misaligned address raises error −23.

```
0 ......... HERE ->    DICT_LIMIT |               | <- data stack   sp0 | <- return stack   rp0
[ system variables | dictionary ] [ TIB PAD TMP ] [ 1024 cells        ] [ 1024 cells           ]
```

With real addresses (decimal; `here` is just after booting, and moves as corelib changes):

| Address   | What                                   | Forth name / Rust constant |
| --------- | -------------------------------------- | -------------------------- |
| 0         | unused: a link of 0 ends the chain     |                            |
| 8         | `dp` (HERE lives in its value, at 32)  | `dp`, `here_ptr`           |
| 40        | `context` (its value is at 64)         | `context`, `context_ptr`   |
| 72        | the rest of the dictionary: `'pad`, `base`, ..., the builtins, then corelib.fs | |
| 45,824    | HERE after booting: the first free byte | `here`                    |
| 1,031,416 | TIB, the input line (256 bytes)        | `'tib @`, `DICT_LIMIT`, `TIB_START` |
| 1,031,672 | PAD (256 bytes)                        | `pad`, `PAD_START`         |
| 1,031,928 | TMP (256 bytes)                        | `tmp @`, `TMP_START`       |
| 1,032,184 | the data stack's limit (1024 cells below sp0) |                     |
| 1,040,376 | the empty data stack; it grows down    | `sp0`, `STACK_START`       |
| 1,048,568 | the empty return stack; it grows down  | `rp0`, `RET_START`         |
| 1,048,576 | the end of memory                      | `DATA_SIZE`                |

- **The dictionary** grows up from address 0. `,` and `allot` stop at `DICT_LIMIT` with "dictionary full". `allot` counts bytes, `,` aligns HERE to a cell first, and `align`/`aligned` round up to a cell.
- **Each word** starts with a link field: `[link][name][code field][parameters]`. The link holds the previous word's link address, and 0 ends the chain. `,` and `allot` simply append to the newest word.
- **The data stack and the return stack** each have 1024 cells, and grow down. Overflow and underflow are errors.
- **The stacks are addressable** from Forth: `sp@` is the address of the top item (the item `n` deep is at `sp@ + n cells`), and `sp0` is `sp@` when the stack is empty. `rp@` and `rp0` work the same for the return stack. `depth`, `pick`, `roll`, `clear`, `.s`, and `.rs` are written in Forth on top of these.
- **Strings are bytes in the same memory.** `c@`, `c!`, `c,` and `cmove` address bytes, `@` and `!` cells. A buffer is `create buf 100 allot` (or the f3 helper `100 s-allot`). Counted strings hold at most 255 characters.

## Memory, word by word

This section shows real dumps, made with `' name >link n dump` (from `debug.fs`), so the layouts can be checked against the diagrams in the Forth books. The addresses come from one build; yours will differ.

### Headers, compared with the classic Forths

| | fig-Forth (1978) | F83 (1983) | f3 |
|---|---|---|---|
| Order | name, link, code, parameters | (view), link, name, code, parameters | link, name, code, parameters |
| Link points to | the previous word's name field | the previous word's link field | the previous word's link field |
| Flags | in the count byte: `$40` immediate, `$20` smudge, `$80` delimiter | in the count byte | in the link field's top bits (`IMMEDIATE_FLAG`, `COMPILE_ONLY_FLAG`) |
| Longest name | 31 | 31 | 255 |
| Hiding a word until `;` | the smudge bit | the smudge bit | `context` isn't updated until `;` (`last` is the newest header) |
| Cell | 2 bytes | 2 bytes | 8 bytes |
| Code field holds | the address of machine code (`DOCOL`, `DOVAR`, ...) | the same | an opcode (`op:definition`, `op:variable`, ...), or a builtin's index |

Moving from code field to name is the awkward direction in every layout, because names vary in length. fig-Forth's `NFA` uses `TRAVERSE`, which scans back for the delimiter bit set on a name's first and last bytes. f3's `>name` searches the dictionary instead, which is slower but needs no marker bits. Only `see`, `forget` and the debug tools use it.

Names are stored little-endian in the cell, so the dump's hex column reads right to left: `65726175717306` is the bytes `06 73 71 75 61 72 65`, that is, a count of 6 and then `s q u a r e`.

### A colon definition: `: square dup * ;`

```
  45824              B2D8               45784       Link field
  45832     65726175717306   28554735555539718             NFA: square
  45840    800000000000005  576460752303423493           Token: DEFINITION
  45848   2000000000000015 2305843009213693973         Builtin: 21: dup
  45856   2000000000000002 2305843009213693954         Builtin:  2: *
  45864    800000000000009  576460752303423497           Token: EXIT
```

* **45824, the link field (lfa):** 45784 is the previous word's link field. `' square >link` gives 45824.
* **45832, the name field (nfa):** 6 `square`, padded to one cell. `context` holds this address once `;` has run. `' square >name` gives 45832.
* **45840, the code field (cfa):** `op:definition`, the equivalent of fig-Forth's `DOCOL`. `' square` gives 45840, and that is what a definition that calls `square` compiles.
* **45848 onwards, the parameter field (pfa):** the body. `dup` and `*` are builtins, so they are compiled as their index in the Rust table (21 and 2), flagged with `BUILTIN_FLAG`. A call to a Forth word would be its cfa. `op:exit` returns (fig-Forth's `;S`).

`see square` prints `: square dup * ;` back by walking the same cells.

Execution: when the inner interpreter (`i_definition` in `inner.rs`) meets 45840 in some definition, it pushes its return address on the return stack, reads the code field at 45840, sees `op:definition`, and continues at 45848. Each builtin cell calls a Rust function directly. `op:exit` pops the return address. This is indirect threading, as in fig-Forth, except that the code field holds an opcode rather than the address of machine code.

### A variable: `variable v  7 v !`

```
  45872               B300               45824       Link field
  45880               7601               30209             NFA: v
  45888    800000000000001  576460752303423489           Token: VARIABLE
  45896                  7                   7            Value
```

The code field is `op:variable` (fig-Forth's `DOVAR`): running `v` pushes the address of the next cell, 45896, where `!` stored 7. A constant is the same with `op:constant` (`DOCON`), which pushes the cell's contents instead.

### A defining word: `: konst create , does> @ ;  5 konst five`

```
  45904              B330               45872       Link field
  45912       74736E6F6B05     128039122856709             NFA: konst
  45920    800000000000005  576460752303423493           Token: DEFINITION
  45928              1270                4720 Definition call: create
  45936  200000000000002C 2305843009213693996         Builtin: 44: ,
  45944    800000000000015  576460752303423509           Token: DOES>
  45952  2000000000000027 2305843009213693991         Builtin: 39: @
  45960    800000000000009  576460752303423497           Token: EXIT

  45968              B350               45904       Link field
  45976         6576696604        435778315780             NFA: five
  45984    800000000000014  576460752303423508           Token: CREATED
  45992               B380               45952   Compiled value
  46000                  5                   5   Compiled value
```

* **`konst`** is an ordinary colon definition. `does>` compiled `op:does` (45944), and the code after it, `@ ;`, starts at 45952.
* **Running `5 konst five`**: `create` builds the header of `five`, with code field `op:created` and an empty `does>` slot. `,` stores 5 at 46000. Then `op:does` stores 45952, the address of the code after it, in `five`'s `does>` slot (45992), and returns from `konst`.
* **Running `five`**: `op:created` pushes the data field address, 46000 (`' five >body`), then runs the code at 45952: `@` fetches 5, and `op:exit` returns.

This is fig-Forth's `<BUILDS ... DOES>` (ANS spells it `CREATE ... DOES>`). There, the code field points at `DODOES` and the first parameter cell holds the address of the `DOES>` code; f3 keeps that address in the cell after `op:created` instead. `see five` prints `Created: five data at 46000 does> code at 45952`.

### A string literal: `: hi ." hi" ;`

```
  46008               B390               45968       Link field
  46016             696802             6907906             NFA: hi
  46024    800000000000005  576460752303423493           Token: DEFINITION
  46032    800000000000004  576460752303423492           Token: STRLIT
  46040             696802             6907906  String Literal: hi
  46048              3930               14640 Definition call: ctype
  46056    800000000000009  576460752303423497           Token: EXIT
```

`."` compiles `op:strlit` followed by the counted string itself, padded to a cell, then a call to `ctype`. At run time `op:strlit` pushes the string's address (46040) and skips past the padding. This is fig-Forth's `(.")`, which also keeps the string inline. `s"` compiles the same literal followed by `count` instead of `ctype`, and `c"` leaves the counted string's address.

## Errors

- **No Forth input can crash f3.** Every error, whether Forth's or the engine's (stack underflow, a bad address, division by zero, an unknown word, a full dictionary...), is reported as `ERROR: <message>`. Then f3 returns to a clean state:
  - empty stacks;
  - interpret mode;
  - any half-built definition removed;
  - any file being included abandoned;
  - raw mode off.

  The rest of the input line is skipped.
- **Errors carry ANS throw codes:** -1 `abort`, -2 `abort"`, -3/-4 stack overflow/underflow, -5/-6 return stack overflow/underflow, -8 dictionary full, -9 invalid address, -10 division by zero, -13 undefined word, -14 compile-only word, -16 missing name, -37 file error. Any other engine error is -256.
- **`catch ( ... xt -- ... 0 | n )`** runs `xt`. If `xt` throws, `catch` restores both stack depths and returns the code.
- **`throw ( n -- )`** raises code `n`; 0 does nothing. Re-throwing the code that `catch` just returned keeps the original message.
- **`abort`** is silent. **`abort" msg"`** raises its message as code -2.
- **The end of input** (stdin closed) exits cleanly, as does a closed stdout (for example `f3 | head`).
- **In the Rust source,** errors are raised by `kernel::throw` / `throw_code`, which unwind (`panic_any(ForthError)`) to `catch`, or to `quit`, which handles each input line. A genuine Rust panic means an engine bug: it is also recovered from, and reported with `(Rust panic at file:line)`. The release profile must keep `panic = "unwind"`.

## Compiled code and execution tokens

A compiled cell is one of:
- **an opcode**: a value with `OPCODE_FLAG` (bit 59) set, such as `op:literal`, `op:branch`, `op:exit`, or the loop opcodes `op:do` `op:qdo` `op:loop` `op:ploop` `op:leave` `op:for` `op:next`, possibly followed by an operand. A `create`d word's code field is `op:created`, followed by the address of its `does>` code (0 for none); `does>` compiles `op:does`.
- **a builtin**: its index in the Rust builtin table, with `BUILTIN_FLAG` (bit 61) set;
- **the cfa** (code field address) of any other word.

Rust defines the opcodes, the flags (`IMMEDIATE_FLAG`, `COMPILE_ONLY_FLAG`, `BUILTIN_FLAG`), and `ADDRESS_MASK` as Forth constants.

An **execution token** (xt), as returned by `'` and `[']`, is always a cfa. `execute` also accepts a flagged builtin index. A builtin's cfa cell holds its flagged index, so `' dup @` gives the value that is compiled for `dup`.

Words flagged `compile-only` (all control structures, `;`, `exit`, `[']`, `[char]`, `literal`, `recurse`, `abort"`) are an error outside a definition.

Counted loops (`do ?do loop +loop leave unloop`, and `for next`) keep three cells on the return stack (exit address, limit, index), so `i` and `j` work the same in both kinds. `n for ... next` runs the body `n` times, with `i` counting down from `n` to 1, and doesn't run at all for `n <= 0`.

## What is in Rust and what is in Forth

A word is written in Rust only if:
1. it needs the host (I/O, files, time, terminal, shell);
2. it's a hot primitive (arithmetic, comparison, basic stack and memory words); or
3. it's needed before Forth exists, or by error recovery (parsing, `find`, number conversion, `:` `;` `create` `,`, and the `quit` loop).

Everything else is in `corelib.fs`: control structures, `see`, `.s`, `depth`, string helpers, and so on. A call to a Forth-defined word costs about 4 ns over a builtin call (see `src/forth/bench.fs`).

## Deviations from ANS Forth

f3 follows ANS Forth where it can. `plan/ANS.md` has the full review and the roadmap; steps 1–3 of it are done.

What is standard now:
- **Names are case-insensitive**, so standard (upper-case) programs work: `DUP`, `dup`, and `Dup` are the same word.
- **`here` and `pad` are words**, as in ANS. The variables behind them are `dp` and `'pad`.
- **`find`** returns `xt 1` for an immediate word and `xt -1` otherwise.
- **`rshift`** is logical; `2/` is the arithmetic shift.
- **`abort" msg"`** takes a flag. `emit` sends any byte. `>in` is a 0-based offset into `source`.
- **The return-stack pairs** `2>r 2r> 2r@ 2rdrop rdrop` follow the ANS order.
- **Defining words are standard:** `create` makes a word that pushes its data-field address, and `, allot` fill it. `does>` gives it an action, and `>body` finds its data field.
- **Also present:** double-cell arithmetic (`s>d m* um* um/mod fm/mod sm/rem */ */mod`), pictured numeric output (`<# # #s #> hold sign`, on which `. u. .r u.r` are built), `postpone`, `:noname`, `[if] [else] [then]`, `value to`, `defer is`, `.(`, `refill`, `source`, `count`, `within`, `u>`, `2*`, `cell+`, `char+`, `chars`, `align`, `aligned`, `unused`, and `environment?`.

- **Strings are `( c-addr u )`:**
  - `type`, `s"` (compiled inside definitions), `parse`, `parse-name`, `accept`, `evaluate`, and the file words (`open-file create-file delete-file read-line write-line included include-file`) take or give address and length.
  - Counted strings remain for names, `c"`, `word`, and `find`; `count` converts.
  - The String word set is present (`compare search /string -trailing fill erase blank move cmove>`), as are `>number`, `2swap 2over 2@ 2!`, and two transient `s"` buffers.
  - `evaluate` interprets its string in place (`source` is the string), and `included` runs the file before the rest of the line.
- **One byte-addressed memory:** `cells`, `chars`, `align`, `aligned`, `c,`, and `create … allot` buffers work as the standard describes.
- **The reference suite:** Gerry Jackson's `core.fr` passes all 638 of its tests.

What is still different, deliberately:
- **f3 helpers:** `ctype ( s -- )` prints a counted string; `s-allot ( n -- s )`, `s, ( c-addr u -- )`, `s-save ( c-addr u -- s )`, and `place ( c-addr u s -- )` build strings in the dictionary; and `parse-to` is the old copying parser.
- **`/` and `mod` truncate toward zero** (symmetric division, which the standard allows). `fm/mod` is the floored version.
- **Missing:** most of the Double word set (`d.`, `dnegate`, ...), and `2constant`/`2variable`.
- **Numbers** accept `#` (decimal), `$` (hex), and `%` (binary) prefixes, and honour `base`.
- **f3 extensions:** `."` also works outside definitions, and there are `for … next`, `compile-only`, and `forget`. f3's own defining words (`constant`, `variable`, and the `sequences.fs` arrays) are built on the header-only `(create)` and `(close)`.

## Testing

- `cargo test` runs the Rust unit tests and `tests/forth.rs`, which drives the real `f3` binary. That covers:
  - the regression suite (`src/forth/regression.fs`);
  - Hayes-style core tests (`src/forth/tester.fs` with `src/forth/core-tests.fs`, about 330 `T{ ... -> ... }T` tests of the ANS-compatible words);
  - the reference ANS core tests, John Hayes' `core.fr` as maintained by Gerry Jackson (`tests/ans/`, all 638 pass);
  - a 10,000-line fuzz test;
  - one test for each bug fixed in the review (`plan/ANALYSIS.md`).
- `cargo clippy --all-targets -- -D warnings` must be clean.
- Benchmarks: `./target/release/f3 -f src/forth/bench.fs`.

# Builtin Words

This is not intended to be full documentation, but a reference to the stack behavior and basic functions of some important words to help with debugging. f3 supports most of the standard stack and arithmetic words.

## System Variables

| WORD    | Notes                                                                                                                                    |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| 'tib    | The address of the text input buffer. TIB is not counted, but the first byte is reserved and not used for text.                          |
| #tib    | The length of the line currently in TIB.                                                                                                 |
| >in     | Pointer to the first unconsumed character in TIB.                                                                                        |
| pad     | ( -- s ) The address of the string buffer PAD (the variable behind it is `'pad`). PAD is a counted string, used by the parser to hold the current token during interpretation. |
| tmp     | Address of a second temporary string buffer used by string functions to stage new strings.                                               |
| here    | ( -- a ) The top of the dictionary, where new elements will be added (the variable behind it is `dp`).                                                             |
| context | Holds the address of the most recent word's name field                                                                                   |
| last    | Holds the address of the name field of the word being defined.                                                                           |
| base    | Radix for numberic I/O. Defaults to 10.                                                                                                  |
| state   | Set to TRUE if compile mode is active, otherwise FALSE.                                                                                  |
| stepper | Controls the stepper / debugger. 0 => off, 1 => trace, -1 => single step.                                                                |

## System Commands

| WORD                      | SIGNATURE | NOTES                                                                                                                                                                                                       |
| ------------------------- | --------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| system" \<shell command>" | ( -- )    | Runs a shell command and returns the output to stdout, printed into the output stream. For example, `system" ls -l"` will pass `ls -l` to sh for execution. `system"` blocks until the command is complete. |
| (system)                  | ( s -- )  | Takes a string pointer on the stack and passes the string to `sh` for execution. Used by `system"`.                                                                                                         |

## I/O

| WORD          | SIGNATURE                     | NOTES                                                                                                                                                                                                                                                                                                                                             |
| ------------- | ----------------------------- | ----------------------------------------------------------------------- |
| query         | ( -- )                        | Read a line of Forth from the terminal. Store in TIB and set #TIB and >IN variables                                                                                                                                                                                                                                                               |
| accept        | ( c-addr u -- u2 )            | Read a line, storing up to u characters at c-addr. Return the number stored. |
| emit          | ( c -- )                      | Send byte c to the terminal.                                                                                                                                                                                                                                                                             |
| flush         | ( -- )                        | Force the output buffer to be flushed to the terminal.                                                                                                                                                                                                                                                                                            |
| space         | ( -- )                        | Prints a single space.                                                                                                                                                                                                                                                                                                                            |
| spaces        | ( u -- )                      | Prints u spaces.                                                                                                                                                                                                                                                                                                                                  |
| .s            | ( -- )                        | Print the contents of the stack, using `base`. Does not consume stack elements. `.rs` prints the return stack.                                                                                                                                                                                                                                    |
| depth pick roll clear | ( -- n ) etc.         | The standard stack words, written in Forth with `sp@` and `sp0`. `pick` and `roll` check for underflow.                                                                                                                                                                                                                                           |
| .             | ( v -- )                      | Print the top of the stack as an integer using the value of the `base` variable as the radix.                                                                                                                                                                                                                                                     |
| u.            | ( u -- )                      | Print the top of the stack as an unsigned value                                                                                                                                                                                                                                                                                                   |
| u.r           | ( u w -- )                    | Print unsigned u right-justified in a field w wide. If w is too small, print the full number anyway                                                                                                                                                                                                                                               |
| .r            | ( n w -- )                    | Print integer n right-justified in a field w wide. If w is too small, print the full number anyway                                                                                                                                                                                                                                                |
| cr            | ( -- )                        | Print a newline.                                                                                                                                                                                                                                                                                                                                  |
| s" \<string>" | ( -- c-addr u )               | The inline string. Inside a definition it is compiled as a literal; outside one it is copied to one of two buffers, used in turn. `c" \<string>"` gives a counted string the same way. |
| type          | ( c-addr u -- )               | Print u characters from c-addr. `ctype ( s -- )` prints a counted string (names, `c"` strings, `word` results). |
| ltype         | ( s w -- )                    | Print a counted string left justified in a field w characters wide. If w is too small, print the entire string anyway. |
| rtype         | ( s w -- )                    | Print a counted string right justified in a field w characters wide. If w is too small, print the entire string anyway. |
| ltell         | ( s u w -- )                  | Print a string of length u left justified in a field w characters wide. If w is too small, print the entire string anyway.                                                                                                                                                                                                                        |
| rtell         | ( s u w -- )                  | Print a string of length u right justified in a field w characters wide. If w is too small, print the entire string anyway.                                                                                                                                                                                                                       |
| r/w r/o w/o   | ( -- fam )                    | File access modes, for `open-file` and `create-file`. `bin ( fam -- fam )` is accepted and does nothing. |
| open-file     | ( c-addr u fam -- file-id ior ) | Open the named file. W/O creates or truncates it; R/W creates it if necessary. The file-id stays valid until `close-file`. ior is 0 on success, -1 on failure. `create-file` ( same stack effect ) always creates or truncates; `delete-file ( c-addr u -- ior )` deletes. |
| close-file    | ( file-id -- ior )            | Close the file associated with file-id, returning a code indicating success or failure.                                                                                                                                                                                                                                                           |
| read-line     | ( c-addr u file-id -- u2 flag ior ) | Read a line of up to `u` characters into the buffer at c-addr (`create buf 256 allot` makes one), without the newline. A longer line is returned in pieces. At end of file, u2 is 0 and flag is FALSE. |
| write-line    | ( c-addr u file-id -- ior )   | Write `u` characters from c-addr, and a newline. |
| file-size     | ( file-id -- u ior )          | The size of the file in characters. |
| file-position | ( file-id -- u ior )          | The current position in the file. |
| included      | ( c-addr u -- )               | Interpret the named file, then carry on with the rest of the line. `include <name>` does the same with the next word; `include-file ( file-id -- )` interprets an open file and closes it. |

## Text interpreter and Compiler

| WORD              | SIGNATURE                 | NOTES                                                                                                                                                                                                                                                                                                                                                    |
| ----------------- | ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| words             | ( -- )                    | Prints a list of all dictionary entries, whether words, builtins, variables or constants. Each word is preceded by its address in the dictionary for debugging purposes.                                                                                                                                                                                 |
| see               | \<word>                   | The Forth decompiler. If \<word> is a builtin, see provides the documentation for that word. If it's a user-defined word, see provides the source code as compiled. This is often different from the original source code, because control structures are compiled down to lower level branch functions, and are not represented in their original form. |
| abort             | ( -- )                    | Ends execution of the current word, clears the stack, and returns to the interpreter's top level                                                                                                                                                                                                                                                         |
| abort"            | ( x -- ) \<message>"      | If x is non-zero, raise an error with the message (throw code -2).                                                                                                                                                                                                                                                              |
| catch             | ( ... xt -- ... 0 \| n )  | Run `xt`. Returns 0 if it completes; if it throws, restores the stack depths and returns the throw code.                                                                                                                                                                                                                                                 |
| throw             | ( n -- )                  | If `n` is not 0, raise an error with throw code `n`. Uncaught, it is reported at the prompt.                                                                                                                                                                                                                                                             |
| quit              | ( -- )                    | Interpreter outer loop: gets a line of input, processes it. Calls `query` and `eval` to do the work.                                                                                                                                                                                                                                                     |
| eval              | ( -- )                    | Interprets the rest of the line in the `TIB`. An error abandons the rest of the line.                                                                                                                                                                                                                                                                    |
| \\                | ( -- )                    | Inline comment. Causes the remainder of the line to be ignored.                                                                                                                                                                                                                                                                                          |
| (                 | ( -- )                    | Text from the left paren to its maching closing paren is ignored. Used for documenting stack signatures in word definitions.                                                                                                                                                                                                                             |
| parse             | ( char -- c-addr u )      | The input up to the next `char`, in place in the input buffer (leading delimiters are not skipped). `parse-name ( -- c-addr u )` skips leading spaces; `word ( char -- s )` returns a counted string in PAD. |
| [char]            | ( -- c )                  | Place the first character of the next token on the stack. Consumes the entire token.                                                                                                                                                                                                                                                                     |
| find              | ( s -- xt 1 \| xt -1 \| s 0 ) | Search the dictionary for the counted string at s: 1 if the word is immediate, -1 if not.                                                                                                                                                                                                                                                                                                       |
| ' \<name>         | ( -- cfa )                | The code field address (execution token) of the next word. It is an error if the word is not found.                                                                                                                                                                                                                                                     |
| ?unique           | ( s -- s )                | Prints a message if the counted string at s is already defined. `create` warns about redefinitions itself (at warning level).                                                                                                                                                                                                                         |
| :                 | ( -- )                    | Sets compile mode to start a definition                                                                                                                                                                                                                                                                                                                  |
| [                 | ( -- )                    | Immediate: set state to interpret mode. Used to force interpretation inside a definition.                                                                                                                                                                                                                                                                |
| ]                 | ( -- )                    | Set state to compile mode. Used inside a definition to undo the effect of a previous `[`.                                                                                                                                                                                                                                                                |
| number?           | (s -- n T \| s F )        | Converts the counted string at s to a number, using `base` and the `#` `$` `%` prefixes. On failure, leaves s and pushes `FALSE`.                                                                                                                                                                                                                         |
| literal           | ( n -- )                  | Takes a number from the stack and compiles it into the current definition.                                                                                                                                                                                                                                                                               |
| , (comma)         | ( v -- )                  | Compiles the value on the stack into the dictionary and updates `here`.                                                                                                                                                                                                                                                                                  |
| create \<name>    | ( -- )                    | Takes a postfix name, and creates a new name field in the dictionary                                                                                                                                                                                                                                                                                     |
| immediate         | ( -- )                    | Marks the most recent definition as immediate by setting a flag on the name field. Immediate words are executed even when compile mode is set. They are most often used to compile control structures that need some level of computation at compile time.                                                                                               |
| immed?            | ( cfa -- T \| F )         | TRUE if the word at cfa is immediate.                                                                                                                                                                                                                                                                                                                    |
| compile-only      | ( -- )                    | Marks the most recent definition as valid only inside a definition.                                                                                                                                                                                                                                                                                      |
| [compile]         | \<name>                   | Delays the compilation of an immediate word. Typically used in the definition of control structures and compiler customization.                                                                                                                                                                                                                          |
| forget-last       | ( -- )                    | Delete the last definition from the dictionary.                                                                                                                                                                                                                                                                                                          |
| forget            | \<name>                   | Delete word `<name>` and any words defined more recently than `<name>`.                                                                                                                                                                                                                                                                                  |

## Timing and Delay

To time a function, precede it with `now` and follow it with `millis` or `micros`, which will place the elapsed time on the stack.

| WORD   | SIGNATURE | NOTES                                                                  |
| ------ | --------- | ---------------------------------------------------------------------- |
| now    | ( -- )    | Captures the current time using Rust's `std::time::Instant` capability |
| millis | ( -- n )  | Places the number of milliseconds since `now` was called on the stack  |
| micros | ( -- n )  | Places the number of microseconds since `now` was called on the stack  |
| ms     | ( n -- )  | Sleep for `n` milliseconds                                             |
| sec    | ( n -- )  | Sleep for `n` seconds                                                  |

## Sequences

By default, Forth provides no data structures beyond the atomic cell, and strings. `sequences.fs` defines arrays, stacks, and (TBD) queues and deques. They are fixed in size, and are allocated in the dictionary.

Typical usage might be `100 array my-array`, which will create a 100 element array, or `10 stack my-stack`, which creates a 10 element stack.
The implementation involves storing a name, which returns the address of the first parameter, a size value, two pointers (used for stacks, queues and deques), and space for the number of elements in the declaration. Operations include:

| WORD  | SIGNATURE       | NOTES                                                                                                         |
| ----- | --------------- | ------------------------------------------------------------------------------------------------------------- |
| array \<name> | ( n -- addr )   | Create an array of `n` elements using the name provided after `array`. Returns the address of the size value. |
| ac@   | ( addr -- n )   | Returns the number of elements in the array                                                                   |
| a@    | ( i addr -- v ) | Returns the value of cell `i` in the array at `addr`.                                                         |
| a!    | ( n i addr -- ) | Stores the value `n` in the array at index `i`.                                                               |
| stack \<name> | ( n -- addr )   | Create a stack of `n` elements using the name provided after `stack`. Returns the address of the size value.  |
| sc@   | ( addr -- n )   | Returns the number of elements in the stack.                                                                  |
| >s    | ( n addr -- )   | Push `n` on the stack at `addr`.                                                                              |
| s>    | ( addr -- n )   | Pop the top value off the stack at `addr`.                                                                    |

## Debugging

A single stepper and trace capability allows for viewing interpreted functions as they execute. When active, it prints the program counter address, a visual indication of the depth of the return stack by indentation, the contents of the stack, and the word being executed.

The single stepper responds to single character commands (followed by Enter):

- `s` => take a single step
- `t` => shift to trace mode
- `c` => continue - turn the stepper off
- `i` => step in (increase stepper-depth)
- `o` => step out (decrease stepper-depth)
- `?` or `h` => print help information

| WORD          | SIGNATURE | NOTES                                                                                                        |
| ------------- | --------- | ------------------------------------------------------------------------------------------------------------ |
| step-on       | ( -- )    | Turns on single stepping with stepper-depth set to 1.                                                        |
| step-off      | ( -- )    | Turns off single stepping.                                                                                   |
| trace-on      | ( -- )    | Turns on tracing.                                                                                            |
| trace-off     | ( -- )    | Turns off tracing.                                                                                           |
| trace-all     | ( -- )    | Sets trace level t 100                                                                                       |
| stepper-depth | VARIABLE  | Trace / step depth, which can be set manually, or by the use of the `i` and `o` commands within the stepper. |

In addition to the debugger, `dump` commands are provided to inspect memory. `dump` attempts to understand what it's looking at, and provides information accordingly. There are also some debug print statements that only print if `debuglevel` is 3 (set with `3 dbg`). The following functions are available:

| WORD             | SIGNATURE         | NOTES                                                                                                |
| ---------------- | ----------------- | ---------------------------------------------------------------------------------------------------- |
| dump             | ( addr cells -- ) | Dump `cells` cells, starting at the provided address.                                                |
| dmp              | ( addr -- )       | Dump 25 cells from the provided address.                                                             |
| dh               | ( -- )            | Dump the top 25 cells from the dictionary. Useful for debugging new definitions and data structures. |
| dump-here        | ( n -- )          | Dump the top n cells.                                                                                |
| dump-help        | ( -- )            | Prints a help string listing available commands.                                                     |
| dump-strings     | ( s_addr -- )     | Dump n strings starting from the first string after s_addr.                                          |
| dump-stringspace | ( -- )            | Dump string buffers and all strings                                                                  |
| dump-buffer      | ( s_addr -- )     | Dump one of the string buffers (typically TIB, PAD, or TMP).                                         |
