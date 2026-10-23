# f3 and ANS Forth: deviations and a route closer to the standard

Review date: 2026-10-22, against commit `70da0ea`. The reference is ANS Forth 1994 / Forth-2012, the Core word set first, then Core Extensions and the Exception, File, String, and Tools sets where f3 already has words.

Every behaviour quoted below was checked against the binary.

## Summary

f3's **inner engine is standard enough.** It has the standard 64-bit cells, the standard execution and exception model, and standard control structures, loops, and comparisons. Phase 5's 227 core-style tests pass.

**What keeps standard programs from running** comes down to five design choices, in order of impact:

1. **Case-sensitive lookup with lower-case names.** Standard programs, including the reference test suite, are written in upper case. `1 DUP` is `DUP ?` in f3. This one choice stops almost every published Forth program on the first line.
2. **Two address spaces.** Cells (`@ ! ,`) and bytes (`c@ c! cmove`) are different memories, so `create buf 80 allot  buf c!` writes somewhere other than `buf`. Standard code mixes the two freely.
3. **Counted strings everywhere,** where ANS uses `( c-addr u )` pairs: `type`, `s"`, `parse`, `accept`, the file words.
4. **`create` makes only a header,** and there is no `does>`. Standard defining words (`create … does>`, `create buf n allot`) don't work. `create foo` leaves `foo` undefined until a private `(close)` runs.
5. **A handful of missing core words:** `evaluate`, `postpone`, `source`, `word`, `count`, `*/`, double-cell arithmetic, and pictured numeric output (`<# # #>`).

Items 1, 4, and 5 are cheap or medium, and don't touch the memory model. Items 2 and 3 are the real design decision. The recommendation (see the roadmap below):
* do steps 1–3 now, which gets f3 to "standard programs that don't do byte arithmetic run";
* decide about a unified byte-addressed memory (step 4) only if running the full reference suite, or real-world Forth code, is a goal.

The review also found two bugs, fixed in step 1:
* **F32:** `rdrop`, `2rdrop`, `2>r`, `2r>`, and `2r@` are colon definitions, so they move the word's own return address instead of the caller's values. They have never worked: `1 2 2>r 2r>` fails with "return stack overflow". Nothing uses them, which is why no test caught it.
* **F33:** `u.` is not unsigned. `-1 u.` prints `/`, because it formats negative "digits". It also prints no trailing space, unlike ANS `U.`.

---

## Deviations, by area

Effort key: **S** = under a day, **M** = 1–3 days, **L** = a week or more. "Breaks" means existing f3 code (`corelib.fs`, `editor.fs`, `sequences.fs`, `debug.fs`, `regression.fs`) that would need changes.

### A. Names and lookup

| # | f3 | ANS | Fix | Effort |
|---|---|---|---|---|
| A1 | Lookup is case-sensitive, and standard words are lower-case (`dup`). `DUP` is unknown. | Standard words are spelled in upper case, and programs use upper case. Practically every system is case-insensitive. | Compare names case-insensitively in `find_word` (`eq_ignore_ascii_case`); the cost is one byte compare per character, at compile time only. | S |
| A2 | Engine constants share names with standard words: `DO LOOP LEAVE FOR NEXT EXIT ABORT LITERAL VARIABLE CONSTANT EXEC ARRAY`. With A1, `DO` and `do` would collide. | No such names. | Rename the constants before doing A1, for example `op:do`, `op:exit`, `op:literal`. They're used only in `corelib.fs` and `debug.fs`. | S |
| A3 | `find ( s -- cfa T \| s F )` gives −1 for every word it finds. | `FIND ( c-addr -- c-addr 0 \| xt 1 \| xt -1 )`: 1 if immediate, −1 if not. | Return 1 or −1 by the immediate flag. Callers in f3 test for non-zero, so nothing breaks. | S |

### B. Memory model

