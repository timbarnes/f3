\ A Hayes-style tester:  T{ code -> expected results }T
\ The stack left by `code` is compared with the expected results. A failure prints the
\ problem and the test line, and is counted in #errors. Based on John Hayes' tester.fr
\ (public domain); here using f3 words (variable + allot, 'tib).

variable #errors
variable start-depth
variable actual-depth
variable actual-results 31 allot                \ room for 32 results

: T{ ( -- )         depth start-depth ! ;

: -> ( ... -- )     depth start-depth @ - dup actual-depth !
                    0 ?do actual-results i + ! loop ;   \ top of stack goes to cell 0

: (fail) ( -- )     1 #errors +!  'tib @ type cr  clear ;

: }T ( ... -- )
    depth start-depth @ - actual-depth @ <>
    if ." WRONG NUMBER OF RESULTS: " (fail) exit then
    actual-depth @ 0 ?do
        actual-results i + @ <>
        if ." INCORRECT RESULT: " (fail) unloop exit then
    loop ;
