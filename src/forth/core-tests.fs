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

\ ---- shifts (rshift is logical; 2/ is the arithmetic shift)
T{ 1 0 lshift -> 1 }T
T{ 1 1 lshift -> 2 }T
T{ 1 2 lshift -> 4 }T
T{ 1 63 lshift -> min-int }T
T{ 1 0 rshift -> 1 }T
T{ 1 1 rshift -> 0 }T
T{ 2 1 rshift -> 1 }T
T{ 4 2 rshift -> 1 }T
T{ max-int 62 rshift -> 1 }T
T{ -1 1 rshift -> max-int }T
T{ min-int 63 rshift -> 1 }T
T{ -8 2/ -> -4 }T
T{ 4 2/ -> 2 }T
T{ 4 2* -> 8 }T
T{ min-int 2/ -> -4611686018427387904 }T

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
variable buf 3 cells allot                      \ buf has 4 cells
T{ 1 buf ! 2 buf cell+ ! 3 buf 2 cells + ! 4 buf 3 cells + ! -> }T
T{ buf @ buf cell+ @ buf 2 cells + @ buf 3 cells + @ -> 1 2 3 4 }T
T{ here 1 , here swap - -> 1 cells }T
T{ 65 tmp @ c! tmp @ c@ -> 65 }T                \ c@ and c! address bytes
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

\ ---- case-insensitive names: standard programs are written in upper case
T{ 1 DUP -> 1 1 }T
T{ : GDU 3 0 DO I LOOP ; GDU -> 0 1 2 }T
T{ : gdl 2 + ; 0 GDL -> 2 }T
T{ 1 2 SWAP OVER -> 2 1 2 }T

\ ---- return-stack pairs (builtins, ANS order: x2 on top of the return stack)
T{ 1 2 2>r 2r@ 2r> -> 1 2 1 2 }T
T{ 1 2 2>r r> r> -> 2 1 }T
T{ 5 >r rdrop -> }T
T{ 1 2 2>r 2rdrop -> }T
T{ : gr4 1 2 2>r 3 2r> ; gr4 -> 3 1 2 }T

\ ---- find reports immediacy
T{ : gimm ; immediate c" gimm" find nip -> 1 }T
T{ c" dup" find nip -> -1 }T
T{ c" no-such-word" find nip -> 0 }T

\ ---- double-cell arithmetic
T{ 5 s>d -> 5 0 }T
T{ -5 s>d -> -5 -1 }T
T{ 0 1 um* -> 0 0 }T
T{ max-uint 2 um* -> -2 1 }T
T{ -3 2 m* -> -6 -1 }T
T{ 7 0 3 um/mod -> 1 2 }T
T{ -7 s>d 2 fm/mod -> 1 -4 }T
T{ -7 s>d 2 sm/rem -> -1 -3 }T
T{ 7 s>d -2 fm/mod -> -1 -4 }T
T{ 2 3 4 */ -> 1 }T
T{ max-int 2 4 */ -> 4611686018427387903 }T      \ needs the double-cell intermediate
T{ 7 2 3 */mod -> 2 4 }T

