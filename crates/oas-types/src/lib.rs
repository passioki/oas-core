//! oas-types: shared data contracts. OS-agnostic; no platform deps allowed.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point { pub x: i32, pub y: i32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }

impl Rect { pub fn contains(&self, p: Point) -> bool {
    p.x >= self.x && p.y >= self.y && p.x < self.x + self.w && p.y < self.y + self.h } }

/// A captured frame. `rgba` is tightly packed, top-left origin.
#[derive(Debug, Clone)]
pub struct Frame { pub width: u32, pub height: u32, pub rgba: Vec<u8> }

impl Frame { pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
    assert_eq!(rgba.len(), (width as usize) * (height as usize) * 4, "frame buffer size mismatch");
    Self { width, height, rgba } } }

/// Named exits (SPEC 1.19 C6): the ONLY places a task may end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Done, NoResource, WindowClosed, NoTarget, ResourceEmpty,
    Exhausted, Failed, Skipped,
}

/// Three-state outcome (SPEC 1.19 C7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome { Done, Failed, Exhausted }

impl Exit { pub fn to_outcome(self) -> Outcome {
    match self {
        Exit::Done | Exit::ResourceEmpty => Outcome::Done,
        Exit::Exhausted => Outcome::Exhausted,
        _ => Outcome::Failed,
    } } }

/// Expiry policy: unifies periodic and temporary tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expire { Session, Daily, Weekly, Monthly, Never }

/// Capture port.
pub trait ScreenSource { fn grab(&mut self) -> Result<Frame, String>; }

/// Input port.
pub trait InputSink {
    fn click(&mut self, p: Point) -> Result<(), String>;
    fn swipe(&mut self, from: Point, to: Point, ms: u32) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn exit_mapping_is_explicit() {
        assert_eq!(Exit::Done.to_outcome(), Outcome::Done);
        assert_eq!(Exit::ResourceEmpty.to_outcome(), Outcome::Done);
        assert_eq!(Exit::Exhausted.to_outcome(), Outcome::Exhausted);
        assert_eq!(Exit::Failed.to_outcome(), Outcome::Failed);
    }
    #[test] fn rect_contains_corners() {
        let r = Rect { x: 10, y: 20, w: 5, h: 5 };
        assert!(r.contains(Point { x: 10, y: 20 }));
        assert!(r.contains(Point { x: 14, y: 24 }));
        assert!(!r.contains(Point { x: 15, y: 24 }));
    }
}
