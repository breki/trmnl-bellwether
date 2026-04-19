//! One-time `RPi` provisioning.
//!
//! Creates the `bellwether` system user and directory
//! tree, copies the local `config.toml` into place, and
//! installs + enables the systemd unit.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::deploy_config;
use crate::deploy_remote;
use crate::helpers::workspace_root;

pub fn deploy_setup() -> Result<()> {
    let project_root = workspace_root();
    let cfg = deploy_config::load(&project_root)?;
    let remote = cfg.remote();
    let deploy_dir = project_root.join("deploy");
    let service_file = deploy_dir.join("bellwether-web.service");
    let config_file = project_root.join("config.toml");

    println!("=== Setup bellwether-web on {remote} ===");
    println!("  deploy_path: {}", cfg.deploy_path);
    println!();

    create_user_and_dirs(&remote, &cfg.deploy_path)?;
    copy_config(&remote, &cfg.deploy_path, &project_root, &config_file)?;
    install_service(&remote, &deploy_dir)?;
    verify(&remote, &cfg.deploy_path)?;
    print_final_message(&service_file, &cfg.rpi_host)?;
    Ok(())
}

const SETUP_USER: &str = r#"set -euo pipefail
DEPLOY_PATH="$1"

if ! id -u bellwether &>/dev/null; then
    sudo useradd --system --shell /usr/sbin/nologin \
        --home-dir "$DEPLOY_PATH" bellwether
    echo "  Created 'bellwether' system user"
else
    echo "  User 'bellwether' already exists"
fi

sudo mkdir -p "$DEPLOY_PATH/frontend-dist"
sudo chown -R bellwether:bellwether "$DEPLOY_PATH"
sudo chmod 750 "$DEPLOY_PATH"
echo "  Directories ready"
"#;

fn create_user_and_dirs(remote: &str, deploy_path: &str) -> Result<()> {
    println!("[1/4] Creating bellwether user and directories...");
    deploy_remote::ssh_bash(remote, SETUP_USER, &[deploy_path])
        .context("creating user and directories on remote")?;
    Ok(())
}

const COPY_CONFIG: &str = r#"set -euo pipefail
umask 077
DEPLOY_PATH="$1"
# Tighten perms on the staging file right away: the
# scp above landed it with the user's default umask
# (typically 0644), exposing secrets to any other
# local user on the Pi until `sudo cp` runs.
chmod 600 ~/bellwether-config-tmp.toml
cleanup() { rm -f ~/bellwether-config-tmp.toml; }
trap cleanup EXIT
sudo cp ~/bellwether-config-tmp.toml "$DEPLOY_PATH/config.toml"
sudo chown bellwether:bellwether "$DEPLOY_PATH/config.toml"
sudo chmod 640 "$DEPLOY_PATH/config.toml"
echo "  config.toml installed"
"#;

fn copy_config(
    remote: &str,
    deploy_path: &str,
    project_root: &Path,
    config_file: &Path,
) -> Result<()> {
    println!();
    println!("[2/4] Copying config.toml...");
    if !config_file.is_file() {
        bail!(
            "{} not found; create it (see config.example.toml) \
             and set trmnl.public_image_base to the LAN-visible \
             URL of this RPi",
            config_file.display(),
        );
    }

    deploy_remote::scp_to(
        remote,
        "config.toml",
        "~/bellwether-config-tmp.toml",
        project_root,
    )
    .context("uploading config.toml")?;
    deploy_remote::ssh_bash(remote, COPY_CONFIG, &[deploy_path])
        .context("installing config.toml")?;
    Ok(())
}

const INSTALL_SERVICE: &str = r#"set -euo pipefail
sudo cp ~/bellwether-web.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable bellwether-web
rm ~/bellwether-web.service
echo "  Service installed and enabled"
"#;

fn install_service(remote: &str, deploy_dir: &Path) -> Result<()> {
    println!();
    println!("[3/4] Installing systemd service...");
    deploy_remote::scp_to(
        remote,
        "bellwether-web.service",
        "~/bellwether-web.service",
        deploy_dir,
    )
    .context("uploading systemd unit")?;
    deploy_remote::ssh_bash(remote, INSTALL_SERVICE, &[])
        .context("installing systemd unit")?;
    Ok(())
}

// Intentionally does NOT `set -e`: we want to report every
// checked item even when earlier ones are missing.
const VERIFY: &str = r#"DEPLOY_PATH="$1"
echo "  User:        $(id bellwether 2>/dev/null || echo 'MISSING')"
echo "  Deploy dir:  $(sudo ls -ld "$DEPLOY_PATH" 2>/dev/null \
    | awk '{print $1, $3, $4}' || echo 'MISSING')"
echo "  Config:      $(sudo test -f "$DEPLOY_PATH/config.toml" \
    && echo 'yes' || echo 'no')"
echo "  Service:     $(systemctl is-enabled bellwether-web 2>/dev/null \
    || echo 'not installed')"
"#;

fn verify(remote: &str, deploy_path: &str) -> Result<()> {
    println!();
    println!("[4/4] Verifying setup...");
    deploy_remote::ssh_bash(remote, VERIFY, &[deploy_path])
        .context("verifying setup on remote")?;
    Ok(())
}

fn print_final_message(service_file: &Path, rpi_host: &str) -> Result<()> {
    let contents =
        fs::read_to_string(service_file).context("reading service file")?;
    let port =
        parse_port(&contents).map_or_else(|| "?".to_owned(), |p| p.to_string());
    println!();
    println!("=== Setup complete ===");
    println!();
    println!("Next steps:");
    println!("  1. Run: cargo xtask deploy");
    println!("  2. Access: http://{rpi_host}:{port}");
    Ok(())
}

fn parse_port(service_contents: &str) -> Option<u16> {
    let flag = "--port";
    let idx = service_contents.find(flag)?;
    let rest = &service_contents[idx + flag.len()..];
    let digits: String = rest
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse::<u16>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_port_from_exec_start() {
        let svc = "[Service]\n\
                   ExecStart=/opt/bellwether/bellwether-web \
                   --config /opt/bellwether/config.toml \
                   --port 9300 --bind 0.0.0.0\n";
        assert_eq!(parse_port(svc), Some(9300));
    }

    #[test]
    fn parse_port_missing() {
        assert_eq!(parse_port("no port here"), None);
    }

    #[test]
    fn parse_port_non_digits_after() {
        let svc = "--port 9300abc";
        assert_eq!(parse_port(svc), Some(9300));
    }

    #[test]
    fn parse_port_rejects_out_of_range() {
        assert_eq!(parse_port("--port 999999"), None);
    }

    #[test]
    fn scripts_all_have_set_minus_e() {
        for (name, script) in [
            ("SETUP_USER", SETUP_USER),
            ("COPY_CONFIG", COPY_CONFIG),
            ("INSTALL_SERVICE", INSTALL_SERVICE),
        ] {
            assert!(
                script.contains("set -euo pipefail"),
                "{name} missing set -euo pipefail"
            );
        }
    }
}
