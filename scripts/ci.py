#!/usr/bin/env python
import argparse
import os
import shutil
import subprocess
import sys
import time
from urllib.request import urlopen
from urllib.error import URLError, HTTPError

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PYTHON = sys.executable or "python"


def run_cmd(cmd, env=None, cwd=None):
    print(f"> {' '.join(cmd)}")
    result = subprocess.run(cmd, env=env, cwd=cwd)
    if result.returncode != 0:
        sys.exit(result.returncode)
    return result


def run_cmd_allow_fail(cmd, env=None, cwd=None):
    print(f"> {' '.join(cmd)}")
    return subprocess.run(cmd, env=env, cwd=cwd)


def step(msg):
    print(f"\n== {msg}")

def is_windows():
    """Check if running on Windows"""
    return sys.platform == "win32" or os.name == "nt"

def cmd_fmt(args):
    step("Running cargo fmt")
    if args.fix:
        run_cmd(["cargo", "fmt", "--all"])
        print("Code formatted successfully")
    else:
        result = run_cmd_allow_fail(["cargo", "fmt", "--all", "--", "--check"])
        if result.returncode == 0:
            print("Code formatting is correct")
        else:
            print(
                "Formatting issues found. Run: python scripts/ci.py fmt --fix"
            )
            sys.exit(result.returncode)


def cmd_clippy(args):
    step("Running cargo clippy")
    if args.fix:
        run_cmd(
            [
                "cargo",
                "clippy",
                "--workspace",
                "--all-targets",
                "--fix",
                "--allow-dirty",
            ]
        )
        print("Clippy issues fixed")
    else:
        result = run_cmd_allow_fail(
            [
                "cargo",
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ]
        )
        if result.returncode == 0:
            print("No clippy warnings found")
        else:
            print(
                "Clippy warnings found. Run: python scripts/ci.py clippy --fix"
            )
            sys.exit(result.returncode)


def cmd_test(_args):
    step("Running cargo test")
    run_cmd(["cargo", "test", "--workspace"])
    print("All tests passed")


def ensure_tool(binary, install_hint=None):
    result = run_cmd_allow_fail([binary, "--version"])
    if result.returncode != 0:
        msg = f"{binary} is not installed"
        if install_hint:
            msg += f". Install with: {install_hint}"
        print(msg)
        sys.exit(1)


def cmd_audit(_args):
    step("Running cargo audit")
    ensure_tool("cargo-audit", "cargo install cargo-audit")
    run_cmd(["cargo", "audit"])
    print("No security vulnerabilities found")


def cmd_deny(_args):
    step("Running cargo deny")
    ensure_tool("cargo-deny", "cargo install cargo-deny")
    run_cmd(["cargo", "deny", "check"])
    print("No licensing or dependency issues found")


def cmd_security(_args):
    step("Running security checks (audit + deny)")
    cmd_audit(_args)
    cmd_deny(_args)
    print("All security checks passed")


def cmd_gts_docs(args):
    step("Validating GTS identifiers in documentation files (DE0903)")
    cmd_args = [
        "cargo",
        "run",
        "-p",
        "gts-docs-validator",
        "--",
        "--exclude",
        "target/*",
        "--exclude",
        "docs/api/*",
        "docs",
        "modules",
        "libs",
        "examples",
    ]
    if getattr(args, 'verbose', False):
        cmd_args.append("--verbose")  # Append to end, after all other args
    result = run_cmd_allow_fail(cmd_args)
    if result.returncode == 0:
        print("All GTS identifiers in documentation are valid")
    else:
        print("Invalid GTS identifiers found in documentation files")
        sys.exit(result.returncode)


