use super::super::register_emquant_client;
use magic_emquant_rs::EmQuantClient;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn registered_emquant_quotes_reject_unverified_beijing_suffix_before_bridge_execution() {
    // A real existing executable satisfies the concrete client's path constructor.
    // It is NOT a bridge response fixture and must never be executed by this request.
    // If the request unexpectedly reaches spawn, the owned test must fail; no SDK install,
    // credentials, environment mutation, shell script or network is used here.
    let bridge = std::env::current_exe().expect("public test executable path");
    let client = EmQuantClient::new(bridge)
        .expect("real concrete client for a regular executable")
        .with_timeout(Duration::from_secs(1))
        .expect("positive concrete-client timeout");
    let mut registry = OperationRegistry::all_unadmitted("offline EmQuant public query test");
    register_emquant_client(&mut registry, Arc::new(client), 16_384)
        .expect("actual production EmQuant registrations");

    let capability = registry
        .capabilities()
        .into_iter()
        .find(|value| value.operation == Operation::RealtimeQuotes && value.provider == "EmQuant")
        .expect("EmQuant quote capability");
    assert!(!capability.repository_admitted);
    assert!(!capability.runtime_available);
    assert!(capability.diagnostic_available);
    assert_eq!(
        capability.exact_scope,
        "runtime-entitled EMQuant quote snapshot diagnostic"
    );
    assert_eq!(
        capability.blocker.as_deref(),
        Some(
            "EMQuant bridge availability and product entitlement do not constitute repository admission"
        )
    );

    // Both instruments are legal public Core identifiers. The distinct Shanghai
    // prefix prevents an empty/duplicate/invalid-envelope error from explaining
    // the expected late Beijing source-suffix refusal.
    let payload = CanonicalPayload::new(
        "magic.market.realtime_quotes.request",
        1,
        br#"{"instruments":[{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},{"exchange":"Beijing","code":"430047","asset_class":"Equity"}]}"#
            .to_vec(),
        16_384,
    )
    .expect("valid JSON/schema/version request control");
    let command = QueryCommand::new(
        "emquant-beijing-suffix-scope",
        Operation::RealtimeQuotes,
        Some("EmQuant".to_owned()),
        payload,
    )
    .expect("public query command");

    // Diagnostic access is still opt-in; available client setup does not promote
    // this capability to repository admission or normal successful market data.
    assert_eq!(
        registry
            .execute(command.clone())
            .expect_err("ordinary query must retain its admission blocker"),
        ServiceError::Unsupported {
            operation: Operation::RealtimeQuotes,
            reason: "EMQuant bridge availability and product entitlement do not constitute repository admission"
                .to_owned(),
        }
    );
    assert_eq!(
        registry
            .execute(command.with_unadmitted_access(true))
            .expect_err("unverified Beijing suffix must not return a quote prefix"),
        ServiceError::Unsupported {
            operation: Operation::RealtimeQuotes,
            reason: "beijing exchange code suffix is not verified by the bundled SDK documentation"
                .to_owned(),
        }
    );
}

#[cfg(windows)]
mod windows_raw_bridge_public_contract {
    use super::register_emquant_client;
    use magic_emquant_rs::EmQuantClient;
    use magic_market_service::{
        CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
    };
    use std::ffi::{OsStr, OsString};
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::windows::fs::MetadataExt;
    use std::path::{Component, PathBuf, Prefix};
    use std::process::{Child, Command, ExitStatus, Stdio};
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const WORKER_KEY: &str = "MAGIC_EMQUANT_QUOTE_TEST_WORKER";
    const WORKER_VALUE: &str = "fixed-raw-quotes-v1";
    const EXACT_TEST: &str = "grpc_production::coverage_tests::emquant_quotes_scope_public_contract::windows_raw_bridge_public_contract::registered_emquant_quotes_preserve_order_and_refuse_short_raw_batch";
    const OUTPUT_CAP: u64 = 65_536;
    const PUBLIC_DEVELOPMENT_KEYS: &[&str] = &[
        "SYSTEMROOT",
        "WINDIR",
        "PATH",
        "TEMP",
        "TMP",
        "TMPDIR",
        "INCLUDE",
        "LIB",
        "LIBPATH",
        "PROGRAMFILES(X86)",
        "PROGRAMDATA",
        "RUSTC",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "LLVM_PROFILE_FILE",
    ];

