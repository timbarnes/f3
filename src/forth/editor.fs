\ Line Editor in Forth
\ Basic line input using key and key?

\ Constants

  1 constant ^A     \ Control-A - move to beginning of line
  2 constant ^B     \ Control-B - move backwards one char
  3 constant ^C     \ Control-C - ETC - exit the editor
  4 constant ^D     \ Control-D - delete forwards
  5 constant ^E     \ Control-E - move to end of line
  6 constant ^F     \ Control-F - move forward one char
  8 constant ^H     \ Control-H - delete backwards (same function as DEL)
 10 constant LF
 11 constant ^K     \ Control-K - kill to end of line
 13 constant CR
 14 constant ^N     \ Control-N - move forward in history
 16 constant ^P     \ Control-P - move backwards in history
 27 constant ESC    \ Escape key
127 constant DEL    \ Backspace key

\ Utility functions

\ Generic

: ascii-range ( -- )                      \ List printable ASCII characters with their numerical values.
    incr-for for
            dup i - dup 4 .r space emit 
            i 1 - 26 mod 0 = if cr 
        then
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

\ Input test words

: raw-key                               \ Get and echo a single key in raw mode. Primarily for testing
    raw-mode-on 
    key dup emit 
    raw-mode-off ;

\ Screen control words

: esc ESC (emit) ;                           \ Send escape character.

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

\ Input words

\ Process an escape, checking for a possible escape sequence from the terminal
\   Up-Arrow    becomes Control-P
\   Down-Arrow  becomes Control-N
\   Left-Arrow  becomes Control-B
\   Right-Arrow becomes Control-F
\   ESC standalone is passed through
: ed-escape-seq ( key -- key )
    drop                    \ we don't need the escape key itself
    50 ms
    key? if
        key dup [char] [ = if
            drop            \ drop the '['
            key             \ get the next character
            case
                [char] A of ^P endof      \ Up arrow    -> Control-P
                [char] B of ^N endof      \ Down arrow  -> Control-N
                [char] C of ^F endof      \ Right arrow -> Control-F
                [char] D of ^B endof      \ Left arrow  -> Control-B
                ( default ) dup    \ Unknown sequence, return as-is
            endcase
        else
            drop ESC       \ Not a '[' after ESC, treat as ESC
        then
    else
        ESC
    then
    ;

\ Get a character or a terminal control sequence and translate the sequence into a single character
\   *** This definition assumes raw mode is on ***
\   \r (Return) becomes \n (Newline)
\   Printable characters and control characters are passed through
: ed-get-key ( -- keyval )
    key dup
    case 
        ESC of ed-escape-seq endof      \ handle escape sequences
        CR  of drop LF       endof      \ replace CR with LF
    endcase
    ;

\ Issue the prompt
: ed-prompt
    ." led> " flush ;

\ Editing functions

\ Store a character and increment the character count
: ed-insert ( s_addr count char -- s_addr count+1 )
    dup emit flush          \ echo the character
    over 3 pick + c!        \ store it in the buffer
    1 +                     \ increment char counter
    ;

\ Process end of line
: ed-eol ( count char -- )
    ed-insert           \ save the newline 
    swap 1 - c!         \ store the character count
    raw-mode-off cr
    ;

\ Delete a character if one or more have been entered
: ed-del ( count char -- count )
    over 0 > if
        \ cursor back, emit a space, cursor back
        1 cursor-back
        BL emit
        1 cursor-back flush
        \ decrement character count
        drop 1 -
    else
        drop
    then ;

\ Move to the end of the line (Control-E)
: ed-line-end ( )
    \ Needs to know prompt length and count
    ;

\ Move to the beginning of the line (Control-A)
: ed-line-start ( -- )
    \ Move the cursor to the start of the line and reissue the prompt
    ;

\ Move forward one character if possible
: ed-forward
    \ Needs position and count
    ;
\ Move forward one character if possible
: ed-back
    \ Needs position and count
    ;

\ Delete the character to the right of the cursor, if possible
: ed-del-forward 
    \ Needs position and count 
    ;

\ Delete to end of line. Does nothing if already at EOL
: ed-del-to-eol
    \ Needs position and count
    ;

: get-line ( -- )
    raw-mode-on ( enable raw mode for direct key input )
    tmp @ 1 +             \ Store chars leaving beginning space for a count
    0 
    ed-prompt                           \ Editor prompt
    begin
        ed-get-key dup
        case
            ^C of                      \ Control-C aborts
                    raw-mode-off
                    drop drop drop
                    ."  Exited with Control-C "
                    cr exit
                endof
            LF  of ed-eol exit    endof  \ Linefeed ends line
            DEL of ed-del         endof  \ DEL deletes the last character
            ^A  of ed-line-start  endof  \ Move to the beginning of line
            ^B  of ed-forward     endof  \ Move one to the right
            ^D  of ed-del-forward endof  \ Delete to the right
            ^E  of ed-line-end    endof  \ Move to the end of the line
            ^K  of ed-del-to-eol  endof  \ Alternative delete forward
            \ default case stores the character and increments the counter
            ed-insert
        endcase
    again
; 