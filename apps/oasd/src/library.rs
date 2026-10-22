//! Global template asset library (ENGINE §22.22 phase 1).
//!
//! A task bundle owns its assets; the library is shared by all tasks. Library
//! templates carry **no ROI** — the runtime searches the whole frame (§22.20),
//! which is exactly how a reusable button/icon should behave.
//!
//! References inside flows use the `lib:<id>` namespace, so a library asset and
//! a task asset can never collide.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_DIR: &str = "library";
pub const DEFAULT_THRESHOLD: f32 = 0.85;
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibraryAsset {
    pub id: String,
    pub file: String,
    #[serde(default)]
    pub threshold: Option<f32>,
    #[serde(default)]
    pub rev: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryFile {
    pub schema_version: u32,
    #[serde(default)]
    pub design_resolution: Option<[u32; 2]>,
    #[serde(default)]
    pub items: Vec<LibraryAsset>,
}

impl Default for LibraryFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            design_resolution: None,
            items: Vec::new(),
        }
    }
}

pub struct Library {
    root: PathBuf,
}

fn crop_rgba(frame: &oas_types::Frame, roi: [i32; 4]) -> Result<oas_types::Frame, String> {
    let [x, y, w, h] = roi;
    if w <= 0 || h <= 0 || x < 0 || y < 0 {
        return Err(format!("roi [x,y,w,h] = {roi:?} must be non-negative with w,h > 0"));
    }
    if (x + w) as u32 > frame.width || (y + h) as u32 > frame.height {
        return Err(format!(
            "roi [x,y,w,h] = {roi:?} is outside the {}x{} frame",
            frame.width, frame.height
        ));
    }
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as u32 {
        let src_y = y as u32 + row;
        let start = ((src_y * frame.width + x as u32) * 4) as usize;
        let end = start + (w as usize) * 4;
        rgba.extend_from_slice(&frame.rgba[start..end]);
    }
    Ok(oas_types::Frame::new(w as u32, h as u32, rgba))
}

impl Library {
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn assets_path(&self) -> PathBuf {
        self.root.join("assets.json")
    }

    fn images_dir(&self) -> PathBuf {
        self.root.join("images")
    }

    pub fn load_file(&self) -> Result<LibraryFile, String> {
        let path = self.assets_path();
        if !path.is_file() {
            return Ok(LibraryFile::default());
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("library: read `{}`: {e}", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("library: parse `{}`: {e}", path.display()))
    }

    pub fn list(&self) -> Result<Vec<LibraryAsset>, String> {
        Ok(self.load_file()?.items)
    }

    /// `lib:<id>` selectors for every library asset (used to validate flows).
    pub fn selector_ids(&self) -> Result<std::collections::BTreeSet<String>, String> {
        Ok(self
            .load_file()?
            .items
            .iter()
            .map(|i| format!("lib:{}", i.id))
            .collect())
    }

    fn next_image_rel(&self) -> Result<String, String> {
        let dir = self.images_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("library: create `{}`: {e}", dir.display()))?;
        let mut seq: u32 = 1;
        loop {
            let rel = format!("images/a{seq:03}.png");
            if !self.root.join(&rel).exists() {
                return Ok(rel);
            }
            seq += 1;
            if seq > 9999 {
                return Err("library: image sequence exhausted".to_string());
            }
        }
    }