| # | f3 | ANS | Fix | Effort |
|---|---|---|---|---|
| B1 | Two spaces: a cell heap (`@ ! , allot`, cell-addressed) and a byte string space (`c@ c! cmove`). `cells` is a no-op, and there's no `c,`. | One byte-addressed data space. Cells are aligned (`cells`, `cell+`, `align`, `aligned`), and `c@` and `@` work on the same addresses. | **Option 1 (recommended first):** keep two spaces, but give every dictionary word a byte area. That isn't enough on its own, because code like `create buf 80 chars allot  buf c!` still writes to a different memory. **Option 2:** unify into one byte array with `i64` cells at 8-byte-aligned addresses. Compiled code, the dictionary, strings, and the stacks all live there, so `cells` = 8, and `c,`, `move`, `fill`, and `here c!` all work. This is the only fix that runs standard address arithmetic. See step 4. | Option 2: L |
| B2 | `here` is a variable: `here @`. | `HERE ( -- addr )` is a word. | Rename the variable to `dp` (used by the engine and `corelib`), and define `: here dp @ ;`. Every `here @` in the `.fs` files becomes `here`, about 60 places. | S |
| B3 | `pad` is a variable holding an address (`pad @`). | `PAD ( -- c-addr )`. | The same as B2: `: pad 'pad @ ;`. | S |
| B4 | `create foo` makes a header only. `foo` is invisible until `(close)` runs, and `create foo 5 ,` leaves `foo` undefined. | `CREATE name` defines a word that returns its data-field address immediately, so `, allot c,` then fill its data. | `create` builds a complete data word (a header, a `CREATED` code field, a `does>` slot, and the back pointer) and closes it. Since F30, `,` and `allot` after a finished word extend its data correctly. Keep the old header-only behaviour as an internal `(create)` for `constant`, `variable`, `string`, and `sequences.fs`. | M |
| B5 | No `does>`. | `CREATE … DOES>` is how standard defining words are built. | The `CREATED` code field has a slot for a `does>` code address, 0 meaning none. `does>` stores the address of the code that follows it into the latest word's slot. When executed, a `CREATED` word pushes its data address, then calls that code if the slot isn't 0. A new opcode and about 30 lines of Rust. | M |
| B6 | `allot` counts cells. | `ALLOT` counts address units (bytes, in practice). | Follows from B1, Option 2. Until then, `cells` is a no-op, so `n cells allot` is correct in f3. | — |

### C. Strings

| # | f3 | ANS | Fix | Effort |
|---|---|---|---|---|
| C1 | `type ( s -- )` takes a counted string; `tell ( s u -- )` takes an address and length. | `TYPE ( c-addr u -- )`. | Rename: `type` becomes today's `tell`, and the counted version becomes `ctype` (or `count type`). `."` compiles `STRLIT count type`. Breaks every `type` call in the `.fs` files (about 40). | M |
| C2 | `s" x"` gives `( s u )`, where `s` is the counted string's address, count byte included. | `S" ( -- c-addr u )`: c-addr is the first character. | Return `s 1+ u`. Callers that do `s" …" drop` to get a counted string use `drop 1-` instead, or better, take `( c-addr u )`. | S, with C1 |
| C3 | No `count`. | `COUNT ( c-addr1 -- c-addr2 u )`. | `: count dup 1+ swap c@ ;` | S |
| C4 | `parse ( c -- b u )` copies the token to PAD as a counted string, and returns PAD and the length. | `PARSE ( char -- c-addr u )` returns the token in place in the input buffer. | Return the address in the TIB (`parse_token` already finds it), without copying. Callers that use PAD afterwards need to change. | S |
| C5 | No `word`. | `WORD ( char -- c-addr )` returns a counted string. | Today's `parse`, but returning just the counted address (in PAD). | S |
| C6 | `accept ( b u -- b u2 )` | `ACCEPT ( c-addr +n1 -- +n2 )` | Drop the extra `b`. Breaks `editor.fs`. | S |
| C7 | File words take counted strings, and `read-line` reads into TMP. | `OPEN-FILE ( c-addr u fam -- fileid ior )`, `READ-LINE ( c-addr u1 fileid -- u2 flag ior )` into the caller's buffer, `INCLUDED ( c-addr u -- )`. `CREATE-FILE`, `DELETE-FILE`, and `BIN` are missing. | Switch to `( c-addr u )` arguments, and read into the caller's buffer. With two address spaces, the buffer must be in string space, which works because `s-allot` exists. | M |
| C8 | No `compare`, `search`, `/string`, `-trailing`, `blank`. | String word set. | Forth definitions on top of `c@` and `cmove`. | S |

### D. Arithmetic and logic

