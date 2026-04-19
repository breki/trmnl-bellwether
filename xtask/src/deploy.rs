//! Repeatable deploy to the Raspberry Pi.
//!
//! Tars the Rust sources and ships them to the `RPi`,
//! builds the release binary there, installs the binary
//! atomically, and restarts the systemd service.

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

    println!("=== Deploy to {remote} ({}) ===", cfg.deploy_path);

    sync_source(&project_root, &remote)?;
    build_on_remote(&remote)?;
    install(&remote, &cfg.deploy_path)?;
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

fn install(remote: &str, deploy_path: &str) -> Result<()> {
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
    Ok(())
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
