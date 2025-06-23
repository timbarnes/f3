\ Line Editor in Forth
\ Basic line input using key and key?

\ Constants

 3 constant ETX     \ End of Text (emitted by Control-C)
10 constant LF
13 constant CR
27 constant ESCAPE  \ Escape key

\ Utility functions

\ Generic

: ascii-range ( -- )                      \ List printable ASCII characters with their numerical values.
    incr-for for
        dup i - dup 4 .r space emit i 1 - 26 mod 0 = if cr then
    next
    drop drop cr
    ;

: ascii-table
    32 100 ascii-range ;

: ascii-letters
    65 26 ascii-range 
    97 26 ascii-range
    ;
    
: ascii-numbers 
    48 10 ascii-range ;

: ascii-symbols 
    32 16 ascii-range
    91  5 ascii-range 
    123 4 ascii-range ;

\ Output

: esc ESCAPE (emit) ;                           \ Send escape character.

: screen-clear 
    \ ascii-preamble 50 emit 74 emit ;
    esc ." [2J" ;

: line-clear
    esc ." [2K" ;

: cursor-home 
    esc ." [HD" ;

: screen-home
    screen-clear
    cursor-home ;

: cursor-up ( n -- )
    esc ." [" dup uwidth .r ." A" ;

: cursor-down ( n -- )
    esc ." [" dup uwidth .r ." B" ;

: cursor-forward ( n -- )
    esc ." [" dup uwidth .r ." C" ;

: cursor-back ( n -- )
    esc ." [" dup uwidth .r ." D" ;

: save-cursor
    esc ." [s" ;

: restore-cursor
    esc ." [u" ;

\ Input utilities

: raw-key                               \ Get and echo a single key in raw mode. Primarily for testing
    raw-mode-on 
    key dup emit 
    raw-mode-off ;

: raw-unit                              \ Get a key or an escape sequence in raw mode
    raw-mode-on
    key dup                             \ wait for a key
    ETX = if
        raw-mode-off
        ." Control-C pressed, exiting. " cr
    else
        dup emit
    then
    raw-mode-off
    ;

: raw-move-test
    raw-mode-on
    50 cursor-forward
    64 emit
    1000 ms 
    20 cursor-back 
    raw-mode-off
    ;

\ Line editor
\
\ Approach: characters are accumulated in TMP (for now). 
\       Prompt is printed and cursor position saved.
\       Edit line offset is saved (length of prompt).
\       Wait for characters, accumulating in TMP if printable.
\       Control-C exits from the line editor, resetting raw-mode
\           and returning to the normal editing mode.
\       Escape sequences are accumulated using a 10ms delay to
\           determine if it's a sequence, or just an escape key.
\       Escape sequences are interpreted in the buffer and reflected
\           on the screen.

\ get-line ( -- ) reads characters from key until newline, stores in TMP
: get-line ( -- )
    raw-mode-on ( enable raw mode for direct key input )
    tmp @ 1 +             \ Store chars leaving beginning space for a count
    0 
    begin
        key
        dup ETX = if 
            raw-mode-off 
            drop drop drop 
            ." Control-C" exit 
        then
        dup dup LF = 
        swap CR = 
        or 
        if
            drop swap c!    \ store the character
            raw-mode-off
            ." Finished with line "
            exit
        else
            ( addr count char -- addr count char )
            dup emit flush
            over 3 pick c!
            1 +             \ increment char counter
        then
        .s flush
    again
    ." End of function "
; 