| # | f3 | ANS | Fix | Effort |
|---|---|---|---|---|
| D1 | `rshift` is arithmetic: `-8 1 rshift` gives −4. | `RSHIFT` is logical. | Make it logical (`(a as u64) >> b`), and add `2/` for the arithmetic shift. Only negative-operand uses change; none were found in the `.fs` files. | S |
| D2 | `/` and `mod` truncate (symmetric division). | Implementation-defined, either symmetric or floored, but it must be documented. `FM/MOD` and `SM/REM` must both exist. | Keep symmetric (documented). Add `sm/rem` and `fm/mod` as primitives (on `i128`). | S |
| D3 | No double-cell words: `s>d m* um* um/mod */ */mod d+ d- d. dnegate dabs 2@ 2!`. | Core has `S>D M* UM* UM/MOD FM/MOD SM/REM */ */MOD`; the Double set has the rest. | Use `i128` in Rust for `m* um* um/mod fm/mod sm/rem`; `*/` and `*/mod` are then Forth one-liners. The `d…` words are Forth. | S–M |
| D4 | `u.` is signed, and prints no space (F33). | `U. ( u -- )` prints unsigned, then a space. | Rewrite `u.`, `u.r`, and `.` on pictured numeric output (D5), or now with `um/mod`. | S |
| D5 | No pictured numeric output: `<# # #s #> hold sign`. | Core. | Forth definitions on `um/mod`, using `hld` (the variable exists already) and a buffer below PAD. Then `.`, `u.`, and `.r` become the standard four-line definitions. | S |
| D6 | No `2* 2/ u> within cell+ char+ chars align aligned`. | Core and Core Ext. | Forth one-liners. `chars`, `align`, and `aligned` are no-ops until B1 Option 2. | S |

### E. Compiler and interpreter

