# Integration evidence index

Provider integration documents describe source-specific endpoint, schema,
provenance, admission, and explicit-failure contracts.

`admissions.tsv` is the machine-readable BR-009 index for every public Rust
constant ending in `_ADMITTED`. An admitted row requires an existing evidence
document, a canonical live-evidence date, at least two bounded live probes, and
at least three serial load calls. A false row requires an explicit blocker.
Source, registry, and evidence files must be Git-tracked regular files inside
the repository; symlinked or untracked evidence is rejected. `-` is the
canonical placeholder for an intentionally empty optional field.

Run the drift check without network access:

```bash
python3 tools/compliance/check_admissions.py
bash tools/compliance/check.sh
```

The registry does not execute probes, grant data rights, or turn a diagnostic
implementation into a production capability. It binds already-recorded
evidence to the exact Rust constant so source and documentation changes cannot
silently diverge.
