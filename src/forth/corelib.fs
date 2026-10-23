: immediate context @ dup @ IMMEDIATE_FLAG or swap ! ;
: compile-only context @ dup @ COMPILE_ONLY_FLAG or swap ! ;
: here dp @ last @ context @ = + ;
: pad 'pad @ ;
: parse pad swap parse-to ;
: \ 1 parse drop drop ; immediate
: ( 41 parse drop drop ; immediate     \ Implements in-line comments

\ The words above come first, because comments need them.
\ here is the next free cell. dp points past the latest finished word's back pointer, which
\ , and allot move up as they extend that word, so then here is dp - 1 (LAST = CONTEXT gives -1).
\ immediate and compile-only set a flag on the most recently defined word.
\ The engine defines the opcodes (VARIABLE, LITERAL, BRANCH, ...), the flags (IMMEDIATE_FLAG, ...),
\ ADDRESS_MASK, and the empty-stack addresses sp0 and rp0 as constants (runtime.rs insert_constants).

1 dbg \ set debuglevel to warnings and errors

\ here points to the slot where the new back pointer goes
\ last and context point to the previous word's name field address

: (close) ( -- )                      \ terminate a definition, writing a back pointer and updating context, last, and here
        last @ 1 - here !           \ write the new back pointer
        here 1 + dp !             \ update HERE
        last @ context !              \ update CONTEXT
        ;

\ create <name> makes a word that pushes the address of its data field, which , and allot fill.
\ does> gives the latest such word an action: the code after does> runs with that address.
\ f3's own defining words (constant, variable, ...) use the header-only (create) and (close).
: create ( <name> -- ) (create) op:created , 0 , (close) ;   \ cfa: op:created, then the does> code
: does> ( -- )      op:does , ; immediate compile-only
: >body ( xt -- a ) 2 + ;                              \ a created word's data field

\ String space. Strings are counted: the first byte is the length (at most 255).
\ s-here is the next free byte. c@ c! cmove address string space; @ ! address the heap.

: s-allot ( n -- s )   s-here @ dup rot + s-here ! ;       \ reserve n bytes of string space
: s-copy ( from to -- ) over c@ 1 + cmove ;                \ copy a counted string
: s-create ( from -- to )                                  \ copy a counted string to new string space
    dup c@ 1 + dup s-allot swap over >r cmove r> ;

: constant ( n -- ) (create) op:constant , ,      \ v constant <name> creates a constant with value v
    (close) ;

: string ( n -- )
    (create) op:strlit ,
    s-create dup 0 swap c! , (close) ;

\ Boolean constants. In fact any non-zero value is interpreted as true, but -1 is traditional.

0 constant FALSE
-1 constant TRUE

: show-stack ( -- ) TRUE show-stack? ! ;    \ show the stack at the prompt
: hide-stack ( -- ) FALSE show-stack? ! ;

\ Markers that control structures leave on the stack while they are compiled, checked by ?pairs
200000 constant MARK_BEGIN
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
    32 parse drop 1 + c@ op:literal , , ; immediate compile-only

: literal ( x -- )  op:literal , , ; immediate compile-only   \ compile x from the stack as a literal

\ variable <name> creates a variable, initialized to zero
: variable ( -- ) (create) op:variable , 0 ,
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
: exec op:exec , ; immediate compile-only

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
    here over - swap ! ;

: if     op:branch0 , here 0 , ; immediate compile-only
: else   op:branch , here 0 , swap _patch-here ; immediate compile-only
: then   _patch-here ; immediate compile-only

\ Finding the address of defined words

: ' ( <name> -- xt ) (') ;                                \ the execution token (cfa) of the next word

