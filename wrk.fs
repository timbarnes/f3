\ Upgrades to dump. Showing better information in the dump-val column.
\ One word look-ahead will help a lot. For example, back-pointers come right before an nfa, except for 'here'
\ and the nfa comes right before a cfa. Some tokens are followed by arguments (literals and branches).
\ These might be easier to compute with a look-back. 

 dump-help 
    ." dump ( addr cells -- addr ) dumps heap data." cr
    ." dump-here ( cells -- )      dumps the top of the heap" cr
    ." Opcode reference: "
    ." 100000=BUILTIN  100001=VARIABLE    100002=CONSTANT  100003=LITERAL" cr
    ." 100004=STRLIT   100005=DEFINITION  100006=BRANCH    100007=BRANCH0" cr
    ." 100008=ABORT    100009=EXIT        100010=BREAK     100012=EXEC" cr
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
    128 mod dup 31 > not if space then drop ;



\ Emit a 20 character substring from the string address in the current cell
\ This is usually not useful, and maybe should be replaced by something that works 
\ properly with STRLIT tokens.
: dump-segment
        '|' emit 
        dup 20 + 
        20 for dup i - c@ emit
           next
        '|' emit drop ;

: token-range? 100000 100012 range ;  \ determines if the value is likely to be a token

: string-range? 0 5000 range ;        \ determines if it 'could' be a string address

\ Display the name of a builtin. 
: dump-builtin 
    dup 132 3 * 1- > 
    if
        ." nfa -> " 
    else
        ." buffer -> "
    then
    dup 18 ltype
    ;

\ The tokens are just integers in a specific range. They should probably be flag bits up high.
: dump-token   ( val -- val )
    dup BUILTIN    = if ." BUILTIN              " exit then
    dup VARIABLE   = if ." VARIABLE             " exit then
    dup CONSTANT   = if ." CONSTANT             " exit then
    dup LITERAL    = if ." LITERAL              " exit then
    dup STRLIT     = if ." STRLIT               " exit then
    dup DEFINITION = if ." DEFINITION           " exit then
    dup BRANCH     = if ." BRANCH               " exit then
    dup BRANCH0    = if ." BRANCH0              " exit then
    dup ABORT      = if ." ABORT                " exit then
    dup EXIT       = if ." EXIT                 " exit then
    dup BREAK      = if ." BREAK                " exit then
    dup EXEC       = if ." EXEC                 " exit then
    ." *UNKNOWN* " drop ;   

: builtin? ( addr -- bool )         \ Is the current cell a pointer to a builtin?
    @ BUILTIN_FLAG and   ;          \ A non-zero value is a yes

: definition? ( addr -- bool )
    1 + @                           \ Look at the next cell
    DEFINITION =     ;

: nfa?  ( addr -- bool )            \ Attempts to identify if an addr contains an nfa
    dup builtin? swap definition? or    ;

: bp?   ( addr -- bool )            \ Identifies back pointer by adjacency to an nfa
    1 + nfa?    ;

: offset? ( addr -- bool )          \ Identifies branch offsets
    1 - @ dup BRANCH = swap BRANCH0 = or    ;

: literal? ( addr -- bool )         \ Identifies integer literals
    1 - @ LITERAL =    ;

: strlit?                           \ Identifies string literals
    1 - @ STRLIT = ;

: variable? ( addr -- bool )        \ Identifies a variable's storage cell
    1 - @ VARIABLE =    ;
    
: constant? ( addr -- bool )        \ Identifies a constant's storage cell
    1 - @ CONSTANT =    ;
    
: .builtin ( addr -- )
    @ ADDRESS_MASK and 2 .r ;

\ The master controller that dispatches to a display function
\ This should probably do look-ahead / look-back, and work through the options in sequence.
\ 
: dump-val ( addr -- )
    space
    dup 0 =                 if ." Bottom of memory " exit then
    dup dup builtin?        if ." Builtin: " .builtin exit else drop then
    dup bp?                 if ." Back pointer"   exit then
    dup variable?           if ." Variable value" exit then
    dup constant?           if ." Constant value" exit then
    dup literal?            if ." Literal value"  exit then
    dup dup strlit?         if ." Strlit: "  type exit else drop then
    dup dup definition?     if ." NFA: "   @ type exit else drop then
    dup @ dup token-range?  if ." Token: "  dump-token drop exit else drop then
    @ dump-segment
    ;

: dump-header
    ." ADDRESS CHAR -------------HEX ------------DECIMAL VALUE? " cr
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
    dup here @ swap - swap 2 + dump ;

: dh 25 dump-here ;