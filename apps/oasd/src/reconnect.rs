//! Live connection self-healing (ENGINE §22.16 纵向② "断线重连").
//!
//! `adb` links drop in real long runs (emulator hiccup, USB/ADB server restart).
//! These wrappers retry the failing operation after asking adb to reconnect, so
//! the interpreter sees a transient error instead of a dead run.

use oas_adapter_pc::{Adb, AdbInput, AdbScreen};
use oas_types::{Frame, InputSink, Point, ScreenSource};

/// How many reconnect attempts one failed operation may trigger.
const MAX_RECONNECTS: u32 = 2;
const RECONNECT_PAUSE_MS: u64 = 800;

pub struct ReconnectingScreen {
    adb: Adb,
    serial: String,
    screen: AdbScreen,
}

impl ReconnectingScreen {
    pub fn new(adb: Adb, serial: &str) -> Self {
        let screen = adb.screen(serial);
        Self {
            adb,
            serial: serial.to_string(),
            screen,
        }
    }
}

impl ScreenSource for ReconnectingScreen {
    fn grab(&mut self) -> Result<Frame, String> {
        let mut last = String::new();
        for attempt in 0..=MAX_RECONNECTS {
            match self.screen.grab() {
                Ok(f) => return Ok(f),
                Err(e) => {
                    last = e;
                    if attempt == MAX_RECONNECTS {
                        break;
                    }
                    eprintln!(
                        "reconnect: screen grab failed ({last}); adb reconnect attempt {}/{}",
                        attempt + 1,
                        MAX_RECONNECTS
                    );
                    let _ = self.adb.reconnect(&self.serial);
                    std::thread::sleep(std::time::Duration::from_millis(RECONNECT_PAUSE_MS));
                    self.screen = self.adb.screen(&self.serial);
                }
            }
        }
        Err(format!(
            "screen grab failed after {MAX_RECONNECTS} reconnect attempt(s): {last}"
        ))
    }
}

pub struct ReconnectingInput {
    adb: Adb,
    serial: String,
    input: AdbInput,
}

impl ReconnectingInput {
    pub fn new(adb: Adb, serial: &str) -> Self {
        let input = adb.input(serial);
        Self {
            adb,
            serial: serial.to_string(),
            input,
        }
    }

    fn retry<T>(&mut self, mut op: impl FnMut(&mut AdbInput) -> Result<T, String>) -> Result<T, String> {
        let mut last = String::new();
        for attempt in 0..=MAX_RECONNECTS {
            match op(&mut self.input) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    last = e;
                    if attempt == MAX_RECONNECTS {
                        break;
                    }
                    eprintln!(
                        "reconnect: input failed ({last}); adb reconnect attempt {}/{}",
                        attempt + 1,
                        MAX_RECONNECTS
                    );
                    let _ = self.adb.reconnect(&self.serial);
                    std::thread::sleep(std::time::Duration::from_millis(RECONNECT_PAUSE_MS));
                    self.input = self.adb.input(&self.serial);
                }
            }
        }
        Err(format!(
            "input failed after {MAX_RECONNECTS} reconnect attempt(s): {last}"
        ))
    }
}

impl InputSink for ReconnectingInput {
    fn click(&mut self, p: Point) -> Result<(), String> {
        self.retry(|input| input.click(p))
    }

    fn swipe(&mut self, from: Point, to: Point, ms: u32) -> Result<(), String> {
        self.retry(|input| input.swipe(from, to, ms))
    }
}