def cmd_cypilot_validate(_args):
    step("Validating cypilot artifacts")
    cypilot_dir = os.path.join(PROJECT_ROOT, ".cypilot")
    git_dir = os.path.join(cypilot_dir, ".git")
    submodule_initialized = (
        os.path.isdir(git_dir) or os.path.isfile(git_dir)
    )
    if not submodule_initialized:
        print("Initializing .cypilot submodule (first run)")
        run_cmd(
            [
                "git", "submodule", "update",
                "--init", "--recursive",
                "--", ".cypilot",
            ],
            cwd=PROJECT_ROOT,
        )
    else:
        # Skip update if on a branch checkout
        result = run_cmd_allow_fail(
            ["git", "-C", cypilot_dir,
             "symbolic-ref", "-q", "HEAD"]
        )
        if result.returncode == 0:
            print("Skipping .cypilot update "
                  "(branch checkout detected)")
        else:
            print("Updating .cypilot via git "
                  "submodule update (detached HEAD)")
            run_cmd(
                [
                    "git", "submodule", "update",
                    "--init", "--recursive",
                    "--", ".cypilot",
                ],
                cwd=PROJECT_ROOT,
            )
    script = os.path.join(
        cypilot_dir, "skills", "cypilot",
        "scripts", "cypilot.py",
    )
    result = run_cmd_allow_fail([PYTHON, script, "validate"])
    if result.returncode == 0:
        print("OK. cypilot validation PASSED")
    else:
        print("ERROR: cypilot validation FAILED")
        sys.exit(result.returncode)


def cmd_check(args):
    step("Running full check suite")
    cmd_fmt(args)
    cmd_cypilot_validate(args)
    cmd_clippy(args)
    cmd_test(args)
    cmd_dylint_test(args)
    cmd_dylint(args)
    cmd_gts_docs(args)
    cmd_security(args)
    print("All checks passed")


def cmd_quickstart(_args):
    step("Starting HyperSpot in quickstart mode")
    data_dir = os.path.join(PROJECT_ROOT, "data")
    if not os.path.isdir(data_dir):
        os.makedirs(data_dir, exist_ok=True)
        print(f"Created data directory: {data_dir}")
    run_cmd(
        [
            "cargo",
            "run",
            "--bin",
            "hyperspot-server",
            "--",
            "--config",
            "config/quickstart.yaml",
            "run",
        ]
    )


def wait_for_health(base_url, timeout_secs=30):
    url = f"{base_url.rstrip('/')}/healthz"
    step(f"Waiting for API to be ready at {url}")
    start = time.time()
    attempt = 0
    while True:
        try:
            attempt += 1
            with urlopen(url, timeout=1) as resp:
                if 200 <= resp.status < 300:
                    print(f"API is ready (after {attempt} attempts)")
                    return
        except (URLError, HTTPError, ConnectionResetError, OSError) as e:
            # Server may be starting up or restarting
            if attempt % 10 == 0:  # Log every 10 attempts
                print(f"Still waiting... (attempt {attempt}, error: {type(e).__name__})")

        if time.time() - start > timeout_secs:
            print(f"ERROR: The API readiness check timed out after {attempt} attempts")
            sys.exit(1)
        time.sleep(1)


def check_pytest():
    step("Checking pytest")
    # First try "python -m pytest"
    result = run_cmd_allow_fail([PYTHON, "-m", "pytest", "--version"])
    if result.returncode == 0:
        return
    # Then try "pytest" directly
    result = run_cmd_allow_fail(["pytest", "--version"])
    if result.returncode == 0:
        return
    print(
        "ERROR: pytest is not installed. Install with: "
        "pip install -r testing/e2e/requirements.txt"
    )
    sys.exit(1)


