\ Memory dump and debugging utilities.

\ Word - display the contents of the dictionary.

variable word-counter

: .word ( bp -- bp )                            \ prints a word name, given the preceding back pointer
                    dup dup cell+ dim 4 u.r t-reset space cell+ @ 13 ltype
                    1 word-counter +!
                    word-counter @ 8 mod
                    if space else cr then @ ;

: words ( -- )
                    0 word-counter !
                    context @ cell-                           \ Get the starting point: the newest link field
                    begin                                       \ loops through the words in the dictionary
                        .word dup not                           \ print a word and test the next pointer
                    until
                        drop cr ;

: print-word ( xt -- )
                    cell- @ 13 ltype ;                          \ print a word name, given the xt as produced by '

\ String Dump capability
\ dump-buffers - dumps the input line, PAD and TMP
\ dump-string  ( s_addr -- ) - dumps a single counted string at s_addr

132 constant buf-len

\ Dump a string buffer from the beginning, for <count> characters
: dump-buffer ( s_addr -- )    \ Dump contents of a string input buffer
    ." Buffer at " dup 0 .r ." , length " dup c@ 0 .r ." : "
    buf-len incr-for
    for dup i - c@ emit next
    drop cr
    26 spaces
    c@ for
        '-' emit
    next cr
    ;

\ Dump a counted string, given a starting address
: dump-string ( s_addr -- )
    dup 5 .r ." /" dup c@ 3 .r ." : "
    dup ctype cr drop ;

\ Print the buffers
: dump-buffers ( -- )
    ." TIB: " source type cr
    ." PAD " cr
    pad dump-buffer
    ." TMP " cr
    tmp @ dump-buffer
    ;


\ Memory Dump Utility
\ Attempts to print useful information from the dictionary / heap for debugging purposes.
\
\ dump      ( addr count -- ) prints count records, starting from addr
\ dump-here ( count -- )      prints count records below HERE, plus two above
\ dh        ( -- )            prints 25 records at the top of the dictionary
\ dmp       ( addr -- )       prints 25 records starting from addr
\ dump-help ( -- )            prints help information

\ Output functions

: dump-help
    ." dump ( addr cells -- addr ) dumps heap data." cr
    ." dump-here ( cells -- )      dumps the top of the heap" cr
    ." Opcodes are cells with OPCODE_FLAG set, and are shown by name." cr
    ;

: dump-addr  ( addr -- addr ) dup 7 .r ;

: dump-hex   ( val -- val ) dup @ 16 hex .r decimal ;

: dump-dec   ( val -- val ) dup @ 20 .r ;

: dump-char  ( val -- val )
    dup
    space ''' emit
    emit
    ''' emit
    space space
    128 mod dup 32 126 range not if space then drop ;

\ Emit a 20 character substring from the string address in the current cell
\ This is usually not useful, and is a last resort if no other identification has been made.
: dump-segment
        18 spaces
        '|' emit
        dup 20 +
        20 for dup i - c@ emit
           next
        '|' emit drop ;

\ The tokens are opcodes: cells with OPCODE_FLAG set.
: dump-token   ( val -- val )
    dup op:variable   = if ." VARIABLE             " exit then
    dup op:constant   = if ." CONSTANT             " exit then
    dup op:literal    = if ." LITERAL              " exit then
    dup op:strlit     = if ." STRLIT               " exit then
    dup op:array      = if ." ARRAY                " exit then
    dup op:definition = if ." DEFINITION           " exit then
    dup op:branch     = if ." BRANCH               " exit then
    dup op:branch0    = if ." BRANCH0              " exit then
    dup op:abort      = if ." ABORT                " exit then
    dup op:exit       = if ." EXIT                 " exit then
    dup op:break      = if ." BREAK                " exit then
    dup op:exec       = if ." EXEC                 " exit then
    dup op:do         = if ." DO                   " exit then
    dup op:qdo        = if ." ?DO                  " exit then
    dup op:loop       = if ." LOOP                 " exit then
    dup op:ploop      = if ." +LOOP                " exit then
    dup op:leave      = if ." LEAVE                " exit then
    dup op:for        = if ." FOR                  " exit then
    dup op:next       = if ." NEXT                 " exit then
    dup op:created    = if ." CREATED              " exit then
    dup op:does       = if ." DOES>                " exit then
    ." *UNKNOWN* " drop ;

: dump-word ( addr -- )             \ Print the name of the word with compile reference at addr
    @ cell- @ ctype
    ;

: dump-builtin ( addr -- )
    @ ADDRESS_MASK and
    dup 2 .r
    ." : " builtin-name ctype
    ;

: dump-immed ( addr -- )            \ Output an immediate word's name
    @ ADDRESS_MASK and 12 ltype
    ;

\ Predicates to identify cell types

: token-range? OPCODE_FLAG and 0<> ;  \ determines if the value is an opcode

: builtin? ( addr -- bool )         \ Is the current cell a pointer to a builtin?
    @ BUILTIN_FLAG and ;            \ A non-zero value is a yes

