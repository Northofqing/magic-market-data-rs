#![forbid(unsafe_code)]

mod analysis;
mod config;
mod discovery;
mod output;
mod runtime;

use std::env;
use std::process::ExitCode;

use config::Config;
use runtime::ProductionService;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let config = match Config::parse(&arguments) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("configuration error: {error}");
            eprintln!("{}", Config::usage());
            return ExitCode::from(2);
        }
    };
    let mut service = match ProductionService::new(config) {
        Ok(service) => service,
        Err(error) => {
            eprintln!("startup error: {error}");
            return ExitCode::from(1);
        }
    };
    match service.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("service stopped: {error}");
            ExitCode::from(1)
        }
    }
}
