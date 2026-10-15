import hashlib, json, os, shutil, subprocess, sys, time, datetime
from pathlib import Path

REPO = Path(r"C:/DevelopFile/magic-market-data-rs")
TARGET_EX = REPO / "target" / "debug" / "examples"
EVID = REPO / "docs" / "evidence" / "2026-10-15-upstream-live-probes-214528"
EVID.mkdir(parents=True, exist_ok=True)
SUMMARY_PATH = EVID / "summary.json"
EXISTING = {}
if SUMMARY_PATH.exists():
    _old = json.loads(SUMMARY_PATH.read_text(encoding="utf-8"))
    EXISTING = {e["id"]: e for e in _old.get("results", [])}
INVALIDATE = {"magic-baidu-rs", "magic-cfets-rs", "magic-cls-rs", "magic-cninfo-rs",
              "magic-eastmoney-rs", "magic-official-news-rs", "magic-cfets-rs"}
for _id in list(EXISTING):
    if _id in INVALIDATE:
        del EXISTING[_id]

def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()

def load_env(path):
    env = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        env[k.strip()] = v.strip().strip('"').strip("'")
    return env

def run(cmd, **kw):
    return subprocess.run(cmd, cwd=REPO, capture_output=True, text=True, **kw)

def git(*args):
    return run(["git", *args]).stdout.strip()

dotenv = load_env(REPO / ".env.local")
base_env = dict(os.environ)
base_env.update(dotenv)

providers = sorted([p for p in (REPO / "crates").glob("*/examples/live_probe.rs")])
plan = []
for rs in providers:
    pkg = rs.parts[-3]
    entry = {"id": pkg, "package": pkg, "example": "live_probe", "args": [], "env": {}, "timeout": 420}
    if pkg == "magic-cfets-rs":
        entry["args"] = ["2026-09-21", "2026-09-23", "2026-09-01", "2026-09-23"]
        entry["timeout"] = 300
    elif pkg == "magic-eastmoney-rs":
        entry["env"] = {"MAGIC_EASTMONEY_DRAGON_DATE": "2026-10-15",
                        "MAGIC_EASTMONEY_POOL_DATE": "2026-10-15",
                        "MAGIC_EASTMONEY_DRAGON_TIGER_DATE": "2026-10-15",
                        "MAGIC_EASTMONEY_LIVE_OPERATION": ""}
    elif pkg == "magic-official-news-rs":
        entry["args"] = ["nbs"]
        entry["timeout"] = 300
    plan.append(entry)

board_env = {
    "MAGIC_TDX_BOARD_SERVER": "117.34.114.14",
    "MAGIC_TDX_BOARD_NAME": "\u4eba\u5de5\u667a\u80fd",
    "MAGIC_TDX_BOARD_CATEGORY": "concept",
}
for i in (1, 2):
    plan.append({"id": f"tdx-board-live-{i}", "package": "magic-tdx-rs", "example": "board_live_probe",
                 "args": [], "env": board_env, "timeout": 180})
for i in (1, 2, 3):
    e = dict(board_env)
    e["MAGIC_TDX_BOARD_LOAD_REQUESTS"] = "3"
    e["MAGIC_TDX_BOARD_LOAD_OPERATION"] = "directory"
    plan.append({"id": f"tdx-board-load-dir-{i}", "package": "magic-tdx-rs", "example": "board_load_probe",
                 "args": [], "env": e, "timeout": 180})
for i in (1, 2, 3):
    e = dict(board_env)
    e["MAGIC_TDX_BOARD_LOAD_REQUESTS"] = "3"
    e["MAGIC_TDX_BOARD_LOAD_OPERATION"] = "constituents"
    plan.append({"id": f"tdx-board-load-con-{i}", "package": "magic-tdx-rs", "example": "board_load_probe",
                 "args": [], "env": e, "timeout": 180})

for i in (1, 2, 3):
    e = dict(board_env); e["MAGIC_TDX_BOARD_LOAD_REQUESTS"] = "3"; e["MAGIC_TDX_BOARD_LOAD_OPERATION"] = "memberships"
    plan.append({"id": f"tdx-board-load-mem-{i}", "package": "magic-tdx-rs", "example": "board_load_probe",
                 "args": [], "env": e, "timeout": 180})
plan.append({"id": "magic-tencent-rs-retry", "package": "magic-tencent-rs", "example": "live_probe",
             "args": [], "env": {"MAGIC_TENCENT_HISTORY_DATE": "2026-09-30"}, "timeout": 300})
plan.append({"id": "magic-tencent-rs-retry2", "package": "magic-tencent-rs", "example": "live_probe",
             "args": [], "env": {"MAGIC_TENCENT_HISTORY_DATE": "2026-10-15"}, "timeout": 300})