| # | f3 | ANS | Fix | Effort |
|---|---|---|---|---|
| E1 | No `postpone`; f3 has the obsolescent `[compile]`. | `POSTPONE name`: for an immediate word, compile it; for any other word, compile code that compiles it. | Forth, once `find` returns immediacy (A3): `: postpone ' dup immed? if , else LITERAL , , ['] , , then ; immediate`. | S |
| E2 | No `evaluate`. | `EVALUATE ( c-addr u -- )` interprets a string. | Add a string source to the reader stack (`FType::Str`), and make `run_lines(depth)` read from it. The machinery exists from `include`. | M |
| E3 | No `source`. `>in` starts at 1 (byte 0 of the TIB is the count byte), and `'tib` and `#tib` are exposed. | `SOURCE ( -- c-addr u )`; `>IN` is a 0-based offset. `TIB` and `#TIB` are obsolescent. | Make `>in` 0-based from the TIB's first character. `parse_token` is the only engine user; `kkey` and `?key` in `corelib` adjust by one. Add `source`. | S |
| E4 | `abort"` is unconditional. | `ABORT" ( x -- )` aborts only if x is non-zero. | Compile `if <message> (error) then`. Change the five `if abort" …" then` uses in `corelib` to plain `abort" …"`, and update the F26 test. | S |
| E5 | No `:noname`, `[if] [else] [then]`, `value`/`to`, `defer`/`is`, `.(`, `environment?`, `unused`. | Core Ext, Tools Ext. | All Forth, or tiny. `:noname` needs `create` without a name (an anonymous header). | S each |
| E6 | `."` also works outside a definition, and `forget` exists. | `."` is compile-only (`.(` is the interpreted form), and `FORGET` is obsolescent. | Harmless extensions. Keep them. | — |
| E7 | `key` returns 0 when nothing is available (from a file reader); `emit` drops non-printable characters (`128 mod`, and only `> 31` is shown). | `KEY` waits; `EMIT` sends any character. | `emit` becomes `(emit)`. Keep the filtering version as an f3 word if the editor needs it (it doesn't seem to). | S |
| E8 | `rdrop 2rdrop 2>r 2r> 2r@` are broken (F32), and `2>r`'s order differs anyway. | `2>R ( x1 x2 -- ) ( R: -- x1 x2 )`, with x2 on top. | Make them builtins: they cannot be written as colon definitions in this design. | S |

### F. Already standard (keep)

`: ; [ ] ' ['] execute immediate literal recurse exit`, `if else then begin while repeat until again`, `do ?do loop +loop i j leave unloop`, `case of endof endcase`, `catch throw abort`, `variable constant`, `base decimal hex`, `state`, `depth pick roll ?dup nip tuck`, the comparisons including `u<`, `and or xor invert lshift`, `+ - * / mod /mod negate abs min max 1+ 1-`, `char [char]`, `\ (`, `.s ? see words bye`. The 64-bit cell size, and `TRUE` = −1.

---

## Recommended roadmap

Each step leaves `cargo test`, the core tests, and `regression.fs` green, and adds tests for what it changes.

### Step 1: names, cheap semantics, missing small words (M, about 2 days)

What it does:
* A2 then A1: rename the engine constants, then make lookup case-insensitive.
* B2 and B3: `here` and `pad` become words.
* A3: `find` returns immediacy.
* D1: logical `rshift` plus `2/`.
* E4: `abort"` takes a flag.
* E7: raw `emit`.
* E8 and F32: return-stack words as builtins.
* D4 and F33: an unsigned `u.`.
* D2, D3, D5, and D6: `sm/rem fm/mod um* um/mod m* s>d */ */mod`, pictured numeric output, and the Forth one-liners.
* E1 `postpone`, E3 `source` and 0-based `>in`, and the E5 extras.

**Payoff:** upper-case standard programs that don't build data structures with `create`, or do byte arithmetic, now run. That covers most textbook examples.

**Breaks:** about 60 `here @` and `pad @` uses in the `.fs` files; the five `abort"` uses; anything relying on arithmetic `rshift` (none found).

### Step 2: standard defining words (M, about 2 days)

What it does:
* B4: `create` makes a complete word.
* B5: `does>`.
* E2: `evaluate`.
* E5: `:noname`.
* `sequences.fs`, `constant`, `variable`, and `string` move to the internal `(create)`.

**Payoff:** `create … does>` defining words, `create buf n cells allot`, and `evaluate`-based code all work.

### Step 3: `( c-addr u )` strings (M, about 3 days)

What it does: C1–C8. The counted-string versions stay available as `ctype` and the like for f3's own code.

**Payoff:** the standard string and file interfaces.

**Breaks:** every `type` and `s"` use in the `.fs` files (`editor.fs` most of all), and the file words' callers.

**Checkpoint:** after steps 1–3, run Gerry Jackson's `core.fr` (`tester.fr` + `core.fr`) itself, not f3's port. Expect the parts that do byte arithmetic on `here` and `create`d buffers to fail, and nothing else.

### Step 4 (decide first): one byte-addressed memory (L, about 1–2 weeks)

B1 Option 2:
* Replace `heap: [i64]` plus `strings: [u8]` with one `Vec<u8>`.
* Cells are 8-byte, little-endian, and aligned.
* Every address held in compiled code, the dictionary links, and the stacks becomes a byte address.
* `cells` = 8.
* Names and string literals move into the dictionary, or into a byte region of the same space.
* The memory map in `doc.md` changes: dictionary, then PAD, TIB, and stacks, all in one space.

What it costs:
* Every address calculation in `corelib.fs`, `sequences.fs`, `debug.fs`, and `editor.fs` (`1+` to the next cell becomes `cell+`, and so on).
* The opcode-operand offsets (in bytes or in cells: pick cells and scale).
* The inner interpreter fetches with `i64::from_le_bytes`. That's about the same speed: one bounds check, then an unaligned-safe load.

What it buys:
* `here c!`, `c,`, `move`, `fill`, and buffers created with `create` all work as standard code expects.
* The full Core word set, and passing the reference `core.fr`, become achievable.

**Recommendation:** do steps 1–3 first. They're worth doing regardless, and most of their work carries over. Take step 4 only if passing the reference suite, or running third-party Forth code, is a goal. If f3 is a personal, minimal Forth, its two-space model is coherent and documented, and steps 1–3 already remove most of the day-to-day friction.

### What stays a documented difference

* Symmetric division (allowed by the standard, but documented, with `fm/mod` provided).
* `."` working outside definitions.
* `forget`.
* `for … next`.
* `compile-only`.
* The `#`, `$`, and `%` number prefixes (these are standard in Forth-2012).
* Without step 4: the two address spaces.
