// system configuration and command line processing

use crate::engine::TF;
use crate::messages::DebugLevel;

use ::clap::{arg, Command};

const VERSION: &str = "alpha.24.3.18";
const WELCOME_MESSAGE: &str = "Welcome to f2.";
const EXIT_MESSAGE: &str = "Finished";
const DEFAULT_CORE: [&str; 3] = ["./corelib.fs", "~/.f2/corelib.fs", "src/forth/corelib.fs"];

macro_rules! push {
    ($self:ident, $val:expr) => {
        $self.stack_ptr -= 1;
        $self.data[$self.stack_ptr] = $val;
    };
}

pub struct Config {
    debug_level: DebugLevel,
    loaded_file: String,
    core_file: String,
    no_core: bool,
    pub run: bool,
}

impl Config {
    pub fn new() -> Config {
        Config {
            debug_level: DebugLevel::Error,
            loaded_file: "".to_owned(),
            core_file: DEFAULT_CORE[0].to_owned(),
            no_core: false,
            run: true,
        }
    }

    /// process_args handles command line argument processing using the clap library
    ///
    pub fn process_args(&mut self) -> &Config {
        // process arguments
        // let msg = Msg::new(); // Create a message handler for argument errors

        let arguments = Command::new("tForth")
            .version(VERSION)
            .author("Tim Barnes")
            .about("A simple Forth interpreter")
            .arg(
                arg!(--debuglevel <VALUE>)
                    .required(false)
                    .value_parser(["error", "warning", "info", "debug"]),
            )
            .arg(arg!(-l --library <VALUE>).required(false))
            .arg(arg!(-f --file <VALUE>).required(false))
            .arg(arg!(-n - -nocore).required(false))
            .get_matches();

        let debuglevel = arguments.get_one::<String>("debuglevel");
        if let Some(debuglevel) = debuglevel {
            match debuglevel.as_str() {
                "debug" => self.debug_level = DebugLevel::Debug,
                "info" => self.debug_level = DebugLevel::Info,
                "warning" => self.debug_level = DebugLevel::Warning,
                _ => self.debug_level = DebugLevel::Warning,
            }
        }

        let library = arguments.get_one::<String>("library");
        if let Some(lib) = library {
            self.core_file = lib.to_string();
        }

        let nocore = arguments.get_one::<bool>("nocore");
        if let Some(nc) = nocore {
            self.no_core = *nc;
        }

        let file = arguments.get_one::<String>("file");
        if let Some(file) = file {
            self.loaded_file = file.clone();
        }
        self
    }

    /// run_forth is the main entry point that performs the cold start operations, loads library files,
    ///     and hands off control to the main interpreter loop
    ///
    pub fn run_forth(&mut self) {
        // create and run the interpreter
        // return when finished
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
        if !self.no_core {
            for path in DEFAULT_CORE {
                load_file(&mut forth, &path);
            }
        }
        if self.loaded_file != "" {
            load_file(&mut forth, &self.loaded_file);
        }

        forth.set_abort_flag(false); // abort flag may have been set by load_file, but is no longer needed.

        println!("{WELCOME_MESSAGE} Version {VERSION}");

        // Enter the interactive loop to read and process input
        // call QUERY to start the r2 engine.
        forth.f_quit();
        // Exit when query gets a bye or EOF.
        println!("{EXIT_MESSAGE}");
    }
}
