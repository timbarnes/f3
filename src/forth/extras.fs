\ Extra definitions, code under development

\ Replacement for `see', in Forth
\ With pretty-print

variable see-indent
variable see-ind-depth
4 see-ind-depth !               \ default indent is 4

: do-indent ( -- )
    see-indent @ spaces ;

\ Type-specific displayers

\ Print a variable from the address of its token
: see-var ( addr -- addr )
    ." variable "
    dup 1- @ type
    ." , value = "
    dup 1+ @ . cr
    ;

\ Print a constant from the address of its token
: see-const ( addr -- addr )
    ." constant "
    dup 1- @ type
    ." , value = "
    dup 1+ @ . cr
    ;

\ Display the values from the array, ten per line
: show-array ( addr -- addr )
    ." array-contents..."
    ;

\ Print an array from the address of its token
: see-array ( addr -- addr )
    ." array "
    dup 1- @ type
    ." , length = "
    dup dup 1+ @ . cr
    show-array cr
    ;

\ Walk through a definition, printing the contents
\   Definition name is on its own line
\   Subsequent words follow on a single line except for branches
\   Following odd branches we indent
\   Following even branches we outdent
\   This will be approximate only...
\
: see-def ( addr -- addr )
    ." : " dup 1- @ type cr
    see-ind-depth @ see-indent +!
    begin
        1+           \ increment the address
        dup @
        case
            EXIT of cr ." ;" cr exit endof
            LITERAL of 1+ @ . space endof
            BRANCH of 1+ @ ." branch:" . space endof
            BRANCH0 of 1+ @ ." branch0:" . space endof
            ." (other). "
        endcase
    again
    ;


\ Main word, takes a postfix word name
: see2 ( -- )
    (')                     \ Returns the address of the token, nfa is 1 previous
    0= if
        ." Not found" cr exit  \ The word was not found
        then
    1-                      \ Point to the nfa
    dup BUILTIN_FLAG and if
        ADDRESS_MASK and
        ." Builtin: " builtin-name type exit
    then
        case
            VARIABLE of see-var endof
            CONSTANT of see-const endof
            ARRAY of see-array endof
            DEFINITION of see-def endof
        endcase
    ;