    /// Crop the last captured frame into the library as a template asset.
    ///
    /// Duplicates are rejected unless `replace` is set (then `rev + 1`).
    pub fn save_template(
        &self,
        frame: &oas_types::Frame,
        roi: [i32; 4],
        id: &str,
        threshold: Option<f32>,
        replace: bool,
    ) -> Result<LibraryAsset, String> {
        oas_task::validate_id(id).map_err(|e| format!("library asset rejected: {e}"))?;
        let t = threshold.unwrap_or(DEFAULT_THRESHOLD);
        if !(0.0..=1.0).contains(&t) || !t.is_finite() {
            return Err(format!(
                "library asset `{id}` rejected: threshold {t} must be within [0,1]"
            ));
        }
        let mut af = self.load_file()?;
        let existing = af.items.iter().position(|i| i.id == id);
        let rev = match (replace, existing) {
            (false, Some(i)) => {
                return Err(format!(
                    "library asset id `{id}` already exists (items[{i}]); \
                     send `replace: true` to overwrite it (rev+1)"
                ))
            }
            (true, None) => {
                return Err(format!(
                    "library asset id `{id}` does not exist; drop `replace` to create it"
                ))
            }
            (false, None) => 1,
            (true, Some(i)) => af.items[i].rev.unwrap_or(1).saturating_add(1),
        };
        let cropped = crop_rgba(frame, roi)?;
        if let Some([dw, dh]) = af.design_resolution {
            if (dw, dh) != (frame.width, frame.height) {
                return Err(format!(
                    "library design resolution mismatch: authored for {dw}x{dh}, frame is {}x{}",
                    frame.width, frame.height
                ));
            }
        } else {
            af.design_resolution = Some([frame.width, frame.height]);
        }
        let png = oas_codec::encode_png(&cropped).map_err(|e| format!("library encode png: {e}"))?;
        let rel = match existing {
            Some(i) => af.items[i].file.clone(),
            None => self.next_image_rel()?,
        };
        let abs = self.root.join(&rel);
        if let Some(dir) = abs.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("library: create `{}`: {e}", dir.display()))?;
        }
        // write image first, then the manifest (a crash leaves an orphan image,
        // never a manifest pointing at a missing file)
        std::fs::write(&abs, png).map_err(|e| format!("library: write `{}`: {e}", abs.display()))?;
        let asset = LibraryAsset {
            id: id.to_string(),
            file: rel,
            threshold: Some(t),
            rev: Some(rev),
        };
        match existing {
            Some(i) => af.items[i] = asset.clone(),
            None => af.items.push(asset.clone()),
        }
        let text = serde_json::to_string_pretty(&af)
            .map_err(|e| format!("library: serialize: {e}"))?;
        std::fs::write(self.assets_path(), text + "\n")
            .map_err(|e| format!("library: write `{}`: {e}", self.assets_path().display()))?;
        Ok(asset)
    }

    /// Load every library template into runtime assets under `lib:<id>`.
    pub fn load_templates(
        &self,
    ) -> Result<Vec<(String, oas_runtime::Template)>, String> {
        let af = self.load_file()?;
        let mut out = Vec::new();
        for item in af.items {
            let path = self.root.join(&item.file);
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("library asset `{}`: read `{}`: {e}", item.id, path.display()))?;
            let frame = oas_codec::decode_png(&bytes)
                .map_err(|e| format!("library asset `{}`: decode png: {e}", item.id))?;
            let gray = oas_perceive::GrayImage::from_frame(&frame);
            out.push((
                format!("lib:{}", item.id),
                oas_runtime::Template {
                    gray,
                    roi: None,
                    threshold: item.threshold.unwrap_or(DEFAULT_THRESHOLD),
                    click_polygon: None,
                },
            ));
        }
        Ok(out)
    }

    /// One library template, for the editor's image-match test.
    pub fn load_one(&self, id: &str) -> Result<Option<(String, oas_runtime::Template)>, String> {
        let want = format!("lib:{id}");
        Ok(self
            .load_templates()?
            .into_iter()
            .find(|(k, _)| *k == want))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "oas-lib-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn frame64() -> oas_types::Frame {
        let mut rgba = vec![9u8; 64 * 48 * 4];
        for i in (3..rgba.len()).step_by(4) {
            rgba[i] = 255;
        }
        for y in 10..21u32 {
            for x in 10..31u32 {
                let i = ((y * 64 + x) * 4) as usize;
                rgba[i] = ((x * 7 + y * 13) % 251) as u8;
            }
        }
        oas_types::Frame::new(64, 48, rgba)
    }

    #[test]
    fn save_list_and_load_templates_roundtrip() {
        let dir = tmp("rt");
        let lib = Library::open(&dir);
        assert!(lib.list().unwrap().is_empty());
        let f = frame64();
        let a = lib
            .save_template(&f, [10, 10, 21, 11], "预设入口", None, false)
            .expect("save");
        assert_eq!(a.rev, Some(1));
        assert!(dir.join(&a.file).is_file());
        let list = lib.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "预设入口");
        let tpls = lib.load_templates().unwrap();
        assert_eq!(tpls.len(), 1);
        assert_eq!(tpls[0].0, "lib:预设入口");
        assert_eq!((tpls[0].1.gray.width, tpls[0].1.gray.height), (21, 11));
        assert!(tpls[0].1.roi.is_none(), "library templates search the whole frame");
        assert_eq!(lib.selector_ids().unwrap().len(), 1);
        // duplicate rejected, replace bumps rev
        assert!(lib
            .save_template(&f, [10, 10, 21, 11], "预设入口", None, false)
            .is_err());
        let b = lib
            .save_template(&f, [12, 12, 10, 10], "预设入口", Some(0.9), true)
            .expect("replace");
        assert_eq!(b.rev, Some(2));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_bad_id_roi_and_threshold_without_writing() {
        let dir = tmp("bad");
        let lib = Library::open(&dir);
        let f = frame64();
        assert!(lib.save_template(&f, [10, 10, 21, 11], "", None, false).is_err());
        assert!(lib.save_template(&f, [60, 10, 21, 11], "x", None, false).is_err());
        assert!(lib.save_template(&f, [10, 10, 0, 11], "x", None, false).is_err());
        assert!(lib
            .save_template(&f, [10, 10, 21, 11], "x", Some(1.5), false)
            .is_err());
        assert!(!lib.assets_path().exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
