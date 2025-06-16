// system configuration and command line processing

use ::clap::{arg, Command};
use crate::internals::messages::DebugLevel;

pub const VERSION: &str = "alpha.25.6.11";
pub const DEFAULT_CORE: [&str; 3] = ["./corelib.fs", "~/.f2/corelib.fs", "src/forth/corelib.fs"];

pub struct Config {
    pub debug_level: DebugLevel,
    pub loaded_file: String,
    pub core_file: String,
    pub no_core: bool,
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

        let arguments = Command::new("f3")
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
}