def kill_existing_server(port):
    """Kill any existing server process on the specified port"""
    try:
        if is_windows():
            # Windows: Use netstat to find PID and taskkill to terminate
            result = subprocess.run(
                ["netstat", "-ano"],
                capture_output=True,
                text=True
            )
            if result.returncode == 0:
                port_str = str(port)
                for line in result.stdout.splitlines():
                    if "LISTENING" not in line:
                        continue
                    parts = line.split()
                    if len(parts) < 5:
                        continue
                    local_addr = parts[1]
                    local_port = local_addr.rsplit(":", 1)[-1]
                    if local_port != port_str:
                        continue
                    pid = parts[-1]
                    print(f"Killing existing server process {pid} on port {port}")
                    run_cmd_allow_fail(["taskkill", "/F", "/PID", pid])
                    time.sleep(1)  # Give it time to die
            return  # Windows handling complete
        # Find process using the port
        if sys.platform == "darwin":  # macOS
            result = run_cmd_allow_fail(["lsof", "-ti", f":{port}"])
        else:  # Linux and others
            result = run_cmd_allow_fail(["fuser", "-k", f"{port}/tcp"])

        if result.returncode == 0 and result.stdout:
            pids = result.stdout.strip().split()
            for pid in pids:
                print(f"Killing existing server process {pid} on port {port}")
                run_cmd_allow_fail(["kill", "-9", pid])
                time.sleep(1)  # Give it time to die
    except Exception:
        # If we can't find or kill the process, continue anyway
        pass


def cmd_e2e(args):
    base_url = os.environ.get("E2E_BASE_URL", "http://localhost:8086")
    check_pytest()

    # Kill any existing server on the port before starting
    port = base_url.split(":")[-1]
    kill_existing_server(port)

    docker_env_started = False
    server_process = None

    if args.docker:
        step("Running E2E tests in Docker mode")

        # Check docker
        result = run_cmd_allow_fail(["docker", "version"])
        if result.returncode != 0:
            print("ERROR: docker is not installed or not in PATH")
            sys.exit(1)

        result = run_cmd_allow_fail(["docker", "compose", "version"])
        if result.returncode != 0:
            print("ERROR: 'docker compose' is not available")
            sys.exit(1)

        # Build image
        step("Building Docker image for E2E tests")
        build_cmd = [
            "docker",
            "build",
            "-f",
            "testing/docker/hyperspot.Dockerfile",
            "-t",
            "hyperspot-api:e2e",
        ]

        # Add build args for cargo features if specified
        if args.features:
            build_cmd.extend(["--build-arg", f"CARGO_FEATURES={args.features}"])

        build_cmd.append(".")
        run_cmd(build_cmd)

        # Start environment
        step("Starting E2E docker-compose environment")
        run_cmd(
            [
                "docker",
                "compose",
                "-f",
                "testing/docker/docker-compose.yml",
                "up",
                "--force-recreate",
                "-d",
            ]
        )
        docker_env_started = True

        # Wait for healthz
        wait_for_health(base_url)
    else:
        step("Running E2E tests in local mode")
        # Start local server automatically
        server_process = None
        try:
            wait_for_health(base_url, timeout_secs=5)
        except SystemExit:
            print("Server not running, starting hyperspot-server...")
            # Create logs directory if it doesn't exist
            logs_dir = os.path.join(PROJECT_ROOT, "logs")
            os.makedirs(logs_dir, exist_ok=True)

            # Start server in background with logs redirected to files
            server_cmd = [
                    "cargo",
                    "run",
                    "--bin",
                    "hyperspot-server",
                    "--features",
                    "users-info-example,tenant-resolver-example",
                    "--",
                    "--config",
            ]
            if is_windows():
                server_cmd.append("config/e2e-local-windows.yaml")
            else:
                server_cmd.append("config/e2e-local.yaml")

            # Redirect stdout and stderr to log files
            server_log_file = os.path.join(
                logs_dir, "hyperspot-e2e.log"
            )
            server_error_file = os.path.join(
                logs_dir, "hyperspot-e2e-error.log"
            )

            with open(server_log_file, "w") as out_file, open(
                server_error_file, "w"
            ) as err_file:
                # Set RUST_LOG to enable debug logging for types_registry module
                server_env = os.environ.copy()
                server_env["RUST_LOG"] = "types_registry=debug,info"
                server_process = subprocess.Popen(
                    server_cmd,
                    stdout=out_file,
                    stderr=err_file,
                    env=server_env,
                )

            print("Server logs redirected to:")
            print(f"  - stdout: {server_log_file}")
            print(f"  - stderr: {server_error_file}")
            print(
                "  - application logs: "
                f"{os.path.join(logs_dir, 'hyperspot-e2e.log')}"
            )
            print(f"  - SQL logs: {os.path.join(logs_dir, 'sql.log')}")
            print(f"  - API logs: {os.path.join(logs_dir, 'api.log')}")

            # Wait for server to be ready
            wait_for_health(base_url, timeout_secs=30)

    # Run pytest
    step("Running pytest")
    env = os.environ.copy()
    env["E2E_BASE_URL"] = base_url

    # Set E2E_DOCKER_MODE flag for the tests to know which mode they're in
    if args.docker:
        env["E2E_DOCKER_MODE"] = "1"

    pytest_cmd = [PYTHON, "-m", "pytest", "testing/e2e", "-vv"]
    if args.pytest_args:
        # argparse.REMAINDER includes the '--' separator if used
        # We need to strip it so pytest doesn't treat following flags as files
        extra_args = args.pytest_args
        if extra_args and extra_args[0] == "--":
            extra_args = extra_args[1:]
        pytest_cmd.extend(extra_args)

    result = run_cmd_allow_fail(pytest_cmd, env=env)
    exit_code = result.returncode

    if args.docker and docker_env_started:
        step("Stopping E2E docker-compose environment")
        run_cmd_allow_fail(
            [
                "docker",
                "compose",
                "-f",
                "testing/docker/docker-compose.yml",
                "down",
                "-v",
            ]
        )

    # Stop server if we started it
    if server_process is not None:
        step("Stopping hyperspot-server")
        server_process.terminate()
        try:
            server_process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            server_process.kill()
            server_process.wait()

    print("")
    if exit_code == 0:
        print("E2E tests passed")
    else:
        print("E2E tests failed")

    sys.exit(exit_code)


