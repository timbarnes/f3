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
#[ignore = "F3 error inside : ; installs a truncated word"]
fn f3_error_inside_definition_does_not_install_word() {
    assert_eq!(stack(": bad 1 nosuch 2 ;\nbad"), "");
}

#[test]
#[ignore = "F4 compile state survives an error"]
fn f4_error_resets_compile_state() {
    assert_eq!(stack(": x nosuch\nstate @"), "0");
}

#[test]
#[ignore = "F5 EOF on stdin loops forever"]
fn f5_stdin_eof_exits() {
    let r = run_raw("1 .\n");
    assert!(!r.timed_out && r.ok, "{}", r.out);
}

#[test]
#[ignore = "F6 input lines truncated at 131 bytes"]
fn f6_long_lines_not_truncated() {
    let line = format!("0{}", " 1 +".repeat(40)); // 161 chars
    assert_eq!(stack(&line), "40");
}

#[test]
#[ignore = "F7 dictionary overflow is unchecked"]
fn f7_dictionary_overflow_is_an_error() {
    assert_survives(": fill 1000000 for 1 , next ; fill");
}

#[test]
#[ignore = "F8 return stack overflow is unchecked"]
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
#[ignore = "F14 non-ASCII input can split a UTF-8 char"]
fn f14_non_ascii_input() {
    assert_survives(&format!("{}é", "a".repeat(130)));
}

#[test]
#[ignore = "F24 clear also resets the return stack"]
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
#[ignore = "Phase 1: any panic kills the process"]
fn stack_underflow_is_recoverable() {
    assert_survives("drop");
}

#[test]
#[ignore = "Phase 1: any panic kills the process"]
fn divide_by_zero_is_recoverable() {
    assert_survives("1 0 /");
}

#[test]
#[ignore = "Phase 1: arithmetic should wrap, not panic"]
fn arithmetic_wraps() {
    assert_eq!(stack("9223372036854775807 1 +"), "-9223372036854775808");
}
