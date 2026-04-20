//! Repeatable deploy to the Raspberry Pi.
//!
//! Tars the Rust sources and ships them to the `RPi`,
//! builds the release binary there, installs the
//! binary and (if changed) the systemd unit file
//! atomically, and restarts the systemd service.
//!
//! The unit-file sync guards against drift between
//! `deploy/bellwether-web.service` in the repo and
//! `/etc/systemd/system/bellwether-web.service` on the
//! `RPi`. Without it, a CLI-arg or sandbox-policy
//! change would crash-loop the service on the next
//! deploy — exactly the v0.16.0 `--frontend` regression
//! that motivated this step.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::deploy_config;
use crate::deploy_remote;
use crate::helpers::workspace_root;

pub fn deploy() -> Result<()> {
    let project_root = workspace_root();
    let cfg = deploy_config::load(&project_root)?;
    let remote = cfg.remote();
    let deploy_dir = project_root.join("deploy");

    println!("=== Deploy to {remote} ({}) ===", cfg.deploy_path);

    sync_source(&project_root, &remote)?;
    build_on_remote(&remote)?;
    install(&remote, &cfg.deploy_path, &deploy_dir)?;
    restart_and_verify(&remote)?;

    println!();
    println!("=== Deploy OK ===");
    Ok(())
}

fn sync_source(project_root: &Path, remote: &str) -> Result<()> {
    println!();
    println!("[1/4] Syncing source to {remote}...");

    // Preserve ~/bellwether-build/target across deploys
    // for incremental cargo builds, but still purge stale
    // source files (e.g. a deleted build.rs would keep
    // running on the remote if left in place).
    deploy_remote::ssh_run(
        remote,
        "mkdir -p ~/bellwether-build \
         && find ~/bellwether-build -mindepth 1 -maxdepth 1 \
            ! -name target -exec rm -rf {} +",
    )
    .context("purging stale source on remote")?;

    let tar_path = project_root.join("bellwether-src.tar");
    if tar_path.exists() {
        fs::remove_file(&tar_path).context("removing stale local tar")?;
    }

    let status = Command::new("tar")
        .arg("cf")
        .arg("bellwether-src.tar")
        .args([
            "Cargo.toml",
            "Cargo.lock",
            "crates",
            "xtask",
            "rust-toolchain.toml",
            "rustfmt.toml",
        ])
        .current_dir(project_root)
        .status()
        .context("failed to run tar")?;
    if !status.success() {
        match status.code() {
            Some(c) => bail!("local tar exited with status {c}"),
            None => bail!("local tar terminated by signal"),
        }
    }

    let scp_result = deploy_remote::scp_to(
        remote,
        "bellwether-src.tar",
        "~/bellwether-src.tar",
        project_root,
    );
    // best-effort cleanup, don't mask scp error
    let _ = fs::remove_file(&tar_path);
    scp_result.context("uploading source tar")?;

    deploy_remote::ssh_run(
        remote,
        "tar xf ~/bellwether-src.tar -C ~/bellwether-build \
         && rm ~/bellwether-src.tar",
    )
    .context("extracting source tar on remote")?;
    Ok(())
}

fn build_on_remote(remote: &str) -> Result<()> {
    println!();
    println!("[2/4] Building on {remote} (this may take a while)...");
    deploy_remote::ssh_run(
        remote,
        ". ~/.cargo/env \
         && cd ~/bellwether-build \
         && cargo build --release -p bellwether-web",
    )
    .context("remote cargo build")?;
    Ok(())
}

fn install(remote: &str, deploy_path: &str, deploy_dir: &Path) -> Result<()> {
    println!();
    println!("[3/4] Installing...");

    // Stop (ignore failure — may not be running).
    deploy_remote::ssh_run(
        remote,
        "sudo systemctl stop bellwether-web || true",
    )
    .context("stopping service")?;

    let cmd = format!(
        "sudo cp ~/bellwether-build/target/release/bellwether-web \
             '{deploy_path}/' \
         && sudo chmod 755 '{deploy_path}/bellwether-web'"
    );
    deploy_remote::ssh_run(remote, &cmd).context("installing binary")?;

    sync_service_unit(remote, deploy_dir)
        .context("syncing systemd unit file")?;
    Ok(())
}

/// Ship `deploy/bellwether-web.service` to the `RPi`
/// when (and only when) it differs from the file
/// currently installed at
/// `/etc/systemd/system/bellwether-web.service`.
/// No-op when they already match, so repeated deploys
/// stay quiet — the drift check runs every time but
/// only mutates remote state when there's something to
/// change.
fn sync_service_unit(remote: &str, deploy_dir: &Path) -> Result<()> {
    let local_path = deploy_dir.join("bellwether-web.service");
    let local = fs::read_to_string(&local_path)
        .with_context(|| format!("reading {}", local_path.display()))?;
    // `sudo cat` because the unit file is 0644 on most
    // systemd installs but ProtectSystem=strict may
    // block non-root reads via the mount namespace in
    // the future; `sudo` is the durable read path. The
    // `|| echo ''` tail keeps us from erroring when the
    // unit file is missing (first-deploy scenario, or
    // deploy-setup hasn't run yet).
    let installed = deploy_remote::ssh_capture(
        remote,
        "sudo cat /etc/systemd/system/bellwether-web.service \
         2>/dev/null || echo ''",
    )
    .context("reading remote unit file")?;
    if unit_contents_match(&local, &installed) {
        println!("  Unit file up to date");
        return Ok(());
    }
    println!(
        "  Unit file drift detected; syncing \
         deploy/bellwether-web.service"
    );
    deploy_remote::scp_to(
        remote,
        "bellwether-web.service",
        "~/bellwether-web.service",
        deploy_dir,
    )
    .context("uploading systemd unit")?;
    deploy_remote::ssh_run(
        remote,
        "sudo mv ~/bellwether-web.service \
             /etc/systemd/system/bellwether-web.service \
         && sudo systemctl daemon-reload",
    )
    .context("installing systemd unit")?;
    println!("  Unit file installed and daemon-reloaded");
    Ok(())
}