def cmd_dylint(_args):
    step("Building dylint lints")
    dylint_dir = os.path.join(PROJECT_ROOT, "dylint_lints")
    run_cmd(["cargo", "build", "--release"], cwd=dylint_dir)
    # Copy toolchain-suffixed names similar to Makefile
    rustc_host = (
        subprocess.check_output(["rustc", "--version", "--verbose"])
        .decode()
        .splitlines()
    )
    host = next((line.split()[-1] for line in rustc_host if line.startswith("host:")), "")
    toolchain = "nightly"
    rust_toolchain_path = os.path.join(dylint_dir, "rust-toolchain.toml")
    if os.path.isfile(rust_toolchain_path):
        with open(rust_toolchain_path, "r", encoding="utf-8") as f:
            for line in f:
                if "channel" in line:
                    toolchain = line.split('"')[1]
                    break
    target_release = os.path.join(dylint_dir, "target", "release")
    for fname in os.listdir(target_release):
        if not fname.startswith("libde") and not fname.startswith("de"):
            continue
        if "@" in fname:
            continue
        if fname.endswith(".dylib"):
            ext = ".dylib"
        elif fname.endswith(".so"):
            ext = ".so"
        elif fname.endswith(".dll"):
            ext = ".dll"
        else:
            continue
        base = fname[: -len(ext)]
        target = f"{base}@{toolchain}-{host}{ext}"
        src = os.path.join(target_release, fname)
        dst = os.path.join(target_release, target)
        try:
            shutil.copyfile(src, dst)
        except OSError:
            pass
    dylint_libs = sorted(
        [
            os.path.join(target_release, f)
            for f in os.listdir(target_release)
            if (f.startswith("libde") or f.startswith("de"))
            and ("@" in f)
            and (
                f.endswith(".dylib")
                or f.endswith(".so")
                or f.endswith(".dll")
            )
        ]
    )
    if not dylint_libs:
        print("ERROR: No dylint libraries found after build.")
        sys.exit(1)
    lib_args = []
    for lib in dylint_libs:
        lib_args.extend(["--lib-path", lib])
    run_cmd(
        ["cargo", f"+{toolchain}", "dylint", *lib_args, "--workspace"],
        cwd=PROJECT_ROOT,
    )
    print("Dylint checks passed")


