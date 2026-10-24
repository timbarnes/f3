//! Black-box tests: run the f3 binary on a script and inspect what it prints.
//!
//! Tests named f<n>_... pin down bugs from plan/ANALYSIS.md. To track a newly found bug,
//! add a test asserting the correct behaviour, marked `#[ignore = "F<n> ..."]`
//! (`cargo test -- --ignored` runs them), and un-ignore it when the bug is fixed.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(5);

struct Run {
    out: String,     // stdout + stderr
    ok: bool,        // exited with status 0
    timed_out: bool, // killed after TIMEOUT
}

fn collect(mut r: impl Read + Send + 'static) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = r.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    })
}

/// Feed `input` to f3 on stdin exactly as given, with the core library loaded.
fn run_raw(input: &str) -> Run {
    run_args(&[], input)
}

fn run_args(args: &[&str], input: &str) -> Run {
    let mut child = Command::new(env!("CARGO_BIN_EXE_f3"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn f3");
    let mut stdin = child.stdin.take().unwrap();
    let input = input.to_owned();
    thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes()); // dropping stdin sends EOF
    });
    let out = collect(child.stdout.take().unwrap());
    let err = collect(child.stderr.take().unwrap());

    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        thread::sleep(Duration::from_millis(5));
    };
    Run {
        out: out.join().unwrap() + &err.join().unwrap(),
        ok: status.is_some_and(|s| s.success()),
        timed_out: status.is_none(),
    }
}

/// Run `src` with the per-line stack display off, then exit.
fn run(src: &str) -> Run {
    run_raw(&format!("hide-stack\n{src}\nbye\n"))
}

/// Run `src` and return the data stack it leaves, bottom first, e.g. "1 2".
fn stack(src: &str) -> String {
    let r = run(&format!("{src}\n.s"));
    assert!(r.ok, "f3 did not exit cleanly:\n{}", r.out);
    let start = r.out.rfind("[ ").expect("no .s output");
    let end = start + r.out[start..].find(']').unwrap();
    r.out[start + 2..end].trim().to_string()
}

/// After `src`, the interpreter must still be alive, and able to define and run a word.
fn assert_survives(src: &str) {
    let r = run(&format!("{src}\n: alive 12345 ; alive ."));
    assert!(!r.timed_out, "f3 hung:\n{}", r.out);
    assert!(
        r.ok && r.out.contains("12345 "),
        "f3 did not recover:\n{}",
        r.out
    );
}

// ---------------------------------------------------------------- baseline

#[test]
fn arithmetic() {
    assert_eq!(stack("1 2 + 10 3 - 6 7 * 20 6 /"), "3 7 42 3");
}

#[test]
fn colon_definition() {
    assert_eq!(stack(": sq dup * ; 7 sq"), "49");
}

#[test]
fn if_else_then() {
    assert_eq!(stack(": t if 1 else 2 then ; 0 t -1 t"), "2 1");
}

#[test]
fn begin_until() {
    assert_eq!(stack(": c 0 begin 1 + dup 5 = until ; c"), "5");
}

#[test]
fn print_number() {
    let r = run("42 . -7 .");
    assert!(r.ok && r.out.contains("42 -7 "), "{}", r.out);
}

#[test]
fn ans_core_tests() {
    // Hayes-style T{ -> }T tests of the words f3 implements with ANS semantics
    let r = run("include src/forth/tester.fs\ninclude src/forth/core-tests.fs");
    assert!(r.ok && r.out.contains("core tests: 0 errors"), "{}", r.out);
}

#[test]
fn regression_suite() {
    let r = run("include src/forth/regression.fs");
    assert!(r.ok, "f3 did not exit cleanly:\n{}", r.out);
    assert!(r.out.contains("All tests passed!"), "{}", r.out);
}

// ------------------------------------------- known bugs (plan/ANALYSIS.md)

#[test]
fn f1_execute_colon_word_inside_definition() {
    assert_eq!(stack(": inc 1 + ; : t ['] inc execute 100 + ; 1 t"), "102");
}

#[test]
fn f2_execute_builtin_xt() {
    assert_eq!(stack("3 ' dup execute"), "3 3");
}

#[test]
fn f3_error_inside_definition_does_not_install_word() {
    assert_eq!(stack(": bad 1 nosuch 2 ;\nbad"), "");
}

