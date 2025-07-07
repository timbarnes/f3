\ Extra definitions, code under development

\ String Dump capability
\ dump-stringspace
\

132 constant buf-len

\ Dump a string buffer from the beginning, for <count> characters
: dump-buffer ( s_addr -- )    \ Dump contents of a string input buffer
    ." Buffer at " dup 0 .r ." , length " dup c@ 0 .r ." : "
    buf-len incr-for
    for dup i - c@ emit next
    drop cr
    26 spaces
    c@ for
        '-' emit
    next cr
    ;

\ Move the address to the next counted string
: next-string ( s_addr -- s_addr )
    c@ + 1+
    ;

\ Dump a counted string, given a starting address
: dump-string ( s_addr -- )
    dup 5 .r ." /" dup c@ . ." : "
    dup type cr drop ;

\ Dump strings from a starting point, stopping when there are no more (count is zero)
: dump-strings ( s_addr -- )
    begin
        s-here @ over < if drop exit then           \ We've run out of strings
        dup dump-string
        dup next-string
    again
    ;

\ Print all string space
: dump-stringspace ( -- )
    ." TIB: " cr
    0 dump-buffer    \ TIB
    ." PAD " cr
    pad @ dump-buffer
    ." TMP " cr
    tmp @ dump-buffer
    tmp @ 132 + dump-strings
    ;
