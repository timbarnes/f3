\ Core word tests, in the style of John Hayes' core.fr (ANS Forth core test suite).
\ They cover the words f3 implements with ANS semantics. Words that f3 deliberately does
\ differently (see doc.md, "Deviations from ANS Forth") are left out or noted.
\ Run: include src/forth/tester.fs  include src/forth/core-tests.fs

hide-stack
0 #errors !

-9223372036854775808 constant min-int
9223372036854775807  constant max-int
0 invert             constant max-uint      \ all bits set
0 constant <false>
-1 constant <true>

\ ---- basic assumptions
T{ -> }T
T{ : bitsset? if 0 0 else 0 then ; -> }T
T{ 0 bitsset? -> 0 }T
T{ 1 bitsset? -> 0 0 }T
T{ -1 bitsset? -> 0 0 }T

\ ---- booleans: invert and or xor
T{ 0 0 and -> 0 }T
T{ 0 1 and -> 0 }T
T{ 1 0 and -> 0 }T
T{ 1 1 and -> 1 }T
T{ 0 invert 1 and -> 1 }T
T{ 1 invert 1 and -> 0 }T
T{ <false> invert -> <true> }T
T{ <true> invert -> <false> }T
T{ 0 0 or -> 0 }T
T{ 0 1 or -> 1 }T
T{ 1 0 or -> 1 }T
T{ 1 1 or -> 1 }T
T{ 0 0 xor -> 0 }T
T{ 0 1 xor -> 1 }T
T{ 1 0 xor -> 1 }T
T{ 1 1 xor -> 0 }T

\ ---- shifts (rshift is arithmetic in f3, so only non-negative values are tested)
T{ 1 0 lshift -> 1 }T
T{ 1 1 lshift -> 2 }T
T{ 1 2 lshift -> 4 }T
T{ 1 63 lshift -> min-int }T
T{ 1 0 rshift -> 1 }T
T{ 1 1 rshift -> 0 }T
T{ 2 1 rshift -> 1 }T
T{ 4 2 rshift -> 1 }T
T{ max-int 62 rshift -> 1 }T

\ ---- comparisons
T{ 0 0= -> <true> }T
T{ 1 0= -> <false> }T
T{ -1 0= -> <false> }T
T{ min-int 0= -> <false> }T
T{ 0 0 = -> <true> }T
T{ 1 1 = -> <true> }T
T{ -1 -1 = -> <true> }T
T{ 1 0 = -> <false> }T
T{ -1 0 = -> <false> }T
T{ 0 1 = -> <false> }T
T{ 0 -1 = -> <false> }T
T{ 0 0< -> <false> }T
T{ -1 0< -> <true> }T
T{ min-int 0< -> <true> }T
T{ 1 0< -> <false> }T
T{ max-int 0< -> <false> }T
T{ 0 1 < -> <true> }T
T{ 1 2 < -> <true> }T
T{ -1 0 < -> <true> }T
T{ -1 1 < -> <true> }T
T{ min-int 0 < -> <true> }T
T{ min-int max-int < -> <true> }T
T{ 0 max-int < -> <true> }T
T{ 0 0 < -> <false> }T
T{ 1 1 < -> <false> }T
T{ 1 0 < -> <false> }T
T{ 2 1 < -> <false> }T
T{ 0 -1 < -> <false> }T
T{ 1 -1 < -> <false> }T
T{ 0 min-int < -> <false> }T
T{ max-int min-int < -> <false> }T
T{ max-int 0 < -> <false> }T
T{ 0 1 > -> <false> }T
T{ 1 0 > -> <true> }T
T{ -1 0 > -> <false> }T
T{ max-int min-int > -> <true> }T
T{ 0 1 u< -> <true> }T
T{ 1 2 u< -> <true> }T
T{ 0 max-uint u< -> <true> }T
T{ max-int max-uint u< -> <true> }T
T{ 0 0 u< -> <false> }T
T{ 1 0 u< -> <false> }T
T{ max-uint 0 u< -> <false> }T
T{ max-uint max-int u< -> <false> }T
T{ 0 1 min -> 0 }T
T{ 1 2 min -> 1 }T
T{ -1 0 min -> -1 }T
T{ min-int max-int min -> min-int }T
T{ 1 1 min -> 1 }T
T{ 0 1 max -> 1 }T
T{ 1 2 max -> 2 }T
T{ -1 0 max -> 0 }T
T{ min-int max-int max -> max-int }T

