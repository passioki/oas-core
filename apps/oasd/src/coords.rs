//! `tasks/<task>/state/coords.json` — coordinate memory for `click_target`
//! (ENGINE §22.11).
//!
//! * only the **live, non-dry** run writes it (`writable=false` for replay/dry);
//! * entries carry the frame size they were recorded at, so a resolution change
//!   is scaled proportionally instead of clicking a stale pixel;
//! * writes are atomic (tmp file + rename), never a half-written JSON.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use oas_runtime::CoordStore;
use oas_types::Point;
use serde::{Deserialize, Serialize};

/// One remembered click point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordEntry {
    /// Frame-absolute pixel at `design`.
    pub point: [i32; 2],
    /// Frame size the point was recorded at.
    pub design: [u32; 2],
    /// How many times this key has been (re)learned.
    #[serde(default)]
    pub hits: u64,
    /// Wall-clock ms of the last successful learn (informational).
    #[serde(default)]
    pub last_ok_ms: u64,
}

/// On-disk contract of `coords.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CoordFile {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub items: BTreeMap<String, CoordEntry>,
}

/// File-backed coordinate memory.
#[derive(Debug)]
pub struct FileCoordStore {
    path: PathBuf,
    writable: bool,
    dirty: bool,
    items: BTreeMap<String, CoordEntry>,
}

impl FileCoordStore {
    /// Load `state_dir/coords.json` (missing file = empty memory).
    pub fn open(state_dir: &Path, writable: bool) -> Self {
        let path = state_dir.join("coords.json");
        let items = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<CoordFile>(&t).ok())
            .map(|f| f.items)
            .unwrap_or_default();
        Self {
            path,
            writable,
            dirty: false,
            items,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Persist pending learns atomically. Read-only stores are a no-op.
    pub fn flush(&mut self) -> Result<(), String> {
        if !self.writable || !self.dirty {
            return Ok(());
        }
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("coords: create `{}` ({e})", dir.display()))?;
        }
        let file = CoordFile {
            schema_version: 1,
            items: self.items.clone(),
        };
        let text = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("coords: serialize ({e})"))?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("coords: write `{}` ({e})", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| format!("coords: rename `{}` ({e})", self.path.display()))?;
        self.dirty = false;
        Ok(())
    }

    fn get_point(&mut self, key: &str, frame_w: u32, frame_h: u32) -> Option<Point> {
        let e = self.items.get(key)?;
        let (dw, dh) = (e.design[0].max(1), e.design[1].max(1));
        let x = (e.point[0] as i64 * frame_w as i64 / dw as i64) as i32;
        let y = (e.point[1] as i64 * frame_h as i64 / dh as i64) as i32;
        if frame_w == 0 || frame_h == 0 {
            return None;
        }
        Some(Point { x, y })
    }

    fn put_point(&mut self, key: &str, point: Point, frame_w: u32, frame_h: u32) {
        if !self.writable {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let e = self.items.entry(key.to_string()).or_insert(CoordEntry {
            point: [point.x, point.y],
            design: [frame_w, frame_h],
            hits: 0,
            last_ok_ms: now,
        });
        e.point = [point.x, point.y];
        e.design = [frame_w, frame_h];
        e.hits += 1;
        e.last_ok_ms = now;
        self.dirty = true;
    }
}

impl CoordStore for FileCoordStore {
    fn get(&mut self, key: &str, frame_w: u32, frame_h: u32) -> Option<Point> {
        self.get_point(key, frame_w, frame_h)
    }

    fn put(&mut self, key: &str, point: Point, frame_w: u32, frame_h: u32) {
        self.put_point(key, point, frame_w, frame_h)
    }
}

/// Shareable wrapper: the interpreter owns a `Box<dyn CoordStore>` while the run
/// driver keeps the `Rc` so it can `flush()` after the interpreter is dropped.
#[derive(Clone)]
pub struct SharedStore(pub Rc<RefCell<FileCoordStore>>);

impl CoordStore for SharedStore {
    fn get(&mut self, key: &str, frame_w: u32, frame_h: u32) -> Option<Point> {
        self.0.borrow_mut().get_point(key, frame_w, frame_h)
    }

    fn put(&mut self, key: &str, point: Point, frame_w: u32, frame_h: u32) {
        self.0.borrow_mut().put_point(key, point, frame_w, frame_h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "oas-coords-{tag}-{}-{}",
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
    fn put_flush_open_roundtrip_and_design_scaling() {
        let dir = tmpdir("roundtrip");
        let mut s = FileCoordStore::open(&dir, true);
        assert!(s.is_empty());
        s.put(
            "返回按钮",
            Point { x: 100, y: 200 },
            1280,
            720,
        );
        s.flush().expect("flush");
        assert!(s.path().exists());

        let mut s2 = FileCoordStore::open(&dir, false);
        assert_eq!(s2.len(), 1);
        // Same resolution: the exact remembered point.
        assert_eq!(s2.get("返回按钮", 1280, 720), Some(Point { x: 100, y: 200 }));
        // Half resolution: scaled proportionally (100,200) -> (50,100).
        assert_eq!(s2.get("返回按钮", 640, 360), Some(Point { x: 50, y: 100 }));
        // Unknown key: None (the chain must fall through / fail).
        assert_eq!(s2.get("不存在", 1280, 720), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_only_store_never_writes() {
        let dir = tmpdir("readonly");
        let mut s = FileCoordStore::open(&dir, false);
        s.put("key", Point { x: 1, y: 2 }, 100, 100);
        s.flush().expect("flush is a no-op");
        assert!(!dir.join("coords.json").exists(), "read-only must not write");
        assert_eq!(s.len(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn learn_twice_increments_hits() {
        let dir = tmpdir("hits");
        let mut s = FileCoordStore::open(&dir, true);
        s.put("k", Point { x: 10, y: 10 }, 100, 100);
        s.put("k", Point { x: 11, y: 12 }, 100, 100);
        s.flush().expect("flush");
        let mut s2 = FileCoordStore::open(&dir, false);
        assert_eq!(s2.get("k", 100, 100), Some(Point { x: 11, y: 12 }));
        let raw = std::fs::read_to_string(dir.join("coords.json")).unwrap();
        assert!(raw.contains("\"hits\": 2"), "{raw}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
