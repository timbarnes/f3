\ Benchmarks. Run with the release build:
\     cargo build --release && echo "include src/forth/bench.fs" | ./target/release/f3
\ (not `-f`: see F25 in plan/ANALYSIS.md)
\ Core load time: time (echo bye | ./target/release/f3)
\
\ Baseline 2026-10-22 (commit 2ca8f01): fib(27) 26 ms, 10M empty for 251 ms

: fib ( n -- f )    dup 2 < if exit then dup 1 - recurse swap 2 - recurse + ;
: spin ( n -- )     for next ;

now 27 fib drop millis       ." fib(27):        " . ." ms" cr
now 10000000 spin millis     ." 10M empty for:  " . ." ms" cr
bye
