---
name: deep-review
description: "Multi-agent code review in an isolated git worktree. Usage: /deep-review pr <number> | /deep-review local [branch]"
argument-hint: "pr <number> | local [branch]"
disable-model-invocation: true
allowed-tools: Bash(python3 */setup-worktree.py:*), Bash(gh pr view:*), Bash(gh pr diff:*), Bash(gh pr list:*), Bash(gh repo view:*), Bash(git worktree:*), Bash(git diff:*), Bash(git log:*), Bash(git branch:*), Bash(git rev-parse:*), Bash(ls:*)
---

# Deep Review

You are performing a multi-agent code review in an isolated git worktree. Follow this workflow precisely.

`SKILL_DIR` = the "Base directory for this skill" path shown above the skill content.

---

## Step 0: Setup Worktree

Run the setup script to validate arguments, detect the source, and create an isolated worktree:

```bash
python3 SKILL_DIR/scripts/setup-worktree.py $ARGUMENTS
```

The script outputs `KEY=VALUE` lines. Read every value and use them for **all** subsequent steps. Do **not** refer to `$ARGUMENTS` again after this point.

| Key | Meaning |
|---|---|
| `MODE` | `pr` or `local` |
| `PR_NUMBER` | Validated PR number (PR mode only) |
| `OWNER_REPO` | Source `owner/repo` (PR mode only) |
| `WORKTREE_PATH` | Absolute path to the created worktree |
| `REPO_ROOT` | Absolute path to the original repo root |
| `HAS_UPSTREAM` | `true` if `upstream` remote was found (PR mode only) |
| `BASE_BRANCH` | Base branch name, e.g. `main` (local mode only) |
| `BRANCH_NAME` | Branch being reviewed (local mode only) |
| `ERROR` | Non-empty if setup failed — print the error and stop |

If `ERROR` is non-empty, print it to the user and stop.

---

## Step 1: Gather Context (parallel Haiku agents)

Launch **three agents in parallel** using `model: "haiku"`. The agents depend on the mode.

### Agent A: Project Guidelines (both modes)
Read CLAUDE.md in the worktree (`WORKTREE_PATH/CLAUDE.md`). If it doesn't exist there, read CLAUDE.md from `REPO_ROOT` instead. Also check for any CLAUDE.md files in subdirectories touched by changed files. Return a summary of guidelines relevant to code review (coding standards, architecture rules, naming conventions, testing requirements).

### Agent B: Change Summary

**PR mode:**
Run `gh pr view <PR_NUMBER> --repo <OWNER_REPO>` and return a structured summary: title, description/body, author, base branch, head branch, state, additions/deletions count, number of files changed.

**Local mode:**
Run `git log <BASE_BRANCH>..HEAD --oneline` in the worktree directory and return a structured summary: branch name, base branch, list of commits, commit count.

### Agent C: Changed Files & Diff

**PR mode:**
Run `gh pr diff <PR_NUMBER> --repo <OWNER_REPO>` and return:
- List of all changed files with their status (added/modified/deleted)
- Per-file line change stats
- The full diff content (preserve it for the review agents)

**Local mode:**
Run `git diff <BASE_BRANCH>...HEAD` in the worktree directory and return:
- List of all changed files with their status (added/modified/deleted)
- Per-file line change stats
- The full diff content (preserve it for the review agents)

---

## Step 2+: Follow Review Workflow

Read the file `SKILL_DIR/references/review-workflow.md` and follow its steps exactly. Pass the context gathered in Step 1 (guidelines, change summary, diff) to the workflow.

The review workflow will handle: deep review with parallel Opus agents, confidence scoring, compiling the review file, and reporting to the user.