def cmd_dylint_test(_args):
    step("Running dylint tests")
    dylint_dir = os.path.join(PROJECT_ROOT, "dylint_lints")
    run_cmd(["cargo", "test"], cwd=dylint_dir)
    print("Dylint tests passed")


def cmd_dylint_list(_args):
    step("Listing dylint lints")
    dylint_dir = os.path.join(PROJECT_ROOT, "dylint_lints")
    target_release = os.path.join(dylint_dir, "target", "release")
    dylint_libs = sorted(
        [
            os.path.join(target_release, f)
            for f in os.listdir(target_release)
            if (f.startswith("libde") or f.startswith("de"))
            and (
                f.endswith(".dylib")
                or f.endswith(".so")
                or f.endswith(".dll")
            )
        ]
    )
    if not dylint_libs:
        print("ERROR: No dylint libraries found. Run 'python scripts/ci.py dylint' first.")
        sys.exit(1)
    for lib in dylint_libs:
        print(f"=== {lib} ===")
        run_cmd(["cargo", "dylint", "list", "--lib-path", lib], cwd=PROJECT_ROOT)


def ensure_nightly_toolchain():
    """Ensure Rust nightly toolchain is installed."""
    result = run_cmd_allow_fail(["rustup", "run", "nightly", "rustc", "--version"])
    if result.returncode != 0:
        print(
            "ERROR: Rust nightly toolchain not installed. "
            "Install with: rustup toolchain install nightly"
        )
        sys.exit(1)


def ensure_cargo_fuzz():
    """Ensure cargo-fuzz is installed."""
    ensure_nightly_toolchain()
    result = run_cmd_allow_fail(["cargo", "+nightly", "fuzz", "--version"])
    if result.returncode != 0:
        print("Installing cargo-fuzz...")
        run_cmd(["cargo", "+nightly", "install", "cargo-fuzz"])


def cmd_fuzz_build(_args):
    step("Building fuzz targets")
    fuzz_dir = os.path.join(PROJECT_ROOT, "fuzz")
    ensure_cargo_fuzz()

    # Build all fuzz targets (no TARGET argument = build all)
    run_cmd(["cargo", "+nightly", "fuzz", "build"], cwd=fuzz_dir)
    print("All fuzz targets built successfully")


def cmd_fuzz_list(_args):
    step("Listing fuzz targets")
    fuzz_dir = os.path.join(PROJECT_ROOT, "fuzz")
    ensure_cargo_fuzz()

    run_cmd(["cargo", "+nightly", "fuzz", "list"], cwd=fuzz_dir)


def cmd_fuzz_run(args):
    step(f"Running fuzz target: {args.target}")
    fuzz_dir = os.path.join(PROJECT_ROOT, "fuzz")
    ensure_cargo_fuzz()

    fuzz_seconds = args.seconds or 60
    if fuzz_seconds <= 0:
        print("ERROR: --seconds must be a positive integer")
        sys.exit(1)
    fuzz_cmd = [
        "cargo", "+nightly", "fuzz", "run", args.target,
        "--", f"-max_total_time={fuzz_seconds}"
    ]

    result = run_cmd_allow_fail(fuzz_cmd, cwd=fuzz_dir)

    if result.returncode != 0:
        print(f"Fuzzing found issues. Check fuzz/artifacts/{args.target}/")
        sys.exit(result.returncode)

    print(f"Fuzzing completed successfully ({fuzz_seconds}s)")


