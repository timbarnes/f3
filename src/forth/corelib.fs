\ Core library for Forth
: parse pad @ swap parse-to ;                       
: \ 1 parse drop drop ; immediate                  
: ( 41 parse drop drop ; immediate                  \ Implements in-line comments
: [char] BL parse drop drop pad @ 1 + c@ ;          \ Place the first char of the next token on the stack

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

0 constant FALSE
-1 constant TRUE

\ Constants referring to inner interpreter opcodes, which are typically compiled into definitions
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

200000 constant MARK_BEGIN
200001 constant MARK_WHILE
200002 constant MARK_FOR
200003 constant MARK_CASE
200004 constant MARK_OF

72057594037927935 constant ADDRESS_MASK                      \ wipes any flags
2305843009213693952 constant BUILTIN_FLAG
4611686018427387904 constant IMMEDIATE_FLAG

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

 : variable ( -- ) create VARIABLE , 0 ,    \ variable <name> creates a variable, initialized to zero
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

( File reader functions )
: included          tmp @ include-file ; \ include-file uses a string pointer on the stack to load a file
: include           tmp @ 32 parse-to included ;

: [ FALSE state ! ; immediate                       \ Turns compile mode off
: ] TRUE state ! ;                                  \ Turns compile mode on

: recurse ( -- )                                    \ Simply compiles the cfa of the word being defined
                    last @ 1 + , ; immediate        \ last points to the latest nfa, so increment

: nip ( a b -- b )  swap drop ;
: tuck ( a b -- b a b ) swap over ;

: _patch-here ( addr -- )
    here @ over - swap ! ;

: if     BRANCH0 , here @ 0 , ; immediate
: else   BRANCH , here @ 0 , swap _patch-here ; immediate
: then   _patch-here ; immediate

: ' (') dup @ dup DEFINITION = if drop else nip then ;

