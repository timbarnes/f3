: immediate context @ dup @ IMMEDIATE_FLAG or swap ! ;
: compile-only context @ dup @ COMPILE_ONLY_FLAG or swap ! ;
: parse pad @ swap parse-to ;
: \ 1 parse drop drop ; immediate
: ( 41 parse drop drop ; immediate     \ Implements in-line comments

\ The words above come first, because comments need them.
\ immediate and compile-only set a flag on the most recently defined word.
\ The engine defines the opcodes (VARIABLE, LITERAL, BRANCH, ...), the flags (IMMEDIATE_FLAG, ...),
\ ADDRESS_MASK, and the empty-stack addresses sp0 and rp0 as constants (runtime.rs insert_constants).

1 dbg \ set debuglevel to warnings and errors

\ here points to the slot where the new back pointer goes
\ last and context point to the previous word's name field address

: (close) ( -- )                      \ terminate a definition, writing a back pointer and updating context, last, and here
        last @ 1 - here @ !           \ write the new back pointer
        here @ 1 + here !             \ update HERE
        last @ context !              \ update CONTEXT
        ;

\ String space. Strings are counted: the first byte is the length (at most 255).
\ s-here is the next free byte. c@ c! cmove address string space; @ ! address the heap.

: s-allot ( n -- s )   s-here @ dup rot + s-here ! ;       \ reserve n bytes of string space
: s-copy ( from to -- ) over c@ 1 + cmove ;                \ copy a counted string
: s-create ( from -- to )                                  \ copy a counted string to new string space
    dup c@ 1 + dup s-allot swap over >r cmove r> ;

: constant ( n -- ) create CONSTANT , ,      \ v constant <name> creates a constant with value v
    (close) ;

: string ( n -- )
    create STRLIT ,
    s-create dup 0 swap c! , (close) ;

\ Boolean constants. In fact any non-zero value is interpreted as true, but -1 is traditional.

0 constant FALSE
-1 constant TRUE

: show-stack ( -- ) TRUE show-stack? ! ;    \ show the stack at the prompt
: hide-stack ( -- ) FALSE show-stack? ! ;

\ Markers that control structures leave on the stack while they are compiled, checked by ?pairs
200000 constant MARK_BEGIN
200001 constant MARK_WHILE
200003 constant MARK_CASE
200004 constant MARK_OF
200005 constant MARK_DO

\ ASCII symbols that are useful for text processing
10  constant '\n'
32  constant BL
34  constant '"'
39  constant '''
41  constant ')'
45  constant '-'
48  constant '0'
58  constant ':'
61  constant '='
65  constant 'A'
91  constant '['
93  constant ']'
124 constant '|'

\ For file I/O
-1 constant R/W
 0 constant R/O
 1 constant W/O

\ char <character> takes the first character of the next token and puts its ASCII value on the stack
: char
    32 parse drop 1 + c@ ;

\ This version is for use inside a definition. It compiles the value into the definition as a literal.
: [char]
    32 parse drop 1 + c@ LITERAL , , ; immediate compile-only

\ variable <name> creates a variable, initialized to zero
: variable ( -- ) create VARIABLE , 0 ,
    (close) ;

: decimal 10 base ! ;
: hex 16 base ! ;
: binary 2 base ! ;

\ Stepper controls
1 stepper-depth !
: step-on           1 stepper-depth ! -1 stepper ! ;
: step-off          0 stepper ! ;
: trace-on          1 stepper ! 2 stepper-depth ! ; immediate
: trace-all         1 stepper ! 100 stepper-depth ! ; immediate
\ : trace ( n -- )    abs stepper-depth ! ;
: trace-off         0 stepper ! ; immediate

: dbg-debug         3 dbg ;
: dbg-info          2 dbg ;
: dbg-warning       1 dbg ;
: dbg-quiet         0 dbg ;
: debug             show-stack step-on ;

\ trace-on

: [compile]         (') , ; immediate compile-only  \ Cause the following word to be compiled in, even if immediate
: exec EXEC , ; immediate compile-only

: text              BL parse ;                      \ Parser shortcut for space-delimited tokens
: s-parse           tmp @ swap parse-to ;           \ Same as text, but loads to tmp instead of pad
: (s") ( -- s u )   tmp @ '"' parse-to ; immediate  \ Parses a double-quoted string into tmp, returning the address and length
: s" ( -- s u ")    tmp @ '"' parse-to ;            \ Places a double-quoted string in tmp

\ File reader functions

\ included gets the file path already in TMP and loads it, leaving a success flag
: included  ( -- bool )
                    tmp @ include-file ; \ include-file uses a string pointer on the stack to load a file

\ include is for interactive use. It gets a file path from the user and loads it.
: include   ( -- )
                    tmp @ BL parse-to
                    drop include-file drop ;        \ include-file only needs the address

: [ FALSE state ! ; immediate                       \ Turns compile mode off
: ] TRUE state ! ;                                  \ Turns compile mode on

: recurse ( -- )                                    \ Simply compiles the cfa of the word being defined
                    last @ 1 + , ; immediate compile-only \ last points to the latest nfa, so increment

: nip ( a b -- b )  swap drop ;
: tuck ( a b -- b a b ) swap over ;

\ if - else - then

: _patch-here ( addr -- ) \ Address patcher for if - else - then
    here @ over - swap ! ;

: if     BRANCH0 , here @ 0 , ; immediate compile-only
: else   BRANCH , here @ 0 , swap _patch-here ; immediate compile-only
: then   _patch-here ; immediate compile-only

\ Finding the address of defined words

: ' ( <name> -- xt ) (') ;                                \ the execution token (cfa) of the next word

: [']               LITERAL , ' , ; immediate compile-only \ compiles a word's cfa into a definition as a literal

: abort" ( -- )     STRLIT , s" drop s-create ,         \ abort with a message. Use inside another word.
                    ['] (error) , ; immediate compile-only

\ Movement within a word definition

: cfa>nfa           1 - ;                                 \ converts an cfa to an nfa
: cfa>val           1 + ;                                 \ from cfa to first parameter cell
: nfa>cfa           1 + ;                                 \ converts an nfa to a cfa
: bp>nfa            1 + ;                                 \ from preceding back pointer to nfa
: bp>cfa            2 + ;

\ Arithmetic and logical operations

: 1- ( n -- n-1 )   1 - ;
: 1+ ( n -- n+1 )   1 + ;
: negate ( n -- -n ) 0 swap - ;
: not               0= ;
: exit ( -- )       BREAK , ; immediate compile-only    \ Pop out of the current definition and reset the Program Counter
: >                 swap < ;
: <=                1 + < ;
: <> ( n -- n )     = 0= ;
: 0>                0 > ;
: 0<>               0= 0= ;
: invert ( n -- ~n ) -1 xor ;

: ?pairs ( tag expected -- )                          \ check that control structures match up
                    <> if abort" unbalanced control structure" then ;

\ Stack operations

: pop ( a -- )      drop ;
: 2dup ( a b -- a b a b ) over over ;
: 2drop ( a b -- )  drop drop ;
: ?dup              dup 0= if else dup then ;

\ The data stack lives in the heap and grows down: sp@ is the address of the top item,
\ the item n deep is at sp@ + n, and sp0 is sp@ when the stack is empty.
: depth ( -- n )    sp@ sp0 swap - ;
: pick ( xn..x0 n -- xn..x0 xn )
                    dup depth 2 - u< 0= if abort" pick: stack underflow" then
                    1 + sp@ + @ ;
: roll ( xn..x0 n -- xn-1..x0 xn )
                    dup depth 2 - u< 0= if abort" roll: stack underflow" then
                    ?dup if swap >r 1- recurse r> swap then ;

\ Return stack operations

: rdrop  ( -- )     r> drop ;
: 2rdrop ( -- )     rdrop rdrop ;
: 2>r ( m n -- )    >r >r ;
: 2r> ( -- n m )    r> r> ;
: 2r@ ( -- n m )    r> r> 2dup >r >r ;

\ \\\\\\\\\\\\\\\\\\\\\\\\\\\\\
\ Memory allocation
\
\ allot ( n -- ) is a builtin: n > 0 reserves n cells of data space, n < 0 releases them.

\ Used to convert to specify cells in Forths that have byte or other addressing modes (e.g. 8 bit processors with 16-bit addresses)
\           Not required, but included for completeness.
: cells ;

\ Numeric operations

: min ( m n -- m | n ) 2dup < if drop else nip then ;
: max ( m n -- m | n ) 2dup > if drop else nip then ;
: abs ( n -- n | -n ) dup 0 < if -1 * then ;
: range ( n l h -- b ) \ Returns TRUE if n is in the range of l to h inclusive
    1 + swap 1 -
    2 pick < 2 roll 2 roll < and ;

\ Control structures

: begin ( -- a tag ) here @ MARK_BEGIN ; immediate compile-only

: while ( -- a tag ) BRANCH0 ,
                    here @ MARK_WHILE 0 , ; immediate compile-only     \ placeholder for the exit offset
: repeat ( -- )     MARK_WHILE ?pairs
                    BRANCH ,                            \ unconditional branch to the beginning of the loop
                    here @ over - 1 + swap !            \ patch forward offset at WHILE's placeholder
                    MARK_BEGIN ?pairs
                    here @ - ,           ; immediate compile-only \ save negative offset to BEGIN

: until             MARK_BEGIN ?pairs BRANCH0 ,
                    here @ - ,           ; immediate compile-only

: again             MARK_BEGIN ?pairs BRANCH ,
                    here @ - ,           ; immediate compile-only

: clear ( ... -- )  begin depth while drop repeat ;   \ empty the data stack

\ Counted loops. FOR, DO and ?DO compile an opcode whose operand is the offset to the loop exit;
\ NEXT, LOOP and +LOOP compile an opcode whose operand is the offset back to the loop body.
\ At run time each loop keeps three cells on the return stack: exit address, limit, index.

: (loop-start) ( opcode -- a tag )
                    , here @ MARK_DO 0 , ;              \ placeholder for the exit offset
: (loop-end) ( a tag opcode -- )
                    , MARK_DO ?pairs                    \ a: address of the exit-offset placeholder
                    dup 1+ here @ - ,                   \ offset back to the loop body
                    here @ over - swap ! ;              \ patch the exit offset

: for ( n -- )      FOR (loop-start) ; immediate compile-only   \ runs the body n times, with i = n .. 1
: next ( -- )       NEXT (loop-end) ; immediate compile-only
: do ( limit start -- )  DO (loop-start) ; immediate compile-only
: ?do ( limit start -- ) QDO (loop-start) ; immediate compile-only \ skips the loop if limit = start
: loop ( -- )       LOOP (loop-end) ; immediate compile-only
: +loop ( n -- )    PLOOP (loop-end) ; immediate compile-only
: leave ( -- )      LEAVE , ; immediate compile-only    \ continue after the end of the innermost loop

\ Case statement: the key is kept on the return stack while the clauses are tested

: case ( key -- )   ['] >r , MARK_CASE ; immediate compile-only
: of ( n -- )       ['] r@ , ['] = ,                    \ compile the test against the key
                    BRANCH0 , here @ MARK_OF 0 , ; immediate compile-only
: endof ( -- )      MARK_OF ?pairs
                    BRANCH , here @ 0 ,                 \ jump to the end of the case, patched by endcase
                    swap _patch-here ; immediate compile-only   \ a failed test continues here
: endcase ( -- )    begin dup MARK_CASE <> while _patch-here repeat drop
                    ['] r> , ['] drop , ; immediate compile-only

\ Takes a typical descending for - next loop, and simplifies reversing the direction of the loop variable
\     usage is : word incr-for for dup i - ... next .. ;
: incr-for ( m n -- m m+n n )
    over over + swap ;

\ system executes a command in the shell. The shell is exited afterwards.
\   This means that commands like 'cd' are not persistent.
\
: system" ( <command> ) tmp @ '"' parse-to drop (system) ;

: sec ( n -- )      1000 * ms ;  \ sleep for n seconds

\ Printing words

: emit ( c -- )     \ print a character if in the printable range
                    128 mod dup 31 > if (emit) else drop then ;

: space ( -- )      BL emit ;

: spaces ( n -- )   dup 0> if for space next else drop then ;
: cr ( -- )         '\n' (emit) ;

: tell ( s l -- )                               \ like type, but length is provided: useful for substrings
                    swap ADDRESS_MASK and
                    swap 2dup + rot drop swap
                    for
                        dup i - c@ emit
                    next
                        drop ;

: type ( s -- )                                 \ Print from the string pointer on the stack
                    ADDRESS_MASK and            \ Wipe out any flags
                    dup c@ swap 1+ swap         \ Get the length to drive the for loop
                    dup 0= if exit then
                    tell ;

: rtell ( s l w -- )                            \ Right justify a string of length l in a field of w characters
                    over - 1 max
                    spaces tell ;

: ltell ( s l w -- ) 2dup swap -
                    nip rot rot
                    tell spaces ;

: rtype ( s w -- )  swap ADDRESS_MASK and dup c@
                    rot swap - spaces type ;

: ltype             swap ADDRESS_MASK and dup c@
                    rot swap - swap type spaces ;

: .tmp              tmp @ type ;                               \ Print the tmp buffer
: .pad              pad @ type ;                               \ Print the pad buffer
: ."  ( -- )        state @                                    \ Compile or print a string
                    if
                        STRLIT ,                               \ Compilation section
                        s" drop s-create ,
                        ['] type ,
                    else
                        s" drop type                           \ Execution (print) section
                    then ; immediate

\ Numeric print words

: /mod              2dup mod rot rot / ;

: .d ( n -- )       dup 10 <
                    if '0' else 10 - 'A' then
                    + emit ;    \ print a single digit

: .- ( -- )         '-' emit ;       \ print a minus sign

: u.    ( u -- )    base @ /mod
                    ?dup if recurse then .d ;

: uwidth ( u -- n ) \ returns the number of digits in an unsigned number
                    base @ /
                    ?dup if recurse 1+ else  1 then ;

: u.r ( u width -- )
                    swap dup uwidth rot swap -
                    spaces u. ;

: (n.) ( n -- )      base @ /mod ?dup if recurse then negate .d ;  \ the digits of n <= 0
: .r ( n width -- )                             \ print n right-aligned in width characters
                    over uwidth - over 0< if 1- then spaces
                    dup 0< if '-' emit else negate then (n.) ;   \ works from the negative side,
                                                                 \ so the most negative number prints too
: . 0 .r space ;

: .s ( -- )         ." [ " depth for i 1- pick . next ." ] " flush ;   \ print the data stack
: .rs ( -- )        ." R: [ "                           \ print the return stack, below .rs's own return
                    rp@ 1+ rp0 over - for dup i + 1- @ . next drop ." ] " ;

: stop"  ( -- )     state @                                    \ Compile or print a string
                    if
                        STRLIT ,                               \ Compilation section
                        s" drop s-create ,
                        ['] type ,
                        ['] .s ,
                        ['] flush ,
                        ['] key , ['] drop ,
                    else
                        s" drop type                           \ Execution (print) section
                        .s
                        ." Stopped: "
                        flush key drop
                    then ; immediate


\ Variable utilities

: +! ( n addr -- )  dup @ rot + swap ! ;
: ?  ( addr -- )    @ . ;

: forget-last ( -- )                            \ delete the most recent definitions
                    here @ 1- @ dup 1+ here !   \ resets HERE to the previous back pointer
                    @ 1+ dup context ! last !   \ resets CONTEXT and LAST
                    ;

: forget ( <name> )                             \ delete <name> and any words since
                    trace-off step-off          \ we're messing with the dictionary, so we don't want to run FIND
                    (') dup
                    if
                        1- dup dup here ! @ s-here !            \ move to nfa and set HERE and S-HERE
                        1- @ 1+ dup context ! last !            \ go back a link and set CONTEXT and LAST
                    else
                        drop
                    then ;

\ : ?stack depth 0= if abort" Stack underflow" then ;

: kkey ( -- c )     >in @ c@ 1 >in +! ;                         \ Get the next character from the TIB

: ?key ( -- c T | F )                                           \ If there's a character in TIB, push it and TRUE
                    #tib @ >in @ < if FALSE else key TRUE then ; \ otherwise push FALSE

: strlen ( s -- n ) c@ ;                                        \ return the count byte from the string

\ Dictionary inspection

: immed? ( cfa -- f ) 1- @ IMMEDIATE_FLAG and 0<> ;              \ is the word at cfa immediate?
: ?unique ( s -- s ) dup find nip if ." redefining " dup type cr then ;

: .name ( cfa -- )  1- @ ADDRESS_MASK and type space ;          \ print the name of the word at cfa
: .operand ( a -- a+1 ) 1+ dup @ . ;
: see-cell ( a -- a' more? )                                    \ print the compiled cell at a
    dup @ case
        LITERAL of .operand TRUE endof
        STRLIT  of 1+ '"' emit space dup @ type '"' emit space TRUE endof
        BRANCH  of ." branch:" .operand TRUE endof
        BRANCH0 of ." branch0:" .operand TRUE endof
        DO      of ." do:" .operand TRUE endof
        QDO     of ." ?do:" .operand TRUE endof
        LOOP    of ." loop:" .operand TRUE endof
        PLOOP   of ." +loop:" .operand TRUE endof
        FOR     of ." for:" .operand TRUE endof
        NEXT    of ." next:" .operand TRUE endof
        LEAVE   of ." leave " TRUE endof
        ABORT   of ." abort " TRUE endof
        BREAK   of ." exit " TRUE endof
        EXEC    of ." exec " TRUE endof
        EXIT    of ." ; " FALSE endof
        r@ BUILTIN_FLAG and
        if r@ ADDRESS_MASK and builtin-name type space else r@ .name then
        TRUE
    endcase
    swap 1+ swap ;
: (see) ( cfa -- )                                              \ decompile the word at cfa
    dup @ BUILTIN_FLAG and if @ ADDRESS_MASK and ." Builtin: " builtin-doc type cr exit then
    dup @ CONSTANT = if ." Constant: " dup .name ." = " 1+ @ . cr exit then
    dup @ VARIABLE = if ." Variable: " dup .name ." = " 1+ @ . cr exit then
    dup @ DEFINITION <> if drop ." not a colon definition" cr exit then
    ." : " dup .name
    dup 1- @ IMMEDIATE_FLAG and >r
    1+ begin see-cell while repeat drop
    r> if ." immediate" then cr ;
: see ( <name> -- ) ' (see) ;

include terminal.fs
include debug.fs
include editor.fs
include sequences.fs
