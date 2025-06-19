( Regression tests )
( Run with standard library loaded )
( test-none is for words leaving no words on the stack )
( test-single is for words leaving a single value on the stack )
( test-dual is for words leaving two values on the stack )
( failing tests leave the test number on the stack )
( test-results checks the stack to see if there were failures )

variable test-num 0 test-num !

( Test Functions: place desired result on the stack, )
( then push args to the test word, then the word, then test-single. )
( If the desired result is equal to the top of the stack, the test passes. )
( Relies on a variable test-num that indicates the number of the test. )

: test-none ( .. -- ) depth 1 test-num +!
    0= if test-num ? ."  Passed" else ."    Failed" test-num @ then cr ;

: test-single ( m n.. -- b ) 1 test-num +!
    = if test-num ? ."  Passed" else ."    Failed"  test-num @ then  cr ;

: test-dual ( j k n.. -- b ) 1 test-num +!
    rot = 
    rot rot = 
    and if test-num ? ."  Passed" else ."    Failed" test-num @ then cr ;

: test-results depth 0= if ." All tests passed!" else ." The following tests failed: " .s clear then ;

: loop-test for i next ;
: nested-loop-test 
    for 
        3 for 
            i . j .
            next i . 
        i 
    next ;


."         Clear has to be the first test" cr
1 2 3 4 5 clear test-none

."         Debugger"
\ 1 1 1 dbg test-single ." warnings and errors"
\ 1 1 2 dbg test-single ." info, warnings and errors"
\ 1 1 3 dbg test-single ." debug, info, warnings and errors"
1 1 0 dbg test-single ." quiet mode (errors only)"
1 1 4 dbg test-single ." invalid value 4"
1 1 -4 dbg test-single ." invalid value -4"
1 1 dbg-warning test-single
1 trace-on stepper @ test-single
0 trace-off stepper @ test-single
0 step-off stepper @ test-single

."         Printing" cr
1 1 23 . test-single
1 1 44 . flush test-single
\ 1 1 .s test-single
1 1 45 emit test-single

."                Loop tests" cr
7 21 7 loop-test + + + + +  test-dual
4 2 4 nested-loop-test - - test-dual
3 3 3 loop-test + test-dual

."         Arithmetic" cr
5 1 4 + test-single
-10 5 15 - test-single
-20 2 -10 * test-single
4 12 3 / test-single

."         Logic" cr
-1 TRUE test-single
0 FALSE test-single

."         Comparisons" cr
FALSE 1 3 > test-single
TRUE 3 1 > test-single
FALSE 3 3 > test-single
FALSE 5 2 < test-single
TRUE 2 5 < test-single
FALSE 2 2 < test-single
FALSE -5 0= test-single
TRUE 0 0= test-single
FALSE 0 0<> test-single
TRUE 5 0<> test-single
TRUE -22 0< test-single
FALSE 0 0< test-single
FALSE 55 0< test-single

."         Bitwise" cr
3 1 2 or test-single ." or"
2 0 2 or test-single
0 0 0 or test-single
3 -1 3 and test-single ." and"
0 0 0 and test-single
0 0 3 and test-single
45 -1 45 and test-single

."        Stack operations" cr
 1 1 100 drop test-single
5 5 6 drop test-single
5 6 5 nip test-single
1 1 2 3 4 5 6 depth drop drop drop drop drop drop test-single
17 17 17 dup test-dual
4 7 7 4 swap test-dual
5 12 5 12 over drop test-dual
9 4 6 9 4 rot drop test-dual
5 ?dup test-single
0 0 ?dup test-single
1 1 1 0 pick test-dual
1 1 0 roll test-single
3 1 3 2 1 0 2 roll drop drop test-dual

."        Variables" cr
5 variable x 5 x ! x @ test-single
42 variable y 40 y ! 2 y +! y @ test-single
42 variable z 42 z ! z ? z @ test-single

."        Constants" cr
12 12 constant months months test-single \ a constant with the value 12

."        Engine" cr
264 s" does-not-exist" drop ?unique test-single
264 s" *" drop ?unique test-single
264 s" min" drop ?unique test-single
: exit-test 22 33 exit 44 ;
22 33 exit-test test-dual

."        Raw mode tests "
0 raw-mode? test-single
raw-mode-on
-1 raw-mode? test-single
raw-mode-off
cr

."        Application tests" cr
1 0 fac test-single
1 1 fac test-single
6 3 fac test-single
479001600 12 fac test-single 
4181 19 fib test-single

."        Run tests "
: test-run 1 2 + ;
3 test-run test-single
3 run test-run test-single
\ 22 run nonexistent test-none \ This should abort and clear the stack

\ === Line Editor Tests ===
."        Line editor tests "
\ Note: These tests require interactive input
\ get-line test-single

test-results  \ Checks to see if all tests passed. Errors, if any, are left on the stack.

forget test-num