    struct FreshDirectory {
        base: PathBuf,
        path: PathBuf,
        removed: bool,
    }

    impl FreshDirectory {
        fn new() -> Result<Self, String> {
            let base = std::env::temp_dir()
                .canonicalize()
                .map_err(|error| error.to_string())?;
            if !matches!(base.components().next(), Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
            {
                return Err(
                    "public fixture requires a local Windows temporary directory".to_owned(),
                );
            }
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            for suffix in 0..32 {
                let path = base.join(format!(
                    "magic-eq-{:x}-{stamp:x}-{suffix:x}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => {
                        return Ok(Self {
                            base,
                            path,
                            removed: false,
                        })
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
            Err("could not create an exclusive public fixture directory".to_owned())
        }

        fn remove(&mut self) -> Result<(), String> {
            if self.removed {
                return Ok(());
            }
            if self.path.parent() != Some(self.base.as_path())
                || !self.path.is_absolute()
                || !self
                    .path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("magic-eq-"))
            {
                return Err("public fixture cleanup scope mismatch".to_owned());
            }
            let mut pending = vec![self.path.clone()];
            while let Some(path) = pending.pop() {
                let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err("public fixture cleanup refuses reparse entries".to_owned());
                }
                if metadata.is_dir() {
                    for entry in fs::read_dir(&path).map_err(|error| error.to_string())? {
                        pending.push(entry.map_err(|error| error.to_string())?.path());
                    }
                } else if !metadata.is_file() {
                    return Err("public fixture cleanup refuses nonregular entries".to_owned());
                }
            }
            fs::remove_dir_all(&self.path).map_err(|error| error.to_string())?;
            self.removed = true;
            Ok(())
        }
    }

    impl Drop for FreshDirectory {
        fn drop(&mut self) {
            if let Err(error) = self.remove() {
                if std::thread::panicking() {
                    eprintln!("public EmQuant fixture cleanup failed: {error}");
                } else {
                    panic!("public EmQuant fixture cleanup failed: {error}");
                }
            }
        }
    }

    struct DirectChild(Option<Child>);

    impl DirectChild {
        fn finish_or_stop(&mut self) -> Result<ExitStatus, String> {
            let child = self
                .0
                .as_mut()
                .ok_or("direct fixture child already closed")?;
            if child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_none()
            {
                child.kill().map_err(|error| error.to_string())?;
            }
            let status = child.wait().map_err(|error| error.to_string())?;
            self.0 = None;
            Ok(status)
        }
    }

    impl Drop for DirectChild {
        fn drop(&mut self) {
            if self.0.is_some() {
                if let Err(error) = self.finish_or_stop() {
                    if std::thread::panicking() {
                        eprintln!("public EmQuant direct-child cleanup failed: {error}");
                    } else {
                        panic!("public EmQuant direct-child cleanup failed: {error}");
                    }
                }
            }
        }
    }

    struct CapturedProcess {
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    }

    fn public_command(executable: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(executable);
        command.env_clear().stdin(Stdio::null());
        for name in PUBLIC_DEVELOPMENT_KEYS {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
    }

    fn read_complete_output(file: &mut File) -> Result<Vec<u8>, String> {
        file.seek(SeekFrom::Start(0))
            .map_err(|error| error.to_string())?;
        let mut bytes = Vec::new();
        (&mut *file)
            .take(OUTPUT_CAP + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > OUTPUT_CAP {
            return Err("public fixture output exceeded its acceptance cap".to_owned());
        }
        if file.metadata().map_err(|error| error.to_string())?.len() != bytes.len() as u64 {
            return Err("public fixture output changed during complete capture".to_owned());
        }
        Ok(bytes)
    }

    fn run_bounded(
        mut command: Command,
        directory: &FreshDirectory,
        label: &str,
        budget: Duration,
    ) -> Result<CapturedProcess, String> {
        let new_log = |suffix: &str| {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(directory.path.join(format!("{label}.{suffix}")))
                .map_err(|error| error.to_string())
        };
        let mut stdout = new_log("stdout")?;
        let mut stderr = new_log("stderr")?;
        command
            .current_dir(&directory.path)
            .env("TEMP", &directory.path)
            .env("TMP", &directory.path)
            .env("TMPDIR", &directory.path)
            .stdout(Stdio::from(
                stdout.try_clone().map_err(|error| error.to_string())?,
            ))
            .stderr(Stdio::from(
                stderr.try_clone().map_err(|error| error.to_string())?,
            ));
        let started = Instant::now();
        let mut owned = DirectChild(Some(command.spawn().map_err(|error| error.to_string())?));
        let observation = loop {
            let size = stdout
                .metadata()
                .and_then(|left| stderr.metadata().map(|right| (left.len(), right.len())));
            match size {
                Ok((left, right)) if left <= OUTPUT_CAP && right <= OUTPUT_CAP => {}
                Ok(_) => break Err("public fixture sampled output cap exceeded".to_owned()),
                Err(error) => break Err(error.to_string()),
            }
            if started.elapsed() >= budget {
                break Err("public fixture child deadline exceeded".to_owned());
            }
            match owned.0.as_mut().expect("live direct child").try_wait() {
                Ok(Some(_)) => break Ok(()),
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => break Err(error.to_string()),
            }
        };
        // Also performed before returning any cap/deadline/metadata error. This
        // closes the direct child only, NOT all compiler/linker descendants.
        let status = owned.finish_or_stop()?;
        observation?;
        Ok(CapturedProcess {
            status,
            stdout: read_complete_output(&mut stdout)?,
            stderr: read_complete_output(&mut stderr)?,
        })
    }

    fn require_success(output: CapturedProcess, label: &str) -> Vec<u8> {
        assert!(
            output.status.success(),
            "{label} failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "{label} unexpected stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

    fn assert_worker_environment() {
        // Refuse a manually set worker marker that still carries a provider
        // credential/override or other non-allowlisted environment into spawn.
        for (name, value) in std::env::vars_os() {
            let name = name.to_string_lossy().to_ascii_uppercase();
            // Windows LLVM adds this exact public initialization marker inside
            // instrumented workers. It is never forwarded by public_command.
            let llvm_runtime_marker = name == "__LLVM_PROFILE_RT_INIT_ONCE"
                && value == OsStr::new("__LLVM_PROFILE_RT_INIT_ONCE");
            assert!(
                name == WORKER_KEY
                    || PUBLIC_DEVELOPMENT_KEYS.contains(&name.as_str())
                    || llvm_runtime_marker,
                "isolated worker inherited a non-allowlisted environment key"
            );
        }
    }

    fn compile_bridge_fixtures(
        directory: &FreshDirectory,
        source_text: &str,
        variants: &[(&str, &str)],
    ) {
        let source = directory.path.join("emquant_quote_bridge_fixture.rs");
        let mut source_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&source)
            .expect("create fixed standalone fixture source");
        source_file
            .write_all(source_text.as_bytes())
            .expect("write exact compiled-in public fixture source");
        drop(source_file);
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
        assert!(
            !rustc.is_empty(),
            "RUSTC must name the installed native Rust compiler"
        );
        let build_started = Instant::now();
        // Only this standalone fixture uses the compiler's bundled COFF linker.
        // The workspace's MSVC linker/profile and the business oracle stay unchanged.
        let mut query = public_command(&rustc);
        query.args(["--print", "target-libdir"]);
        let library_output = require_success(
            run_bounded(query, directory, "target-library", Duration::from_secs(30))
                .expect("bounded installed compiler library query"),
            "installed compiler library query",
        );
        let library_text =
            String::from_utf8(library_output).expect("installed compiler library path is UTF-8");
        let library_text = library_text.trim_end_matches(['\r', '\n']);
        assert!(
            !library_text.is_empty() && !library_text.contains(['\r', '\n']),
            "installed compiler must return exactly one library path"
        );
        let library = PathBuf::from(library_text);
        assert!(
            library.is_absolute() && library.file_name() == Some(OsStr::new("lib")),
            "installed compiler target library shape"
        );
        let linker = library
            .parent()
            .expect("target library parent")
            .join("bin")
            .join("rust-lld.exe");
        for entry in linker.ancestors() {
            let metadata = fs::symlink_metadata(entry).expect("bundled linker path metadata");
            assert_eq!(
                metadata.file_attributes() & 0x400,
                0,
                "bundled fixture linker refuses reparse entries"
            );
        }
        let metadata = fs::metadata(&linker).expect("installed bundled fixture linker");
        assert!(
            metadata.is_file() && metadata.len() > 0,
            "installed bundled fixture linker must be a nonempty file"
        );
        let mut linker_option = OsString::from("linker=");
        linker_option.push(linker.as_os_str());
        for (name, cfg) in variants.iter().copied() {
            let remaining = Duration::from_secs(120)
                .checked_sub(build_started.elapsed())
                .expect("fixture compiler total deadline");
            assert!(!remaining.is_zero(), "fixture compiler total deadline");
            let mut compiler = public_command(&rustc);
            compiler
                .args([
                    "--verbose",
                    "--edition=2021",
                    "--crate-type=bin",
                    "--crate-name=eq",
                    "-C",
                    "debuginfo=0",
                    "--cfg",
                    cfg,
                ])
                .arg("-C")
                .arg(&linker_option)
                .args(["-C", "linker-flavor=lld-link"])
                .arg(&source)
                .arg("-o")
                .arg(directory.path.join(format!("{name}.exe")));
            let output = run_bounded(
                compiler,
                directory,
                name,
                remaining.min(Duration::from_secs(60)),
            )
            .expect("bounded standalone rustc");
            assert!(
                require_success(output, "standalone fixture compiler").is_empty(),
                "standalone fixture compiler unexpected stdout"
            );
        }
    }

    fn exercise_public_query() {
        assert_worker_environment();
        let mut directory = FreshDirectory::new().expect("exclusive public build directory");
        compile_bridge_fixtures(
            &directory,
            include_str!("../fixtures/emquant_quote_bridge_fixture.rs"),
            &[
                ("complete", "emquant_fixture_complete"),
                ("short", "emquant_fixture_short"),
            ],
        );

        let payload = CanonicalPayload::new("magic.market.realtime_quotes.request", 1,
            br#"{"instruments":[{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},{"exchange":"Shenzhen","code":"000001","asset_class":"Equity"}]}"#.to_vec(),
            16_384).expect("legal public two-instrument request");
        let command = QueryCommand::new(
            "emquant-offline-quote-cardinality",
            Operation::RealtimeQuotes,
            Some("EmQuant".to_owned()),
            payload,
        )
        .expect("public query command")
        .with_unadmitted_access(true);
        let make_registry = |name: &str| {
            let client = EmQuantClient::new(directory.path.join(format!("{name}.exe")))
                .expect("fresh actual standalone bridge executable")
                .with_timeout(Duration::from_secs(5))
                .expect("positive bridge timeout");
            let mut registry = OperationRegistry::all_unadmitted("offline EmQuant public fixture");
            register_emquant_client(&mut registry, Arc::new(client), 16_384)
                .expect("real production EmQuant registrations");
            registry
        };
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let result = make_registry("complete")
            .execute(command.clone())
            .expect("complete raw bridge response through public Execute");
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let observed = result
            .observed_at
            .parse::<u64>()
            .expect("Unix-seconds observation");
        assert_eq!(
            result.observed_at,
            observed.to_string(),
            "canonical seconds, not source time"
        );
        assert!(
            before <= observed && observed <= after,
            "independent observation bracket"
        );
        let batch = format!("eastmoney:{observed}:quote");
        assert_eq!(result.provider, "EmQuant");
        assert_eq!(result.batch_id, batch);
        assert_eq!(result.source_at.as_deref(), Some("2026-07-22 10:00:00"));
        assert!(!result.complete);
        assert!(!result.repository_admitted);
        assert_eq!(result.diagnostic_blocker.as_deref(), Some(
            "EMQuant bridge availability and product entitlement do not constitute repository admission"
        ));
        assert_eq!(result.records.len(), 2);
        for record in &result.records {
            assert_eq!(record.schema(), "magic.market.quote");
            assert_eq!(record.schema_version(), 1);
        }
        let wire: Vec<serde_json::Value> = result
            .records
            .iter()
            .map(|record| {
                serde_json::from_slice(record.data()).expect("returned canonical quote JSON")
            })
            .collect();
        assert_eq!(
            wire,
            vec![
                serde_json::json!({
                    "instrument": {"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
                    "name":"贵州茅台", "price":1300.0, "previous_close":1290.0,
                    "open":1295.0, "high":1305.0, "low":1288.0,
                    "change_percent":{"value":0.7752,"unit":"Percent"},
                    "volume":100.0, "amount":130000.0, "status":"Available",
                    "source_at":"2026-07-22 10:00:00", "observed_at":observed.to_string(),
                    "provider":"Eastmoney", "batch_id":batch
                }),
                serde_json::json!({
                    "instrument": {"exchange":"Shenzhen","code":"000001","asset_class":"Equity"},
                    "name":"平安银行", "price":12.5, "previous_close":12.2,
                    "open":12.3, "high":12.6, "low":12.1,
                    "change_percent":{"value":2.459,"unit":"Percent"},
                    "volume":200.0, "amount":2500.0, "status":"Available",
                    "source_at":"2026-07-22 10:00:01", "observed_at":observed.to_string(),
                    "provider":"Eastmoney", "batch_id":batch
                }),
            ]
        );
        assert_eq!(
            make_registry("short")
                .execute(command)
                .expect_err("short raw response must not publish a successful prefix"),
            ServiceError::FailedPrecondition(
                "invalid EMQuant response: quote cardinality mismatch: requested 2, received 1"
                    .to_owned()
            )
        );
        directory
            .remove()
            .expect("checked public build-directory cleanup");
    }

    fn run_isolated_worker(exact_test: &str, worker_value: &str) {
        let mut directory = FreshDirectory::new().expect("exclusive public worker-log directory");
        // current_exe is a libtest WORKER only, never an EmQuant bridge.
        let mut worker = public_command(std::env::current_exe().expect("current libtest worker"));
        worker
            .args(["--exact", exact_test, "--test-threads=1", "--nocapture"])
            .env(WORKER_KEY, worker_value);
        let output = run_bounded(worker, &directory, "worker", Duration::from_secs(180))
            .expect("bounded isolated public-query worker");
        let stdout = require_success(output, "isolated public-query worker");
        let stdout = std::str::from_utf8(&stdout).expect("complete public worker stdout UTF8");
        let summaries: Vec<_> = stdout
            .lines()
            .filter(|line| line.starts_with("test result:"))
            .collect();
        assert_eq!(summaries.len(), 1, "exactly one worker summary");
        assert!(
            summaries[0].starts_with("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;"),
            "worker must report one actual passing test"
        );
        let passed_line = format!("test {exact_test} ... ok");
        assert_eq!(
            stdout
                .lines()
                .filter(|line| { *line == passed_line.as_str() })
                .count(),
            1,
            "the exact selected worker test must pass"
        );
        // Do not echo the worker's summary into the top-level test stream.
        directory
            .remove()
            .expect("checked public worker-log cleanup");
    }

    #[test]
    fn registered_emquant_quotes_preserve_order_and_refuse_short_raw_batch() {
        if std::env::var_os(WORKER_KEY).as_deref() == Some(OsStr::new(WORKER_VALUE)) {
            exercise_public_query();
            return;
        }
        run_isolated_worker(EXACT_TEST, WORKER_VALUE);
    }

    fn exercise_admitted_daily_bars_query() {
        assert_worker_environment();
        let mut directory = FreshDirectory::new().expect("exclusive public CSD build directory");
        compile_bridge_fixtures(
            &directory,
            include_str!("../fixtures/emquant_csd_bridge_fixture.rs"),
            &[("csd", "emquant_fixture_csd")],
        );
        let client = EmQuantClient::new(directory.path.join("csd.exe"))
            .expect("fresh actual standalone CSD bridge executable")
            .with_timeout(Duration::from_secs(5))
            .expect("positive bridge timeout");
        let mut registry = OperationRegistry::all_unadmitted("offline EmQuant public CSD fixture");
        register_emquant_client(&mut registry, Arc::new(client), 16_384)
            .expect("real production EmQuant registrations");
        let payload = CanonicalPayload::new(
            "magic.market.historical_bars.request",
            1,
            br#"{"instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},"interval":"Day","start":"2026-07-20","end":"2026-07-22","limit":2}"#.to_vec(),
            16_384,
        )
        .expect("legal public bounded daily request");
        // 2026-10-15: the repository admission was withdrawn (expired
        // entitlement), so the public route must now refuse without the
        // unadmitted opt-in: fail-closed first, diagnostic execution second.
        let refused = registry
            .execute(
                QueryCommand::new(
                    "emquant-offline-csd-public-refusal",
                    Operation::HistoricalBars,
                    Some("EmQuant".to_owned()),
                    payload.clone(),
                )
                .expect("public query command without diagnostic opt-in"),
            )
            .expect_err("withdrawn admission must refuse public execution");
        match refused {
            magic_market_service::ServiceError::Unsupported { reason, .. } => {
                assert!(
                    reason.contains("withdrawn"),
                    "refusal must carry the withdrawal blocker: {reason}"
                );
            }
            other => panic!("expected fail-closed Unsupported refusal, got {other:?}"),
        }
        let command = QueryCommand::new(
            "emquant-offline-csd-bounded-suffix",
            Operation::HistoricalBars,
            Some("EmQuant".to_owned()),
            payload,
        )
        .expect("public query command without diagnostic opt-in")
        .with_unadmitted_access(true);
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let result = registry
            .execute(command)
            .expect("diagnostic CSD raw response through public Execute");
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let observed = result
            .observed_at
            .parse::<u64>()
            .expect("Unix-seconds observation");
        assert_eq!(
            result.observed_at,
            observed.to_string(),
            "canonical seconds"
        );
        assert!(
            before <= observed && observed <= after,
            "independent observation bracket"
        );
        let batch = format!("eastmoney:{observed}:bars");
        assert_eq!(result.provider, "EmQuant");
        assert_eq!(result.batch_id, batch);
        assert_eq!(result.source_at.as_deref(), Some("2026-07-22"));
        // Unadmitted diagnostic execution keeps the evidence but must not
        // claim completeness or admission.
        assert!(!result.complete);
        assert!(!result.repository_admitted);
        assert!(result
            .diagnostic_blocker
            .as_deref()
            .is_some_and(|blocker| blocker.contains("withdrawn")));
        assert_eq!(result.records.len(), 2);
        for record in &result.records {
            assert_eq!(record.schema(), "magic.market.bar");
            assert_eq!(record.schema_version(), 1);
        }
        let wire: Vec<serde_json::Value> = result
            .records
            .iter()
            .map(|record| {
                serde_json::from_slice(record.data()).expect("returned canonical bar JSON")
            })
            .collect();
        // These complete literal rows are independent of the SDK/Core constructors.
        // The three source days retain their order; limit=2 keeps the latest suffix.
        assert_eq!(
            wire,
            vec![
                serde_json::json!({
                    "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
                    "interval":"Day", "bar_start":"2026-07-21", "bar_end":"2026-07-21",
                    "open":1300.0, "high":1320.0, "low":1298.0, "close":1310.0,
                    "volume":110.0, "amount":144100.0, "adjustment":"Unadjusted",
                    "source_at":"2026-07-21", "observed_at":observed.to_string(),
                    "provider":"Eastmoney", "batch_id":batch
                }),
                serde_json::json!({
                    "instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
                    "interval":"Day", "bar_start":"2026-07-22", "bar_end":"2026-07-22",
                    "open":1310.0, "high":1330.0, "low":1308.0, "close":1320.0,
                    "volume":120.0, "amount":158400.0, "adjustment":"Unadjusted",
                    "source_at":"2026-07-22", "observed_at":observed.to_string(),
                    "provider":"Eastmoney", "batch_id":batch
                }),
            ]
        );
        directory
            .remove()
            .expect("checked public CSD build-directory cleanup");
    }

    #[test]
    fn registered_emquant_daily_bars_preserve_ordered_bounded_suffix_and_source_evidence() {
        const CSD_WORKER_VALUE: &str = "fixed-raw-csd-v1";
        const CSD_EXACT_TEST: &str = "grpc_production::coverage_tests::emquant_quotes_scope_public_contract::windows_raw_bridge_public_contract::registered_emquant_daily_bars_preserve_ordered_bounded_suffix_and_source_evidence";
        if std::env::var_os(WORKER_KEY).as_deref() == Some(OsStr::new(CSD_WORKER_VALUE)) {
            exercise_admitted_daily_bars_query();
            return;
        }
        run_isolated_worker(CSD_EXACT_TEST, CSD_WORKER_VALUE);
    }
}