#[test]
fn f4_error_resets_compile_state() {
    assert_eq!(stack(": x nosuch\nstate @"), "0");
}

#[test]
fn f5_stdin_eof_exits() {
    let r = run_raw("1 .\n");
    assert!(!r.timed_out && r.ok, "{}", r.out);
}

#[test]
fn f6_long_lines_not_truncated() {
    let line = format!("0{}", " 1 +".repeat(40)); // 161 chars
    assert_eq!(stack(&line), "40");
}

#[test]
fn f7_dictionary_overflow_is_an_error() {
    // filling the dictionary with , is permanent, so only check the error and that f3 lives on
    let r = run(": fill 1000000 for 1 , next ; fill\n12345 .");
    assert!(
        r.ok && r.out.contains("dictionary full") && r.out.contains("12345 "),
        "{}",
        r.out
    );
}

#[test]
fn f8_runaway_recursion_is_an_error() {
    assert_survives(": deep recurse ; deep");
}

#[test]
fn f9_number_honours_base() {
    assert_eq!(stack("hex ff decimal"), "255");
}

#[test]
fn f10_do_loop() {
    assert_eq!(stack(": lp 3 0 do i loop ; lp"), "0 1 2");
}

#[test]
fn f14_non_ascii_input() {
    assert_survives(&format!("{}é", "a".repeat(130)));
}

#[test]
fn f24_clear_inside_definition() {
    assert_eq!(stack(": z 1 clear 2 ; z"), "2");
}

#[test]
fn f25_loaded_file_sees_core_words() {
    let path = std::env::temp_dir().join("f3_f25.fs");
    std::fs::write(&path, "hide-stack 12345 .\n").unwrap(); // hide-stack: don't let .s output match
    let r = run_args(&["-f", path.to_str().unwrap()], "bye\n");
    assert!(r.ok && r.out.contains("12345 "), "{}", r.out);
}

#[test]
fn f26_abort_quote() {
    let r = run(": t true abort\" oops\" ; : f false abort\" no\" 9 ; 1 2 t\nf .s");
    assert!(
        r.ok && r.out.contains("oops") && r.out.contains("[ 9 ]") && !r.out.contains("no"),
        "{}",
        r.out
    );
}

#[test]
fn f27_negative_allot() {
    assert_eq!(stack("here 10 allot -10 allot here ="), "-1");
}

#[test]
fn f29_tick_unknown_word_is_an_error() {
    let r = run(": t ['] nosuch ;");
    assert!(r.ok && r.out.contains("nosuch"), "{}", r.out);
}

#[test]
fn stack_underflow_is_recoverable() {
    assert_survives("drop");
}

#[test]
fn divide_by_zero_is_recoverable() {
    assert_survives("1 0 /");
}

#[test]
fn arithmetic_wraps() {
    assert_eq!(stack("9223372036854775807 1 +"), "-9223372036854775808");
}

// ------------------------------------------------------------- Phase 2

#[test]
fn do_loop_variants() {
    assert_eq!(stack(": t 10 0 do i 3 +loop ; t"), "0 3 6 9");
    // a negative step stops after crossing the boundary between limit-1 and limit
    assert_eq!(stack(": t 0 10 do i -3 +loop ; t"), "10 7 4 1");
    assert_eq!(stack(": t 0 3 do i -1 +loop ; t"), "3 2 1 0");
    assert_eq!(stack(": t 5 5 ?do i loop 99 ; t"), "99");
    assert_eq!(
        stack(": t 10 0 do i dup 3 = if leave then loop ; t"),
        "0 1 2 3"
    );
    assert_eq!(
        stack(": t 3 1 do 13 11 do j i loop loop ; t"),
        "1 11 1 12 2 11 2 12"
    );
    assert_eq!(
        stack(": t 10 0 do i 2 = if unloop exit then loop 99 ; t 7"),
        "7"
    );
}

#[test]
fn for_next() {
    assert_eq!(stack(": t 3 for i next ; t"), "3 2 1");
    assert_eq!(stack(": t 0 for i next 99 ; t"), "99"); // used to run ~2^64 times
    assert_eq!(
        stack(": t 2 for 2 for j i next next ; t"),
        "2 2 2 1 1 2 1 1"
    );
}

