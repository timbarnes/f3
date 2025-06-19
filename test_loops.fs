\ Test loop constructs

: test-for-next
    5 for
        i .
    next ;

: test-begin-until
    0
    begin
        1 +
        dup .
        dup 5 >= 
    until
    drop ;

: test-begin-while-repeat
    0
    begin
        dup 5 < 
    while
        dup .
        1 +
    repeat
    drop ;

: test-all-loops
    cr ." Testing FOR-NEXT loop:" cr
    test-for-next
    cr ." Testing BEGIN-UNTIL loop:" cr
    test-begin-until
    cr ." Testing BEGIN-WHILE-REPEAT loop:" cr
    test-begin-while-repeat
    cr ." All loop tests completed." cr ;

test-all-loops 