summary = {
    "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "client_timezone": "Asia/Irkutsk",
    "git_head": git("rev-parse", "HEAD"),
    "initial_git_status": git("status", "--porcelain").splitlines(),
    "rustc": run(["rustc", "--version"]).stdout.strip(),
    "cargo": run(["cargo", "--version"]).stdout.strip(),
    "cargo_lock_sha256": sha256(REPO / "Cargo.lock"),
    "admissions_sha256_before": sha256(REPO / "docs/integrations/admissions.tsv"),
    "credential_presence": {k: (k in dotenv) for k in ["EASTMONEY_API_KEY", "FRED_API_KEY", "HITHINK_FINANCE_API_KEY", "IWENCAI_API_KEY"]},
    "scope": ("Targeted re-probe batch: one bounded run of every provider main live_probe, plus TDX board "
              "probes (2x board_live_probe, 3x directory and 3x constituents board_load_probe with "
              "concurrency=1, at most 3 requests each). No serial-load probes beyond the board load probes "
              "and no admission promotion. EMQuant was not re-run: its SDK bridge is not built in this workspace."),
    "notes": ["Shared live_probe.exe target collisions invalidated earlier baidu/cfets/cls/cninfo entries and "
              "the unconfigured eastmoney run; those entries were rebuilt by touching each example source "
              "before cargo build.",
              "The official-news live_probe requires a source argument; this batch ran nbs only.",
              "The eastmoney main run was repeated with the 2026-10-15 expected source dates after the "
              "2026-09-30-date run (kept as magic-eastmoney-rs.20260930-run.*.log) mixed sample-date "
              "expectations; its dragon-tiger failure reproduced the 2026-10-07 finding."],
    "profile": "dev (locked offline build; network enabled only in probe execution)",
    "results": list(EXISTING.values()),
    "notes": [],
}

for item in plan:
    if item["id"] in EXISTING:
        print(f"[resume-skip] {item['id']}", flush=True)
        continue
    entry = dict(item)
    entry["started_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    t0 = time.time()
    src_example = REPO / "crates" / item["package"] / "examples" / (item["example"] + ".rs")
    os.utime(src_example, None)
    build = run(["cargo", "build", "-p", item["package"], "--example", item["example"], "--locked", "--offline"])
    exe_src = TARGET_EX / (item["example"] + ".exe")
    if build.returncode != 0 or not exe_src.exists():
        entry.update(status="build_failed", exit_code=build.returncode,
                     stdout_log=None, stderr_log=None,
                     build_stderr=build.stderr[-4000:])
        entry["elapsed_seconds"] = round(time.time() - t0, 3)
        entry["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        summary["results"].append(entry)
        print(f"[build_failed] {item['id']}", flush=True)
        SUMMARY_PATH.write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
        continue
    exe_dst = EVID / f"{item['package']}__{item['id']}.exe"
    shutil.copy2(exe_src, exe_dst)
    entry["binary_sha256"] = sha256(exe_dst)
    env = dict(base_env)
    env.update(item["env"])
    name = item['id'] if item['id'].startswith("magic-") else "magic-" + item['id']
    out_p = EVID / f"{name}.stdout.log"
    err_p = EVID / f"{name}.stderr.log"
    try:
        proc = subprocess.run([str(exe_dst), *item["args"]], cwd=REPO, env=env,
                              capture_output=True, text=True, timeout=item["timeout"])
        out_p.write_text(proc.stdout, encoding="utf-8")
        err_p.write_text(proc.stderr, encoding="utf-8")
        entry.update(status="completed" if proc.returncode == 0 else "failed", exit_code=proc.returncode)
    except subprocess.TimeoutExpired as exc:
        out_p.write_text((exc.stdout or "") if isinstance(exc.stdout, str) else "", encoding="utf-8")
        err_p.write_text((exc.stderr or "") if isinstance(exc.stderr, str) else "", encoding="utf-8")
        entry.update(status="timeout", exit_code=None)
    entry["stdout_log"] = str(out_p.relative_to(REPO)).replace("\\", "/")
    entry["stderr_log"] = str(err_p.relative_to(REPO)).replace("\\", "/")
    entry["elapsed_seconds"] = round(time.time() - t0, 3)
    entry["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    summary["results"].append(entry)
    SUMMARY_PATH.write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"[{entry['status']}] {item['id']} ({entry['elapsed_seconds']}s)", flush=True)

summary["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
summary["admissions_sha256_after"] = sha256(REPO / "docs/integrations/admissions.tsv")
counts = {}
for e in summary["results"]:
    counts[e["status"]] = counts.get(e["status"], 0) + 1
summary["counts"] = counts
SUMMARY_PATH.write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
print("DONE", EVID, flush=True)
