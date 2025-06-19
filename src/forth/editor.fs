\ Line Editor in Forth
\ Basic line input using key and key?

\ get-line ( -- ) reads characters from key until newline, stores in TMP
: get-line ( -- )
    raw-mode-on ( enable raw mode for direct key input )
    tmp @ ( addr )
    0 ( addr count )
    begin
        key ( addr count -- addr count char )
        dup 10 = ( addr count char -- addr count char flag )
        over 13 = ( addr count char flag -- addr count char flag flag )
        or ( addr count char flag flag -- addr count char flag )
        if
            drop ( addr count char -- addr count )
            emit ( addr count -- addr count )
        else
            ( addr count char -- addr count char )
            dup emit ( addr count char -- addr count char ) ( echo the character )
            over ( addr count char -- addr count char addr )
            + ( addr count char addr -- addr count char+addr )
            c! ( addr count char+addr -- addr count )
            1+ ( addr count -- addr count+1 )
        then
        ( addr count -- addr count )
        dup 10 = ( addr count -- addr count flag )
        over 13 = ( addr count flag -- addr count flag flag )
        or ( addr count flag flag -- addr count flag )
    until
    drop ( addr count -- addr count )
    swap ( count addr )
    over c!  ( store count as first byte )
    raw-mode-off ( disable raw mode )
; 