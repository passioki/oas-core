//! oas-obs: structured events + **record / replay** (P0-3b).
//!
//! Why replay matters (design doc §6):
//! * offline verification: a recorded run can be re-driven without a device;
//! * knowledge capture: a real run becomes a `flow.json` draft source;
//! * game-update triage: after a patch, replay tells you exactly which step broke.
//!
//! ## Assertion rule (measured in P0-4)
//! Two screenshots of the same screen differed by 0.39% because of dynamic UI
//! (activity banner / chat). Therefore replay assertions compare the
//! **event/action sequence**, never raw pixels.
//!
//! ## On-disk layout
//! ```text
//! <dir>/meta.json      {schema_version, created_at, source, width, height, frames}
//! <dir>/events.json    [{i, step, op, detail, ts_ms}, ...]
//! <dir>/frames/0000.png ...
//! ```

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use oas_codec::{decode_png, encode_png, frame_hash};
use oas_types::{Frame, ScreenSource};

/// Bump when the on-disk layout changes (clean-room rule: no silent readers).
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub schema_version: u32,
    pub created_at_ms: u128,
    pub source: String,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub i: usize,
    pub step: u32,
    pub op: String,
    pub detail: String,
    pub ts_ms: u128,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub struct Recorder {
    dir: PathBuf,
    source: String,
    frames: usize,
    width: u32,
    height: u32,
    events: Vec<Event>,
}

impl Recorder {
    pub fn create(dir: impl AsRef<Path>, source: impl Into<String>) -> Result<Self, String> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(dir.join("frames"))
            .map_err(|e| format!("create {}: {e}", dir.display()))?;
        Ok(Self {
            dir,
            source: source.into(),
            frames: 0,
            width: 0,
            height: 0,
            events: Vec::new(),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Grab one frame from `src` and persist it. Returns the frame hash.
    pub fn capture(&mut self, src: &mut impl ScreenSource) -> Result<u64, String> {
        let frame = src.grab()?;
        let png = encode_png(&frame)?;
        let name = format!("frames/{:04}.png", self.frames);
        std::fs::write(self.dir.join(&name), &png).map_err(|e| format!("write {name}: {e}"))?;
        self.width = frame.width;
        self.height = frame.height;
        let hash = frame_hash(&frame);
        self.event(0, "grab", &format!("{name} hash={hash:016x}"));
        self.frames += 1;
        Ok(hash)
    }

    pub fn event(&mut self, step: u32, op: &str, detail: &str) {
        let i = self.events.len();
        self.events.push(Event {
            i,
            step,
            op: op.to_string(),
            detail: detail.to_string(),
            ts_ms: now_ms(),
        });
    }

    pub fn finish(self) -> Result<Meta, String> {
        let meta = Meta {
            schema_version: SCHEMA_VERSION,
            created_at_ms: now_ms(),
            source: self.source,
            width: self.width,
            height: self.height,
            frames: self.frames,
        };
        let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
        std::fs::write(self.dir.join("meta.json"), meta_json)
            .map_err(|e| format!("write meta.json: {e}"))?;
        let ev_json = serde_json::to_string_pretty(&self.events).map_err(|e| e.to_string())?;
        std::fs::write(self.dir.join("events.json"), ev_json)
            .map_err(|e| format!("write events.json: {e}"))?;
        Ok(meta)
    }
}

/// Frame source backed by a recording. Fails loudly when exhausted: a replay
/// that silently repeats the last frame would hide real divergences.
pub struct ReplaySource {
    frames: Vec<PathBuf>,
    idx: usize,
}

impl ReplaySource {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, String> {
        let frames_dir = dir.as_ref().join("frames");
        let mut frames: Vec<PathBuf> = std::fs::read_dir(&frames_dir)
            .map_err(|e| format!("read {}: {e}", frames_dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "png").unwrap_or(false))
            .collect();
        frames.sort();
        if frames.is_empty() {
            return Err(format!("no frames in {}", frames_dir.display()));
        }
        Ok(Self { frames, idx: 0 })
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn remaining(&self) -> usize {
        self.frames.len().saturating_sub(self.idx)
    }
}

impl ScreenSource for ReplaySource {
    fn grab(&mut self) -> Result<Frame, String> {
        let path = self
            .frames
            .get(self.idx)
            .ok_or_else(|| format!("replay exhausted after {} frame(s)", self.idx))?
            .clone();
        self.idx += 1;
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        decode_png(&bytes)
    }
}

/// Read the event log of a recording (used by assertions / tooling).
pub fn read_events(dir: impl AsRef<Path>) -> Result<Vec<Event>, String> {
    let p = dir.as_ref().join("events.json");
    let text = std::fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", p.display()))
}

pub fn read_meta(dir: impl AsRef<Path>) -> Result<Meta, String> {
    let p = dir.as_ref().join("meta.json");
    let text = std::fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", p.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeFrames {
        frames: Vec<Frame>,
        idx: usize,
    }

    impl ScreenSource for FakeFrames {
        fn grab(&mut self) -> Result<Frame, String> {
            let f = self
                .frames
                .get(self.idx)
                .ok_or("fake exhausted")?
                .clone();
            self.idx += 1;
            Ok(f)
        }
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("oas_obs_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn frame(seed: u8) -> Frame {
        Frame::new(8, 6, (0..(8 * 6 * 4)).map(|i| (i as u8).wrapping_add(seed)).collect())
    }

    #[test]
    fn record_then_replay_is_frame_identical() {
        let dir = tmpdir("roundtrip");
        let originals = vec![frame(0), frame(7), frame(42)];
        let mut src = FakeFrames {
            frames: originals.clone(),
            idx: 0,
        };

        let mut rec = Recorder::create(&dir, "test").expect("create recorder");
        let mut hashes = Vec::new();
        for _ in 0..3 {
            hashes.push(rec.capture(&mut src).expect("capture"));
        }
        rec.event(1, "click", "I_FIRE");
        let meta = rec.finish().expect("finish");
        assert_eq!(meta.frames, 3);
        assert_eq!(meta.width, 8);
        assert_eq!(meta.height, 6);
        assert_eq!(meta.schema_version, SCHEMA_VERSION);

        // replay: same frames, same order, same hashes
        let mut replay = ReplaySource::open(&dir).expect("open replay");
        assert_eq!(replay.len(), 3);
        assert_eq!(replay.remaining(), 3);
        for (i, want) in originals.iter().enumerate() {
            let got = replay.grab().expect("grab");
            assert_eq!(frame_hash(&got), frame_hash(want), "frame {i} differs");
            assert_eq!(frame_hash(&got), hashes[i]);
        }
        // exhausted => loud error, never silent repeat
        assert!(replay.grab().is_err());

        // events are readable and ordered
        let events = read_events(&dir).expect("events");
        assert_eq!(events.len(), 4); // 3 grab + 1 click
        assert_eq!(events[3].op, "click");
        assert_eq!(events[3].step, 1);
        assert!(events[0].i < events[3].i);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replay_of_empty_dir_fails_loudly() {
        let dir = tmpdir("empty");
        std::fs::create_dir_all(dir.join("frames")).unwrap();
        assert!(ReplaySource::open(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
