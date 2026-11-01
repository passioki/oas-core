//! Derived frame/asset relations. **No persistence here.**
//!
//! The only stored relation is `Asset.positions` (asset -> frames). Everything
//! else in this module is computed on demand for the read-only relation browser:
//!   - frame -> assets
//!   - asset <-> asset co-occurrence
//!   - frame <-> frame shared assets
//!
//! Keeping these derived avoids a second source of truth and keeps the model
//! usable even while captures/assets are still incomplete.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FrameRelation {
    pub id: u64,
    pub name: String,
    pub category: String,
    pub assets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AssetRelation {
    pub id: String,
    pub kind: String,
    pub positions: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AssetCooccurrence {
    pub a: String,
    pub b: String,
    pub count: usize,
    pub frames: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FrameSharedAssets {
    pub a: u64,
    pub b: u64,
    pub assets: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationSnapshot {
    pub task: String,
    pub frames: Vec<FrameRelation>,
    pub assets: Vec<AssetRelation>,
    pub asset_cooccurrence: Vec<AssetCooccurrence>,
    pub frame_shared_assets: Vec<FrameSharedAssets>,
}

const MAX_PAIRS_OUT: usize = 2000;

/// Build a relation snapshot from existing frame metadata and asset positions.
/// `frames` is `(id, name, category)`; `assets` is `(id, kind, positions)`.
pub fn build(
    task: &str,
    frames: &[(u64, String, String)],
    assets: &[(String, String, Vec<u64>)],
) -> RelationSnapshot {
    let known_frames: BTreeSet<u64> = frames.iter().map(|(id, _, _)| *id).collect();
    let mut frame_assets: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    for (id, _, _) in frames {
        frame_assets.entry(*id).or_default();
    }

    let mut asset_relations = Vec::new();
    for (id, kind, positions) in assets {
        let positions: Vec<u64> = positions
            .iter()
            .copied()
            .filter(|f| known_frames.contains(f))
            .collect();
        // positions are the canonical stored relation. Drop unknown frame ids
        // from the derived view instead of crashing on stale data.
        for f in &positions {
            frame_assets.entry(*f).or_default().insert(id.clone());
        }
        asset_relations.push(AssetRelation {
            id: id.clone(),
            kind: kind.clone(),
            positions,
        });
    }

    let frames_out: Vec<FrameRelation> = frames
        .iter()
        .map(|(id, name, category)| FrameRelation {
            id: *id,
            name: name.clone(),
            category: category.clone(),
            assets: frame_assets
                .get(id)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default(),
        })
        .collect();

    // Asset co-occurrence: assets sharing at least one frame.
    let mut co: BTreeMap<(String, String), (usize, Vec<u64>)> = BTreeMap::new();
    for (fid, ids) in &frame_assets {
        let list: Vec<&String> = ids.iter().collect();
        for i in 0..list.len() {
            for j in (i + 1)..list.len() {
                let (a, b) = if list[i] < list[j] {
                    (list[i].clone(), list[j].clone())
                } else {
                    (list[j].clone(), list[i].clone())
                };
                let e = co.entry((a, b)).or_insert_with(|| (0, Vec::new()));
                e.0 += 1;
                e.1.push(*fid);
            }
        }
    }
    let mut asset_cooccurrence: Vec<AssetCooccurrence> = co
        .into_iter()
        .map(|((a, b), (count, mut frames))| {
            frames.sort_unstable();
            frames.dedup();
            AssetCooccurrence {
                a,
                b,
                count,
                frames,
            }
        })
        .collect();
    asset_cooccurrence.sort_by(|x, y| {
        y.count
            .cmp(&x.count)
            .then_with(|| x.a.cmp(&y.a))
            .then_with(|| x.b.cmp(&y.b))
    });
    asset_cooccurrence.truncate(MAX_PAIRS_OUT);

    // Frame pairs sharing at least one asset.
    let mut frame_pairs: BTreeMap<(u64, u64), BTreeSet<String>> = BTreeMap::new();
    for (aid, kind, positions) in assets {
        if kind == "template_set" {
            continue;
        }
        if positions.len() < 2 {
            continue;
        }
        let mut pos: Vec<u64> = positions
            .iter()
            .copied()
            .filter(|f| known_frames.contains(f))
            .collect();
        pos.sort_unstable();
        pos.dedup();
        for i in 0..pos.len() {
            for j in (i + 1)..pos.len() {
                frame_pairs
                    .entry((pos[i], pos[j]))
                    .or_default()
                    .insert(aid.clone());
            }
        }
    }
    let mut frame_shared_assets: Vec<FrameSharedAssets> = frame_pairs
        .into_iter()
        .map(|((a, b), assets)| FrameSharedAssets {
            a,
            b,
            assets: assets.into_iter().collect(),
        })
        .collect();
    frame_shared_assets.sort_by(|x, y| {
        y.assets
            .len()
            .cmp(&x.assets.len())
            .then_with(|| x.a.cmp(&y.a))
            .then_with(|| x.b.cmp(&y.b))
    });
    frame_shared_assets.truncate(MAX_PAIRS_OUT);

    RelationSnapshot {
        task: task.to_string(),
        frames: frames_out,
        assets: asset_relations,
        asset_cooccurrence,
        frame_shared_assets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames() -> Vec<(u64, String, String)> {
        vec![
            (4, "我的式神".into(), "式神录".into()),
            (6, "预设界面".into(), "式神录".into()),
            (7, "即将更换预设".into(), "式神录".into()),
        ]
    }

    #[test]
    fn frame_assets_and_asset_pair_are_derived_from_positions() {
        let assets = vec![
            ("预设".to_string(), "ocr_line".to_string(), vec![4, 6]),
            ("确认".to_string(), "ocr_line".to_string(), vec![6, 7]),
        ];
        let snap = build("返回庭院", &frames(), &assets);
        let f4 = snap.frames.iter().find(|f| f.id == 4).unwrap();
        assert_eq!(f4.assets, vec!["预设"]);
        let pair = snap
            .asset_cooccurrence
            .iter()
            .find(|p| p.a == "确认" && p.b == "预设")
            .unwrap();
        assert_eq!(pair.count, 1);
        assert_eq!(pair.frames, vec![6]);
    }

    #[test]
    fn shared_frames_and_unknown_positions_are_handled() {
        let assets = vec![("预设".to_string(), "ocr_line".to_string(), vec![4, 6, 999])];
        let snap = build("返回庭院", &frames(), &assets);
        assert_eq!(snap.assets[0].positions, vec![4, 6]);
        let shared = snap
            .frame_shared_assets
            .iter()
            .find(|p| p.a == 4 && p.b == 6)
            .unwrap();
        assert_eq!(shared.assets, vec!["预设"]);
        assert!(snap.frames.iter().all(|f| f.id != 999));
    }
}
