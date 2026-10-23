// f3 main program
// Version 0.1
//
// Boots the interpreter, loads the core and any specified files, and runs the Forth interpreter loop.
// The boot process is separated from the run process to allow for better error handling and recovery.
// Errors during boot will not start the interpreter loop, while errors during execution will
// reset the interpreter to the prompt, clearing the data stack and return stack.

mod internals {
    pub mod builtin;
    pub mod compiler;
    pub mod console;
    pub mod debug;
    pub mod files;
    pub mod general;
    pub mod inner;
    pub mod messages;
    pub mod terminal;
    //pub mod tui;
}
mod config;
mod kernel;
mod runtime;

use config::{Config, DEFAULT_CORE, VERSION};
use kernel::ForthError;
use runtime::ForthRuntime;
use std::panic::{catch_unwind, AssertUnwindSafe};

const WELCOME_MESSAGE: &str = "Welcome to f3.";
const EXIT_MESSAGE: &str = "Finished";

fn boot_forth(config: &Config) -> ForthRuntime {
    let mut forth = ForthRuntime::new();

    // --- Bootstrapping Phase ---
    // Each file is interpreted to the end before the next one starts. Errors in a file are
    // reported and abandon the rest of that file, like any other Forth error.
    let boot_result = catch_unwind(AssertUnwindSafe(|| {
        forth.cold_start();

        if !config.no_core {
            match DEFAULT_CORE.iter().find(|path| forth.load_file(path)) {
                Some(path) => println!("Loaded core file: {path}"),
                None => eprintln!("No core file found: tried {}", DEFAULT_CORE.join(", ")),
            }
        }

        if let Some(file) = &config.loaded_file {
            if forth.load_file(file) {
                println!("Loaded user file: {file}");
            } else {
                eprintln!("Can't open file: {file}");
            }
        }
    }));

    if boot_result.is_err() {
        eprintln!("❌ Fatal error during initialization. Aborting.");
        std::process::exit(1);
    }
    forth // Return the initialized interpreter
}

fn run_forth(forth: &mut ForthRuntime) {
    println!("{WELCOME_MESSAGE} Version {VERSION}");

    // --- Interactive Loop Phase ---
    forth.f_dot_s();
    forth.f_quit(); // main interpreter loop; recovers from errors itself
    println!("{EXIT_MESSAGE}");
}

fn main() {
    // Forth errors (ForthError) are reported by quit, so the hook stays silent for them.
    // Any other panic is an engine bug: quit still recovers, but say where it happened.
    std::panic::set_hook(Box::new(|info| {
        if info.payload().downcast_ref::<ForthError>().is_none() {
            if let Some(location) = info.location() {
                eprintln!("(Rust panic at {location})");
            }
        }
    }));

    let mut config = Config::new();
    config.process_args();

    let mut interpreter = boot_forth(&config);
    run_forth(&mut interpreter);
}
