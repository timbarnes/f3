: immediate context @ cell - dup @ IMMEDIATE_FLAG or swap ! ;
: compile-only context @ cell - dup @ COMPILE_ONLY_FLAG or swap ! ;
: here dp @ ;
: pad 'pad @ ;
: cells cell * ;
: cell+ cell + ;
: cell- cell - ;
: \ 1 parse drop drop ; immediate
: ( 41 parse drop drop ; immediate     \ Implements in-line comments

\ The words above come first, because comments need them.
\ cell (from the engine) is the number of address units per cell; cells cell+ cell- use it.
\ here is the next free byte. Each word starts with a link field holding the previous word's link
\ address, then its name (a count byte and the characters, padded to a cell), code field and
\ parameters, as in fig-Forth, so , and allot append. context and last hold name field addresses.
\ immediate and compile-only set a flag in the link field of the most recently defined word.
\ The engine defines the opcodes (VARIABLE, LITERAL, BRANCH, ...), the flags (IMMEDIATE_FLAG, ...),
\ ADDRESS_MASK, and the empty-stack addresses sp0 and rp0 as constants (runtime.rs insert_constants).

1 dbg \ set debuglevel to warnings and errors

\ last points to the name field of the word being defined; context to the newest finished word's

: (close) ( -- )                      \ finish a word made with (create): make it findable
        last @ context !              \ update CONTEXT
        ;

\ create <name> makes a word that pushes the address of its data field, which , and allot fill.
\ does> gives the latest such word an action: the code after does> runs with that address.
\ f3's own defining words (constant, variable, ...) use the header-only (create) and (close).
: create ( <name> -- ) (create) op:created , 0 , (close) ;   \ cfa: op:created, then the does> code
: does> ( -- )      op:does , ; immediate compile-only
: >body ( xt -- a ) 2 cells + ;                              \ a created word's data field

\ Strings. Characters are bytes, in the same memory as cells. Counted strings have a count
\ byte, then the characters (at most 255). Names and string literals are counted strings,
\ compiled into the dictionary.

: aligned ( a -- a' ) cell 1 - + 0 cell - and ;        \ round up to the next cell boundary
: align ( -- )      here aligned dp ! ;
: count ( s -- c-addr u ) dup 1 + swap c@ ;                \ a counted string's characters and length
: name> ( nfa -- xt ) dup c@ 1 + + aligned ;               \ from a name field to its code field
: c, ( c -- )       here c! 1 allot ;                     \ compile a character
: s-allot ( n -- s ) here swap allot ;                    \ reserve n bytes in the dictionary
: s-copy ( from to -- ) over c@ 1 + cmove ;                \ copy a counted string
: place ( c-addr u s -- ) over over c! 1 + swap cmove ;    \ store characters as a counted string at s
: s, ( c-addr u -- ) dup 1 + s-allot place align ;         \ compile a counted string, then align
: s-save ( c-addr u -- s ) here >r s, r> ;                 \ ... and leave its address
: s-create ( from -- to ) count s-save ;                   \ copy a counted string into the dictionary

: constant ( n -- ) (create) op:constant , ,      \ v constant <name> creates a constant with value v
    (close) ;

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
    parse-name drop c@ ;

\ This version is for use inside a definition. It compiles the value into the definition as a literal.
: [char]
    parse-name drop c@ op:literal , , ; immediate compile-only

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

: word ( char -- s ) pad swap parse-to drop ;      \ the next char-delimited word, counted, in PAD

\ include <file> interprets a file (included takes the name as c-addr u)
: include ( <name> -- ) parse-name included ;

: [ FALSE state ! ; immediate                       \ Turns compile mode off
: ] TRUE state ! ;                                  \ Turns compile mode on

: recurse ( -- )                                    \ Simply compiles the cfa of the word being defined
                    last @ name> , ; immediate compile-only \ last points to the latest nfa

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
                    >in @ parse-name pad place >in !    \ copy the name, leaving it in the input for '
                    ' drop pad find                     \ ' reports an undefined name; find, immediacy
                    1 = if , else op:literal , , ['] , , then ; immediate compile-only
                                                        \ an immediate word is compiled as a call, others compile a ,

: abort" ( x -- )   op:branch0 , here 0 ,               \ if x is non-zero, abort with the message (ANS)
                    op:strlit , '"' parse s,
                    ['] (error) , _patch-here ; immediate compile-only

\ Movement within a word definition

: cfa>val           cell+ ;                               \ from cfa to first parameter cell

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
: char+ ( c -- c' ) 1+ ;                              \ characters are bytes, so chars does nothing
: chars ( n -- n ) ;

\ String literals. s" gives ( c-addr u ), c" a counted string; both compile a literal inside a
\ definition. Outside one, the string is copied to one of two buffers, TMP and tmp2, in turn,
\ so two strings can be in use at once (as Forth-2012 requires).
create tmp2 256 allot
variable 's-buf
: (s-buf) ( -- s )  's-buf @ 0= dup 's-buf ! if tmp @ else tmp2 then ;
: s" ( -- c-addr u ) '"' parse state @ if op:strlit , s, ['] count , else (s-buf) dup >r place r> count then ;
    immediate
: c" ( -- s )       '"' parse state @ if op:strlit , s, else (s-buf) dup >r place r> then ; immediate

: ?pairs ( tag expected -- )                          \ check that control structures match up
                    <> abort" unbalanced control structure" ;

\ Stack operations

: pop ( a -- )      drop ;
: 2dup ( a b -- a b a b ) over over ;
: 2drop ( a b -- )  drop drop ;
: ?dup              dup 0= if else dup then ;

\ The data stack lives in the heap and grows down: sp@ is the address of the top item,
\ the item n deep is at sp@ + n cells, and sp0 is sp@ when the stack is empty.
: depth ( -- n )    sp@ sp0 swap - cell / ;
: pick ( xn..x0 n -- xn..x0 xn )
                    dup depth 2 - u< 0= abort" pick: stack underflow"
                    1 + cells sp@ + @ ;
: roll ( xn..x0 n -- xn-1..x0 xn )
                    dup depth 2 - u< 0= abort" roll: stack underflow"
                    ?dup if swap >r 1- recurse r> swap then ;
: 2swap ( a b c d -- c d a b ) rot >r rot r> ;
: 2over ( a b c d -- a b c d a b ) 3 pick 3 pick ;
: 2! ( x1 x2 a -- ) swap over ! cell+ ! ;              \ x2 at a, x1 in the next cell
: 2@ ( a -- x1 x2 ) dup cell+ @ swap @ ;

\ Return stack operations

\ rdrop 2rdrop 2>r 2r> 2r@ are builtins: as colon definitions they would move their own return address

\ \\\\\\\\\\\\\\\\\\\\\\\\\\\\\
\ Memory allocation
\
\ allot ( n -- ) is a builtin: n > 0 reserves n cells of data space, n < 0 releases them.

\ Used to convert to specify cells in Forths that have byte or other addressing modes (e.g. 8 bit processors with 16-bit addresses)
\           Not required, but included for completeness.


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
                    dup cell+ here - ,                \ offset back to the loop body
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
: system" ( <command> ) '"' parse tmp @ place tmp @ (system) ;

: sec ( n -- )      1000 * ms ;  \ sleep for n seconds

\ Printing words

: space ( -- )      BL emit ;

: spaces ( n -- )   dup 0> if for space next else drop then ;
: cr ( -- )         '\n' emit ;

: type ( c-addr u -- )                          \ print u characters from c-addr (ANS)
                    swap ADDRESS_MASK and
                    swap 2dup + rot drop swap
                    for
                        dup i - c@ emit
                    next
                        drop ;

: ctype ( s -- )                                 \ print a counted string (f3: names and literals are counted)
                    ADDRESS_MASK and            \ Wipe out any flags
                    dup c@ swap 1+ swap         \ Get the length to drive the for loop
                    dup 0= if exit then
                    type ;

: rtell ( s l w -- )                            \ Right justify a string of length l in a field of w characters
                    over - 1 max
                    spaces type ;

: ltell ( s l w -- ) 2dup swap -
                    nip rot rot
                    type spaces ;

: rtype ( s w -- )  swap ADDRESS_MASK and dup c@
                    rot swap - spaces ctype ;

: ltype             swap ADDRESS_MASK and dup c@
                    rot swap - swap ctype spaces ;

: .tmp              tmp @ ctype ;                               \ Print the tmp buffer
: .pad              pad ctype ;                               \ Print the pad buffer
: ."  ( -- )        '"' parse state @                          \ Compile or print a string
                    if
                        op:strlit , s,                         \ Compilation section
                        ['] ctype ,
                    else
                        type                                   \ Execution (print) section
                    then ; immediate

\ Numeric print words: ANS pictured numeric output. <# starts a number, # converts one digit,
\ #s the rest, hold and sign add characters, and #> leaves ( c-addr u ) for type.

: /mod              2dup mod rot rot / ;
: s>d ( n -- d )    dup 0< ;                            \ sign-extend to a double-cell number
: */mod ( n1 n2 n3 -- rem quot ) >r m* r> sm/rem ;     \ n1*n2/n3 with a double-cell intermediate
: */ ( n1 n2 n3 -- n4 ) */mod nip ;
: ud/mod ( ud u -- urem udquot ) >r 0 r@ um/mod r> swap >r um/mod r> ;

create #buf 130 allot #buf 130 + constant #end                    \ the pictured-output buffer, filled backwards
: <# ( -- )         #end hld ! ;
: hold ( c -- )     hld @ 1 - dup hld ! c! ;
: sign ( n -- )     0< if [char] - hold then ;
: # ( ud -- ud' )   base @ ud/mod rot dup 9 > if 7 + then [char] 0 + hold ;
: #s ( ud -- 0 0 )  begin # 2dup or 0= until ;
: #> ( ud -- c-addr u ) 2drop hld @ #end over - ;

: u. ( u -- )       0 <# #s #> type space ;
: u.r ( u width -- ) >r 0 <# #s #> r> over - spaces type ;
: .r ( n width -- ) >r dup abs 0 <# #s rot sign #> r> over - spaces type ;   \ abs leaves the most
: . ( n -- )        0 .r space ;                                            \ negative number as its
                                                                            \ unsigned value: correct
: .s ( -- )         ." [ " depth for i 1- pick . next ." ] " flush ;   \ print the data stack
: .rs ( -- )        ." R: [ "                           \ print the return stack, below .rs's own return
                    rp@ cell+ rp0 over - cell / for dup i cells + cell- @ . next drop ." ] " ;

: stop"  ( -- )     '"' parse state @                          \ Compile or print a string
                    if
                        op:strlit , s,                         \ Compilation section
                        ['] ctype ,
                        ['] .s ,
                        ['] flush ,
                        ['] key , ['] drop ,
                    else
                        type                                   \ Execution (print) section
                        .s
                        ." Stopped: "
                        flush key drop
                    then ; immediate


\ Variable utilities

\ Headers: [link | flags][name: count byte, characters, padding][code field][parameters].
\ A name can't be found from the code field by stepping back (names vary in length), so >name
\ searches the dictionary, newest first, as some classic Forths do.
: >name ( xt -- nfa | 0 )                       \ the name field of the word with this xt, or 0
                    last @ cell-                \ the newest link field
                    begin dup while
                        2dup cell+ name> = if nip cell+ exit then
                        @ ADDRESS_MASK and
                    repeat nip ;
: >link ( xt -- lfa ) >name cell- ;             \ the link field of the word with this xt
: >flags ( xt -- flags ) >link @ ADDRESS_MASK invert and ; \ its IMMEDIATE_FLAG and COMPILE_ONLY_FLAG

: +! ( n addr -- )  dup @ rot + swap ! ;
: ?  ( addr -- )    @ . ;

: forget-last ( -- )                            \ delete the most recent definition
                    context @ cell- dup dp !    \ HERE goes back to its link field
                    @ ADDRESS_MASK and cell+ dup context ! last ! \ CONTEXT and LAST go back to the previous word
                    ;

: forget ( <name> )                             \ delete <name> and any words since
                    trace-off step-off          \ we're messing with the dictionary, so we don't want to run FIND
                    (') >link dup dp !                      \ HERE goes back to the word's link field
                    @ ADDRESS_MASK and cell+ dup context ! last ! ; \ CONTEXT and LAST to the previous word

\ : ?stack depth 0= if abort" Stack underflow" then ;

: source ( -- c-addr u ) 'tib @ #tib @ ;                      \ the input line; >in is an offset into it
: kkey ( -- c )     source drop >in @ + c@ 1 >in +! ;           \ Get the next character from the TIB

: ?key ( -- c T | F )                                           \ If there's a character in TIB, push it and TRUE
                    >in @ #tib @ < if kkey TRUE else FALSE then ; \ otherwise push FALSE

: strlen ( s -- n ) c@ ;                                        \ return the count byte from the string

\ Dictionary inspection

: immed? ( cfa -- f ) >flags IMMEDIATE_FLAG and 0<> ;               \ is the word at cfa immediate?
: ?unique ( s -- s ) dup find nip if ." redefining " dup ctype cr then ;

: .name ( cfa -- )  >name ?dup if ctype else ." ?" then space ;     \ print the name of the word at cfa
: .operand ( a -- a' ) cell+ dup @ . ;
: see-cell ( a -- a' more? )                                    \ print the compiled cell at a
    dup @ case
        op:literal of .operand TRUE endof
        op:strlit  of cell+ '"' emit space dup ctype '"' emit space    \ inline, padded to a cell
                      dup c@ 1+ aligned + cell- TRUE endof
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
        if dup ADDRESS_MASK and builtin-name ctype space else dup .name then
        TRUE swap                                               \ endcase drops the key
    endcase
    swap cell+ swap ;
: (see) ( cfa -- )                                              \ decompile the word at cfa
    dup @ BUILTIN_FLAG and if @ ADDRESS_MASK and ." Builtin: " builtin-doc ctype cr exit then
    dup @ op:constant = if ." Constant: " dup .name ." = " cell+ @ . cr exit then
    dup @ op:variable = if ." Variable: " dup .name ." = " cell+ @ . cr exit then
    dup @ op:created = if ." Created: " dup .name ." data at " dup >body .
                          cell+ @ ?dup if ." does> code at " . then cr exit then
    dup @ op:definition <> if drop ." not a colon definition" cr exit then
    ." : " dup .name
    dup immed? >r
    cell+ begin see-cell while repeat drop
    r> if ." immediate" then cr ;
: see ( <name> -- ) ' (see) ;

\ Conditional compilation. [else] skips input, across lines, to the matching [then] or [else];
\ it recognizes the three words by looking them up, so their case doesn't matter.
variable '[if]  variable '[else]
: [then] ( -- ) ; immediate
: [else] ( -- )     1 begin
                        begin bl word dup c@ while      ( level s ) \ the next word, counted
                            find if
                                dup '[if] @ = if drop 1+ else
                                dup '[else] @ = if drop 1- dup if 1+ then else
                                ['] [then] = if 1- then then then
                            else drop then
                            ?dup 0= if exit then
                        repeat drop
                    refill 0= until drop ; immediate
: [if] ( flag -- )  0= if postpone [else] then ; immediate
' [if] '[if] !  ' [else] '[else] !

\ value / to, defer / is
: value ( x <name> -- ) constant ;                     \ a constant whose value TO can change
: to ( x <name> -- ) ' cell+ state @ if postpone literal postpone ! else ! then ; immediate
: (unset) ( -- )    true abort" deferred word not set" ;
: defer ( <name> -- )                                  \ a word that runs the xt stored by IS
                    (create) op:definition , op:literal , here 4 cells + , ['] @ , ['] execute , op:exit ,
                    ['] (unset) , (close) ;            \ the xt cell: 6 cells after the cfa
: is ( xt <name> -- ) ' 6 cells + state @ if postpone literal postpone ! else ! then ; immediate

: .( ( -- )         ')' parse type ; immediate          \ print the text up to the next )
\ String words. Strings are ( c-addr u ): an address and a length.
: /string ( c-addr u n -- c-addr+n u-n ) tuck - >r + r> ;
: -trailing ( c-addr u -- c-addr u2 ) begin dup while 2dup + 1- c@ bl = while 1- repeat then ;
: fill ( c-addr u char -- ) swap 0 ?do 2dup swap i + c! loop 2drop ;
: erase ( c-addr u -- ) 0 fill ;
: blank ( c-addr u -- ) bl fill ;
: move ( from to u -- ) cmove ;                         \ cmove copies overlapping ranges correctly
: cmove> ( from to u -- ) cmove ;
: compare ( c-addr1 u1 c-addr2 u2 -- n )               \ -1, 0 or 1, comparing characters
    rot 2dup 2>r min 0 ?do
        over i + c@ over i + c@ - ?dup
        if nip nip 0< 2* 1+ unloop 2r> 2drop exit then
    loop 2drop
    2r> 2dup = if 2drop 0 else < if 1 else -1 then then ;
: search ( c-addr1 u1 c-addr2 u2 -- c-addr3 u3 flag )   \ find string 2 in string 1
    2>r 2dup
    begin dup r@ < 0= while
        over r@ 2r@ compare 0= if 2swap 2drop 2r> 2drop true exit then
        1 /string
    repeat
    2drop 2r> 2drop false ;
: bin ( fam -- fam ) ;                                  \ files are always binary

\ Number conversion: >number accumulates digits in base into a double-cell number
: d+ ( d1 d2 -- d ) rot + >r tuck + swap over u> r> swap - ;
: m+ ( d n -- d )   s>d d+ ;
: ud* ( ud u -- ud ) dup >r um* drop swap r> um* rot + ;
: digit? ( c -- n flag )                                \ the value of digit c, and whether it is one in base
    dup [char] 0 [char] 9 1+ within if [char] 0 - else
    dup [char] A [char] Z 1+ within if [char] A - 10 + else
    dup [char] a [char] z 1+ within if [char] a - 10 + else drop base @ then then then
    dup base @ u< ;
: >number ( ud1 c-addr1 u1 -- ud2 c-addr2 u2 )          \ stops at the first non-digit
    begin dup while
        over c@ digit? 0= if drop exit then
        >r 2swap base @ ud* r> m+ 2swap 1 /string
    repeat ;
: environment? ( c-addr u -- false ) 2drop false ;
: unused ( -- u )   DICT_LIMIT here - ;                 \ cells left in the dictionary

include terminal.fs
include debug.fs
include editor.fs
include sequences.fs
