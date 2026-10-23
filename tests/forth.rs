//! Black-box tests: run the f3 binary on a script and inspect what it prints.
//!
//! Tests marked `#[ignore = "F<n> ..."]` pin down known bugs from plan/ANALYSIS.md.
//! They assert the *correct* behaviour, so they fail today.
//! Run them with `cargo test -- --ignored`, and un-ignore each one as it is fixed.

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
    assert!(r.ok && r.out.contains("12345 "), "f3 did not recover:\n{}", r.out);
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
fn regression_suite() {
    let r = run("include src/forth/regression.fs");
    assert!(r.ok, "f3 did not exit cleanly:\n{}", r.out);
    assert!(r.out.contains("All tests passed!"), "{}", r.out);
}

// ------------------------------------------- known bugs (plan/ANALYSIS.md)

#[test]
#[ignore = "F1 execute inside a definition clobbers the return stack"]
fn f1_execute_colon_word_inside_definition() {
    assert_eq!(stack(": inc 1 + ; : t ['] inc execute 100 + ; 1 t"), "102");
}

#[test]
#[ignore = "F2 execute of a builtin xt"]
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
    assert!(r.ok && r.out.contains("dictionary full") && r.out.contains("12345 "), "{}", r.out);
}

#[test]
fn f8_runaway_recursion_is_an_error() {
    assert_survives(": deep recurse ; deep");
}

#[test]
#[ignore = "F9 number? ignores BASE"]
fn f9_number_honours_base() {
    assert_eq!(stack("hex ff decimal"), "255");
}

#[test]
#[ignore = "F10 do ... loop unfinished"]
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
#[ignore = "F25 -f file runs before the core library"]
fn f25_loaded_file_sees_core_words() {
    let path = std::env::temp_dir().join("f3_f25.fs");
    std::fs::write(&path, "hide-stack 12345 .\n").unwrap(); // hide-stack: don't let .s output match
    let r = run_args(&["-f", path.to_str().unwrap()], "bye\n");
    assert!(r.ok && r.out.contains("12345 "), "{}", r.out);
}

#[test]
#[ignore = "F26 abort\" is defined before type, so it does nothing"]
fn f26_abort_quote() {
    let r = run(": t abort\" oops\" ; 1 2 t\n.s");
    assert!(r.ok && r.out.contains("oops") && r.out.contains("[ ]"), "{}", r.out);
}

#[test]
#[ignore = "F27 negative allot grows the dictionary"]
fn f27_negative_allot() {
    assert_eq!(stack("here @ 10 allot -10 allot here @ ="), "-1");
}

#[test]
#[ignore = "F29 ' and ['] of an unknown word return 0 silently"]
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

/// Phase 1 exit check: 10k lines of random words and numbers (plus small colon definitions)
/// must never kill f3, hang it, or hit a Rust panic (an engine bug rather than a Forth error).
#[test]
fn fuzz_random_input_never_crashes() {
    const WORDS: &[&str] = &[
        "+", "-", "*", "/", "mod", "lshift", "rshift", "<", "=", "0=", "0<", "and", "or",
        "dup", "drop", "swap", "over", "rot", "pick", "roll", "depth", "clear", "@", "c@",
        ">r", "r>", "r@", "i", "j", ".", "negate", "abs", "min", "max", "1+", "1-", "2dup",
        "2drop", "nip", "tuck", ">", "<>", "0>", "?dup", "not", "nosuch",
    ];
    const NUMBERS: &[&str] = &[
        "0", "1", "-1", "2", "7", "63", "64", "65", "255", "1000", "65535", "65536",
        "9223372036854775807", "-9223372036854775808",
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
            .map(|_| if next(3) == 0 { NUMBERS[next(NUMBERS.len())] } else { WORDS[next(WORDS.len())] })
            .collect();
        if line % 10 == 0 {
            src += &format!(": fz {} ; fz\n", tokens.join(" "));
        } else {
            src += &tokens.join(" ");
            src += "\n";
        }
    }
    let r = run(&format!("{src}: alive 12345 ; alive ."));
    assert!(!r.timed_out, "f3 hung");
    assert!(r.ok && r.out.contains("12345 "), "f3 did not survive");
    let (errors, oks) = (r.out.matches("ERROR:").count(), r.out.matches("ok ").count());
    eprintln!("fuzz: {errors} errors, {oks} lines ok");
    assert!(errors > 1000 && oks > 1000, "fuzz input is not exercising both paths");
    if let Some(i) = r.out.find("Rust panic") {
        panic!("engine panic: {}", &r.out[i.saturating_sub(200)..(i + 200).min(r.out.len())]);
    }
}