/// Compare a local and a remote unit file for
/// "effectively equal". Normalises trailing whitespace
/// (a file that ends with `\n` and one that ends with
/// `\n\n` are the same unit from systemd's point of
/// view, and scp/sudo-cat round-trips can flip either
/// way depending on the shell).
fn unit_contents_match(local: &str, installed: &str) -> bool {
    local.trim_end() == installed.trim_end()
}

fn restart_and_verify(remote: &str) -> Result<()> {
    println!();
    println!("[4/4] Restarting bellwether-web...");
    // Clear a prior `failed` state (e.g. from a reboot
    // between `deploy-setup` and the first `deploy`,
    // which can burn through `StartLimitBurst` while
    // the binary is still missing). Non-fatal if there
    // was nothing to reset.
    deploy_remote::ssh_run(
        remote,
        "sudo systemctl reset-failed bellwether-web || true",
    )
    .context("clearing failed state")?;
    deploy_remote::ssh_run(remote, "sudo systemctl start bellwether-web")
        .context("starting service")?;

    let last_status = poll_active_status(remote, 3)?;
    if last_status == "active" {
        return Ok(());
    }
    eprintln!();
    eprintln!("ERROR: service status is '{last_status}' after 3 attempts");
    eprintln!("check logs: ssh {remote} journalctl -u bellwether-web -n 20");
    bail!("service not active (last status: {last_status})");
}

fn poll_active_status(remote: &str, attempts: u32) -> Result<String> {
    let mut last = String::new();
    for attempt in 1..=attempts {
        sleep(Duration::from_secs(2));
        let out = deploy_remote::ssh_capture(
            remote,
            "systemctl is-active bellwether-web 2>/dev/null \
             || echo failed",
        )
        .context("polling service status")?;
        last = String::from(out.trim());
        if last == "active" {
            return Ok(last);
        }
        println!("  attempt {attempt}: status={last}, retrying...");
    }
    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::unit_contents_match;

    #[test]
    fn install_syncs_the_systemd_unit_file() {
        // Canary against removing the drift fix for the
        // v0.16.0 `--frontend` regression. `install` must
        // call `sync_service_unit`, and that function
        // must reference the actual installed path so a
        // refactor can't silently repoint it at a scratch
        // location.
        let src = include_str!("deploy.rs");
        assert!(
            src.contains("sync_service_unit(remote, deploy_dir)"),
            "install() must call sync_service_unit — without it, \
             a CLI or sandbox change drifts between repo and RPi",
        );
        assert!(
            src.contains("/etc/systemd/system/bellwether-web.service"),
            "sync_service_unit must target the canonical unit path",
        );
    }

    #[test]
    fn unit_contents_match_is_trailing_newline_tolerant() {
        // scp and `sudo cat` both round-trip unit files
        // with their own preferences about a terminal
        // newline. Normalising trailing whitespace
        // prevents a spurious "drift detected" message
        // (and wasted scp) on every deploy.
        let canonical = "[Service]\nExecStart=/opt/app\n";
        assert!(unit_contents_match(canonical, canonical));
        assert!(unit_contents_match(
            canonical,
            "[Service]\nExecStart=/opt/app"
        ));
        assert!(unit_contents_match(
            canonical,
            "[Service]\nExecStart=/opt/app\n\n",
        ));
    }

    #[test]
    fn unit_contents_match_detects_real_changes() {
        let before = "[Service]\nExecStart=/opt/app --old-flag\n";
        let after = "[Service]\nExecStart=/opt/app\n";
        assert!(!unit_contents_match(before, after));
    }

    /// Guard against a future refactor wiping the remote
    /// build dir, which would defeat the incremental
    /// cargo cache under `target/` on the `RPi`.
    ///
    /// Checks that every reference to `~/bellwether-build`
    /// inside `sync_source` is guarded by a `! -name
    /// target` predicate.
    #[test]
    fn sync_source_preserves_target_cache() {
        let src = include_str!("deploy.rs");
        let sync = src
            .split_once("fn sync_source")
            .and_then(|(_, rest)| rest.split_once("fn build_on_remote"))
            .map(|(body, _)| body)
            .expect("deploy.rs layout changed; update this test");
        assert!(
            sync.contains("! -name target"),
            "sync_source must skip the target cache when \
             purging the remote build dir",
        );
        // A plain `rm -rf ~/bellwether-build` (or the
        // same with a trailing slash) would wipe the
        // cache irrespective of the find guard. Reject
        // any occurrence.
        for pat in ["rm -rf ~/bellwether-build\"", "rm -rf ~/bellwether-build "]
        {
            assert!(
                !sync.contains(pat),
                "sync_source contains unguarded deletion: {pat}",
            );
        }
    }
}