: [']               op:literal , ' , ; immediate compile-only \ compiles a word's cfa into a definition as a literal

: postpone ( <name> -- )                              \ compile the compilation behaviour of <name>:
                    ' dup 1 - @ IMMEDIATE_FLAG and      \ an immediate word is compiled as a call,
                    if , else op:literal , , ['] , , then ; immediate compile-only  \ others compile a ,

: abort" ( x -- )   op:branch0 , here 0 ,               \ if x is non-zero, abort with the message (ANS)
                    op:strlit , s" drop s-create ,
                    ['] (error) , _patch-here ; immediate compile-only

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
: exit ( -- )       op:break , ; immediate compile-only    \ Pop out of the current definition and reset the Program Counter
: >                 swap < ;
: <=                1 + < ;
: <> ( n -- n )     = 0= ;
: 0>                0 > ;
: 0<>               0= 0= ;
: invert ( n -- ~n ) -1 xor ;
: 2* ( n -- 2n )    1 lshift ;
: u> ( u1 u2 -- f ) swap u< ;
: within ( n lo hi -- f ) over - >r - r> u< ;         \ lo <= n < hi, also across the wrap
: cell+ ( a -- a' ) 1+ ;                              \ heap addresses count cells, so these are
: char+ ( c -- c' ) 1+ ;                              \ simple, and chars, align, aligned do nothing
: chars ( n -- n ) ;
: align ( -- ) ;
: aligned ( a -- a ) ;
: count ( s -- c-addr u ) dup 1+ swap c@ ;           \ a counted string's characters and length

: ?pairs ( tag expected -- )                          \ check that control structures match up
                    <> abort" unbalanced control structure" ;

\ Stack operations

: pop ( a -- )      drop ;
: 2dup ( a b -- a b a b ) over over ;
: 2drop ( a b -- )  drop drop ;
: ?dup              dup 0= if else dup then ;

\ The data stack lives in the heap and grows down: sp@ is the address of the top item,
\ the item n deep is at sp@ + n, and sp0 is sp@ when the stack is empty.
: depth ( -- n )    sp@ sp0 swap - ;
: pick ( xn..x0 n -- xn..x0 xn )
                    dup depth 2 - u< 0= abort" pick: stack underflow"
                    1 + sp@ + @ ;
: roll ( xn..x0 n -- xn-1..x0 xn )
                    dup depth 2 - u< 0= abort" roll: stack underflow"
                    ?dup if swap >r 1- recurse r> swap then ;

\ Return stack operations

\ rdrop 2rdrop 2>r 2r> 2r@ are builtins: as colon definitions they would move their own return address

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

: begin ( -- a tag ) here MARK_BEGIN ; immediate compile-only

\ while leaves its forward reference below BEGIN's, untagged like if's, as ANS specifies:
\ so `begin .. while .. while .. repeat .. else .. then` works, with else/then resolving the extra while.
: while ( a tag -- orig a tag ) op:branch0 ,
                    here 0 , rot rot ; immediate compile-only   \ placeholder for the exit offset
: repeat ( orig a tag -- ) MARK_BEGIN ?pairs
                    op:branch , here - ,                 \ branch back to BEGIN
                    _patch-here ; immediate compile-only \ WHILE exits to here

: until             MARK_BEGIN ?pairs op:branch0 ,
                    here - ,           ; immediate compile-only

: again             MARK_BEGIN ?pairs op:branch ,
                    here - ,           ; immediate compile-only

: clear ( ... -- )  begin depth while drop repeat ;   \ empty the data stack

\ Counted loops. FOR, DO and ?DO compile an opcode whose operand is the offset to the loop exit;
\ NEXT, LOOP and +LOOP compile an opcode whose operand is the offset back to the loop body.
\ At run time each loop keeps three cells on the return stack: exit address, limit, index.

: (loop-start) ( opcode -- a tag )
                    , here MARK_DO 0 , ;              \ placeholder for the exit offset
: (loop-end) ( a tag opcode -- )
                    , MARK_DO ?pairs                    \ a: address of the exit-offset placeholder
                    dup 1+ here - ,                   \ offset back to the loop body
                    here over - swap ! ;              \ patch the exit offset

: for ( n -- )      op:for (loop-start) ; immediate compile-only   \ runs the body n times, with i = n .. 1
: next ( -- )       op:next (loop-end) ; immediate compile-only
: do ( limit start -- )  op:do (loop-start) ; immediate compile-only
: ?do ( limit start -- ) op:qdo (loop-start) ; immediate compile-only \ skips the loop if limit = start
: loop ( -- )       op:loop (loop-end) ; immediate compile-only
: +loop ( n -- )    op:ploop (loop-end) ; immediate compile-only
: leave ( -- )      op:leave , ; immediate compile-only    \ continue after the end of the innermost loop

\ Case statement (ANS): the key stays on the stack. OF tests it and drops it on a match;
\ the default clause sees the key on top, and ENDCASE drops it.

: case ( -- tag )   MARK_CASE ; immediate compile-only
: of ( -- orig tag ) ['] over , ['] = ,                 \ compile: over = if drop
                    op:branch0 , here 0 , ['] drop , MARK_OF ; immediate compile-only
: endof ( orig tag -- orig2 ) MARK_OF ?pairs
                    op:branch , here 0 ,                 \ jump past ENDCASE's drop, patched by endcase
                    swap _patch-here ; immediate compile-only   \ a failed test continues here
: endcase ( tag orig.. -- ) ['] drop ,                  \ the default path drops the key
                    begin dup MARK_CASE <> while _patch-here repeat drop ; immediate compile-only

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

: space ( -- )      BL emit ;

: spaces ( n -- )   dup 0> if for space next else drop then ;
: cr ( -- )         '\n' emit ;

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
: .pad              pad type ;                               \ Print the pad buffer
: ."  ( -- )        state @                                    \ Compile or print a string
                    if
                        op:strlit ,                               \ Compilation section
                        s" drop s-create ,
                        ['] type ,
                    else
                        s" drop type                           \ Execution (print) section
                    then ; immediate

\ Numeric print words: ANS pictured numeric output. <# starts a number, # converts one digit,
\ #s the rest, hold and sign add characters, and #> leaves ( c-addr u ) for tell.

: /mod              2dup mod rot rot / ;
: s>d ( n -- d )    dup 0< ;                            \ sign-extend to a double-cell number
: */mod ( n1 n2 n3 -- rem quot ) >r m* r> sm/rem ;     \ n1*n2/n3 with a double-cell intermediate
: */ ( n1 n2 n3 -- n4 ) */mod nip ;
: ud/mod ( ud u -- urem udquot ) >r 0 r@ um/mod r> swap >r um/mod r> ;

130 s-allot 130 + constant #end                     \ the pictured-output buffer, filled backwards
: <# ( -- )         #end hld ! ;
: hold ( c -- )     hld @ 1 - dup hld ! c! ;
: sign ( n -- )     0< if [char] - hold then ;
: # ( ud -- ud' )   base @ ud/mod rot dup 9 > if 7 + then [char] 0 + hold ;
: #s ( ud -- 0 0 )  begin # 2dup or 0= until ;
: #> ( ud -- c-addr u ) 2drop hld @ #end over - ;

: u. ( u -- )       0 <# #s #> tell space ;
: u.r ( u width -- ) >r 0 <# #s #> r> over - spaces tell ;
: .r ( n width -- ) >r dup abs 0 <# #s rot sign #> r> over - spaces tell ;   \ abs leaves the most
: . ( n -- )        0 .r space ;                                            \ negative number as its
                                                                            \ unsigned value: correct
: .s ( -- )         ." [ " depth for i 1- pick . next ." ] " flush ;   \ print the data stack
: .rs ( -- )        ." R: [ "                           \ print the return stack, below .rs's own return
                    rp@ 1+ rp0 over - for dup i + 1- @ . next drop ." ] " ;

: stop"  ( -- )     state @                                    \ Compile or print a string
                    if
                        op:strlit ,                               \ Compilation section
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
                    dp @ 1- @ dup 1+ dp !   \ resets HERE to the previous back pointer
                    @ 1+ dup context ! last !   \ resets CONTEXT and LAST
                    ;

: forget ( <name> )                             \ delete <name> and any words since
                    trace-off step-off          \ we're messing with the dictionary, so we don't want to run FIND
                    (') dup
                    if
                        1- dup dup dp ! @ s-here !            \ move to nfa and set HERE and S-HERE
                        1- @ 1+ dup context ! last !            \ go back a link and set CONTEXT and LAST
                    else
                        drop
                    then ;

\ : ?stack depth 0= if abort" Stack underflow" then ;

: source ( -- c-addr u ) 'tib @ 1+ #tib @ ;                    \ the input line; >in is an offset into it
: kkey ( -- c )     source drop >in @ + c@ 1 >in +! ;           \ Get the next character from the TIB

: ?key ( -- c T | F )                                           \ If there's a character in TIB, push it and TRUE
                    >in @ #tib @ < if kkey TRUE else FALSE then ; \ otherwise push FALSE

: strlen ( s -- n ) c@ ;                                        \ return the count byte from the string

\ Dictionary inspection

: immed? ( cfa -- f ) 1- @ IMMEDIATE_FLAG and 0<> ;              \ is the word at cfa immediate?
: ?unique ( s -- s ) dup find nip if ." redefining " dup type cr then ;

: .name ( cfa -- )  1- @ ADDRESS_MASK and type space ;          \ print the name of the word at cfa
: .operand ( a -- a+1 ) 1+ dup @ . ;
: see-cell ( a -- a' more? )                                    \ print the compiled cell at a
    dup @ case
        op:literal of .operand TRUE endof
        op:strlit  of 1+ '"' emit space dup @ type '"' emit space TRUE endof
        op:branch  of ." branch:" .operand TRUE endof
        op:branch0 of ." branch0:" .operand TRUE endof
        op:do      of ." do:" .operand TRUE endof
        op:qdo     of ." ?do:" .operand TRUE endof
        op:loop    of ." loop:" .operand TRUE endof
        op:ploop   of ." +loop:" .operand TRUE endof
        op:for     of ." for:" .operand TRUE endof
        op:next    of ." next:" .operand TRUE endof
        op:leave   of ." leave " TRUE endof
        op:abort   of ." abort " TRUE endof
        op:break   of ." exit " TRUE endof
        op:exec    of ." exec " TRUE endof
        op:does    of ." does> " TRUE endof
        op:exit    of ." ; " FALSE endof
        dup BUILTIN_FLAG and
        if dup ADDRESS_MASK and builtin-name type space else dup .name then
        TRUE swap                                               \ endcase drops the key
    endcase
    swap 1+ swap ;
: (see) ( cfa -- )                                              \ decompile the word at cfa
    dup @ BUILTIN_FLAG and if @ ADDRESS_MASK and ." Builtin: " builtin-doc type cr exit then
    dup @ op:constant = if ." Constant: " dup .name ." = " 1+ @ . cr exit then
    dup @ op:variable = if ." Variable: " dup .name ." = " 1+ @ . cr exit then
    dup @ op:created = if ." Created: " dup .name ." data at " dup >body .
                          1+ @ ?dup if ." does> code at " . then cr exit then
    dup @ op:definition <> if drop ." not a colon definition" cr exit then
    ." : " dup .name
    dup 1- @ IMMEDIATE_FLAG and >r
    1+ begin see-cell while repeat drop
    r> if ." immediate" then cr ;
: see ( <name> -- ) ' (see) ;

\ Conditional compilation. [else] skips input, across lines, to the matching [then] or [else];
\ it recognizes the three words by looking them up, so their case doesn't matter.
variable '[if]  variable '[else]
: [then] ( -- ) ; immediate
: [else] ( -- )     1 begin
                        begin text nip while            ( level ) \ the next token is in pad
                            pad find if
                                dup '[if] @ = if drop 1+ else
                                dup '[else] @ = if drop 1- dup if 1+ then else
                                ['] [then] = if 1- then then then
                            else drop then
                            ?dup 0= if exit then
                        repeat
                    refill 0= until drop ; immediate
: [if] ( flag -- )  0= if postpone [else] then ; immediate
' [if] '[if] !  ' [else] '[else] !

\ value / to, defer / is
: value ( x <name> -- ) constant ;                     \ a constant whose value TO can change
: to ( x <name> -- ) ' 1+ state @ if postpone literal postpone ! else ! then ; immediate
: (unset) ( -- )    true abort" deferred word not set" ;
: defer ( <name> -- )                                  \ a word that runs the xt stored by IS
                    (create) op:definition , op:literal , here 4 + , ['] @ , ['] execute , op:exit ,
                    ['] (unset) , (close) ;            \ the xt cell: 6 cells after the cfa
: is ( xt <name> -- ) ' 6 + state @ if postpone literal postpone ! else ! then ; immediate

: .( ( -- )         41 parse drop type ; immediate     \ print the text up to the next )
: environment? ( c-addr u -- false ) 2drop false ;
: unused ( -- u )   DICT_LIMIT here - ;                 \ cells left in the dictionary

include terminal.fs
include debug.fs
include editor.fs
include sequences.fs
