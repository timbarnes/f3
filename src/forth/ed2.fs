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
    esc ." [2J" flush ;

: line-clear
    esc ." [2K" flush ;

: cursor-home 
    esc ." [HD" flush ;

: screen-home
    screen-clear
    cursor-home ;

: cursor-up ( n -- )
    esc ." [" 0 .r ." A"  flush ;

: cursor-down ( n -- )
    esc ." [" 0 .r ." B" flush ;

: cursor-forward ( n -- )
    esc ." [" 0 .r ." C" flush ;

: cursor-back ( n -- )
    esc ." [" 0 .r ." D" flush ;

: save-cursor
    esc ." [s" flush ;

: restore-cursor
    esc ." [u" flush ;

: insert-char
    esc ." [" 0 .r ." @" ;

: delete-char
    esc ." [" 0 .r ." P" ;

: erase-chars
    esc ." [" 0 .r ." K" ;

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

\ Variables and memory management
\       The line we read in goes in the TMP buffer
\       We keep its starting address on the stack
\       The current character is managed on the stack
\       Character and cursor counts are kept in variables
\       Forth automatically initializes these to 0.

variable ed-char-count        \ the number of characters in the buffer
variable ed-cursor-position   \ location for edit and move functions

: delta-to-end
    ed-char-count @ ed-cursor-position @ - ;

\ Tests for cursor at beginning of line
: ed-buffer-start? ( -- )
    ed-cursor-position @ 0= ;

\ Tests for cursor at end of line
: ed-buffer-end? ( -- )
    ed-cursor-position @
    ed-char-count @
    = ;

\ \\\\\\\\\\\\\\\\\\\\\\\\\
\ Editor utilities

\ Initialize editor variables, get buffer address, and issue prompt
: ed-init 
    0 ed-char-count !
    0 ed-cursor-position !
    tmp @ 1 +
    ;

\ \\\\\\\\\\\\\\\\\\\\\\\\\
\ Display utilities

\ Issue the prompt
: ed-prompt
    ." led> " flush 
    ;

\ Redraw the characters in the buffer at the current cursor position
\ : ed-draw-buffer ( s_addr -- s_addr )
\     ed-char-count @
\     dup 0 > if 
\         \ cr ." ed-draw-buffer " cr
\         incr-for for dup i - c@ emit next drop
\     then
\     ;

\ Repaint the line
\ : ed-repaint    ( s_addr -- s_addr )
\     cr ." Repainting " cr
\     line-clear
\     CR (emit)
\     ed-prompt
\     ed-draw-buffer
\     ed-char-count @ ed-cursor-position @ -
\     dup 0 > if cursor-back else drop then
\     ;

\ \\\\\\\\\\\\\\\\\\\\\\\\\\
\ Buffer utilities

\ Copy a single character from one location to another in string space
: char-copy ( dest source -- )
    c@ swap c!
    ;

\ shift a character left by 1
: char-left ( s_addr -- )
    dup 1- swap
    char-copy ;

: chars-left ( s_addr -- s_addr )
    \ start at the beginning, move forwards to avoid overwrites
    delta-to-end               \ the count
    1 > if
        dup ed-char-count @ +
        over ed-cursor-position @ + 1+
        begin 
            dup char-left
            1+
            2dup =
        until
        drop drop
    then
    ;

\ shift a character right by 1
: char-right ( s_addr -- )
    dup 1+ swap
    char-copy ;

: chars-right ( s_addr -- s_addr )
    \ start at the end, move backwards to avoid overwrites
    delta-to-end             \ the count
    dup 0 > if
        over ed-cursor-position @ +   \ the starting address in the buffer
        swap 
        for
            dup i + 1- char-right
        next
        drop
    else
        drop
    then
    ;

\ \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\
\ Input utilities

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

\ \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\
\ Editing functions - these are the operations directly bound to keys

\ Store a character and increment the character count
: ed-insert ( s_addr char -- s_addr )
    ed-buffer-end? not if
        1 insert-char         \ shift the characters to the right of the cursor
        swap chars-right swap \ we have to move the char out of the way
    then
    dup emit flush
    over
    ed-cursor-position @ +
    c!
    1 ed-char-count +!
    1 ed-cursor-position +!
    ;

\ Process end of line
: ed-eol ( s_addr char -- s_addr )
    ed-char-count @ ed-cursor-position !  \ move to the end of the line
    ed-insert                   \ save the newline 
    ed-char-count @
    swap 1 - c!                 \ store the character count
    raw-mode-off cr
    ;

\ Delete a character if one or more have been entered
\    *** Currently ignores ed-cursor-position ***
: ed-del ( s_addr char -- s_addr )
    drop
    ed-char-count @ 0= if exit then
    -1 ed-cursor-position +!    
    chars-left
    -1 ed-char-count +!
    1 cursor-back
    1 delete-char flush
    ;

\ Move to the end of the line (Control-E)
: ed-line-end ( char -- )
    drop
    ed-char-count @
    ed-cursor-position @ -
    cursor-forward 
    ed-char-count @ ed-cursor-position !
    ;

\ Move to the beginning of the line (Control-A)
: ed-line-start ( char -- )
    drop
    ed-cursor-position @
    cursor-back 
    0 ed-cursor-position !
    ;

\ Move forward one character if possible
: ed-forward
    drop
    ed-char-count @
    ed-cursor-position @ -
    0 > if
        1 cursor-forward
        1 ed-cursor-position +!
    then
    ;
\ Move back one character if possible
: ed-back
    drop
    ed-cursor-position @
    0 > if
        1 cursor-back
        -1 ed-cursor-position +!
    then
    ;

\ Delete the character to the right of the cursor, if possible
: ed-del-forward    ( s_addr char -- s_addr )
    drop
    ed-buffer-end? if exit then
    chars-left
    1 delete-char flush
    -1 ed-char-count +!
    ;

\ ^K: Delete to end of line. Does nothing if already at EOL
: ed-del-to-eol ( char -- )
    drop
    ed-cursor-position @
    ed-char-count !         \ set the char count to the current cursor position
    delta-to-end erase-chars flush
    ;

: get-line ( -- )
    raw-mode-on ( enable raw mode for direct key input )
    ed-init
    ed-prompt
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
            ^B  of ed-back        endof  \ Move one to the right
            ^D  of ed-del-forward endof  \ Delete to the right
            ^E  of ed-line-end    endof  \ Move to the end of the line
            ^F  of ed-forward     endof  \ Move forward one character
            ^K  of ed-del-to-eol  endof  \ Delete to end of line
            \ default case stores the character and increments the counter
            ed-insert
        endcase
    again
; 
