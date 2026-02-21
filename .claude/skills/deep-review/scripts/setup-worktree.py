#!/usr/bin/env python3
"""Setup an isolated git worktree for code review.

Usage:
    python3 setup-worktree.py pr <PR_NUMBER>
    python3 setup-worktree.py local [BRANCH]

Output: KEY=VALUE pairs consumed by the skill prompt.
"""

import subprocess
import sys
from pathlib import Path


def run(cmd: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True, check=check)


def emit(
    mode: str = "",
    pr_number: str = "",
    owner_repo: str = "",
    worktree_path: str = "",
    repo_root: str = "",
    has_upstream: str = "",
    base_branch: str = "",
    branch_name: str = "",
    error: str = "",
) -> None:
    print(f"MODE={mode}")
    print(f"PR_NUMBER={pr_number}")
    print(f"OWNER_REPO={owner_repo}")
    print(f"WORKTREE_PATH={worktree_path}")
    print(f"REPO_ROOT={repo_root}")
    print(f"HAS_UPSTREAM={has_upstream}")
    print(f"BASE_BRANCH={base_branch}")
    print(f"BRANCH_NAME={branch_name}")
    print(f"ERROR={error}")


def fail(msg: str, **kw: str) -> None:
    emit(error=msg, **kw)
    sys.exit(0)  # exit clean so the caller always sees KEY=VALUE output


def get_repo_root() -> Path:
    return Path(run(["git", "rev-parse", "--show-toplevel"]).stdout.strip())


def check_worktree_exists(worktree_path: str, ctx: dict[str, str]) -> None:
    result = run(["git", "worktree", "list"], check=False)
    if worktree_path in result.stdout:
        fail(
            f"Worktree already exists at {worktree_path}"
            f" — to re-review, first run: git worktree remove {worktree_path}",
            **ctx,
        )


def cmd_pr(args: list[str]) -> None:
    if not args or not args[0]:
        fail("Usage: /deep-review pr <PR number>", mode="pr")

    pr_number = args[0]
    if not pr_number.isdigit() or pr_number == "0" or pr_number.startswith("0"):
        fail(f"PR number must be a positive integer, got: {pr_number}", mode="pr")

    repo_root = get_repo_root()
    ctx: dict[str, str] = {"mode": "pr", "pr_number": pr_number, "repo_root": str(repo_root)}

    # Detect upstream repo
    has_upstream = "false"
    result = run(["git", "remote", "get-url", "upstream"], check=False)

    if result.returncode == 0:
        has_upstream = "true"
        upstream_url = result.stdout.strip()
        result = run(
            ["gh", "repo", "view", upstream_url, "--json", "nameWithOwner", "-q", ".nameWithOwner"],
            check=False,
        )
        if result.returncode != 0:
            fail(
                f"Cannot resolve upstream remote: {result.stderr.strip()}",
                **ctx,
                has_upstream=has_upstream,
            )
        owner_repo = result.stdout.strip()
    else:
        result = run(
            [
                "gh", "repo", "view",
                "--json", "nameWithOwner,parent",
                "-q", "if .parent then .parent.nameWithOwner else .nameWithOwner end",
            ],
            check=False,
        )
        if result.returncode != 0:
            fail("Cannot detect source repo. Check gh auth status.", **ctx)
        owner_repo = result.stdout.strip()

    ctx["owner_repo"] = owner_repo
    ctx["has_upstream"] = has_upstream

    # Calculate worktree path
    repo_name = repo_root.name
    worktree_path = str(repo_root.parent / f"{repo_name}-worktrees" / f"pr-{pr_number}")
    ctx["worktree_path"] = worktree_path

    check_worktree_exists(worktree_path, ctx)

    # Fetch PR head
    Path(worktree_path).parent.mkdir(parents=True, exist_ok=True)

    if has_upstream == "true":
        fetch_ref = ["git", "fetch", "upstream", f"pull/{pr_number}/head"]
    else:
        fetch_url = f"https://github.com/{owner_repo}.git"
        fetch_ref = ["git", "fetch", fetch_url, f"pull/{pr_number}/head"]

    result = run(fetch_ref, check=False)
    if result.returncode != 0:
        fail(
            f"git fetch failed (PR #{pr_number} may not exist): {result.stderr.strip()}",
            **ctx,
        )

    # Create worktree
    result = run(["git", "worktree", "add", worktree_path, "FETCH_HEAD"], check=False)
    if result.returncode != 0:
        fail(f"git worktree add failed: {result.stderr.strip()}", **ctx)

    emit(**ctx)


def cmd_local(args: list[str]) -> None:
    repo_root = get_repo_root()
    ctx: dict[str, str] = {"mode": "local", "repo_root": str(repo_root)}

    # Determine branch name
    if args and args[0]:
        branch_name = args[0]
        # Verify branch exists
        result = run(["git", "rev-parse", "--verify", branch_name], check=False)
        if result.returncode != 0:
            fail(f"Branch '{branch_name}' does not exist.", **ctx)
    else:
        result = run(["git", "branch", "--show-current"], check=False)
        if result.returncode != 0 or not result.stdout.strip():
            fail("Cannot determine current branch (detached HEAD?).", **ctx)
        branch_name = result.stdout.strip()

    ctx["branch_name"] = branch_name

    # Fail if on main/master
    if branch_name in ("main", "master"):
        fail(f"Cannot review '{branch_name}' — switch to a feature branch first.", **ctx)

    # Detect base branch
    for candidate in ("main", "master"):
        result = run(["git", "rev-parse", "--verify", candidate], check=False)
        if result.returncode == 0:
            base_branch = candidate
            break
    else:
        fail("Cannot find 'main' or 'master' branch.", **ctx)

    ctx["base_branch"] = base_branch

    # Calculate worktree path
    repo_name = repo_root.name
    safe_branch = branch_name.replace("/", "-")
    worktree_path = str(repo_root.parent / f"{repo_name}-worktrees" / f"local-{safe_branch}")
    ctx["worktree_path"] = worktree_path

    check_worktree_exists(worktree_path, ctx)

    # Create worktree
    Path(worktree_path).parent.mkdir(parents=True, exist_ok=True)
    result = run(["git", "worktree", "add", "--detach", worktree_path, branch_name], check=False)
    if result.returncode != 0:
        fail(f"git worktree add failed: {result.stderr.strip()}", **ctx)

    emit(**ctx)


def main() -> None:
    if len(sys.argv) < 2:
        fail("Usage: /deep-review pr <number> | /deep-review local [branch]")

    subcommand = sys.argv[1]
    sub_args = sys.argv[2:]

    if subcommand == "pr":
        cmd_pr(sub_args)
    elif subcommand == "local":
        cmd_local(sub_args)
    else:
        fail(f"Unknown subcommand '{subcommand}'. Usage: /deep-review pr <number> | /deep-review local [branch]")


if __name__ == "__main__":
    main()