#[test]
fn f21_compile_only_words_error_when_interpreted() {
    let r = run("5 for i . next\n12345 .");
    assert!(
        r.ok && r.out.contains("only valid inside a definition") && r.out.contains("12345 "),
        "{}",
        r.out
    );
}

#[test]
fn number_prefixes() {
    assert_eq!(stack("$ff #10 %101 $-10 hex 10 decimal"), "255 10 5 -16 16");
    assert_eq!(stack("$ffffffffffffffff"), "-1");
}

#[test]
fn execute_data_words() {
    assert_eq!(
        stack("variable v 5 v ! ' v execute @ 7 constant c ' c execute"),
        "5 7"
    );
}

#[test]
fn f13_file_words() {
    let dir = std::env::temp_dir();
    let (a, b) = (dir.join("f3_fw_a.txt"), dir.join("f3_fw_b.txt"));
    let (a, b) = (a.to_str().unwrap(), b.to_str().unwrap());
    let src = format!(
        "s\" {a}\" W/O open-file drop constant fa\n\
         s\" {b}\" W/O open-file drop constant fb\n\
         s\" hello\" fa write-line\n\
         fa close-file\n\
         s\" more\" fb write-line\n\
         fb close-file drop drop drop drop\n\
         s\" {a}\" R/O open-file drop constant fr\n\
         100 s-allot constant rb\n\
         fr file-size drop rb 100 fr read-line rb 100 fr read-line fr file-position drop"
    );
    // sizes and lengths: 6 bytes; "hello" read (5 TRUE 0); then end of file (0 FALSE 0); position 6
    assert_eq!(stack(&src), "6 5 -1 0 0 0 0 6");
}

#[test]
fn f16_see_constant() {
    let r = run("7 constant seven see seven");
    assert!(r.out.contains("Constant: seven = 7"), "{}", r.out);
}

#[test]
fn f17_debuglevel_matches_dbg() {
    assert_eq!(stack("2 dbg debuglevel 1 dbg"), "2");
}

#[test]
fn f28_builtin_name_uses_tmp() {
    assert_eq!(stack("0 builtin-name tmp @ = 0 builtin-name c@"), "-1 1");
}

// ------------------------------------------------------------- Phase 3

#[test]
fn stack_words_in_forth() {
    assert_eq!(stack("1 2 3 2 pick"), "1 2 3 1");
    assert_eq!(stack("1 2 3 2 roll"), "2 3 1");
    assert_eq!(stack("1 2 3 depth"), "1 2 3 3");
    assert_eq!(stack("1 2 3 clear depth"), "0");
    let r = run("1 5 pick\n12345 .");
    assert!(
        r.out.contains("pick: stack underflow") && r.out.contains("12345 "),
        "{}",
        r.out
    );
}

#[test]
fn see_in_forth() {
    let r = run(": sq dup * ; see sq see dup");
    assert!(r.out.contains(": sq dup * ; "), "{}", r.out);
    assert!(r.out.contains("Builtin: dup ( n -- n n )"), "{}", r.out);
    let r = run(": t 3 0 do i . loop .\" hi\" ; see t");
    assert!(
        r.out.contains(": t 3 0 do:") && r.out.contains("\" hi\""),
        "{}",
        r.out
    );
}

#[test]
fn unbalanced_control_structure_is_an_error() {
    let r = run(": t if again ;\n12345 .");
    assert!(
        r.out.contains("unbalanced control structure") && r.out.contains("12345 "),
        "{}",
        r.out
    );
}

#[test]
fn string_words_in_forth() {
    assert_eq!(stack("c\" hello\" s-create c@"), "5");
    assert_eq!(stack("c\" abc\" pad s-copy pad 1+ c@"), "97");
}

#[test]
fn new_primitives() {
    assert_eq!(stack("5 3 xor -1 1 u< 1 -1 u< 0 invert"), "6 0 -1 -1");
    assert_eq!(stack("' if immed? ' dup immed?"), "-1 0");
}