def cmd_fuzz(args):
    step("Running smoke test fuzzing on all targets")
    fuzz_dir = os.path.join(PROJECT_ROOT, "fuzz")

    # Build all targets first
    cmd_fuzz_build(args)

    # Get list of targets
    result = subprocess.run(
        ["cargo", "+nightly", "fuzz", "list"],
        cwd=fuzz_dir,
        capture_output=True,
        text=True
    )

    if result.returncode != 0:
        print("Failed to list fuzz targets")
        sys.exit(1)

    targets = result.stdout.strip().split('\n')
    smoke_time = args.seconds or 30
    if smoke_time <= 0:
        print("ERROR: --seconds must be a positive integer")
        sys.exit(1)

    failed_targets = []

    for target in targets:
        target = target.strip()
        if not target:
            continue

        print(f"\n=== Fuzzing {target} for {smoke_time}s ===")
        fuzz_cmd = [
            "cargo", "+nightly", "fuzz", "run", target,
            "--", f"-max_total_time={smoke_time}"
        ]

        result = run_cmd_allow_fail(fuzz_cmd, cwd=fuzz_dir)

        if result.returncode != 0:
            failed_targets.append(target)
            print(f"❌ {target} found issues")
        else:
            print(f"✅ {target} passed")

    if failed_targets:
        print(f"\n❌ Fuzzing found issues in: {', '.join(failed_targets)}")
        print("Check fuzz/artifacts/ for crash details")
        sys.exit(1)

    print(f"\n✅ All fuzz targets passed ({smoke_time}s each)")


def cmd_fuzz_clean(_args):
    step("Cleaning fuzzing artifacts")
    fuzz_dir = os.path.join(PROJECT_ROOT, "fuzz")

    artifacts_dir = os.path.join(fuzz_dir, "artifacts")
    corpus_dir = os.path.join(fuzz_dir, "corpus")
    target_dir = os.path.join(fuzz_dir, "target")

    for d in [artifacts_dir, target_dir]:
        if os.path.exists(d):
            shutil.rmtree(d)
            print(f"Removed {d}")

    # Clean corpus but keep .gitkeep files
    if os.path.exists(corpus_dir):
        for item in os.listdir(corpus_dir):
            item_path = os.path.join(corpus_dir, item)
            if os.path.isdir(item_path):
                # Remove contents but keep the directory and .gitkeep
                for subitem in os.listdir(item_path):
                    if subitem != ".gitkeep":
                        subitem_path = os.path.join(item_path, subitem)
                        if os.path.isfile(subitem_path):
                            os.remove(subitem_path)
                        elif os.path.isdir(subitem_path):
                            shutil.rmtree(subitem_path)

    print("Fuzzing artifacts cleaned")


def cmd_all(args):
    step("Running full build and testing pipeline")
    cmd_check(args)
    step("Running SQLite integration tests")
    run_cmd(
        [
            "cargo",
            "test",
            "-p",
            "modkit-db",
            "--features",
            "sqlite,integration",
            "--",
            "--nocapture",
        ]
    )
    step("Building release (stable)")
    run_cmd(["cargo", "+stable", "build", "--release"])
    step("Running e2e-local")
    cmd_e2e(argparse.Namespace(docker=False, pytest_args=[]))
    print("All (full pipeline) completed")


