// f3 main program
// Version 0.1
// 
// Boots the interpreter, loads the core and any specified files, and runs the Forth interpreter loop.
// The boot process is separated from the run process to allow for better error handling and recovery.
// Errors during boot will not start the interpreter loop, while errors during execution will 
// reset the interpreter to the prompt, clearing the data stack and return stack.


mod config;
mod kernel;
mod messages;
mod files;
mod internals;

use config::{Config, DEFAULT_CORE, VERSION};
use kernel::TF;
use std::panic::{catch_unwind, AssertUnwindSafe};

const WELCOME_MESSAGE: &str = "Welcome to f3.";
const EXIT_MESSAGE: &str = "Finished";

fn boot_forth(config: &Config) -> TF {

    fn load_file(interpreter: &mut TF, file_name: &str) {
        println!("Loading file: {}", file_name);
            TF::u_set_string(
                interpreter,
                interpreter.heap[interpreter.tmp_ptr] as usize,
                file_name,
            );
            interpreter.push(interpreter.heap[interpreter.tmp_ptr]);
            interpreter.f_include_file();
    }   

    let mut forth = TF::new();

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

fn run_forth(forth: &mut TF) {
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
            Err(_) => {
                eprintln!("⚠️  Error during execution. Resetting interpreter to prompt.");
                forth.f_abort(); // You implement this in TF
                forth.set_abort_flag(false);
                // Optionally: clear input buffer, set diagnostic flags, etc.
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
