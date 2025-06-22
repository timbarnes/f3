\ Case statement

: case      ( val -- val )
    ['] >r ,                \ put the key on the return stack for later use
    here @ MARK_CASE >c 
    ;
    immediate

: of        ( -- )          \ the "if" part
    ['] r@ ,                \ get the key from the return stack
    ['] = ,                 \ compile the test
    BRANCH0 , 999 ,         \ compile the branch. If the key doesn't match, we will skip
    here @ MARK_OF >c       \ push a marker
    ; 
    immediate

: endof     ( -- )          \ the "then" part
    BRANCH ,            \ compile the branch
    here @ MARK_OF >c       \ push a second marker for this clause
    888 , 
    ;
    immediate

: endcase   ( -- )          \ patch the branches, exiting when the MARK_CASE is found
    .scr
    begin
        c> MARK_CASE =          \ is the first marker a CASE marker?
        if 
            ['] r> ,            \ pop the key off the return stack
            ['] drop ,          \ dump the key
            drop                \ the address
            exit                \ we've finished
        else                    \ it's an 'endof' marker, so patch jump to the end. patch_addr is on the stack
            dup dup             \ save extra copies of the address for the second patch operation
            .scr
            here @ swap -
            swap  
            ." endof patch:" .scr ! \ store the patch
            c>                  \ get the corresponding 'of' marker
            drop 1 -               \ we could test the type...
            .scr
            dup rot swap - 1 + swap
            ." of patch" .scr !            \ store the delta between 'of' and 'endof'
        then
    again
    ;
    immediate