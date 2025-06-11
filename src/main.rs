// f2 main program
// Version 0.1

mod config;
mod engine;
mod messages;
mod files;
mod internals;

use config::{Config, DEFAULT_CORE, VERSION};

const WELCOME_MESSAGE: &str = "Welcome to f3.";
const EXIT_MESSAGE: &str = "Finished";

macro_rules! push {
    ($self:ident, $val:expr) => {
        $self.stack_ptr -= 1;
        $self.data[$self.stack_ptr] = $val;
    };
}

fn run_forth(config: &Config) {
    use engine::TF;
    fn load_file(interpreter: &mut TF, file_name: &str) {
        TF::u_set_string(
            interpreter,
            interpreter.data[interpreter.tmp_ptr] as usize,
            file_name,
        );
        push!(interpreter, interpreter.data[interpreter.tmp_ptr]);
        interpreter.f_include_file();
    }

    let mut forth = TF::new();
    forth.cold_start();
    if !config.no_core {
        for path in DEFAULT_CORE {
            load_file(&mut forth, &path);
        }
    }
    if config.loaded_file != "" {
        load_file(&mut forth, &config.loaded_file);
    }

    forth.set_abort_flag(false); // abort flag may have been set by load_file, but is no longer needed.

    println!("{WELCOME_MESSAGE} Version {VERSION}");

    // Enter the interactive loop to read and process input
    // call QUERY to start the r2 engine.
    forth.f_quit();
    // Exit when query gets a bye or EOF.
    println!("{EXIT_MESSAGE}");
}

fn main() {
    let mut config = Config::new();
    config.process_args();

    if config.run {
        run_forth(&config);
    }
}
