\ Control structure bug

: test-loop  ( -- n )
    0                       \ initialize counter
    begin
        dup 0 =              \ check if counter is zero
    while
        drop                \ drop counter
        5                   \ return test value
    repeat ;

see test-loop
20 dump-here

5 test-loop .s clear

: test-until begin 1 - dup dup 0 = until drop ;

see test-loop
40 dump-here

5 test-loop .s clear
