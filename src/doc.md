# f3 Documentation

f3 is written and tested on MacOS, but does not use any MacOS-specific calls, so it should compile on Linux and Windows.
The implementation is a combination of Rust and Forth - the core capabilities are implemented in Rust, including the memory model, the compiler and interpreter, and a range of basic operations like arithmetic and stack operations. The Rust binary is just over 1Mb in size.

The data space is an array of i64, which stores all the words, contains the return and calculation stacks, and has space for additional data storage as needed.
Within Rust, the i64 values are cast to usize as needed.

Strings are stored in a separate array of chars, and builtin words are implemented through a separate jump table of function pointers.

Compilation results in the address of words being stored so the engine can simply jump to the code for any referenced word. For builtin functions, the address of the function pointer is stored, with a flag to indicate that it's a builtin function.

Because strings are stored separately, the names of words are captured in a single word address in data space, pointing into string space. Forth counted strings are used throughout (first 'character' is the length of the string).

Three string buffers are provided as follows:

- TIB - the text input buffer is the location where input is loaded for parsing, a line at a time.
- PAD - a working area where each token is placed after parsing.
- TMP - a second working area where strings are staged before either being printed or embedded in a definition. or string variable.

The dictionary is a linked list, implemented directly in the data array, using back pointers to string words together like this:

| n                  | n+1                      | n+2                 | n+3                          | n+4                   | n+5 |
| ------------------ | ------------------------ | ------------------- | ---------------------------- | --------------------- | --- |
| back-pointer       | name field               | code field          | args ...                     | back pointer          | ... |
| ..to previous word | points into string space | indicates word type | additional data for the word | points back to cell n |

So for example, a word defined as follows: `: double 2 * ;` would be stored like this:

| n   | n+1       | n+2  | n+3       | n+4          | n+5            |  n+6        |  n+7   | |
| --- | --- | --- | ---- | --------- | ------------ | -------------- | -------- | --- |
| back-pointer   | points to "double" in string space    | DEFINITION      | LITERAL      | 2     | address of \*       | EXIT      | back pointer     | ... |
| points to the previous definition | also has "immediate" flag as required | indicates this is a `:` (colon) definition | indicates the next value should be pushed on the stack | the value to push on the stack | address includes a flag indicating `*` is a builtin | acts like a return, ending execution of this word | contains n (address of the next pointer back) |

The builtin flag disinguishes between a builtin function accessed through the jump table, and a word defined in Forth. This lets the interpreter know to look in the builtin array for a function pointer, rather than looking in data space for a definition.

This system I believe is roughly equivalent to indirect threading, which allows a simple state-machine like function to step through a definition, executing words in sequence on the basis of their code addresses. When a new word is entered, the interpreter pushes a return address on the return stack. At the end of the execution of a word (or when the `exit` word is called explicitly), the return stack is popped and the program counter updated accordingly.

The definitions include their names, because the compiler is incremental. When a new word is defined in terms of other words, `find` is called to search back through the dictionary. Once found, we compile the address of the word, rather than the name. So interpretation is a matter of following address links, and all name searching is done at compile time.

The inclusion of the names in the dictionary also supports the `see` operation, which decompiles user definitions, and provides basic documentation for builtin functions. Note that the decompiled version of a function is not identical to the original source code, because control structures (for example) generate branch code and insert that into the definition. This is one of Forth's superpowers. The engine only provides `BRANCH` and `BRANCH0` primitives. All the higher level control structures are implemented in Forth. The source code for these is in `corelib.fs`.

## Memory management and memory errors

