//! oasd library: the engine application (CLI + HTTP surfaces) as a reusable
//! library (Phase 1a of the Runner/Studio split).
//!
//! Before this split the whole application was a single binary target
//! (`main.rs` + sibling modules). Phase 1 of `RUNNER-SPLIT-DESIGN.md §3.5` turns
//! it into **lib + thin bin** so the future `oas-runner` repository can ship its
//! own, independent application instead of copying this code:
//!
//! ```text
//! apps/oasd/src/lib.rs   -> this file: the modules (capabilities)
//! apps/oasd/src/cli.rs   -> the oasd CLI (`run`, and `run_as` for other apps)
//! apps/oasd/src/main.rs  -> 3-line binary: `oasd::cli::run()`
//! oas-runner/apps/runner -> 3-line binary: `oasd::cli::run_as("runner")`
//! ```
//!
//! Every module is `pub` on purpose: they are the application's capability
//! surface (and the tests now live on the lib target, not the bin target).
//! `ocr` is only present when the `ocr` cargo feature is on, exactly as before.

pub mod bundled_data;
pub mod checkpoint;
pub mod cli;
pub mod config;
pub mod coords;
pub mod device_lease;
pub mod exchange;
pub mod global_assets;
pub mod library;
pub mod package_cmd;
pub mod queue;
pub mod reconnect;
pub mod relations;
pub mod run;
pub mod runner_config;
pub mod schedule;
pub mod scheduler;
pub mod serve;
pub mod stats;
pub mod task;

#[cfg(feature = "ocr")]
pub mod ocr;
