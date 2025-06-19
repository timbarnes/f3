// system configuration and command line processing

use argh::FromArgs;

pub const VERSION: &str = "alpha.25.6.19";
pub const DEFAULT_CORE: &[&str] = &["./corelib.fs", "~/.f2/corelib.fs", "src/forth/corelib.fs"];

#[derive(FromArgs)]
/// command line arguments for f3.
pub struct Config {
    /// load a file at startup.
    #[argh(option, short = 'f')]
    pub loaded_file: Option<String>,

    /// skip loading the core files.
    #[argh(switch, short = 'n')]
    pub no_core: bool,

    /// run the interpreter.
    #[argh(switch, short = 'r')]
    pub run: bool,
}

impl Config {
    pub fn new() -> Self {
        Self {
            loaded_file: None,
            no_core: false,
            run: true,
        }
    }

    pub fn process_args(&mut self) {
        let args: Config = argh::from_env();
        self.loaded_file = args.loaded_file;
        self.no_core = args.no_core;
        self.run = args.run;
    }
}
