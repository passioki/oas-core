//! Checkpoint / resume (ENGINE §22.16 纵向② "检查点恢复").
//!
//! After every successful **top-level** step the runtime calls the progress sink;
//! this module persists `{flow_name, steps, next_step}` atomically. `--resume`
//! validates the checkpoint against the current flow and skips the completed
//! prefix. A `Done` run clears the file; a failed run keeps it.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use oas_runtime::ProgressSink;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub schema_version: u32,
    pub flow_name: String,
    pub steps: u32,
    pub next_step: u32,
    #[serde(default)]
    pub updated_ms: u128,
}

pub struct FileCheckpoint {
    path: PathBuf,
    flow_name: String,
    steps: u32,
    next_step: u32,
    writable: bool,
    dirty: bool,
    last_error: Option<String>,
}

impl FileCheckpoint {
    pub fn open(
        path: &Path,
        flow_name: &str,
        steps: usize,
        writable: bool,
        next_step: usize,
    ) -> Self {
        Self {
            path: path.to_path_buf(),
            flow_name: flow_name.to_string(),
            steps: steps as u32,
            next_step: next_step as u32,
            writable,
            dirty: false,
            last_error: None,
        }
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn next_step(&self) -> usize {
        self.next_step as usize
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load a checkpoint file; `None` = no file. A corrupt file is a loud error.
    pub fn load(path: &Path) -> Result<Option<Checkpoint>, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("checkpoint `{}`: {e}", path.display())),
        };
        let c: Checkpoint = serde_json::from_str(&text)
            .map_err(|e| format!("checkpoint `{}` is not valid JSON: {e}", path.display()))?;
        Ok(Some(c))
    }

    /// Persist pending progress (atomic tmp+rename). Read-only = no-op.
    pub fn flush(&mut self) -> Result<(), String> {
        if let Some(e) = self.last_error.take() {
            return Err(e);
        }
        if !self.writable || !self.dirty {
            return Ok(());
        }
        self.write_now()
    }

    /// Remove the checkpoint (run completed). Read-only = no-op.
    pub fn clear(&mut self) -> Result<(), String> {
        if !self.writable {
            return Ok(());
        }
        match std::fs::remove_file(&self.path) {
            Ok(()) => {
                self.next_step = 0;
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!(
                "checkpoint: cannot remove `{}` ({e})",
                self.path.display()
            )),
        }
    }

    fn write_now(&mut self) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("checkpoint: create `{}` ({e})", dir.display()))?;
        }
        let c = Checkpoint {
            schema_version: 1,
            flow_name: self.flow_name.clone(),
            steps: self.steps,
            next_step: self.next_step,
            updated_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
        };
        let text = serde_json::to_string_pretty(&c)
            .map_err(|e| format!("checkpoint: serialize ({e})"))?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text)
            .map_err(|e| format!("checkpoint: write `{}` ({e})", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| format!("checkpoint: rename `{}` ({e})", self.path.display()))?;
        Ok(())
    }

    fn observe_step(&mut self, index: usize) {
        self.next_step = index as u32 + 1;
        self.dirty = true;
        if self.writable {
            if let Err(e) = self.write_now() {
                self.last_error = Some(e);
            }
        }
    }
}

impl ProgressSink for FileCheckpoint {
    fn step_done(&mut self, index: usize, _op: &str) {
        self.observe_step(index);
    }
}

/// Shareable wrapper: the interpreter owns the sink, the driver flushes/clears.
#[derive(Clone)]
pub struct SharedCheckpoint(pub Rc<RefCell<FileCheckpoint>>);

impl ProgressSink for SharedCheckpoint {
    fn step_done(&mut self, index: usize, op: &str) {
        self.0.borrow_mut().step_done(index, op);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "oas-ckpt-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).expect("tmpdir");
        p
    }

    #[test]
    fn checkpoint_roundtrip_and_clear() {
        let dir = tmpdir("roundtrip");
        let path = dir.join("state/checkpoint.json");
        let mut c = FileCheckpoint::open(&path, "S1", 5, true, 0);
        c.step_done(0, "click");
        c.step_done(1, "wait");
        let loaded = FileCheckpoint::load(&path).expect("load").expect("file");
        assert_eq!(loaded.flow_name, "S1");
        assert_eq!(loaded.steps, 5);
        assert_eq!(loaded.next_step, 2);
        c.clear().expect("clear");
        assert!(FileCheckpoint::load(&path).expect("load2").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_only_checkpoint_never_writes() {
        let dir = tmpdir("ro");
        let path = dir.join("checkpoint.json");
        let mut c = FileCheckpoint::open(&path, "S1", 3, false, 0);
        c.step_done(0, "click");
        c.flush().expect("flush no-op");
        assert!(!path.exists());
        assert!(FileCheckpoint::load(&path).expect("load").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
