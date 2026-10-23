\ Benchmarks. Run with the release build:
\     cargo build --release && ./target/release/f3 -f src/forth/bench.fs
\ Core load time: time (echo bye | ./target/release/f3)
\
\ Baseline 2026-10-22 (commit 2ca8f01): fib(27) 26 ms, 10M empty for 251 ms
\ After Phase 2 (native loop opcodes):    fib(27) 21 ms, 10M empty for  27 ms
\ After Phase 4 (best of 15):             fib(27) 14 ms, empty 19, nop calls 60, dup drop 47, variable 45
\     (Phase 3 on the same machine:       fib(27) 20 ms, empty 24, nop calls 83, dup drop 86, variable 99)

: fib ( n -- f )    dup 2 < if exit then dup 1 - recurse swap 2 - recurse + ;
: spin ( n -- )     for next ;
: nop ;
: calls ( n -- )    for nop next ;                 \ colon-call overhead: (calls - spin) / n
: prims ( n -- )    1 swap for dup drop next drop ; \ two builtins per iteration
variable v
: vars ( n -- )     for v drop next ;              \ a data word per iteration

now 27 fib drop millis       ." fib(27):        " . ." ms" cr
now 10000000 spin millis     ." 10M empty for:  " . ." ms" cr
now 10000000 calls millis    ." 10M nop calls:  " . ." ms" cr
now 10000000 prims millis    ." 10M dup drop:   " . ." ms" cr
now 10000000 vars millis     ." 10M variable:   " . ." ms" cr
bye
