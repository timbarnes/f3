\ Test immediate words

: test-immediate immediate ;

: test-compile-mode
    [ 1 2 + ] literal . ;

: test-all-immediate
    cr ." Testing immediate words:" cr
    test-compile-mode
    cr ." Immediate test completed." cr ;

test-all-immediate 