: builtin-name? ( addr -- bool )     \ Builtin nfa is before the builtin reference
    cell+ builtin?
    ;

: definition-name? ( addr -- bool )
    cell+ @                         \ Look at the next cell
    op:definition = ;

: variable-name?    ( addr -- bool )
    cell+ @
    op:variable = ;

: constant-name?    ( addr -- bool )
    cell+ @
    op:constant = ;

: array-name?    ( addr -- bool )
    cell+ @
    op:array = ;

: _nfa?  ( addr -- bool )            \ Attempts to identify if an addr contains an nfa
    \ Five types: builtin, definition, constant, and variable ;
    dup builtin?
    swap dup builtin-name?
    swap dup definition-name?
    swap dup variable-name?
    swap dup constant-name?
    swap array-name?
    or or or or or
        ;

: bp?   ( addr -- bool )            \ Identifies back pointer by adjacency to dictionary entries
    cell+
    dup @ token-range? if
        drop FALSE
    else
        cell+
        dup dup builtin? swap cell- _nfa? and
        swap @ dup op:definition =
        swap dup op:array =
        swap dup op:variable =
        swap op:constant =
        or or or or
    then ;

: nfa?  ( addr -- bool )
    cell- bp?
    ;

: immediate? ( addr -- bool )
    @ IMMEDIATE_FLAG and
    ;

: compiled-value? ( addr -- bool )  \ Identifies non-nfa values, typically in immediate words
    dup cell- bp? not                   \ These values appear inside a word, not after a back pointer
    swap bp? not
    and
    ;

: offset-value? ( addr -- bool )          \ Identifies branch offsets
    cell- @ dup op:branch = swap op:branch0 = or    ;

: lit-value? ( addr -- bool )         \ Identifies integer literals
    cell- @ op:literal =    ;

: strlit-value?                           \ Identifies string literals
    cell- @ op:strlit = ;

: var-value? ( addr -- bool )        \ Identifies a variable's storage cell
    cell- @ op:variable =    ;

: const-value? ( addr -- bool )        \ Identifies a constant's storage cell
    cell- @ op:constant =    ;

: def-call?  ( addr -- )           \ Identifies a compiled call to a Forth definition
    @ @ op:definition =
    ;

: var-call?  ( addr -- )            \ Identifies a compiled reference to a variable
    @ @
    dup op:variable =
    swap op:constant =
    or
    ;

\ The master controller that dispatches to a display function
\ Analysis proceeds from the most specific to the most general cases.
\
: dump-val ( addr -- )
    space
    \ Deterministic cases
    dup 0 =                 if ." Bottom of memory "                 exit then
    dup here =            if ."             HERE "                 exit then
    dup here >            if ."       Free space "                 exit then
    dup @ 0 <               if ."            Value "                 exit then
    dup dup builtin?        if ."         Builtin: "    dump-builtin exit else drop then
    dup dup immediate?      if inverse ." *Immediate* NFA: " dump-immed t-reset exit else drop then
    \ Non-deterministic (heuristic) cases.
    \
    \ A range of integer values used by the system as tokens, that *might* be tokens
    dup @ dup token-range?  if ."           Token: " dump-token drop exit else drop then
    dup lit-value?          if ."  Integer Literal "                 exit then
    dup var-value?          if ."            Value "                 exit then
    dup const-value?        if ." Constant Literal "                 exit then
    dup dup strlit-value?   if ."  String Literal: "            ctype exit else drop then
    dup offset-value?       if ."    Branch offset "                 exit then
    dup dup def-call?       if ." Definition call: "       dump-word exit else drop then
    dup dup var-call?       if ."    Var/Const ref "       dump-word exit else drop then
    dup bp?                 if ."     Back Pointer " cr              exit then
    dup dup nfa?            if inverse ."             NFA: " @ 12 ltype t-reset exit else drop then
    dup compiled-value?     if ."   Compiled value "                 exit then
    @ dump-segment
    ;

: dump-header
    dim ." ADDRESS CHAR -------------HEX ------------DECIMAL ------------KIND VALUE" t-reset cr
    ;
\ : dump-val ."  some value " .s ;
\ Output a single cell's information
: dump-cell
        dup
        dump-addr @
        dump-char
        dump-hex
        dump-dec
        dump-val
        cr drop
    ;

\ Display the contents of a number of cells
: dump       ( addr cells -- addr )
    dump-header
    0 ?do dup i cells + dump-cell loop
    drop ;

\ dump-here dumps the top n cells. Useful for seeing recent dictionary entries.
: dump-here     ( n -- )
    dup here swap cells - swap dump
    dim ." **************** HERE ********** TOP OF HEAP *********************" t-reset cr
    here 2 dump ;

: dh 25 dump-here ;
: dmp 25 dump ;


\ \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\
\ Debug print commands.
\   These only produce output if the dbg variable is set to 4 (debug).
\
: debug? debuglevel 3 = ;

: d. debug?     ( n -- n )
    if dup . then ;

: d.cr debug?   ( -- )
    if cr then ;

: d.s debug?    ( -- )
    if .s then ;