#[test]
fn header_words() {
    // [link | flags][name][code field]: name> and >name are inverses, >link is just before the name
    assert_eq!(stack(": abcdefghij ; ' abcdefghij dup >name name> ="), "-1");
    assert_eq!(stack("' dup >name c@ ' dup >link cell+ ' dup >name ="), "3 -1");
    assert_eq!(stack("' if >flags IMMEDIATE_FLAG and 0<> ' dup >flags"), "-1 0");
    assert_eq!(stack(": hh ; immediate ' hh immed? forget hh ' dup immed?"), "-1 0");
}

// ------------------------------------------------------------- Phase 4

#[test]
fn tracer_follows_stepper_variable() {
    // the inner interpreter caches `stepper`; storing to it with ! must switch tracing on and off
    let r = run(": t 1 2 + ;\n1 stepper ! 5 stepper-depth !\nt\n0 stepper !\nt\n.s");
    assert_eq!(r.out.matches(" EXIT ").count(), 1, "{}", r.out); // only the first t is traced
    assert!(r.out.contains("[ 3 3 ]"), "{}", r.out);
}

// ------------------------------------------- ANS step 1 (plan/ANS.md)

#[test]
fn ans_output_words() {
    // u. is unsigned with a trailing space (F33); emit sends any byte; .( prints at once
    let r = run("-1 u. 255 u. -5 . 7 emit .( done)");
    assert!(r.out.contains("18446744073709551615 255 -5 \u{7}done"), "{:?}", r.out);
}

#[test]
fn ans_conditional_compilation_across_lines() {
    assert_eq!(stack("0 [if]\n1 2\n[else]\n3\n[then]"), "3");
}

#[test]
fn f32_return_stack_pairs() {
    assert_eq!(stack("1 2 2>r 2r>"), "1 2");
}

// ------------------------------------------- ANS step 2 (plan/ANS.md)

#[test]
fn ans_create_does() {
    let r = run(": const create , does> @ ; 42 const answer see answer answer .");
    assert!(r.out.contains("Created: answer data at") && r.out.contains("does> code at"), "{}", r.out);
    assert!(r.out.contains("42 "), "{}", r.out);
    let r = run("variable x : d does> ; : t x d ; t\n12345 .");
    assert!(r.out.contains("does>: the latest word was not made by create") && r.out.contains("12345"), "{}", r.out);
}

// ------------------------------------------- ANS step 3 (plan/ANS.md)

#[test]
fn ans_file_and_include_words() {
    let dir = std::env::temp_dir();
    let (src, out) = (dir.join("f3_inc.fs"), dir.join("f3_out.txt"));
    std::fs::write(&src, ": inc-word 41 ;\n").unwrap();
    let (src, out) = (src.to_str().unwrap(), out.to_str().unwrap());
    let script = format!(
        "s\" {src}\" included inc-word\n\
         s\" {src}\" R/O open-file drop include-file inc-word\n\
         s\" {out}\" R/W create-file drop constant fo\n\
         s\" line one\" fo write-line drop fo file-size drop fo close-file drop\n\
         s\" {out}\" delete-file s\" {out}\" R/O open-file nip"
    );
    // included and include-file interpret the file; the file has 9 bytes; delete-file works
    assert_eq!(stack(&script), "41 41 9 0 -1");
    let r = run("s\" /no/such/file.fs\" included\n12345 .");
    assert!(r.out.contains("ERROR: /no/such/file.fs") && r.out.contains("12345"), "{}", r.out);
}

#[test]
fn ans_accept() {
    // accept reads the next input line into the buffer: here, the line after the command
    let r = run("64 s-allot constant ab ab 64 accept ab swap type\nhello there");
    assert!(r.out.contains("hello there"), "{}", r.out);
}

// ------------------------------------------- step 4 stage 0 (plan/STEP4.md)

#[test]
fn f34_sequences_stacks_and_arrays() {
    assert_eq!(stack("3 stack st 10 st >s 20 st >s st s> st s>"), "20 10");
    assert_eq!(stack("5 array ar 7 2 ar a! 9 4 ar a! 2 ar a@ 4 ar a@ ar ac@"), "7 9 5");
}

// ------------------------------------------------------------- Phase 5

