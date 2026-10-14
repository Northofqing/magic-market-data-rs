#![forbid(unsafe_code)]

#[cfg(not(windows))]
compile_error!("this offline CSD bridge fixture is a Windows executable");
#[cfg(not(emquant_fixture_csd))]
compile_error!("select the fixed EmQuant CSD fixture response cfg");

use std::ffi::OsStr;
use std::io::{self, Write};
use std::process::ExitCode;

// Independent native bridge shape from the existing SDK default_bridge.sh CSD sample.
// Only stdout is used; this program reads no environment, files or network and spawns no child.
const RESPONSE: &str = concat!(
    r#"{"records":[{"date":"2026/7/20","code":"600519.SH","values":{"OPEN":1290,"HIGH":1310,"LOW":1288,"CLOSE":1300,"VOLUME":100,"AMOUNT":130000}},{"date":"2026/7/21","code":"600519.SH","values":{"OPEN":1300,"HIGH":1320,"LOW":1298,"CLOSE":1310,"VOLUME":110,"AMOUNT":144100}},{"date":"2026/7/22","code":"600519.SH","values":{"OPEN":1310,"HIGH":1330,"LOW":1308,"CLOSE":1320,"VOLUME":120,"AMOUNT":158400}}]}"#,
    "\n"
);

fn main() -> ExitCode {
    let mut arguments = std::env::args_os();
    let _executable = arguments.next();
    let expected = [
        "--history",
        "csd",
        "600519.SH",
        "OPEN,HIGH,LOW,CLOSE,VOLUME,AMOUNT",
        "2026-07-20",
        "2026-07-22",
        "Period=1,AdjustFlag=1,Order=1",
    ];
    let valid = expected
        .iter()
        .all(|expected| arguments.next().as_deref() == Some(OsStr::new(expected)))
        && arguments.next().is_none();
    if !valid {
        eprintln!("offline EmQuant CSD fixture rejected arguments");
        return ExitCode::from(2);
    }
    let mut output = io::stdout().lock();
    if output
        .write_all(RESPONSE.as_bytes())
        .and_then(|()| output.flush())
        .is_err()
    {
        eprintln!("offline EmQuant CSD fixture could not write its response");
        return ExitCode::from(3);
    }
    ExitCode::SUCCESS
}
