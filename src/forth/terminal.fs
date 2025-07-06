\ \\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\
\ Terminal Control
\
\ These words provide for manipulation of the terminal.
\   They depend on crossterm in the Rust binary for raw control,
\   and the raw-mode-on and raw-mode-off builtins.
\
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

\ Color support

\ Emit CSI prefix (Control Sequence Introducer)
: csi ( -- ) esc ." [" ;

\ Generic SGR (Select Graphic Rendition) sequence
: sgr ( n -- ) csi 0 .r ." m" ;

\ Reset all attributes
: t-reset   ( -- ) 0 sgr ;

\ -------- Text styles --------
: bold      ( -- ) 1 sgr ;
: dim       ( -- ) 2 sgr ;
: italic    ( -- ) 3 sgr ;
: underline ( -- ) 4 sgr ;
: inverse   ( -- ) 7 sgr ;
: strike    ( -- ) 9 sgr ;

\ -------- Foreground colors --------
: black     ( -- ) 30 sgr ;
: red       ( -- ) 31 sgr ;
: green     ( -- ) 32 sgr ;
: yellow    ( -- ) 33 sgr ;
: blue      ( -- ) 34 sgr ;
: magenta   ( -- ) 35 sgr ;
: cyan      ( -- ) 36 sgr ;
: white     ( -- ) 37 sgr ;

\ -------- Bright foregrounds --------
: br-black   ( -- ) 90 sgr ;
: br-red     ( -- ) 91 sgr ;
: br-green   ( -- ) 92 sgr ;
: br-yellow  ( -- ) 93 sgr ;
: br-blue    ( -- ) 94 sgr ;
: br-magenta ( -- ) 95 sgr ;
: br-cyan    ( -- ) 96 sgr ;
: br-white   ( -- ) 97 sgr ;

\ -------- Background colors --------
: bg-black   ( -- ) 40 sgr ;
: bg-red     ( -- ) 41 sgr ;
: bg-green   ( -- ) 42 sgr ;
: bg-yellow  ( -- ) 43 sgr ;
: bg-blue    ( -- ) 44 sgr ;
: bg-magenta ( -- ) 45 sgr ;
: bg-cyan    ( -- ) 46 sgr ;
: bg-white   ( -- ) 47 sgr ;

\ -------- Bright backgrounds --------
: bg-br-black   ( -- ) 100 sgr ;
: bg-br-red     ( -- ) 101 sgr ;
: bg-br-green   ( -- ) 102 sgr ;
: bg-br-yellow  ( -- ) 103 sgr ;
: bg-br-blue    ( -- ) 104 sgr ;
: bg-br-magenta ( -- ) 105 sgr ;
: bg-br-cyan    ( -- ) 106 sgr ;
: bg-br-white   ( -- ) 107 sgr ;

\ -------- 256-color support --------
: fg256 ( n -- )  \ 0–255
  csi ." 38;5;" . ." m" ;

: bg256 ( n -- )  \ 0–255
  csi ." 48;5;" . ." m" ;

\ -------- True color (RGB) --------
: fg-rgb ( r g b -- )
  csi ." 38;2;" 0 .r ." ;" 0 .r ." ;" 0 .r ." m" ;

: bg-rgb ( r g b -- )
  csi ." 48;2;" 0 .r ." ;" 0 .r ." ;" 0 .r ." m" ;