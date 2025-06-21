\ Extra definitions, code under development

\ Memory Dump capability
\ dump-help, dump, dump-stringspace (under development)
\


132 constant buf-len

: dump-buffer ( s_addr -- )    \ Dump contents of a string input buffer
    buf-len incr-for
    for dup i - c@ emit next
    drop drop cr
    ;

: dump-strings ( s_addr -- )    \ Dump the counted strings with their addresses
    dup                         ( s_addr s_addr )
    begin
        c@                      ( s_addr c )
        dup                     ( s_addr c c )
        0=                      ( s_addr c bool )
        if                      ( s_addr c )
            ." Done " cr        ( s_addr c )
            drop                ( s_addr )
            exit                
        else
            2dup + 2 roll 
            type space      ( s_addr c)
            +                   ( s_addr c+s_addr )
        then
    again
    ;

: dump-stringspace ( s_addr -- s_addr ).  \ dump n cells from string space
    ." TIB: " cr
    0 dump-buffer    \ TIB
    ." PAD " cr
    pad @ dump-buffer
    ." TMP " cr
    tmp @ dump-buffer
    tmp @ 132 + dump-strings
    ;