#[test]
fn f30_allot_after_a_variable_keeps_the_dictionary() {
    // buf gets 11 cells; words defined afterwards, and all earlier words, still work
    let src = "variable buf 10 cells allot : foo 7 ; 42 buf 10 cells + ! foo buf 10 cells + @ 1 2 +";
    assert_eq!(stack(src), "7 42 3");
    assert_eq!(stack("here 10 allot -10 allot here = 1 2 +"), "-1 3");
    // the same for , after a finished word
    assert_eq!(stack("variable x 1 , 2 , : foo 7 ; foo x 2 cells + @ 1 2 +"), "7 2 3");
}

#[test]
fn catch_and_throw() {
    assert_eq!(stack(": ok 7 ; ' ok catch"), "7 0");
    assert_eq!(stack(": t 1 2 3 99 throw ; 5 ' t catch"), "5 99"); // depth restored, then the code
    assert_eq!(stack(": bad 1 0 / ; ' bad catch"), "-10"); // engine errors have ANS codes
    assert_eq!(stack("' drop catch"), "-4");
    assert_eq!(stack("' abort catch"), "-1");
    assert_eq!(stack(": a -1 abort\" boom\" ; ' a catch"), "-2");
    assert_eq!(stack("0 throw 1"), "1");
    // the return stack is restored too, including inside a loop
    assert_eq!(stack(": inner 1 >r 2 >r 5 throw ; : outer ['] inner catch ; outer 1 2 +"), "5 3");
    assert_eq!(stack(": t 3 0 do ['] drop catch drop loop 99 ; t"), "99");
    // re-throwing keeps the original message; an uncaught user code is reported
    let r = run(": bad 1 0 / ; ' bad catch throw\n42 throw\n12345 .");
    assert!(r.out.contains("ERROR: division by zero"), "{}", r.out);
    assert!(r.out.contains("ERROR: uncaught exception 42") && r.out.contains("12345 "), "{}", r.out);
}

/// Phase 1 exit check: 10k lines of random words and numbers (plus small colon definitions)
/// must never kill f3, hang it, or hit a Rust panic (an engine bug rather than a Forth error).
#[test]
fn fuzz_random_input_never_crashes() {
    const WORDS: &[&str] = &[
        "+", "-", "*", "/", "mod", "lshift", "rshift", "<", "=", "0=", "0<", "and", "or", "dup",
        "drop", "swap", "over", "rot", "pick", "roll", "depth", "clear", "@", "c@", ">r", "r>",
        "r@", "i", "j", ".", "negate", "abs", "min", "max", "1+", "1-", "2dup", "2drop", "nip",
        "tuck", ">", "<>", "0>", "?dup", "not", "nosuch",
    ];
    const NUMBERS: &[&str] = &[
        "0",
        "1",
        "-1",
        "2",
        "7",
        "63",
        "64",
        "65",
        "255",
        "1000",
        "65535",
        "65536",
        "9223372036854775807",
        "-9223372036854775808",
    ];
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut next = |n: usize| {
        seed ^= seed << 13; // xorshift64: deterministic, so failures reproduce
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let mut src = String::new();
    for line in 0..10_000 {
        let tokens: Vec<&str> = (0..1 + next(8))
            .map(|_| {
                if next(3) == 0 {
                    NUMBERS[next(NUMBERS.len())]
                } else {
                    WORDS[next(WORDS.len())]
                }
            })
            .collect();
        if line % 10 == 0 {
            src += &format!(": fz {} ; fz\n", tokens.join(" "));
        } else {
            src += &tokens.join(" ");
            src += "\n";
        }
    }
    if let Ok(path) = std::env::var("F3_FUZZ_DUMP") {
        std::fs::write(path, &src).unwrap(); // to reproduce a failure by hand
    }
    let r = run(&format!("{src}: alive 12345 ; alive ."));
    assert!(!r.timed_out, "f3 hung");
    assert!(r.ok && r.out.contains("12345 "), "f3 did not survive");
    let (errors, oks) = (
        r.out.matches("ERROR:").count(),
        r.out.matches("ok ").count(),
    );
    eprintln!("fuzz: {errors} errors, {oks} lines ok");
    assert!(
        errors > 1000 && oks > 1000,
        "fuzz input is not exercising both paths"
    );
    if let Some(i) = r.out.find("Rust panic") {
        let start = r.out.floor_char_boundary(i.saturating_sub(300));
        let end = r.out.ceil_char_boundary((i + 100).min(r.out.len()));
        panic!("engine panic:\n{}", &r.out[start..end]);
    }
}
