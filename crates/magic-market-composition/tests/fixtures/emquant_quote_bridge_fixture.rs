#![forbid(unsafe_code)]

#[cfg(not(windows))]
compile_error!("this offline bridge fixture is a Windows executable");
#[cfg(any(
    all(emquant_fixture_complete, emquant_fixture_short),
    not(any(emquant_fixture_complete, emquant_fixture_short))
))]
compile_error!("select exactly one fixed EmQuant fixture response cfg");

use std::ffi::OsStr;
use std::io::{self, Write};
use std::process::ExitCode;

#[cfg(emquant_fixture_complete)]
const RESPONSE: &str = concat!(
    r#"{"records":[{"date":"2026-07-22","code":"000001.SZ","values":{"TIME":"10:00:01","NAME":"平安银行","NOW":12.5,"PRECLOSE":12.2,"OPEN":12.3,"HIGH":12.6,"LOW":12.1,"PCTCHANGE":2.459,"VOLUME":200,"AMOUNT":2500}},{"date":"2026-07-22","code":"600519.SH","values":{"TIME":"10:00:00","NAME":"贵州茅台","NOW":1300,"PRECLOSE":1290,"OPEN":1295,"HIGH":1305,"LOW":1288,"PCTCHANGE":0.7752,"VOLUME":100,"AMOUNT":130000}}]}"#,
    "\n"
);
#[cfg(emquant_fixture_short)]
const RESPONSE: &str = concat!(
    r#"{"records":[{"date":"2026-07-22","code":"600519.SH","values":{"TIME":"10:00:00","NAME":"贵州茅台","NOW":1300,"PRECLOSE":1290,"OPEN":1295,"HIGH":1305,"LOW":1288,"PCTCHANGE":0.7752,"VOLUME":100,"AMOUNT":130000}}]}"#,
    "\n"
);

fn main() -> ExitCode {
    let mut arguments = std::env::args_os();
    let _executable = arguments.next();
    let valid = arguments.next().as_deref() == Some(OsStr::new("600519.SH,000001.SZ"))
        && arguments.next().as_deref()
            == Some(OsStr::new(
                "TIME,NAME,NOW,PRECLOSE,OPEN,HIGH,LOW,PCTCHANGE,VOLUME,AMOUNT",
            ))
        && arguments.next().is_none();
    if !valid {
        eprintln!("offline EmQuant snapshot fixture rejected arguments");
        return ExitCode::from(2);
    }
    let mut output = io::stdout().lock();
    if output
        .write_all(RESPONSE.as_bytes())
        .and_then(|()| output.flush())
        .is_err()
    {
        eprintln!("offline EmQuant snapshot fixture could not write its response");
        return ExitCode::from(3);
    }
    ExitCode::SUCCESS
}
