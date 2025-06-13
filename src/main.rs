// f3 main program
// Version 0.1
// 
// Boots the interpreter, loads the core and any specified files, and runs the Forth interpreter loop.
// The boot process is separated from the run process to allow for better error handling and recovery.
// Errors during boot will not start the interpreter loop, while errors during execution will 
// reset the interpreter to the prompt, clearing the data stack and return stack.


mod internals;
mod config;
mod runtime;
mod messages;
mod files;
mod kernel;

use config::{Config, DEFAULT_CORE, VERSION};
use runtime::ForthRuntime;
use std::panic::{catch_unwind, AssertUnwindSafe};

const WELCOME_MESSAGE: &str = "Welcome to f3.";
const EXIT_MESSAGE: &str = "Finished";

fn boot_forth(config: &Config) -> ForthRuntime {

    fn load_file(interpreter: &mut ForthRuntime, file_name: &str) {
        let addr = interpreter.kernel.get(interpreter.tmp_ptr) as usize;
        println!("Loading file: {}", file_name);
            interpreter.kernel.set_string(addr, file_name);
            let tmp = interpreter.kernel.get(interpreter.tmp_ptr);
            interpreter.kernel.push(tmp);
            interpreter.f_include_file();
    }   

    let mut forth = ForthRuntime::new();

    // --- Bootstrapping Phase ---
    let boot_result = catch_unwind(AssertUnwindSafe(|| {
        forth.cold_start();

        if !config.no_core {
            for path in DEFAULT_CORE {
                load_file(&mut forth, &path);
            }
        }

        if config.loaded_file != "" {
            load_file(&mut forth, &config.loaded_file);
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
                // Optionally re-raise to get Rust’s full backtrace output
                std::panic::resume_unwind(err);
            }       
        }
    }
}

fn main() {
    let mut config = Config::new();
    config.process_args();

    if config.run {
        let mut interpreter = boot_forth(&config);
        run_forth(&mut interpreter);
    }
}
