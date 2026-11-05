//! Thin binary entry point (Phase 1a of the Runner/Studio split).
//!
//! All CLI logic lives in the `oasd` library (`oasd::cli`), so the future
//! `oas-runner` application can reuse the exact same entry point via
//! `oasd::cli::run_as("runner")` instead of copying anything. This target only
//! forwards to it — do not add logic here.

fn main() -> std::process::ExitCode {
    oasd::cli::run()
}
