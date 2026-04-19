pub mod clients;
pub mod config;
pub mod dashboard;
pub mod publish;
pub mod render;
pub mod telemetry;
pub mod weather;

/// Returns the version string from Cargo.toml.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