: [']               LITERAL , ' , ; immediate                    \ compiles a word's cfa into a definition as a literal
: cfa>nfa           1 - ;                                        \ converts an cfa to an nfa
: nfa>cfa           1 + ;                                        \ converts an nfa to a cfa
: bp>nfa            1 + ;                                        \ from preceding back pointer to nfa
: bp>cfa            2 + ; 

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
: <> ( n -- n )     = 0= ;
: 0>                0 > ;
: 0<>               0= 0= ;
: min ( m n -- m | n ) 2dup < if drop else nip then ;
: max ( m n -- m | n ) 2dup > if drop else nip then ;
: abs ( n -- n | -n ) dup 0 < if -1 * then ;


: begin ( -- )      here @ MARK_BEGIN >c ; immediate
: while ( -- )      BRANCH0 ,
                    here @ MARK_WHILE >c
                    999 ,                ; immediate
: repeat ( -- )     c> drop                             \ pop while branch placeholder address
                    here @ over - swap !                \ patch forward offset at WHILE's placeholder
                    BRANCH ,                            \ unconditional branch to the beginning of the loop 
                    c> drop                             \ pop begin address
                    here @ swap -                       \ compute negative offset to BEGIN
                    ,                                   \ emit negative offset
                ; immediate

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

: case              here @ MARK_CASE >c  ; immediate
: of                
                    ['] over , ['] = ,  ( )
                    [compile] if
                    here @ MARK_OF >c
                    ['] drop ,           ; immediate
: endof             [compile] else
                    here @ MARK_OF >c drop   ; immediate
: endcase .s
    ['] drop ,
    begin
        c>                          ( addr tag )
        dup MARK_OF                 ( addr tag tag MARK_OF )
        =                           ( addr tag bool )
        if                          ( addr tag )
            drop dup                ( addr addr )
            here @                  ( addr addr HERE )
            - swap !                ( )
        else
            dup MARK_CASE           ( addr tag MARK_CASE )
            =                       ( addr bool )
            if                      ( addr )
                2drop               ( )
                exit
            else
                abort               ( )
            then
        then
    again
; immediate

\ Takes a typical descending for - next loop, and simplifies reversing the direction of the loop variable
\     usage is : word incr-for for dup i - ... next .. ;

: incr-for ( m n -- m m+n n )
    over over + swap ;

\ \ case: mark start of case, push control marker
\ : case
\     here @ MARK_CASE >c
\ ; immediate

\ \ of: compile "over =", then compile BRANCH0 with placeholder,
\ \ push address of placeholder on control stack as MARK_OF,
\ \ compile drop to remove compared value if match succeeds.
\ : of
\     ['] over ,       \ copy case value for compare
\     ['] = ,          \ compare with input
\     BRANCH0 , 0 ,    \ compile branch0 with placeholder
\     here @ MARK_OF >c
\     ['] drop ,       \ drop compared value if true
\ ; immediate

\ \ endof: compile unconditional BRANCH with placeholder,
\ \ push placeholder address on control stack as MARK_OF.
\ : endof
\     BRANCH , 0 ,
\     here @ MARK_OF >c
\ ; immediate

\ \ endcase: patch all MARK_OF placeholders to jump to here,
\ \ pop MARK_CASE marker and exit.
\ : endcase
\     begin
\         c>                     \ get addr tag pair from control stack
\         dup MARK_OF = if
\             \ patch MARK_OF branch
\             here @ swap - swap !
\             drop                \ drop tag after patching
\         else
\             dup MARK_CASE = if
\                 drop drop        \ drop addr and tag
\                 abort             \ exit the loop cleanly
\             else
\                 \ unknown marker, error or break infinite loop safely
\                 \." ERROR: Unknown control marker in endcase" cr
\                 abort
\             then
\         then
\     again
\ ; immediate

: system" ( <command> ) tmp @ '"' parse-to drop (system) ;
: sec ( n -- )      1000 * ms ;  \ sleep for n seconds

: abort" STRLIT , s" .s drop s-create , ['] type , ['] abort , ; immediate \ abort with a message. Use inside another word.

: emit ( c -- )     \ print a character if in the printable range
                    128 mod dup 31 > if (emit) else drop then ;

: space ( -- )      BL emit ;
: test ( n -- ) 10 for drop next 123 ;
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

\ mumeric functions

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

: +! ( n addr -- )  dup @ rot + swap ! ;
: ?  ( addr -- )    @ . ;


\ Implementation of word

variable word-counter

: .word ( bp -- bp )                            \ prints a word name, given the preceding back pointer
                    dup dup 1+ 4 u.r space 1+ @ 13 ltype 
                    1 word-counter +! 
                    word-counter @ 8 mod
                    if space else cr then @ ;   

: words ( -- )
                    0 word-counter !
                    here @ 1- @                                 \ Get the starting point: the top back pointer
                    begin                                       \ loops through the words in the dictionary
                        .word dup not                           \ print a word and test the next pointer
                    until 
                        drop ;   

: print-word ( xt -- ) 
                    1- @ 13 ltype ;                 \ print a word name, given the xt as produced by '

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

: forget-last ( -- )                            \ delete the most recent definition
                    here @ 1- @ dup 1+ here !                   \ resets HERE to the previous back pointer
                    @ 1+ dup context ! last !                   \ resets CONTEXT and LAST
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
                                                
( Application functions )

\ : run-tests  s" src/forth/regression.fs" included ; \ Run the regression tests


\ Returns TRUE if n is in the range of l to h inclusive
\
: range ( n l h -- b )
    1 + swap 1 - 
    2 pick < 2 roll 2 roll < and ;

: dump-help 
    ." dump ( addr cells -- addr ) dumps heap data. Opcode reference:" cr
    ." 100000=BUILTIN  100001=VARIABLE    100002=CONSTANT  100003=LITERAL" cr
    ." 100004=STRLIT   100005=DEFINITION  100006=BRANCH    100007=BRANCH0" cr
    ." 100008=ABORT    100009=EXIT        100010=BREAK     100012=EXEC" cr
    ." ADDRESS --------------HEX ------------DECIMAL CHAR STRING " cr 
    ;

: dump-addr  ( addr -- addr ) dup 5 .r ;

: dump-hex   ( val -- val ) dup 20 hex .r decimal ;

: dump-dec   ( val -- val ) dup 20 .r ;

: dump-char  ( val -- val ) 
    dup dup
    space ''' emit 
    emit 
    ''' emit 
    space space 
    128 mod dup 31 > not if space then drop ;

: dump-segment
        '|' emit 
        dup 20 + 
        20 for dup i - c@ emit
           next
        '|' emit drop ;

: is-token-range 100000 100012 range ;  \ determines if the value is likely to be a token

: is-string-range 0 5000 range ;        \ determines if it could be a string address

: dump-builtin 
    dup 132 3 * 1- > 
    if
        ." nfa -> " 
    else
        ." buffer -> "
    then
    dup 18 ltype
    ;

: dump-token 
    dup 100000 = if ." BUILTIN              " exit then
    dup 100001 = if ." VARIABLE             " exit then
    dup 100002 = if ." CONSTANT             " exit then
    dup 100003 = if ." LITERAL              " exit then
    dup 100004 = if ." STRLIT               " exit then
    dup 100005 = if ." DEFINITION           " exit then
    dup 100006 = if ." BRANCH               " exit then
    dup 100007 = if ." BRANCH0              " exit then
    dup 100008 = if ." ABORT                " exit then
    dup 100009 = if ." EXIT                 " exit then
    dup 100010 = if ." BREAK                " exit then
    dup 100011 = if ." EXEC.                " exit then
    ." *UNKNOWN* " drop ;   
 
: dump-string ( s_addr -- s_addr )      \ print 20 characters from s_addr
    dup dup ADDRESS_MASK and = not      \ It's a builtin
    if
        dup ." BUILTIN # " ADDRESS_MASK and . exit
    then
    dup is-token-range                  ( s_addr mod_addr bool )
    if
        dump-token 
    else
        dup is-string-range 
        if 
            dup c@ 31 > 
            if 
                dump-segment
            else
                dump-builtin 
            then
        else
            ." -No string-" drop
        then
    then
    ;

: dump-name ( s_addr -- )   \ Print a builtin or definition name, if appropriate
    \ If it has a builtin flag or precedes a DEFINITION tag, show the name
    dup BUILTIN_FLAG and            \ check to see if there's a builtin flag
    if 
        dup 1 - 15 ltype
    then ;

: dump       ( addr cells -- addr )
    ." ADDRESS --------------HEX ------------DECIMAL CHAR STRING " cr
    incr-for for dup i - 
        dump-addr @ 
        dump-hex
        dump-dec
        dump-char
        dump-string
        \ dump-name
        cr drop 
    next 
    drop drop ;

cr ." Library loaded." cr

\ run ( -- ) executes a word by name from the input buffer, aborting if not found
: run ( -- )
    tmp @ 32 parse-to ( -- b u )
    if ( b u )
        find ( b u -- cfa T | b F )
        if ( cfa )
            execute
        else
            drop
            \ abort" word not found"
        then
    else
        drop
        \ abort" no word to run"
    then
;

