// f2 main program
// Version 0.1

mod config;
mod engine;
mod messages;
mod files;
//mod tokenizer;
mod internals;

use config::Config;

fn main() {
    let mut config = Config::new();
    config.process_args();

    if config.run {
        config.run_forth();
    }
}
