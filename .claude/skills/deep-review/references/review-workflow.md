# Review Workflow

This document defines the shared review logic used by both PR and local branch reviews. Follow these steps after gathering context (project guidelines, change summary, and diff).

---

## Step 1: Deep Review (parallel Opus agents)

Launch **four review agents in parallel** using `model: "opus"`. Each agent receives: the change summary from context gathering, the relevant CLAUDE.md guidelines, and the diff. Each agent works in the **worktree directory** (use `WORKTREE_PATH` for all file reads).

### Agent 1: Bug & Logic Review
- Read the changed files in the worktree
- Scan for bugs, logic errors, off-by-one errors, null/None handling, race conditions, resource leaks, security vulnerabilities (injection, XSS, auth bypass)
- Focus on **what changed**, not pre-existing code
- For each issue found, provide: file path, line number, description, impact, suggested fix

### Agent 2: Architecture & Design Review
- Evaluate separation of concerns, layer violations, API design, module boundaries
- Check for unnecessary coupling, god objects, missing abstractions
- Apply any architecture-specific rules from CLAUDE.md guidelines
- For each issue found, provide: file path, line number, description, impact, suggested fix

### Agent 3: CLAUDE.md Compliance Review
- Audit changes against project guidelines from CLAUDE.md
- Check all conventions, naming rules, error handling patterns, and structural requirements defined in the guidelines
- For each issue found, provide: file path, line number, the specific guideline violated, description, suggested fix

### Agent 4: Testing & Coverage Review
- Examine test files in the diff
- Check if new code paths have corresponding tests
- Evaluate test quality: are edge cases covered? Are error paths tested?
- Check for test anti-patterns: flaky tests, hardcoded values, missing assertions, tests that test implementation details instead of behavior
- For each issue found, provide: file path, line number, description, what's missing, suggested fix

---

## Step 2: Confidence Scoring (parallel Haiku agents)

For each issue found across all four review agents, launch **Haiku agents** to score confidence (0-100).

Scoring rubric:
- **0-24:** False positive or pre-existing issue not introduced by these changes
- **25-49:** Might be real but could be a false positive
- **50-74:** Real issue but a nitpick or very low impact
- **75-89:** Verified real issue, important to address
- **90-100:** Confirmed high-impact issue (bug, security, data loss)

**Filter out all issues scoring below 75.** Keep the confidence score on remaining issues.

---

## Step 3: Compile Review File

Write the review file to the appropriate path based on mode:
- **PR mode:** `<REPO_ROOT>/REVIEW_pr_<PR_NUMBER>.md`
- **Local mode:** `<REPO_ROOT>/REVIEW_<BRANCH_NAME>.md` (with `/` replaced by `-` in branch name)

### Header (mode-specific)

**PR mode:**
```markdown
# PR Review: #<number> — <title>

**Author:** <author>
**Branch:** <head> -> <base>
**URL:** <pr_url>
**Date:** <today's date>
**Files changed:** <count> (+<additions>, -<deletions>)
```

**Local mode:**
```markdown
# Branch Review: <branch_name>

**Branch:** <branch_name> (base: <base_branch>)
**Date:** <today's date>
**Commits:** <commit count>
**Files changed:** <count> (+<additions>, -<deletions>)
```

### Body (identical for both modes)

```markdown
## Summary
<1-3 sentence description of what the changes do, based on the commit messages/PR body and diff analysis>

## Strengths
<Specific positive observations — good patterns used, clean code, thorough tests, etc. Include file:line references where relevant.>

## Issues

### Critical (Must Fix)
<Bugs, security issues, data loss risks, correctness problems. Confidence >= 90.>

### Important (Should Fix)
<Architecture violations, missing error handling, test gaps, design issues. Confidence 75-89.>

### Minor (Nice to Have)
<Style improvements, minor optimizations, documentation suggestions. Confidence 50-74 that survived filtering.>

For each issue, use this format:

- **<concise title>** — `<file>:<line>`
  - **What:** <description of the issue>
  - **Why:** <impact — what could go wrong>
  - **Fix:** <concrete suggestion for how to fix it>
  - **Confidence:** <score>/100

If a severity category has no issues, write "None found."

## Recommendations
<General improvement suggestions that don't map to specific lines — patterns to adopt, testing strategies, documentation to add, etc.>

## Assessment
**Ready to merge?** [Yes / Yes, with minor fixes / No, needs changes]
**Reasoning:** <1-2 sentences explaining the verdict>
```

---

## Step 4: Report to User

After writing the review file, print a summary to the user:

**PR mode:**
```
## Review Complete

**PR:** #<number> — <title>
**Verdict:** <Yes / Yes, with minor fixes / No, needs changes>

| Severity | Count |
|----------|-------|
| Critical | <n>   |
| Important| <n>   |
| Minor    | <n>   |

**Review file:** `<REPO_ROOT>/REVIEW_pr_<PR_NUMBER>.md`
**Worktree:** `<WORKTREE_PATH>`

When done, clean up with: `git worktree remove <WORKTREE_PATH>`
```

**Local mode:**
```
## Review Complete

**Branch:** <branch_name>
**Verdict:** <Yes / Yes, with minor fixes / No, needs changes>

| Severity | Count |
|----------|-------|
| Critical | <n>   |
| Important| <n>   |
| Minor    | <n>   |

**Review file:** `<REPO_ROOT>/REVIEW_<BRANCH_NAME>.md`
**Worktree:** `<WORKTREE_PATH>`

When done, clean up with: `git worktree remove <WORKTREE_PATH>`
```