\ ---- stack operations
T{ 1 2 2drop -> }T
T{ 1 2 2dup -> 1 2 1 2 }T
T{ -1 ?dup -> -1 -1 }T
T{ 0 ?dup -> 0 }T
T{ 1 ?dup -> 1 1 }T
T{ 0 1 depth -> 0 1 2 }T
T{ 0 depth -> 0 1 }T
T{ depth -> 0 }T
T{ 1 2 drop -> 1 }T
T{ 0 drop -> }T
T{ 1 dup -> 1 1 }T
T{ 1 2 over -> 1 2 1 }T
T{ 1 2 3 rot -> 2 3 1 }T
T{ 1 2 swap -> 2 1 }T
T{ 1 2 nip -> 2 }T
T{ 1 2 tuck -> 2 1 2 }T
T{ 1 2 3 0 pick -> 1 2 3 3 }T
T{ 1 2 3 2 pick -> 1 2 3 1 }T
T{ 1 2 3 0 roll -> 1 2 3 }T
T{ 1 2 3 1 roll -> 1 3 2 }T
T{ 1 2 3 2 roll -> 2 3 1 }T

\ ---- return stack
T{ : gr1 >r r> ; -> }T
T{ : gr2 >r r@ r> drop ; -> }T
T{ 123 gr1 -> 123 }T
T{ 123 gr2 -> 123 }T
T{ 1 gr1 -> 1 }T

\ ---- arithmetic (/ and mod truncate toward zero: symmetric division)
T{ 0 5 + -> 5 }T
T{ 5 0 + -> 5 }T
T{ 0 -5 + -> -5 }T
T{ -1 1 + -> 0 }T
T{ -1 -2 + -> -3 }T
T{ max-int 1 + -> min-int }T
T{ 0 5 - -> -5 }T
T{ 5 0 - -> 5 }T
T{ 1 2 - -> -1 }T
T{ -1 -2 - -> 1 }T
T{ min-int 1 - -> max-int }T
T{ 0 1+ -> 1 }T
T{ -1 1+ -> 0 }T
T{ 2 1- -> 1 }T
T{ 0 1- -> -1 }T
T{ 0 negate -> 0 }T
T{ 1 negate -> -1 }T
T{ -1 negate -> 1 }T
T{ 2 negate -> -2 }T
T{ 0 abs -> 0 }T
T{ 1 abs -> 1 }T
T{ -1 abs -> 1 }T
T{ 0 0 * -> 0 }T
T{ 0 1 * -> 0 }T
T{ 1 2 * -> 2 }T
T{ 3 3 * -> 9 }T
T{ -3 3 * -> -9 }T
T{ -3 -3 * -> 9 }T
T{ 0 1 / -> 0 }T
T{ 1 1 / -> 1 }T
T{ 2 1 / -> 2 }T
T{ 7 2 / -> 3 }T
T{ -7 2 / -> -3 }T
T{ 7 -2 / -> -3 }T
T{ 0 1 mod -> 0 }T
T{ 7 2 mod -> 1 }T
T{ -7 2 mod -> -1 }T
T{ 7 -2 mod -> 1 }T
T{ 7 2 /mod -> 1 3 }T
T{ -7 2 /mod -> -1 -3 }T

\ ---- memory
variable v1
T{ 123 v1 ! -> }T
T{ v1 @ -> 123 }T
T{ 1 v1 +! v1 @ -> 124 }T
T{ -1 v1 +! v1 @ -> 123 }T
T{ 99 constant c1 c1 -> 99 }T
variable buf 3 allot                            \ buf has 4 cells
T{ 1 buf ! 2 buf 1+ ! 3 buf 2 + ! 4 buf 3 + ! -> }T
T{ buf @ buf 1+ @ buf 2 + @ buf 3 + @ -> 1 2 3 4 }T
T{ here @ 1 , here @ swap - -> 1 }T             \ f3: HERE is a variable, so `here @`
T{ 65 tmp @ c! tmp @ c@ -> 65 }T                \ c@ and c! address string space
T{ char A -> 65 }T
T{ : gc1 [char] X ; gc1 -> 88 }T

\ ---- if else then
T{ : gi1 if 123 then ; -> }T
T{ : gi2 if 123 else 234 then ; -> }T
T{ 0 gi1 -> }T
T{ 1 gi1 -> 123 }T
T{ -1 gi1 -> 123 }T
T{ 0 gi2 -> 234 }T
T{ 1 gi2 -> 123 }T
T{ : gi3 if 1 if 2 else 3 then else 4 then ; 0 gi3 -1 gi3 -> 4 2 }T