Forth does not provide automatic memory management, and in general does not protect the user from illegal memory accesses. It should therefore be understood that once the dictionary or any data the program uses is corrupt, all bets are off, and a restart is usually indicated. Within the Rust code, the compiler does its best to protect the programmer, however Forth allows any address to be passed to store and load (`!`, `c!`,`@ and `c@`) words, so it's easy to cause a panic on bounds violations. Fortunately the binary is small, and startup is quick, so it's not generally too much of a problem.

I may add my own bounds checks to the store and load words, but this will not eliminate all crashes, because many memory accesses occur inside the Rust engine, and a corrupt dictionary, for example, can and will cause problems. There might be a couple of approaches to fixing this:

- Add macros for store and load that do their own bounds checking and cause a Forth `abort` rather than a Rust panic
- Create my own panic handler that looks for bounds errors specifically and runs `abort` rather than crashing the program.

Note however that neither of these solutions are really enough, because in most cases the bad load or store is the result of a bug that may well have corrupted the dictionary, so recovery may still be impossible.

## Future work

The current regression test suite is quite simplistic. It only examines stack results. Implementing the standard test harness would be a good step forward.

# Builtin Words

This is not intended to be full documentation, but a reference to the stack behavior and basic functions of some important words to help with debugging. f3 supports most of the standard stack and arithmetic words.

## System Variables

| WORD    | Notes                                                                                                                                    |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| 'tib    | The address of the text input buffer. TIB is not counted, but the first byte is reserved and not used for text.                          |
| #tib    | The length of the line currently in TIB.                                                                                                 |
| >in     | Pointer to the first unconsumed character in TIB.                                                                                        |
| pad     | Address of the temporary string buffer PAD. PAD is a counted string, used by the parser to hold the current token during interpretation. |
| tmp     | Address of a second temporary string buffer used by string functions to stage new strings.                                               |
| here    | The location of the top of the dictionary, where new elements will be added.                                                             |
| s-here  | The location of the top of string space, where new strings will be added.                                                                |
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
| accept        | ( b u -- b u )                | Read up to u characters, placing them in b. Return the number of characters actually read.                                                                                                                                                                                                                                                        |
| emit          | ( c -- )                      | Print a character, if it's in the printable range from space to 0x7F.                                                                                                                                                                                                                                                                             |
| flush         | ( -- )                        | Force the output buffer to be flushed to the terminal.                                                                                                                                                                                                                                                                                            |
| space         | ( -- )                        | Prints a single space.                                                                                                                                                                                                                                                                                                                            |
| spaces        | ( u -- )                      | Prints u spaces.                                                                                                                                                                                                                                                                                                                                  |
| .s            | ( -- )                        | Print the contents of the stack. Does not consume stack elements.                                                                                                                                                                                                                                                                                 |
| .             | ( v -- )                      | Print the top of the stack as an integer using the value of the `base` variable as the radix.                                                                                                                                                                                                                                                     |
| u.            | ( u -- )                      | Print the top of the stack as an unsigned value                                                                                                                                                                                                                                                                                                   |
| u.r           | ( u w -- )                    | Print unsigned u right-justified in a field w wide. If w is too small, print the full number anyway                                                                                                                                                                                                                                               |
| .r            | ( n w -- )                    | Print integer n right-justified in a field w wide. If w is too small, print the full number anyway                                                                                                                                                                                                                                                |
| cr            | ( -- )                        | Print a newline.                                                                                                                                                                                                                                                                                                                                  |
| s" \<string>" | ( -- )                        | Print the inline string                                                                                                                                                                                                                                                                                                                           |
| type          | ( s -- )                      | Print a string, using the top of stack as a pointer to the string.                                                                                                                                                                                                                                                                                |
| ltype         | ( s w -- )                    | Print a string left justified in a field w characters wide. If w is too small, print the entire string anyway.                                                                                                                                                                                                                                    |
| rtype         | ( s w -- )                    | Print a string right justified in a field w characters wide. If w is too small, print the entire string anyway.                                                                                                                                                                                                                                   |
| tell          | ( s u -- )                    | Print the string at s, of length u                                                                                                                                                                                                                                                                                                                |
| ltell         | ( s u w -- )                  | Print a string of length u left justified in a field w characters wide. If w is too small, print the entire string anyway.                                                                                                                                                                                                                        |
| rtell         | ( s u w -- )                  | Print a string of length u right justified in a field w characters wide. If w is too small, print the entire string anyway.                                                                                                                                                                                                                       |
| r/w           | ( -- )                        | Set file mode to read/write, for file operations.                                                                                                                                                                                                                                                                                                 |
| r/o           | ( -- )                        | Set file mode to read only, for file operations.                                                                                                                                                                                                                                                                                                  |
| w/o           | ( -- )                        | Set file mode to write-only, for file operations.                                                                                                                                                                                                                                                                                                 |
| open-file     | ( s u fam -- file-id ior )    | Open the file named at `s`, string length `u`, with file access mode `fam`. The file-id is an index into a vector of open files, within which the information for the file is kept. This can be accessed by other operations like `file-size` and `file-position`. ior is an i/o system result provided by the operating system. 0 means success. |
| close-file    | ( file-id -- ior )            | Close the file associated with file-id, returning a code indicating success or failure.                                                                                                                                                                                                                                                           |
| read-line     | ( s u file-id -- u flag ior ) | Read up to `u` characters from a file, stopping at the first linefeed, or at the max length `u`. Returns the number of characters read, a flag indicating success or failure, and an io result code.                                                                                                                                              |
| write-line    | ( s u file-id -- ior )        | Write `u` characters from `s` to a file, returning an i/o result code `ior`.                                                                                                                                                                                                                                                                      |

## Text interpreter and Compiler

| WORD              | SIGNATURE                 | NOTES                                                                                                                                                                                                                                                                                                                                                    |
| ----------------- | ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| words             | ( -- )                    | Prints a list of all dictionary entries, whether words, builtins, variables or constants. Each word is preceded by its address in the dictionary for debugging purposes.                                                                                                                                                                                 |
| see               | \<word>                   | The Forth decompiler. If \<word> is a builtin, see provides the documentation for that word. If it's a user-defined word, see provides the source code as compiled. This is often different from the original source code, because control structures are compiled down to lower level branch functions, and are not represented in their original form. |
| abort             | ( -- )                    | Ends execution of the current word, clears the stack, and returns to the interpreter's top level                                                                                                                                                                                                                                                         |
| abort"            | \<message>"               | Print the message and call abort                                                                                                                                                                                                                                                                                                                         |
| quit              | ( -- )                    | Interpreter outer loop: gets a line of input, processes it. Calls `query` and `eval` to do the work.                                                                                                                                                                                                                                                     |
| eval              | ( -- )                    | Interprets a line of input from the `TIB`. Exits when the line is finished, or if `abort` is called.                                                                                                                                                                                                                                                     |
| text              | ( -- b u )                | Gets a space-delimited token from the `TIB`, starting at offset `>IN`. Places it in `PAD`. Returns the address of `PAD` and the number of characters in the token, or 0 if no token could be ready (typically end of line condition).                                                                                                                    |
| \\                | ( -- )                    | Inline comment. Causes the remainder of the line to be ignored.                                                                                                                                                                                                                                                                                          |
| (                 | ( -- )                    | Text from the left paren to its maching closing paren is ignored. Used for documenting stack signatures in word definitions.                                                                                                                                                                                                                             |
| parse             | ( c -- b u )              | Gets a token from `PAD` delimited by `c`. Returns `PAD` address and count.                                                                                                                                                                                                                                                                               |
| (parse)           | ( b u c -- b u delta )    | Find a `c`-delimited token in the string buffer at `b`, of length `u`. Return the pointer to the buffer, the length of the token, and the offset from the start of the buffer to the start of the token.                                                                                                                                                 |
| [char]            | ( -- c )                  | Place the first character of the next token on the stack. Consumes the entire token.                                                                                                                                                                                                                                                                     |
| find              | ( s -- cfa T \| s FALSE ) | Search the dictionary for the token with string at s. Used by `$interpret` and `$compile` to identify the current token.                                                                                                                                                                                                                                 |
| ' \<name>         | ( -- cfa \| FALSE )       | Looks for the (postfix) name in the dictionary. Returns its code field address if found, otherwise FALSE (= 0). If the word is not found, it displays an error message.                                                                                                                                                                                  |
| unique?           | ( s -- s )                | Checks to see if the given string is already defined. If so, returns quietly; otherwise returns `FALSE`.                                                                                                                                                                                                                                                 |
| :                 | ( -- )                    | Sets compile mode to start a definition                                                                                                                                                                                                                                                                                                                  |
| [                 | ( -- )                    | Immediate: set state to interpret mode. Used to force interpretation inside a definition.                                                                                                                                                                                                                                                                |
| ]                 | ( -- )                    | Set state to compile mode. Used inside a definition to undo the effect of a previous `[`.                                                                                                                                                                                                                                                                |
| number?           | (s -- n T \| s F )        | Attempts to convert the string at s to a number. If successful, push the number and a `TRUE` flag. If not successful, leave the string address on the stack, and push `FALSE`. Used inside `$compile` and `$interpret`.                                                                                                                                  |
| literal           | ( n -- )                  | Takes a number from the stack and compiles it into the current definition.                                                                                                                                                                                                                                                                               |
| $interpret        | ( s -- )                  | Called from `eval` to interpret the string at s, either as a word or a number. If neither, `abort`.                                                                                                                                                                                                                                                      |
| $compile          | ( s -- )                  | Called from `eval` to compile the string at s as a word or number. If neither, `abort`.                                                                                                                                                                                                                                                                  |
| , (comma)         | ( v -- )                  | Compiles the value on the stack into the dictionary and updates `here`.                                                                                                                                                                                                                                                                                  |
| create \<name>    | ( -- )                    | Takes a postfix name, and creates a new name field in the dictionary                                                                                                                                                                                                                                                                                     |
| immediate         | ( -- )                    | Marks the most recent definition as immediate by setting a flag on the name field. Immediate words are executed even when compile mode is set. They are most often used to compile control structures that need some level of computation at compile time.                                                                                               |
| immed? ( cfa -- T | F )                       | Tests the word with code field address on the stack, and returns TRUE if it's an immediate word, otherwise FALSE.                                                                                                                                                                                                                                        |
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
| array <name> | ( n -- addr )   | Create an array of `n` elements using the name provided after `array`. Returns the address of the size value. |
| ac@   | ( addr -- n )   | Returns the number of elements in the array                                                                   |
| a@    | ( i addr -- v ) | Returns the value of cell `i` in the array at `addr`.                                                         |
| a!    | ( n i addr -- ) | Stores the value `n` in the array at index `i`.                                                               |
| stack <name> | ( n -- addr )   | Create a stack of `n` elements using the name provided after `stack`. Returns the address of the size value.  |
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
| Nstep-off     | ( -- )    | Turns off single stepping.                                                                                   |
| trace-on      | ( -- )    | Turns on tracing.                                                                                            |
| trace-off     | ( -- )    | Turns off tracing.                                                                                           |
| trace-all     | ( -- )    | Sets trace level t 100                                                                                       |
| stepper-depth | VARIABLE  | Trace / step depth, which can be set manually, or by the use of the `i` and `o` commands within the stepper. |

In addition to the debugger, `dump` commands are provided to inspect memory. `dump` attempts to understand what it's looking at, and provides information accordingly. There are also some debug print statements that only print if `debuglevel` is set to 4. The following functions are available:

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
