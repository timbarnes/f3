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
mod runtime;
mod kernel;

use config::{Config, DEFAULT_CORE, VERSION};
use runtime::ForthRuntime;
use kernel::STACK_START;
use std::panic::{catch_unwind, AssertUnwindSafe};

const WELCOME_MESSAGE: &str = "Welcome to f3.";
const EXIT_MESSAGE: &str = "Finished";

fn boot_forth(config: &Config) -> ForthRuntime {

    fn load_file(interpreter: &mut ForthRuntime, file_name: &str) {
        let addr = interpreter.kernel.get(interpreter.tmp_ptr) as usize;
        println!("Loading file: {}", file_name);
        println!("DEBUG: stack_ptr before loading {}: {}", file_name, interpreter.kernel.stack_ptr);
        interpreter.kernel.string_set(addr, file_name);
        let tmp = interpreter.kernel.get(interpreter.tmp_ptr);
        interpreter.kernel.push(tmp);
        println!("DEBUG: stack_ptr after pushing tmp: {}", interpreter.kernel.stack_ptr);
        interpreter.f_include_file();
        println!("DEBUG: stack_ptr after f_include_file: {}", interpreter.kernel.stack_ptr);
        // Don't assert here as the stack might legitimately have content from the file
    }   

    let mut forth = ForthRuntime::new();

    // --- Bootstrapping Phase ---
    let boot_result = catch_unwind(AssertUnwindSafe(|| {
        forth.cold_start();

        if !config.no_core {
            for path in DEFAULT_CORE {
                load_file(&mut forth, &path);
                let result = forth.kernel.pop();
                println!("DEBUG: After popping result for {}, stack_ptr: {}", path, forth.kernel.stack_ptr);
                if result != 0 {
                    println!("Loaded core file: {}", path);
                } else {
                    println!("Failed to load core file: {}", path);
                }
            }
        }

        if let Some(file) = &config.loaded_file {
            load_file(&mut forth, file);
            let result = forth.kernel.pop();
            println!("DEBUG: After popping result for {}, stack_ptr: {}", file, forth.kernel.stack_ptr);
            if result != 0 {
                println!("Loaded user file: {}", file);
            } else {
                println!("Failed to load user file: {}", file);
            }
        }
        
        // Assert that stack pointer is correct after file loading
        assert_eq!(forth.kernel.stack_ptr, STACK_START, "Stack pointer should be {} after file loading, but is {}", STACK_START, forth.kernel.stack_ptr);
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
    loop {
        let result = catch_unwind(AssertUnwindSafe(|| {   
            forth.set_abort_flag(false);
            println!("Entering f_quit");
            forth.f_quit();  // main interpreter loop
        }));

        match result {
            Ok(_) => {
                println!("{EXIT_MESSAGE}");
                break;
            }
            Err(err) => {
            eprintln!("⚠️  Error during execution. Resetting interpreter to prompt.");

                if let Some(msg) = err.downcast_ref::<&str>() {
                    eprintln!("panic message: {}", msg);
                } else if let Some(msg) = err.downcast_ref::<String>() {
                    eprintln!("panic message: {}", msg);
                } else {
                    eprintln!("panic payload is not a string.");
                }

                // Print the backtrace if RUST_BACKTRACE is set
                // Optionally re-raise to get Rust's full backtrace output
                std::panic::resume_unwind(err);
            }       
        }
    }
}

fn main() {
    let mut config = Config::new();
    config.process_args();

    let mut interpreter = boot_forth(&config);
    run_forth(&mut interpreter);
}
