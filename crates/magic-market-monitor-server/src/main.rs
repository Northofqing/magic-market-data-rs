#![forbid(unsafe_code)]

mod analysis;
mod config;
mod discovery;
mod logging;
mod output;
mod runtime;

use std::env;
use std::process::ExitCode;

use config::Config;
use logging::Level;
use runtime::ProductionService;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let config = match Config::parse(&arguments) {
        Ok(config) => config,
        Err(error) => {
            logging::event(
                Level::Error,
                "tdx_monitor",
                "configuration_failed",
                format_args!("detail={error}"),
            );
            eprintln!("{}", Config::usage());
            return ExitCode::from(2);
        }
    };
    let mut service = match ProductionService::new(config) {
        Ok(service) => service,
        Err(error) => {
            logging::event(
                Level::Error,
                "tdx_monitor",
                "startup_failed",
                format_args!("detail={error}"),
            );
            return ExitCode::from(1);
        }
    };
    match service.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            logging::event(
                Level::Error,
                "tdx_monitor",
                "service_stopped",
                format_args!("detail={error}"),
            );
            ExitCode::from(1)
        }
    }
}