\ ---- begin while repeat until again
T{ : gi4 begin dup 1+ dup 5 > until ; -> }T
T{ 3 gi4 -> 3 4 5 6 }T
T{ 5 gi4 -> 5 6 }T
T{ 6 gi4 -> 6 7 }T
T{ : gi5 begin dup 2 > while dup 5 < while dup 1+ repeat 123 else 345 then ; -> }T
T{ : gi6 begin dup 5 < while dup 1+ repeat ; -> }T
T{ 2 gi6 -> 2 3 4 5 }T
T{ 7 gi6 -> 7 }T
T{ : gi7 0 begin 1+ dup 3 = if exit then again ; gi7 -> 3 }T

\ ---- do loop +loop i j leave unloop ?do
T{ : gd1 do i loop ; -> }T
T{ 4 1 gd1 -> 1 2 3 }T
T{ 2 -1 gd1 -> -1 0 1 }T
T{ : gd2 do i -1 +loop ; -> }T
T{ 1 4 gd2 -> 4 3 2 1 }T
T{ -1 2 gd2 -> 2 1 0 -1 }T
T{ : gd3 do 1 0 do j loop loop ; -> }T
T{ 4 1 gd3 -> 1 2 3 }T
T{ : gd4 do 1 0 do j loop -1 +loop ; -> }T
T{ 1 4 gd4 -> 4 3 2 1 }T
T{ : gd5 123 swap 0 do i 4 > if drop 234 leave then loop ; -> }T
T{ 1 gd5 -> 123 }T
T{ 5 gd5 -> 123 }T
T{ 6 gd5 -> 234 }T
T{ : gd6 0 swap 0 do i 1+ 0 do i j + 3 = if i unloop i unloop exit then 1+ loop loop ; -> }T
T{ 1 gd6 -> 1 }T
T{ 2 gd6 -> 3 }T
T{ 3 gd6 -> 4 1 2 }T
T{ : gd7 ?do i loop ; 3 3 gd7 -> }T
T{ 5 3 gd7 -> 3 4 }T
T{ : gd8 10 0 do i 4 +loop ; gd8 -> 0 4 8 }T

\ ---- colon definitions, recursion, exit
T{ : gdx 123 ; gdx -> 123 }T
T{ : gdx2 gdx 234 ; gdx2 -> 123 234 }T
T{ : gr3 dup if dup 1- recurse * else drop 1 then ; 5 gr3 -> 120 }T
T{ : ge1 1 exit 2 ; ge1 -> 1 }T

\ ---- immediate, [ ], literal, state
T{ : gt1 123 ; -> }T
T{ ' gt1 execute -> 123 }T
T{ : gt2 ['] gt1 ; immediate -> }T
T{ gt2 execute -> 123 }T
T{ : gs1 [ 3 4 + ] literal ; gs1 -> 7 }T
T{ : gs2 state @ ; immediate -> }T
T{ gs2 -> 0 }T
T{ : gs3 gs2 literal ; gs3 -> -1 }T
T{ : imm1 123 ; immediate : imm2 imm1 ; -> 123 }T

\ ---- execute with builtins and data words
T{ 1 2 ' + execute -> 3 }T
T{ ' v1 execute -> v1 }T
T{ ' c1 execute -> 99 }T

\ ---- case of endof endcase (Forth 2012)
: cs1 case 1 of 111 endof 2 of 222 endof 3 of 333 endof 999 swap endcase ;
T{ 1 cs1 -> 111 }T
T{ 2 cs1 -> 222 }T
T{ 3 cs1 -> 333 }T
T{ 4 cs1 -> 999 }T

\ ---- catch throw (exception word set)
: t1 9 ;
: c1 1 2 3 ['] t1 catch ;
T{ c1 -> 1 2 3 9 0 }T
: t2 8 0 throw ;
T{ : c2 1 2 ['] t2 catch ; c2 -> 1 2 8 0 }T
: t3 7 8 9 99 throw ;
T{ : c3 1 2 ['] t3 catch ; c3 -> 1 2 99 }T
: t4 1- dup 0> if recurse else 999 throw -222 then ;
T{ : c4 3 4 5 10 ['] t4 catch -111 ; c4 -> 3 4 5 0 999 -111 }T
T{ ' drop catch -> -4 }T
T{ : c5 1 0 / ; ' c5 catch -> -10 }T

\ ---- base and number conversion
T{ hex ff decimal -> 255 }T
T{ $10 #10 %10 -> 16 10 2 }T
T{ base @ -> 10 }T

." core tests: " #errors @ 0 .r ."  errors" cr
