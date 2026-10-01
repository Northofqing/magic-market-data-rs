//! Explicitly started, serial official-publication collection journal.

use std::{
    env,
    fs::{File, OpenOptions},
    io::{BufWriter, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use magic_market_composition::{
    collect_official_publications_round, official_publication_registry,
};

const MAXIMUM_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;

struct Config {
    output: PathBuf,
    interval: Duration,
    limit: u32,
    rounds: u64,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Config>, String> {
    let mut args = args.into_iter();
    let mut output = None;
    let mut interval = 300;
    let mut limit = 5;
    let mut rounds = 0;
    while let Some(flag) = args.next() {
        if flag == "--help" {
            return Ok(None);
        }
        let value = args
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag.as_str() {
            "--output" if !value.trim().is_empty() => output = Some(PathBuf::from(value)),
            "--interval-secs" => interval = value.parse().map_err(|_| "invalid interval")?,
            "--limit" => limit = value.parse().map_err(|_| "invalid limit")?,
            "--rounds" => rounds = value.parse().map_err(|_| "invalid rounds")?,
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }
    if !(60..=86_400).contains(&interval) || !(1..=20).contains(&limit) {
        return Err("interval must be 60..86400 seconds and limit must be 1..20".into());
    }
    Ok(Some(Config {
        output: output.ok_or("--output is required")?,
        interval: Duration::from_secs(interval),
        limit,
        rounds,
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(config) = parse_args(env::args().skip(1))? else {
        println!("official-news-collector --output PATH [--interval-secs 300] [--limit 5] [--rounds 0]\n0 rounds means continuous; sleep follows each completed round; all eight admitted sources are queried; each response/failure is appended and flushed; Ctrl+C stops the process.");
        return Ok(());
    };
    // Open without truncation; do not create a scheduler or discover credentials.
    let (_journal_lease, mut journal) = open_journal(&config.output)?;
    let registry = official_publication_registry(Duration::from_secs(15), MAXIMUM_PAYLOAD_BYTES)?;
    let mut round = 1_u64;
    loop {
        let summary = collect_official_publications_round(
            &registry,
            round,
            config.limit,
            MAXIMUM_PAYLOAD_BYTES,
            &mut journal,
        )?;
        eprintln!(
            "{}",
            serde_json::json!({"round": round, "queries": summary})
        );
        if config.rounds != 0 && round >= config.rounds {
            break;
        }
        round = round
            .checked_add(1)
            .ok_or("collection round counter overflow")?;
        thread::sleep(config.interval);
    }
    Ok(())
}

fn open_journal(output: &Path) -> std::io::Result<(File, BufWriter<File>)> {
    let mut lock_path = output.as_os_str().to_os_string();
    lock_path.push(".lock");
    let lease = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    // Lock a sidecar so Windows readers can read the journal while we append.
    lease.try_lock().map_err(std::io::Error::other)?;
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(output)?;
    if file.metadata()?.len() != 0 {
        file.seek(SeekFrom::End(-1))?;
        let mut last = [0];
        file.read_exact(&mut last)?;
        if last != *b"\n" {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                "existing journal has an incomplete trailing line; preserve it and use another output path"));
        }
    }
    Ok((lease, BufWriter::new(file)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn journal_excludes_other_collectors_but_remains_readable_and_rejects_partial_tail() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let output = env::temp_dir().join(format!(
            "official-journal-{}-{suffix}.ndjson",
            std::process::id()
        ));
        let (lease, mut journal) = open_journal(&output).unwrap();
        assert!(open_journal(&output).is_err());
        journal.write_all(b"{\"fixture\":0}\n").unwrap();
        journal.flush().unwrap();
        assert_eq!(
            std::fs::read_to_string(&output).unwrap(),
            "{\"fixture\":0}\n"
        );
        drop(journal);
        drop(lease);
        std::fs::write(&output, b"{\"incomplete\":").unwrap();
        assert!(open_journal(&output).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"{\"incomplete\":");
        std::fs::remove_file(&output).unwrap();
        let mut lock_path = output.into_os_string();
        lock_path.push(".lock");
        std::fs::remove_file(lock_path).unwrap();
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn collection_requires_output_and_valid_bounds_before_opening_files() {
        assert!(parse_args(args(&[])).is_err());
        for (flag, value) in [
            ("--interval-secs", "0"),
            ("--interval-secs", "59"),
            ("--interval-secs", "86401"),
            ("--limit", "0"),
            ("--limit", "21"),
            ("--rounds", "-1"),
        ] {
            assert!(parse_args(args(&["--output", "fixture.ndjson", flag, value])).is_err());
        }
        let config = parse_args(args(&["--output", "fixture.ndjson", "--rounds", "2"]))
            .unwrap()
            .unwrap();
        assert_eq!(config.rounds, 2);
        assert_eq!(config.limit, 5);
        assert_eq!(config.interval, Duration::from_secs(300));
        assert!(parse_args(args(&["--help"])).unwrap().is_none());
    }
}
