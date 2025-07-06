\ Memory dump and debugging utilities.

\ Word - display the contents of the dictionary.

variable word-counter

: .word ( bp -- bp )                            \ prints a word name, given the preceding back pointer
                    dup dup 1+ dim 4 u.r t-reset space 1+ @ 13 ltype 
                    1 word-counter +! 
                    word-counter @ 8 mod
                    if space else cr then @ ;   

: words ( -- )
                    0 word-counter !
                    here @ 1- @                                 \ Get the starting point: the top back pointer
                    begin                                       \ loops through the words in the dictionary
                        .word dup not                           \ print a word and test the next pointer
                    until 
                        drop cr ;   

: print-word ( xt -- ) 
                    1- @ 13 ltype ;                 \ print a word name, given the xt as produced by '


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
    ." Opcode reference: "
    ." 100000=BUILTIN  100001=VARIABLE    100002=CONSTANT  100003=LITERAL" cr
    ." 100004=STRLIT   100005=DEFINITION  100006=BRANCH    100007=BRANCH0" cr
    ." 100008=ABORT    100009=EXIT        100010=BREAK     100011=EXEC" cr
    ." 100012=ARRAY" cr
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

\ The tokens are just integers in a specific range. They should probably be flag bits up high.
: dump-token   ( val -- val )
    dup BUILTIN    = if ." BUILTIN              " exit then
    dup VARIABLE   = if ." VARIABLE             " exit then
    dup CONSTANT   = if ." CONSTANT             " exit then
    dup LITERAL    = if ." LITERAL              " exit then
    dup STRLIT     = if ." STRLIT               " exit then
    dup ARRAY      = if ." ARRAY                " exit then
    dup DEFINITION = if ." DEFINITION           " exit then
    dup BRANCH     = if ." BRANCH               " exit then
    dup BRANCH0    = if ." BRANCH0              " exit then
    dup ABORT      = if ." ABORT                " exit then
    dup EXIT       = if ." EXIT                 " exit then
    dup BREAK      = if ." BREAK                " exit then
    dup EXEC       = if ." EXEC                 " exit then
    ." *UNKNOWN* " drop ;   

: dump-word ( addr -- )             \ Print the name of the word with compile reference at addr
    @ 1 - @ type
    ;

: dump-builtin ( addr -- )
    @ ADDRESS_MASK and 
    dup 2 .r 
    ." : " builtin-name type 
    ;

: dump-immed ( addr -- )            \ Output an immediate word's name
    @ ADDRESS_MASK and type
    ;

\ Predicates to identify cell types

: token-range? 100000 100012 range ;  \ determines if the value is likely to be a token

: string-range? 0 5000 range ;        \ determines if it 'could' be a string address

: builtin? ( addr -- bool )         \ Is the current cell a pointer to a builtin?
    @ BUILTIN_FLAG and ;            \ A non-zero value is a yes

: builtin-name? ( addr -- bool )     \ Builtin nfa is before the builtin reference
    1 + builtin?
    ;

: definition-name? ( addr -- bool )
    1 + @                           \ Look at the next cell
    DEFINITION = ;

: variable-name?    ( addr -- bool )
    1 + @
    VARIABLE = ;

: constant-name?    ( addr -- bool )
    1 + @
    CONSTANT = ;

: array-name?    ( addr -- bool )
    1 + @
    ARRAY = ;

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
    1 +
    dup @ token-range? if
        drop FALSE
    else
        1 +
        dup dup builtin? swap 1 - _nfa? and
        swap @ dup DEFINITION = 
        swap dup ARRAY =
        swap dup VARIABLE =
        swap CONSTANT =
        or or or or
    then ;

: nfa?  ( addr -- bool )
    1 - bp?
    ;

: immediate? ( addr -- bool )
    @ IMMEDIATE_FLAG and
    ;

: compiled-value? ( addr -- bool )  \ Identifies non-nfa values, typically in immediate words
    dup 1 - bp? not                     \ These values appear inside a word, not after a back pointer
    swap bp? not
    and
    ;

: offset-value? ( addr -- bool )          \ Identifies branch offsets
    1 - @ dup BRANCH = swap BRANCH0 = or    ;

: lit-value? ( addr -- bool )         \ Identifies integer literals
    1 - @ LITERAL =    ;

: strlit-value?                           \ Identifies string literals
    1 - @ STRLIT = ;

: var-value? ( addr -- bool )        \ Identifies a variable's storage cell
    1 - @ VARIABLE =    ;
    
: const-value? ( addr -- bool )        \ Identifies a constant's storage cell
    1 - @ CONSTANT =    ;
    
: def-call?  ( addr -- )           \ Identifies a compiled call to a Forth definition
    @ @ DEFINITION = 
    ;

: var-call?  ( addr -- )            \ Identifies a compiled reference to a variable
    @ @ 
    dup VARIABLE =
    swap CONSTANT = 
    or
    ;

\ The master controller that dispatches to a display function
\ Analysis proceeds from the most specific to the most general cases.
\ 
: dump-val ( addr -- )
    space
    \ Deterministic cases
    dup 0 =                 if ." Bottom of memory "                 exit then
    dup here @ =            if ."             HERE "                 exit then
    dup here @ >            if ."       Free space "                 exit then
    dup @ 0 <               if ."            Value "                 exit then
    dup dup builtin?        if ."         Builtin: "    dump-builtin exit else drop then
    dup dup immediate?      if ." *Immediate* NFA: " inverse dump-immed t-reset exit else drop then
    \ Non-deterministic (heuristic) cases.
    \
    \ A range of integer values used by the system as tokens, that *might* be tokens
    dup @ dup token-range?  if ."           Token: " dump-token drop exit else drop then
    dup lit-value?          if ."  Integer Literal "                 exit then
    dup var-value?          if ."            Value "                 exit then
    dup const-value?        if ." Constant Literal "                 exit then
    dup dup strlit-value?   if ."  String Literal: "          @ type exit else drop then
    dup offset-value?       if ."    Branch offset "                 exit then
    dup dup def-call?       if ." Definition call: "       dump-word exit else drop then
    dup dup var-call?       if ."    Var/Const ref "       dump-word exit else drop then
    dup bp?                 if ."     Back Pointer " cr              exit then
    dup dup nfa?            if ."             NFA: " @ inverse type t-reset exit else drop then
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
    incr-for for dup i - 
        dump-cell
    next 
    drop drop ;

\ dump-here dumps the top n cells. Useful for seeing recent dictionary entries.
: dump-here     ( n -- )
    dup here @ 1 - swap - swap dump
    dim ." **************** HERE ********** TOP OF HEAP *********************" t-reset cr
    here @ 2 dump ;

: dh 25 dump-here ;
: dmp 25 dump ;


\ \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\
\ Debug print commands.
\   These only produce output if the dbg variable is set to 4 (debug).
\
: debug? debuglevel 4 = ;

: d. debug?     ( n -- n ) 
    if dup . then ;

: d.cr debug?   ( -- ) 
    if cr then ;

: d.s debug?    ( -- ) 
    if .s then ;
