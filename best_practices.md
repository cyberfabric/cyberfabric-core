
<b>Pattern 1: In library/module code, avoid writing directly to stdout/stderr with print!/println!/eprintln!/dbg!; use structured logging (e.g., tracing) or return data to callers, and only allow direct output in explicitly user-facing binaries/CLI paths with narrowly scoped exceptions.
</b>

Example code before:
```
pub fn do_work() -> Result<()> {
    println!("starting work");
    eprintln!("warning: something odd");
    Ok(())
}
```

Example code after:
```
pub fn do_work() -> Result<()> {
    tracing::info!("starting work");
    tracing::warn!("something odd");
    Ok(())
}

// If a CLI must print machine-readable output:
#[allow(unknown_lints, de1301_no_print_macros)]
pub fn print_config_yaml(yaml: &str) {
    println!("{yaml}");
}
```

<details><summary>Examples for relevant past discussions:</summary>

- https://github.com/cyberfabric/cyberfabric-core/pull/475#discussion_r2771740365
- https://github.com/cyberfabric/cyberfabric-core/pull/478#discussion_r2772918171
</details>


___

<b>Pattern 2: Design CI automation to be deterministic and non-spammy: avoid sleeping inside jobs, ensure notifications are de-duplicated/rate-limited, and keep concurrency grouping minimal and consistent to prevent unnecessary parallel runs.
</b>

Example code before:
```
concurrency:
  group: api-contract-${{ github.workflow }}-${{ github.ref }}

# (inside a job)
- run: |
    sleep 300
    ./notify.sh
```

Example code after:
```
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

on:
  schedule:
    - cron: "*/10 * * * *"

# In-script de-duplication / rate-limit marker:
const alreadyCommented = comments.some(c =>
  c.user.type === "Bot" && c.body.includes("has no human reviewers assigned")
);
if (alreadyCommented) return;
```

<details><summary>Examples for relevant past discussions:</summary>

- https://github.com/cyberfabric/cyberfabric-core/pull/469#discussion_r2771252165
- https://github.com/cyberfabric/cyberfabric-core/pull/490#discussion_r2775131938
</details>


___

<b>Pattern 3: When temporarily disabling/removing enforcement rules (custom lints, policy checks), require an explicit replacement plan (issue/task link, timeline, or equivalent rule) so contract/purity guarantees are not silently lost.
</b>

Example code before:
```
[workspace]
members = [
  # "de0101_no_serde_in_contract",
  # "de0102_no_toschema_in_contract",
]
```

Example code after:
```
[workspace]
members = [
  # TEMP DISABLED: de0101_no_serde_in_contract
  # Tracking: https://github.com/org/repo/issues/1234
  # Restore criteria: replace with DE130x equivalent once toolchain X is fixed.
  # "de01_contract_layer/de0101_no_serde_in_contract",
]
```

<details><summary>Examples for relevant past discussions:</summary>

- https://github.com/cyberfabric/cyberfabric-core/pull/469#discussion_r2771269040
- https://github.com/cyberfabric/cyberfabric-core/pull/469#discussion_r2771277461
</details>


___

<b>Pattern 4: Keep tests and docs stable across environments by avoiding fragile OS-dependent runtime behavior in unit tests and by preventing documentation drift: centralize canonical guidance, don’t duplicate section-by-section outlines, and enforce consistent naming/terminology across docs and code.
</b>

Example code before:
```
#[tokio::test]
async fn sends_sigterm() {
    let mut child = Command::new("/bin/sh").arg("-c").arg("sleep 30").spawn().unwrap();
    assert!(send_terminate_signal(&child));
}

// docs/README repeats the full section outline for every template page...
```

Example code after:
```
// Prefer narrow, deterministic tests (pure logic) and integration tests for OS behavior:
#[test]
fn pid_conversion_rejects_out_of_range() {
    assert!(i32::try_from(u32::MAX).is_err());
}

// Docs: keep only a brief pointer to the canonical template pages:
// "See PRD.md / DESIGN.md / ADR/*.md for the authoritative structure."
```

<details><summary>Examples for relevant past discussions:</summary>

- https://github.com/cyberfabric/cyberfabric-core/pull/300#discussion_r2721113798
- https://github.com/cyberfabric/cyberfabric-core/pull/400#discussion_r2752999605
- https://github.com/cyberfabric/cyberfabric-core/pull/328#discussion_r2733771616
</details>


___
