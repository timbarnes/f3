: parse pad @ swap parse-to ;

: \ 1 parse drop drop ; immediate

: ( 41 parse drop drop ; immediate     \ Implements in-line comments

1 dbg \ set debuglevel to warnings and errors

\ here points to the slot where the new back pointer goes
\ last and context point to the previous word's name field address

: (close) ( -- )                      \ terminate a definition, writing a back pointer and updating context, last, and here
        last @ 1 - here @ !           \ write the new back pointer
        here @ 1 + here !             \ update HERE
        last @ context !              \ update CONTEXT
        ;

: constant ( n -- ) create 100002 , ,      \ v constant <name> creates a constant with value v
    (close) ;

: string ( n -- )
    create 100004 ,
    s-create dup 0 swap c! , (close) ;

\ Boolean constants. In fact any non-zero value is interpreted as true, but -1 is traditional.

0 constant FALSE
-1 constant TRUE

\ Constants referring to inner interpreter opcodes, which are typically compiled into definitions
\ These are used by dump to identify tokens in the dictionary / heap

100000 constant BUILTIN
100001 constant VARIABLE
100002 constant CONSTANT
100003 constant LITERAL
100004 constant STRLIT
100005 constant DEFINITION
100006 constant BRANCH
100007 constant BRANCH0
100008 constant ABORT
100009 constant EXIT
100010 constant BREAK
100011 constant EXEC
100012 constant ARRAY

200000 constant MARK_BEGIN
200001 constant MARK_WHILE
200002 constant MARK_FOR
200003 constant MARK_CASE
200004 constant MARK_OF

\ Flags and masks used to identify special words and extract addresses
72057594037927935 constant ADDRESS_MASK                      \ wipes any flags
1 61 lshift constant BUILTIN_FLAG
1 62 lshift constant IMMEDIATE_FLAG

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
    32 parse drop 1 + c@ LITERAL , , ; immediate

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

: [compile]         (') , ; immediate               \ Cause the following word to be compiled in, even if immediate
: exec EXEC , ; immediate

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
                    last @ 1 + , ; immediate        \ last points to the latest nfa, so increment

: nip ( a b -- b )  swap drop ;
: tuck ( a b -- b a b ) swap over ;

\ if - else - then

: _patch-here ( addr -- ) \ Address patcher for if - else - then
    here @ over - swap ! ;

: if     BRANCH0 , here @ 0 , ; immediate
: else   BRANCH , here @ 0 , swap _patch-here ; immediate
: then   _patch-here ; immediate

\ Finding the address of defined words

: ' (') dup @ dup DEFINITION = if drop else nip then ;

: [']               LITERAL , ' , ; immediate             \ compiles a word's cfa into a definition as a literal

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
: pop ( a -- )      drop ;
: 2dup ( a b -- a b a b ) over over ;
: 2drop ( a b -- )  drop drop ;
: ?dup              dup 0= if else dup then ;
: rdrop ( -- )      r> drop ;                           \ Pop a return address off the stack
: exit ( -- )       BREAK , ; immediate                 \ Pop out of the current definition and reset the Program Counter
: >                 swap < ;
: <=                1 + < ;
: <> ( n -- n )     = 0= ;
: 0>                0 > ;
: 0<>               0= 0= ;

\ \\\\\\\\\\\\\\\\\\\\\\\\\\\\\
\ Memory allocation
\
\ If n > 0, reserve n cells of data space. If n < 0, release |n| cells. If n is zero, no action is taken.
\            Allot updates `here`, but does not create a back pointer.
: allot ( n -- )
   dup 0= if
     drop exit
   then
   dup 0> if
     here @ + here !
   else
     here @ swap - here !
     then
   ;

\ Numeric operations

: min ( m n -- m | n ) 2dup < if drop else nip then ;
: max ( m n -- m | n ) 2dup > if drop else nip then ;
: abs ( n -- n | -n ) dup 0 < if -1 * then ;
: range ( n l h -- b ) \ Returns TRUE if n is in the range of l to h inclusive
    1 + swap 1 -
    2 pick < 2 roll 2 roll < and ;

\ Return stack operations in addition to the builtins: >r, r> and r@

: rdrop     r> drop ;
: 2rdrop    rdrop rdrop ;
: 2>r       >r >r ;
: 2r>       r> r> ;
: 2r@       r> r> 2dup 2>r ;

\ Control structures

: begin ( -- )      here @ MARK_BEGIN >c ; immediate

: while ( -- )      BRANCH0 ,
                    here @ MARK_WHILE >c
                    999 ,                ; immediate
: repeat ( -- )     c> drop                             \ pop while branch placeholder address
                    BRANCH ,                            \ unconditional branch to the beginning of the loop
                    here @ over - 1 + swap !            \ patch forward offset at WHILE's placeholder
                    c> drop                             \ pop begin address
                    here @ - ,           ; immediate    \ save negative offset to BEGIN                          \ emit negative offset

: for               here @ MARK_FOR >c
                    ['] >r ,             ; immediate

: next ( -- )       c>  drop                            \ get FOR addr
                    ['] r> ,                            \ compile saving the loop index
                    LITERAL , 1 ,
                    ['] - ,                             \ compile decrementing the index
                    ['] dup ,
                    ['] 0= ,                            \ compile dup and 0 test
                    BRANCH0 ,                           \ compile branch0 backwards
                    here @ - ,                          \ patch the backwards branch0
                    ['] drop ,           ; immediate    \ patch forward branch0

: until             c> drop BRANCH0 ,
                    here @ - ,           ; immediate

: again             c> drop BRANCH ,
                    here @ - ,           ; immediate

\ \\\ Under development

: do ( m n -- )     \ Start an iterative loop from m to n
   ['] 2>r ,        \ Put both values on the return stack
   here @ MARK_BEGIN >c
   ; immediate

: loop ( -- )       \ Close an iterative loop, incrementing and checking the loop variable
    ['] 2r@ ,       \ Get the loop values, leaving them on the stack
    ['] <           \ If in range...
    BRANCH0 , 8 ,   \ Jump forward, over the increment and branch code
    LITERAL , 1 ,   \ increment the loop variable
    ['] r> ,
    ['] + ,
    ['] >r
    c> drop
    BRANCH , here @ swap - ,
    ; immediate

\ Case statement

: case      ( val -- val )
    ['] >r ,                \ put the key on the return stack for later use
    here @ MARK_CASE >c     \ issues a marker to be retrieved by 'endcase'
    ;
    immediate

: of        ( -- )          \ the "if" part
    ['] r@ ,                \ compile getting the key from the return stack
    ['] = ,                 \ compile the test
    BRANCH0 , 999 ,         \ compile the branch. If the key doesn't match, we will skip
    here @ MARK_OF >c       \ push a marker
    ;
    immediate

: endof     ( -- )          \ the "then" part
    BRANCH ,            \ compile the branch
    here @ MARK_OF >c       \ push a second marker for this clause
    888 ,
    ;
    immediate

: endcase   ( -- )          \ patch the branches, exiting when the MARK_CASE is found
    begin
        c> MARK_CASE =          \ is the first marker a CASE marker?
        if
            ['] r> ,            \ pop the key off the return stack
            ['] drop ,          \ dump the key
            drop                \ the address
            exit                \ we've finished
        else                    \ it's an 'endof' marker, so patch jump to the end. patch_addr is on the stack
            dup dup             \ save extra copies of the address for the second patch operation
            here @ swap -
            swap
            !                   \ store the patch
            c>                  \ get the corresponding 'of' marker
            drop 1 -            \ we could test the type...
            dup rot swap - 1 + swap
            !                   \ store the delta between 'of' and 'endof'
        then
    again
    ;
    immediate

\ Takes a typical descending for - next loop, and simplifies reversing the direction of the loop variable
\     usage is : word incr-for for dup i - ... next .. ;
: incr-for ( m n -- m m+n n )
    over over + swap ;

\ system executes a command in the shell. The shell is exited afterwards.
\   This means that commands like 'cd' are not persistent.
\
: system" ( <command> ) tmp @ '"' parse-to drop (system) ;

: sec ( n -- )      1000 * ms ;  \ sleep for n seconds

: abort" STRLIT , s" .s drop s-create , ['] type , ['] abort , ; immediate \ abort with a message. Use inside another word.

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

: .r ( n width -- )
                    swap dup 0<
                    if
                        negate 1 swap rot 1-
                    else
                        0 swap rot
                    then
                    swap dup uwidth rot swap -
                    spaces swap
                    if '-' emit then
                    u. ;

: . 0 .r space ;

\ Variable utilities

: +! ( n addr -- )  dup @ rot + swap ! ;
: ?  ( addr -- )    @ . ;

\ Dictionary traversal functions
\   Takes an execution address on the stack, and execs it on every word, via its preceding back pointer
\   The word being executed must consume the address: it's signature is ( bp xt -- ).

\ : print-name ( bp -- )
\                     dup 1+ @ 13 ltype ;

\ : traverse-exec ( xt bp -- xt )
\                     over swap exec ;            \ pass bp to xt, keep xt

\ : traverse-next ( bp -- bp' )
\                     @ ;                         \ follow the link

\ : (traverse-words) ( xt bp -- xt )
\                     begin
\                         dup                    \ check bp != 0
\                     while
\                         trace-all
\                         \traverse-exec          \ call xt with bp
\                         \traverse-next          \ follow link
\                         trace-all
\                     repeat
\                     drop ;                     \ drop final null bp
\ trace-off
\ : traverse-words ( xt -- )
\                     here @ 1- @                 \ get initial bp (as in words)
\                     (traverse-words) ;

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

include src/forth/terminal.fs
include src/forth/debug.fs
include src/forth/editor.fs
include src/forth/sequences.fs