def build_parser():
    parser = argparse.ArgumentParser(
        description="HyperSpot CI utility (Python, cross-platform)",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    # fmt
    p_fmt = subparsers.add_parser("fmt", help="Check or fix code formatting")
    p_fmt.add_argument("--fix", action="store_true", help="Auto-format code")
    p_fmt.set_defaults(func=cmd_fmt)

    # clippy
    p_clippy = subparsers.add_parser("clippy", help="Run clippy lints")
    p_clippy.add_argument("--fix", action="store_true", help="Auto-fix clippy issues")
    p_clippy.set_defaults(func=cmd_clippy)

    # test
    p_test = subparsers.add_parser("test", help="Run unit tests")
    p_test.set_defaults(func=cmd_test)

    # audit
    p_audit = subparsers.add_parser("audit", help="Run cargo audit")
    p_audit.set_defaults(func=cmd_audit)

    # deny
    p_deny = subparsers.add_parser("deny", help="Run cargo deny checks")
    p_deny.set_defaults(func=cmd_deny)

    # security
    p_sec = subparsers.add_parser("security", help="Run security checks (audit + deny)")
    p_sec.set_defaults(func=cmd_security)

    # check
    p_check = subparsers.add_parser("check", help="Run full check suite (fmt + clippy + test + security)")
    p_check.add_argument("--fix", action="store_true", help="Auto-fix formatting and clippy issues")
    p_check.set_defaults(func=cmd_check)

    # quickstart
    p_qs = subparsers.add_parser("quickstart", help="Start server in quickstart mode")
    p_qs.set_defaults(func=cmd_quickstart)

    # e2e
    p_e2e = subparsers.add_parser("e2e", help="Run end-to-end tests")
    p_e2e.add_argument(
        "--docker",
        action="store_true",
        help="Run tests in Docker environment instead of local server",
    )
    p_e2e.add_argument(
        "--features",
        default="users-info-example",
        help="Cargo features to enable for Docker build (default: users-info-example)",
    )
    p_e2e.add_argument(
        "pytest_args",
        nargs=argparse.REMAINDER,
        help="Extra arguments passed to pytest (use -- to separate)",
    )
    p_e2e.set_defaults(func=cmd_e2e)

    # dylint
    p_dylint = subparsers.add_parser("dylint", help="Build and run dylint lints")
    p_dylint.set_defaults(func=cmd_dylint)

    # dylint-test
    p_dylint_test = subparsers.add_parser("dylint-test", help="Run dylint UI tests")
    p_dylint_test.set_defaults(func=cmd_dylint_test)

    # dylint-list
    p_dylint_list = subparsers.add_parser("dylint-list", help="List available dylint lints")
    p_dylint_list.set_defaults(func=cmd_dylint_list)

    # fuzz-build
    p_fuzz_build = subparsers.add_parser("fuzz-build", help="Build all fuzz targets")
    p_fuzz_build.set_defaults(func=cmd_fuzz_build)

    # fuzz-list
    p_fuzz_list = subparsers.add_parser("fuzz-list", help="List all fuzz targets")
    p_fuzz_list.set_defaults(func=cmd_fuzz_list)

    # fuzz-run
    p_fuzz_run = subparsers.add_parser("fuzz-run", help="Run specific fuzz target")
    p_fuzz_run.add_argument("target", help="Name of fuzz target to run")
    p_fuzz_run.add_argument("--seconds", type=int, help="Fuzzing duration in seconds (default: 60)")
    p_fuzz_run.set_defaults(func=cmd_fuzz_run)

    # fuzz
    p_fuzz = subparsers.add_parser("fuzz", help="Run smoke test fuzzing on all targets")
    p_fuzz.add_argument("--seconds", type=int, default=30, help="Seconds per target (default: 30)")
    p_fuzz.set_defaults(func=cmd_fuzz)

    # fuzz-clean
    p_fuzz_clean = subparsers.add_parser("fuzz-clean", help="Clean fuzzing artifacts")
    p_fuzz_clean.set_defaults(func=cmd_fuzz_clean)

    # cypilot-validate
    p_cypilot = subparsers.add_parser("cypilot-validate", help="Validate cypilot artifacts (specs, code, templates)")
    p_cypilot.set_defaults(func=cmd_cypilot_validate)

    # gts-docs
    p_gts_docs = subparsers.add_parser("gts-docs", help="Validate GTS identifiers in .md and .json files (DE0903)")
    p_gts_docs.add_argument("-v", "--verbose", action="store_true", help="Show verbose output")
    p_gts_docs.set_defaults(func=cmd_gts_docs)

    # all
    p_all = subparsers.add_parser("all", help="Run full pipeline (Makefile all equivalent)")
    p_all.add_argument("--fix", action="store_true", help="Auto-fix formatting/clippy")
    p_all.set_defaults(func=cmd_all)

    return parser


def main():
    os.chdir(PROJECT_ROOT)
    parser = build_parser()
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