\ ---- pictured numeric output and small words
T{ 0 0 <# #s #> nip -> 1 }T
T{ -1 0 <# #s #> nip -> 20 }T                     \ 2^64-1 has 20 digits
T{ 0 0 1 within -> <true> }T
T{ 1 0 1 within -> <false> }T
T{ -1 0 1 within -> <false> }T
T{ 1 2 u> -> <false> }T
T{ -1 1 u> -> <true> }T
T{ 0 cell+ -> 1 cells }T
T{ 1 char+ 3 chars -> 2 3 }T
T{ c" abc" count nip -> 3 }T
T{ unused 0 > -> <true> }T
T{ s" x" environment? -> <false> }T
T{ source nip 0 > -> <true> }T

\ ---- postpone, :noname, value/to, defer/is, [if]
T{ : gpd postpone dup ; immediate : gpq 4 gpd ; gpq -> 4 4 }T
T{ : gpi postpone if ; immediate : gpj gpi 1 else 2 then ; 0 gpj -> 2 }T
T{ :noname 3 4 + ; execute -> 7 }T
T{ 9 value vv vv -> 9 }T
T{ 4 to vv vv -> 4 }T
T{ : gtv 6 to vv ; gtv vv -> 6 }T
T{ defer gdd ' dup is gdd 5 gdd -> 5 5 }T
T{ : gsd ['] drop is gdd ; gsd 1 2 gdd -> 1 }T
T{ 1 [if] 11 [else] 22 [then] -> 11 }T
T{ 0 [if] 11 [else] 22 [then] -> 22 }T
T{ 0 [IF] 1 [IF] 2 [THEN] 3 [ELSE] 4 [THEN] -> 4 }T
T{ : gab 0 abort" never" 5 ; gab -> 5 }T
T{ : gab2 1 abort" always" ; ' gab2 catch -> -2 }T

\ ---- create does> >body (from core.fr)
T{ here 1 allot here swap - -> 1 }T
T{ : does1 does> @ 1 + ; -> }T
T{ : does2 does> @ 2 + ; -> }T
T{ create cr1 -> }T
T{ cr1 -> here }T
T{ ' cr1 >body -> here }T
T{ 1 , -> }T
T{ cr1 @ -> 1 }T
T{ does1 -> }T
T{ cr1 -> 2 }T
T{ does2 -> }T
T{ cr1 -> 3 }T
T{ : weird: create does> 1 + does> 2 + ; -> }T
T{ weird: w1 -> }T
T{ ' w1 >body -> here }T
T{ w1 -> here 1 + }T
T{ w1 -> here 2 + }T
T{ : gcon create , does> @ ; 42 gcon g42 g42 -> 42 }T
T{ : garr create cells allot does> swap cells + ; 5 garr ga 9 2 ga ! 2 ga @ -> 9 }T
T{ create gbuf 3 , 4 , gbuf @ gbuf cell+ @ -> 3 4 }T
T{ variable gv1 3 cells allot here gv1 4 cells + = -> <true> }T

\ ---- evaluate
T{ s" 123" evaluate -> 123 }T
T{ s" 1 2 +" evaluate -> 3 }T
T{ : ge3 s" 123" evaluate ; ge3 -> 123 }T
T{ s" : ge5 77 ;" evaluate ge5 -> 77 }T
T{ s" 1 2 +" evaluate 10 * -> 30 }T              \ the rest of the line still runs
T{ : ge6 s" nosuch" evaluate ; ' ge6 catch 5 -> -13 5 }T

\ ---- strings (c-addr u) and the String word set
T{ s" abc" nip -> 3 }T
T{ : gs4 s" xyz" ; gs4 nip -> 3 }T
T{ s" abc" drop c@ -> 97 }T
T{ c" abc" c@ -> 3 }T
T{ s" abc" s" abc" compare -> 0 }T
T{ s" abc" s" abd" compare -> -1 }T
T{ s" abd" s" abc" compare -> 1 }T
T{ s" ab" s" abc" compare -> -1 }T
T{ s" abc" s" ab" compare -> 1 }T
T{ s" " s" " compare -> 0 }T
T{ s" hello world" s" wor" search nip nip -> <true> }T
T{ s" hello world" s" wor" search drop nip -> 5 }T
T{ s" hello" s" xyz" search nip nip -> <false> }T
T{ s" hello" 2 /string nip -> 3 }T
T{ s" hi   " -trailing nip -> 2 }T
T{ s" " -trailing nip -> 0 }T
T{ 0 0 s" 123" >number nip -> 123 0 0 }T
T{ 0 0 s" 12x" >number nip -> 12 0 1 }T
T{ 4 s-allot constant gsb gsb 4 char z fill gsb 3 + c@ -> 122 }T
T{ gsb 4 erase gsb c@ -> 0 }T
T{ s" abcd" gsb swap move gsb 1+ c@ -> 98 }T
T{ parse-name word1 nip -> 5 }T
T{ char ) parse abc) nip -> 3 }T
T{ bl word zz c@ -> 2 }T

." core tests: " #errors @ 0 .r ."  errors" cr
