//! P1-b integration: a recorded bundle drives the interpreter offline.
//!
//! This is the loop the whole rewrite depends on:
//!   record (device)  ->  bundle on disk  ->  interpreter replays it  ->  events
//!
//! No device, no account, no OCR: only frames + assets + a flow.

use std::collections::BTreeSet;

use oas_flow::Flow;
use oas_obs::{Recorder, ReplaySource};
use oas_perceive::GrayImage;
use oas_runtime::{AssetSet, Budget, Interpreter, RunResult, Template};
use oas_types::{Exit, Frame, InputSink, Outcome, Point, Rect, ScreenSource};

/// Same patterned 10x10 tile as the unit tests (flat tiles have zero variance,
/// so NCC would be undefined - that pitfall is intentional here too).
fn tile_px(x: u32, y: u32) -> u8 {
    (((x * 7 + y * 13) % 251) + 3) as u8
}

fn frame_with_tile(tile_at: Option<(u32, u32)>) -> Frame {
    let (w, h) = (64u32, 48u32);
    let mut rgba = vec![30u8; (w * h * 4) as usize];
    if let Some((tx, ty)) = tile_at {
        for y in 0..10u32 {
            for x in 0..10u32 {
                let v = tile_px(x, y);
                let i = (((ty + y) * w + (tx + x)) * 4) as usize;
                rgba[i] = v;
                rgba[i + 1] = v;
                rgba[i + 2] = v;
                rgba[i + 3] = 255;
            }
        }
    }
    Frame::new(w, h, rgba)
}

struct FakeScreen {
    frames: Vec<Frame>,
    idx: usize,
}

impl ScreenSource for FakeScreen {
    fn grab(&mut self) -> Result<Frame, String> {
        let f = self.frames.get(self.idx).ok_or("fake screen exhausted")?.clone();
        self.idx += 1;
        Ok(f)
    }
}

struct RecordingInput {
    clicks: Vec<Point>,
}

impl InputSink for RecordingInput {
    fn click(&mut self, p: Point) -> Result<(), String> {
        self.clicks.push(p);
        Ok(())
    }
    fn swipe(&mut self, _f: Point, _t: Point, _ms: u32) -> Result<(), String> {
        Ok(())
    }
}

fn assets() -> AssetSet {
    let gray = GrayImage::new(
        10,
        10,
        (0..100u32).map(|i| tile_px(i % 10, i / 10)).collect(),
    );
    let mut set = AssetSet::new();
    set.insert(
        "I_TILE",
        Template {
            gray,
            roi: None,
            threshold: 0.9,
            click_polygon: None,
        },
    );
    set
}

fn tmpdir(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("oas_rt_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

#[test]
fn recorded_bundle_drives_the_interpreter_offline() {
    let dir = tmpdir("replay_drive");

    // 1) record 3 frames (tile present at (12,10) in every frame)
    let mut rec = Recorder::create(&dir, "fake").expect("recorder");
    let mut src = FakeScreen {
        frames: (0..3).map(|_| frame_with_tile(Some((12, 10)))).collect(),
        idx: 0,
    };
    for _ in 0..3 {
        rec.capture(&mut src).expect("capture");
    }
    rec.finish().expect("finish recording");

    // 2) replay the bundle through the interpreter
    let replay = ReplaySource::open(&dir).expect("open bundle");
    let mut interp = Interpreter::new(
        replay,
        RecordingInput { clicks: vec![] },
        assets(),
        Budget {
            max_wall_s: 30.0,
            max_steps: 100,
        },
    );

    let flow = Flow::from_json(
        r#"{"schema_version":1,"name":"sample","steps":[
             {"op":"appear","args":{"sel":"I_TILE"}},
             {"op":"click","args":{"sel":"I_TILE"}},
             {"op":"log","args":{"text":"'tile clicked'"}},
             {"op":"finish","args":{"exit":"done"}}]}"#,
    )
    .expect("flow");

    // load-time asset check would normally run before this
    let known: BTreeSet<String> = ["I_TILE".to_string()].into_iter().collect();
    assert!(flow.validate_assets(&known).is_ok());

    let result = interp.run(&flow);
    assert_eq!(result, RunResult::Finished(Exit::Done), "events: {:?}", interp.events());
    assert_eq!(result.outcome(), Outcome::Done);

    // every step is visible in the structured log
    let ops: Vec<&str> = interp.events().iter().map(|e| e.op.as_str()).collect();
    assert_eq!(ops, vec!["appear", "click", "log", "finish"]);
    let click = &interp.events()[1];
    assert!(click.detail.contains("target=(12,10)"), "{}", click.detail);

    // and the action sequence is what the recording implies
    // (RecordingInput lives inside the interpreter; the event detail carries it)

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn replay_that_runs_out_fails_loudly() {
    let dir = tmpdir("replay_short");
    let mut rec = Recorder::create(&dir, "fake").expect("recorder");
    let mut src = FakeScreen {
        frames: vec![frame_with_tile(Some((12, 10)))],
        idx: 0,
    };
    rec.capture(&mut src).expect("capture");
    rec.finish().expect("finish");

    // an asset that can never match forces wait_until to re-grab; the bundle
    // only holds ONE frame, so the second grab must fail loudly.
    let mut set = assets();
    let never = GrayImage::new(10, 10, (0..100u32).map(|i| (i % 5) as u8).collect());
    set.insert(
        "I_NEVER",
        Template {
            gray: never,
            roi: Some(Rect { x: 0, y: 0, w: 64, h: 48 }),
            threshold: 0.99,
            click_polygon: None,
        },
    );

    let replay = ReplaySource::open(&dir).expect("open");
    let mut interp = Interpreter::new(
        replay,
        RecordingInput { clicks: vec![] },
        set,
        Budget::default(),
    );
    let flow = Flow::from_json(
        r#"{"schema_version":1,"name":"short","steps":[
             {"op":"wait_until","args":{"cond":"appear('I_NEVER')","timeout_s":3.0}},
             {"op":"finish","args":{"exit":"done"}}]}"#,
    )
    .expect("flow");

    match interp.run(&flow) {
        RunResult::Failed(msg) => assert!(
            msg.contains("exhausted"),
            "expected a loud 'replay exhausted' error, got: {msg}"
        ),
        other => panic!("expected Failed, got {other:?} (events: {:?})", interp.events()),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
