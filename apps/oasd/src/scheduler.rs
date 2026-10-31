//! 调度器 v3 纯函数层（ENGINE-DESIGN §22.34 / DELIVERY-PLAN M2 · N37）。
//!
//! This module is the **mechanism** layer: no filesystem, no clock, no globals.
//! Everything that needs IO lives in `schedule.rs`; everything here is a pure
//! function over `(config, state, now)` so the whole scheduling matrix is unit
//! testable (cross-midnight windows, weekly quota resets, cooldowns, pass
//! order, wake calculation, temp rules, exit classification).
//!
//! # 三层分离（§22.34.1）
//! * **统一任务模板**（M0）: only `meta.exits` is consumed here — result
//!   classification via [`parse_meta_exits`]. Missing `exits` keeps the
//!   documented default (`done -> success`, everything else retryable).
//! * **调度器 v3**: periodic clock / guards / candidate filtering / wake /
//!   result recording. **No game constants** (no 05:00, no maintenance day, no
//!   built-in task list — §22.34.4).
//! * **玩家配置** `scheduler.json` v3: calendar / guards / entries — every
//!   value comes from the player.
//!
//! # Time model
//! Instants are **epoch milliseconds (UTC)**. "Local time" is
//! `utc_ms + utc_offset_min * 60_000`; we do not depend on `chrono`/`time`
//! (civil-date math is Howard Hinnant's algorithm, pure integers). The offset
//! is a player/config value ([`Calendar::utc_offset_min`]); the engine itself
//! hardcodes no timezone (the binary resolves a default, see `schedule.rs`).
//!
//! Cycle ids are what quota counters hang off:
//! * `day`  = civil date of the **logical** day. With `day_reset = "05:00"`,
//!   local 04:59 still belongs to the previous cycle and 05:00 starts the next.
//! * `week` = civil date of the week start. `week_reset = {weekday: 0, time:
//!   "05:00"}` means Monday 05:00 local (weekday 0 = Monday, matching
//!   `docs/scheduler-architecture.md`).
//!
//! # Pass / entry semantics（唯一权威）
//! * The `entries` array order **is** the execution order — nothing re-sorts.
//! * Per pass every runnable entry runs **at most once**.
//! * periodic: must be inside `windows` ∩ quota not full ∩ not cooling down.
//! * temp (临时任务 B): every pass, once; **no window / quota / completion
//!   memory**. The executor resets the task `state/` before *and* after each
//!   temp run (`schedule.rs`); this module only encodes "temp is always
//!   runnable while guards allow".
//! * `one_shot` / `continue` are deleted by design: a run only classifies as
//!   success / resource / blocked / retryable / fatal.
//!
//! # scheduler.json v3（玩家配置）
//! ```json
//! {
//!   "schema_version": 3,
//!   "config": {
//!     "mode": "loop",                    // "loop" | "stop" (default stop)
//!     "loop_interval_s": 60,             // seconds between passes in loop mode
//!     "loop_rounds": 0,                  // 0 = unlimited passes in loop mode
//!     "utc_offset_min": 480,             // optional; local = utc + offset
//!     "calendar": {
//!       "day_reset": "05:00",            // logical day boundary (default 00:00)
//!       "week_reset": { "weekday": 0, "time": "05:00" }  // 0 = Monday
//!     },
//!     "guards": {
//!       "active_hours": [["08:00", "23:00"]],          // empty = always active
//!       "blackouts": [{"days": [1], "start": "02:00", "end": "02:10",
//!                      "reason": "维护"}]
//!     },
//!     "defaults": { "retry": 1, "cooldown": 300 }
//!   },
//!   "entries": [{
//!     "entry_id": "e1",
//!     "task": "任务1",
//!     "enabled": true,
//!     "lifecycle": "periodic",          // "periodic" | "temp"
//!     "windows": [{"days": [], "start": "08:00", "end": "12:00"}],
//!     "quota": {"per": "day", "times": 2},
//!     "params": {},
//!     "retry": 1,
//!     "cooldown": 300
//!   }]
//! }
//! ```
//!
//! Windows / active_hours / blackouts are **time-of-day ranges** with an
//! optional weekday filter (empty = every day). `end <= start` crosses
//! midnight, `end == start` means "the whole day". Everything is validated
//! before a single byte of state is written.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use oas_types::Exit;

/// Schema version of `scheduler.json` (v3 replaces the v1 `every_s` plan).
pub const SCHEDULE_SCHEMA_VERSION: u32 = 3;
/// State file schema version (same document family as the config).
pub const STATE_SCHEMA_VERSION: u32 = 3;

pub const MS_PER_MIN: i64 = 60_000;
pub const MS_PER_SEC: i64 = 1_000;
pub const DAY_MS: i64 = 86_400_000;
pub const WEEK_MS: i64 = 7 * DAY_MS;
/// Upper bound for one `next_wake` sleep (§22.34.4 step 7: "30s 上限").
pub const MAX_SLEEP_MS: i64 = 30_000;

pub const DEFAULT_LOOP_INTERVAL_S: u64 = 60;
pub const DEFAULT_RETRY: u32 = 1;
pub const DEFAULT_COOLDOWN_S: u64 = 300;
pub const DEFAULT_UTC_OFFSET_MIN: i32 = 480; // Asia/Shanghai; overridable

pub const MAX_RETRY: u32 = 10;
pub const MAX_COOLDOWN_S: u64 = 7 * 24 * 3600;
pub const MAX_QUOTA_TIMES: u32 = 1000;
pub const MAX_ENTRY_ID_LEN: usize = 128;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lifecycle {
    /// 周期任务: windows + quota + cooldown.
    Periodic,
    /// 临时任务 B: 每轮按 entries 顺序执行一次，执行前后重置 state，无完成记忆.
    Temp,
}

impl Lifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Lifecycle::Periodic => "periodic",
            Lifecycle::Temp => "temp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuotaPer {
    Day,
    Week,
}

impl QuotaPer {
    pub fn as_str(self) -> &'static str {
        match self {
            QuotaPer::Day => "day",
            QuotaPer::Week => "week",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    /// 一轮结束后短暂间隔再开下一轮（temp 每轮再跑一次）.
    Loop,
    /// 跑完一轮（或等待到下一个可跑时刻后跑完一轮）就退出.
    Stop,
}

impl RunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RunMode::Loop => "loop",
            RunMode::Stop => "stop",
        }
    }
}

/// The only five result classes (§22.34.2). No `continue` / `stop`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultClass {
    /// 完成（done / resource_empty by default）.
    Success,
    /// 资源耗尽（no tickets / no stamina / no target）— 任务正常结束，不再重试.
    Resource,
    /// 环境阻塞（维护/不在目标页面）— 冷却 + ALERT.
    Blocked,
    /// 可重试失败 — 用尽 retry 后冷却.
    Retryable,
    /// 致命错误 — 停机（`--keep-going` 覆盖）.
    Fatal,
}

impl ResultClass {
    pub fn as_str(self) -> &'static str {
        match self {
            ResultClass::Success => "success",
            ResultClass::Resource => "resource",
            ResultClass::Blocked => "blocked",
            ResultClass::Retryable => "retryable",
            ResultClass::Fatal => "fatal",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "success" => Ok(ResultClass::Success),
            "resource" => Ok(ResultClass::Resource),
            "blocked" => Ok(ResultClass::Blocked),
            "retryable" => Ok(ResultClass::Retryable),
            "fatal" => Ok(ResultClass::Fatal),
            other => Err(format!(
                "unknown result `{other}` (only success/resource/blocked/retryable/fatal)"
            )),
        }
    }

    /// Quota counts **completed** runs only (success/resource). A blocked or
    /// retryable run did not spend the allowance — the cooldown gates retries.
    pub fn consumes_quota(self) -> bool {
        matches!(self, ResultClass::Success | ResultClass::Resource)
    }

    /// Classes that put the entry into cooldown.
    pub fn sets_cooldown(self) -> bool {
        matches!(self, ResultClass::Blocked | ResultClass::Retryable)
    }
}

/// Why the global guards stopped a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardBlock {
    Blackout,
    OutsideActiveHours,
}

impl GuardBlock {
    pub fn as_str(self) -> &'static str {
        match self {
            GuardBlock::Blackout => "blackout",
            GuardBlock::OutsideActiveHours => "outside_active_hours",
        }
    }
}

// ---------------------------------------------------------------------------
// Time helpers (pure integer math; no chrono/time dependency)
// ---------------------------------------------------------------------------

/// Parse `"HH:MM"` (accepts `H:MM`, optional `:00` seconds) into minutes of day.
pub fn parse_hhmm(s: &str) -> Result<i64, String> {
    let raw = s.trim();
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(format!("time `{raw}`: expected HH:MM"));
    }
    let hour: i64 = parts[0]
        .parse()
        .map_err(|_| format!("time `{raw}`: hour `{}` is not a number", parts[0]))?;
    let min: i64 = parts[1]
        .parse()
        .map_err(|_| format!("time `{raw}`: minute `{}` is not a number", parts[1]))?;
    if !(0..=23).contains(&hour) {
        return Err(format!("time `{raw}`: hour must be 0..=23"));
    }
    if !(0..=59).contains(&min) {
        return Err(format!("time `{raw}`: minute must be 0..=59"));
    }
    if parts.len() == 3 {
        let sec: i64 = parts[2]
            .parse()
            .map_err(|_| format!("time `{raw}`: second is not a number"))?;
        if sec != 0 {
            return Err(format!("time `{raw}`: seconds must be 00 (minute resolution)"));
        }
    }
    Ok(hour * 60 + min)
}

/// `300 -> "05:00"`.
pub fn format_hhmm(minutes: i64) -> String {
    let m = minutes.rem_euclid(24 * 60);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Days since 1970-01-01 -> (year, month, day). Hinnant's civil_from_days.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (y + i64::from(m <= 2), m, d)
}

/// (year, month, day) -> days since 1970-01-01. Hinnant's days_from_civil.
/// Inverse of [`civil_from_days`]; used by tests and future tooling.
#[allow(dead_code)]
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Epoch day -> weekday, 0 = Monday … 6 = Sunday (project convention,
/// `docs/scheduler-architecture.md`).
pub fn weekday_of_epoch_day(day: i64) -> u8 {
    (day + 3).rem_euclid(7) as u8
}

/// First epoch day >= 0 whose weekday is `weekday` (0 = Monday).
pub fn week_anchor_day(weekday: u8) -> i64 {
    (i64::from(weekday) - 3).rem_euclid(7)
}

/// `utc_ms` -> `"YYYY-MM-DDTHH:MM:SS"` in the configured local offset.
pub fn format_local_datetime(utc_ms: i64, utc_offset_min: i32) -> String {
    let local = utc_ms + i64::from(utc_offset_min) * MS_PER_MIN;
    let day = local.div_euclid(DAY_MS);
    let tod = local.rem_euclid(DAY_MS);
    let (y, m, d) = civil_from_days(day);
    let secs = tod / MS_PER_SEC;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// `"YYYY-MM-DD"` for an epoch day.
pub fn format_date(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------------------
// Calendar + cycles
// ---------------------------------------------------------------------------

/// Player calendar: when the logical day / week starts, and the local offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calendar {
    /// Minutes after local midnight that start a new logical day.
    pub day_reset_min: i64,
    /// 0 = Monday … 6 = Sunday.
    pub week_reset_weekday: u8,
    /// Minutes after local midnight that start the reset weekday.
    pub week_reset_min: i64,
    /// Local time = UTC + this offset.
    pub utc_offset_min: i32,
}

impl Default for Calendar {
    fn default() -> Self {
        Calendar {
            day_reset_min: 0,
            week_reset_weekday: 0,
            week_reset_min: 0,
            utc_offset_min: DEFAULT_UTC_OFFSET_MIN,
        }
    }
}

impl Calendar {
    pub fn to_local(&self, utc_ms: i64) -> i64 {
        utc_ms + i64::from(self.utc_offset_min) * MS_PER_MIN
    }

    pub fn from_local(&self, local_ms: i64) -> i64 {
        local_ms - i64::from(self.utc_offset_min) * MS_PER_MIN
    }

    /// Logical day index containing `utc_ms`.
    pub fn day_index_at(&self, utc_ms: i64) -> i64 {
        let local = self.to_local(utc_ms);
        (local - self.day_reset_min * MS_PER_MIN).div_euclid(DAY_MS)
    }

    /// UTC instant at which logical day `idx` starts.
    pub fn day_start_utc(&self, idx: i64) -> i64 {
        self.from_local(idx * DAY_MS + self.day_reset_min * MS_PER_MIN)
    }

    fn week_anchor_local(&self) -> i64 {
        week_anchor_day(self.week_reset_weekday) * DAY_MS + self.week_reset_min * MS_PER_MIN
    }

    /// Logical week index containing `utc_ms`.
    pub fn week_index_at(&self, utc_ms: i64) -> i64 {
        (self.to_local(utc_ms) - self.week_anchor_local()).div_euclid(WEEK_MS)
    }

    /// UTC instant at which logical week `idx` starts.
    pub fn week_start_utc(&self, idx: i64) -> i64 {
        self.from_local(self.week_anchor_local() + idx * WEEK_MS)
    }

    /// Start date of logical week `idx`.
    pub fn week_start_date(&self, idx: i64) -> (i64, u32, u32) {
        civil_from_days(week_anchor_day(self.week_reset_weekday) + idx * 7)
    }
}

/// Current cycle ids (the keys quota counters are stored under).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cycles {
    pub day_index: i64,
    pub week_index: i64,
    /// Civil date of the logical day, e.g. `"2026-10-23"`.
    pub day: String,
    /// Civil date of the week start, e.g. `"2026-10-19"` (Monday).
    pub week: String,
}

pub fn cycle_ids(cal: &Calendar, utc_ms: i64) -> Cycles {
    let day_index = cal.day_index_at(utc_ms);
    let week_index = cal.week_index_at(utc_ms);
    let (wy, wm, wd) = cal.week_start_date(week_index);
    Cycles {
        day_index,
        week_index,
        day: format_date(day_index),
        week: format!("{wy:04}-{wm:02}-{wd:02}"),
    }
}

pub fn next_day_reset_ms(cal: &Calendar, utc_ms: i64) -> i64 {
    cal.day_start_utc(cal.day_index_at(utc_ms) + 1)
}

pub fn next_week_reset_ms(cal: &Calendar, utc_ms: i64) -> i64 {
    cal.week_start_utc(cal.week_index_at(utc_ms) + 1)
}

/// Weekly quota depends on `week_reset`; this reports whether any enabled entry
/// uses a weekly quota (used by `next_wake` to avoid needless candidates).
fn has_weekly_quota(sched: &Scheduler) -> bool {
    sched.entries.iter().any(|e| {
        e.enabled && matches!(e.quota.as_ref().map(|q| q.per), Some(QuotaPer::Week))
    })
}

// ---------------------------------------------------------------------------
// Time windows (windows / active_hours / blackouts are all this shape)
// ---------------------------------------------------------------------------

/// A time-of-day range, optionally restricted to weekdays.
///
/// * `days` empty = every day; 0 = Monday … 6 = Sunday.
/// * `end_min <= start_min` crosses midnight; `end_min == start_min` = full day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeWindow {
    pub days: Vec<u8>,
    pub start_min: i64,
    pub end_min: i64,
    /// Original strings, kept for status/report echo.
    pub start: String,
    pub end: String,
}

impl TimeWindow {
    pub fn from_parts(days: &[u8], start: &str, end: &str, ctx: &str) -> Result<Self, String> {
        for d in days {
            if *d > 6 {
                return Err(format!("{ctx}.days: weekday {d} out of range 0..=6"));
            }
        }
        let mut uniq: BTreeSet<u8> = BTreeSet::new();
        uniq.extend(days.iter().copied());
        let start_min =
            parse_hhmm(start).map_err(|e| format!("{ctx}.start: {e}"))?;
        let end_min = parse_hhmm(end).map_err(|e| format!("{ctx}.end: {e}"))?;
        Ok(TimeWindow {
            days: uniq.into_iter().collect(),
            start_min,
            end_min,
            start: start.trim().to_string(),
            end: end.trim().to_string(),
        })
    }

    pub fn is_full_day(&self) -> bool {
        self.start_min == self.end_min
    }

    fn applies_to_day(&self, epoch_day: i64) -> bool {
        if self.days.is_empty() {
            return true;
        }
        self.days.contains(&weekday_of_epoch_day(epoch_day))
    }

    /// Is `local_ms` inside this window?
    pub fn contains(&self, local_ms: i64) -> bool {
        let day = local_ms.div_euclid(DAY_MS);
        let tod = local_ms.rem_euclid(DAY_MS) / MS_PER_MIN;
        if self.is_full_day() {
            return self.applies_to_day(day);
        }
        if self.start_min < self.end_min {
            // Same-day range: [start, end).
            self.applies_to_day(day) && tod >= self.start_min && tod < self.end_min
        } else {
            // Cross-midnight: starts on day D, ends on D+1.
            (self.applies_to_day(day) && tod >= self.start_min)
                || (self.applies_to_day(day - 1) && tod < self.end_min)
        }
    }

    /// Earliest start instant strictly after `local_ms`.
    ///
    /// Returns `None` for an always-open window (empty `days` + full day). Even
    /// if the window is open *now* this returns the **next** opening, so callers
    /// that need "next time this becomes available" must only call it when
    /// [`contains`](Self::contains) is false.
    pub fn next_start(&self, local_ms: i64) -> Option<i64> {
        if self.is_full_day() && self.days.is_empty() {
            return None;
        }
        let day0 = local_ms.div_euclid(DAY_MS);
        let mut best: Option<i64> = None;
        for off in 0..=7i64 {
            let day = day0 + off;
            if !self.applies_to_day(day) {
                continue;
            }
            let cand = day * DAY_MS + self.start_min * MS_PER_MIN;
            if cand > local_ms {
                best = Some(best.map_or(cand, |b| b.min(cand)));
            }
        }
        best
    }

    /// If `local_ms` is inside the window, the instant the current occurrence
    /// ends. Always-open windows never end (`None`).
    pub fn current_end(&self, local_ms: i64) -> Option<i64> {
        if !self.contains(local_ms) || self.is_full_day() {
            return None;
        }
        let day = local_ms.div_euclid(DAY_MS);
        let tod = local_ms.rem_euclid(DAY_MS) / MS_PER_MIN;
        if self.start_min < self.end_min {
            Some(day * DAY_MS + self.end_min * MS_PER_MIN)
        } else if tod >= self.start_min {
            Some((day + 1) * DAY_MS + self.end_min * MS_PER_MIN)
        } else {
            Some(day * DAY_MS + self.end_min * MS_PER_MIN)
        }
    }
}

/// A blackout also carries the human reason shown in the ALERT/status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blackout {
    pub window: TimeWindow,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Validated configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Defaults {
    pub retry: u32,
    pub cooldown_s: u64,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            retry: DEFAULT_RETRY,
            cooldown_s: DEFAULT_COOLDOWN_S,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quota {
    pub per: QuotaPer,
    pub times: u32,
}

/// One validated `entries[]` item. `entries` order is the execution order.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub entry_id: String,
    pub task: String,
    pub enabled: bool,
    pub lifecycle: Lifecycle,
    pub windows: Vec<TimeWindow>,
    pub quota: Option<Quota>,
    pub params: BTreeMap<String, Value>,
    pub retry: u32,
    pub cooldown_s: u64,
}

impl Entry {
    /// Total attempts per pass execution (initial try + retries).
    pub fn max_attempts(&self) -> u32 {
        self.retry.saturating_add(1)
    }
}

/// Fully validated scheduler config (pure value — safe to pass around/tests).
#[derive(Debug, Clone, PartialEq)]
pub struct Scheduler {
    pub mode: RunMode,
    pub loop_interval_s: u64,
    pub loop_rounds: u64,
    pub calendar: Calendar,
    pub active_hours: Vec<TimeWindow>,
    pub blackouts: Vec<Blackout>,
    pub defaults: Defaults,
    pub entries: Vec<Entry>,
}

impl Scheduler {
    pub fn entry(&self, entry_id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.entry_id == entry_id)
    }
}

// ---------------------------------------------------------------------------
// Raw (serde) config — validated into the structs above
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
struct RawScheduler {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    config: RawConfig,
    #[serde(default)]
    entries: Vec<RawEntry>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawConfig {
    #[serde(default)]
    mode: Option<RunMode>,
    #[serde(default)]
    loop_interval_s: Option<u64>,
    #[serde(default)]
    loop_rounds: Option<u64>,
    #[serde(default)]
    utc_offset_min: Option<i32>,
    #[serde(default)]
    calendar: RawCalendar,
    #[serde(default)]
    guards: RawGuards,
    #[serde(default)]
    defaults: RawDefaults,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawCalendar {
    #[serde(default)]
    day_reset: Option<String>,
    #[serde(default)]
    week_reset: Option<RawWeekReset>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawWeekReset {
    #[serde(default)]
    weekday: Option<u8>,
    #[serde(default)]
    time: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawGuards {
    #[serde(default)]
    active_hours: Vec<[String; 2]>,
    #[serde(default)]
    blackouts: Vec<RawBlackout>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawBlackout {
    #[serde(default)]
    days: Vec<u8>,
    #[serde(default)]
    start: String,
    #[serde(default)]
    end: String,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawDefaults {
    #[serde(default)]
    retry: Option<u32>,
    #[serde(default)]
    cooldown: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawEntry {
    #[serde(default)]
    entry_id: String,
    #[serde(default)]
    task: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    lifecycle: Option<Lifecycle>,
    #[serde(default)]
    windows: Vec<RawWindow>,
    #[serde(default)]
    quota: Option<RawQuota>,
    #[serde(default)]
    params: serde_json::Map<String, Value>,
    #[serde(default)]
    retry: Option<u32>,
    #[serde(default)]
    cooldown: Option<u64>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawWindow {
    #[serde(default)]
    days: Vec<u8>,
    #[serde(default)]
    start: String,
    #[serde(default)]
    end: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawQuota {
    #[serde(default)]
    per: Option<QuotaPer>,
    #[serde(default)]
    times: u32,
}

// ---------------------------------------------------------------------------
// Config validation (pure, fail-fast, zero writes)
// ---------------------------------------------------------------------------

/// Validate `scheduler.json` with the default local offset.
/// (The binary resolves the player offset and calls the `_with_offset` variant;
/// this convenience form is used by tests/tooling.)
#[allow(dead_code)]
pub fn validate_scheduler(text: &str) -> Result<Scheduler, String> {
    validate_scheduler_with_offset(text, DEFAULT_UTC_OFFSET_MIN)
}

/// Validate `scheduler.json`; `default_offset_min` is used when the config
/// itself does not set `config.utc_offset_min`.
///
/// Every error is prefixed with a `path` into the JSON document so a player can
/// fix it without reading engine source. **No IO, no state** — the caller may
/// rely on "invalid config writes nothing".
pub fn validate_scheduler_with_offset(
    text: &str,
    default_offset_min: i32,
) -> Result<Scheduler, String> {
    let raw: RawScheduler = serde_json::from_str(text)
        .map_err(|e| format!("scheduler config: invalid JSON: {e}"))?;

    if raw.schema_version != SCHEDULE_SCHEMA_VERSION {
        let hint = if raw.schema_version == 1 || raw.schema_version == 2 {
            " — the v1 `every_s` plan is replaced by scheduler v3 (ENGINE §22.34); \
             rewrite it as {schema_version:3, config:{...}, entries:[...]}"
        } else {
            ""
        };
        return Err(format!(
            "scheduler config: schema_version must be {SCHEDULE_SCHEMA_VERSION}, got {}{hint}",
            raw.schema_version
        ));
    }
    // An empty `entries` array is valid: deleting all tasks must leave the
    // scheduler page in a clean "no scheduled tasks" state instead of a config
    // parse error. The daemon simply has nothing to run.

    let calendar = build_calendar(&raw.config, default_offset_min)?;

    let mode = raw.config.mode.unwrap_or(RunMode::Stop);
    let loop_interval_s = raw.config.loop_interval_s.unwrap_or(DEFAULT_LOOP_INTERVAL_S);
    if loop_interval_s > 86_400 {
        return Err(format!(
            "scheduler config: config.loop_interval_s must be <= 86400, got {loop_interval_s}"
        ));
    }
    let loop_rounds = raw.config.loop_rounds.unwrap_or(0);

    let mut active_hours = Vec::new();
    for (i, pair) in raw.config.guards.active_hours.iter().enumerate() {
        let ctx = format!("config.guards.active_hours[{i}]");
        active_hours.push(TimeWindow::from_parts(&[], &pair[0], &pair[1], &ctx)?);
    }

    let mut blackouts = Vec::new();
    for (i, b) in raw.config.guards.blackouts.iter().enumerate() {
        let ctx = format!("config.guards.blackouts[{i}]");
        if b.start.is_empty() || b.end.is_empty() {
            return Err(format!("{ctx}: `start` and `end` are required (HH:MM)"));
        }
        let window = TimeWindow::from_parts(&b.days, &b.start, &b.end, &ctx)?;
        blackouts.push(Blackout {
            window,
            reason: b.reason.clone(),
        });
    }

    let defaults = Defaults {
        retry: match raw.config.defaults.retry {
            Some(r) => {
                if r > MAX_RETRY {
                    return Err(format!(
                        "scheduler config: config.defaults.retry must be <= {MAX_RETRY}, got {r}"
                    ));
                }
                r
            }
            None => DEFAULT_RETRY,
        },
        cooldown_s: match raw.config.defaults.cooldown {
            Some(c) => {
                if c > MAX_COOLDOWN_S {
                    return Err(format!(
                        "scheduler config: config.defaults.cooldown must be <= {MAX_COOLDOWN_S}s, got {c}"
                    ));
                }
                c
            }
            None => DEFAULT_COOLDOWN_S,
        },
    };

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut entries = Vec::with_capacity(raw.entries.len());
    for (i, e) in raw.entries.iter().enumerate() {
        let entry = validate_entry(e, i, &defaults)?;
        if !seen.insert(entry.entry_id.clone()) {
            return Err(format!(
                "scheduler config: duplicate entry_id `{}` (entries[{i}])",
                entry.entry_id
            ));
        }
        entries.push(entry);
    }

    Ok(Scheduler {
        mode,
        loop_interval_s,
        loop_rounds,
        calendar,
        active_hours,
        blackouts,
        defaults,
        entries,
    })
}

fn build_calendar(raw: &RawConfig, default_offset_min: i32) -> Result<Calendar, String> {
    let day_reset_min = match &raw.calendar.day_reset {
        Some(s) => parse_hhmm(s).map_err(|e| format!("config.calendar.day_reset: {e}"))?,
        None => 0,
    };
    let (week_reset_weekday, week_reset_min) = match &raw.calendar.week_reset {
        Some(w) => {
            let weekday = w.weekday.unwrap_or(0);
            if weekday > 6 {
                return Err(format!(
                    "config.calendar.week_reset.weekday must be 0..=6 (0 = Monday), got {weekday}"
                ));
            }
            let time = match &w.time {
                Some(t) => parse_hhmm(t)
                    .map_err(|e| format!("config.calendar.week_reset.time: {e}"))?,
                None => day_reset_min,
            };
            (weekday, time)
        }
        None => (0, day_reset_min),
    };
    let utc_offset_min = raw.utc_offset_min.unwrap_or(default_offset_min);
    if !(-720..=840).contains(&utc_offset_min) {
        return Err(format!(
            "scheduler config: config.utc_offset_min must be -720..=840, got {utc_offset_min}"
        ));
    }
    Ok(Calendar {
        day_reset_min,
        week_reset_weekday,
        week_reset_min,
        utc_offset_min,
    })
}

fn validate_entry(raw: &RawEntry, i: usize, defaults: &Defaults) -> Result<Entry, String> {
    let ctx = format!("entries[{i}]");
    let entry_id = raw.entry_id.trim().to_string();
    if entry_id.is_empty() {
        return Err(format!("{ctx}.entry_id must not be empty"));
    }
    if entry_id.len() > MAX_ENTRY_ID_LEN {
        return Err(format!(
            "{ctx}.entry_id must be <= {MAX_ENTRY_ID_LEN} chars, got {}",
            entry_id.len()
        ));
    }
    if entry_id.chars().any(|c| c.is_control()) {
        return Err(format!("{ctx}.entry_id must not contain control characters"));
    }
    let task = raw.task.trim().to_string();
    if task.is_empty() {
        return Err(format!("{ctx}.task must not be empty"));
    }
    oas_task::validate_task_name(&task).map_err(|e| format!("{ctx}.task: {e}"))?;

    let lifecycle = raw.lifecycle.unwrap_or(Lifecycle::Periodic);

    let mut windows = Vec::with_capacity(raw.windows.len());
    for (w, win) in raw.windows.iter().enumerate() {
        let wctx = format!("{ctx}.windows[{w}]");
        if win.start.is_empty() || win.end.is_empty() {
            return Err(format!("{wctx}: `start` and `end` are required (HH:MM)"));
        }
        if lifecycle == Lifecycle::Temp {
            return Err(format!(
                "{ctx}: lifecycle=temp must not declare `windows` (§22.34.3: 临时任务无时间窗)"
            ));
        }
        windows.push(TimeWindow::from_parts(&win.days, &win.start, &win.end, &wctx)?);
    }

    let quota = match &raw.quota {
        Some(q) => {
            if lifecycle == Lifecycle::Temp {
                return Err(format!(
                    "{ctx}: lifecycle=temp must not declare `quota` (§22.34.3: 临时任务无配额)"
                ));
            }
            let per = q
                .per
                .ok_or_else(|| format!("{ctx}.quota.per is required (\"day\" | \"week\")"))?;
            if q.times == 0 {
                return Err(format!("{ctx}.quota.times must be >= 1"));
            }
            if q.times > MAX_QUOTA_TIMES {
                return Err(format!(
                    "{ctx}.quota.times must be <= {MAX_QUOTA_TIMES}, got {}",
                    q.times
                ));
            }
            Some(Quota {
                per,
                times: q.times,
            })
        }
        None => None,
    };

    let retry = match raw.retry {
        Some(r) => {
            if r > MAX_RETRY {
                return Err(format!("{ctx}.retry must be <= {MAX_RETRY}, got {r}"));
            }
            r
        }
        None => defaults.retry,
    };
    let cooldown_s = match raw.cooldown {
        Some(c) => {
            if c > MAX_COOLDOWN_S {
                return Err(format!(
                    "{ctx}.cooldown must be <= {MAX_COOLDOWN_S}s, got {c}"
                ));
            }
            c
        }
        None => defaults.cooldown_s,
    };

    Ok(Entry {
        entry_id,
        task,
        enabled: raw.enabled,
        lifecycle,
        windows,
        quota,
        params: raw.params.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        retry,
        cooldown_s,
    })
}

// ---------------------------------------------------------------------------
// meta.exits (M0 contract) — result classification
// ---------------------------------------------------------------------------

/// Canonical name of one of the 8 flow exits (`oas_flow::parse_exit`).
pub fn exit_name(exit: Exit) -> &'static str {
    match exit {
        Exit::Done => "done",
        Exit::NoResource => "no_resource",
        Exit::WindowClosed => "window_closed",
        Exit::NoTarget => "no_target",
        Exit::ResourceEmpty => "resource_empty",
        Exit::Exhausted => "exhausted",
        Exit::Failed => "failed",
        Exit::Skipped => "skipped",
    }
}

/// Built-in default result per exit name (used when `meta.exits` is absent).
///
/// `done -> success` and the rest is deliberately **not fatal**: a task that
/// ends on `failed` is retried/cooldown-gated, not a scheduler stop. `meta.exits`
/// can override any of these per task (§22.34.2).
pub fn default_exit_class(name: &str) -> ResultClass {
    match name {
        "done" | "resource_empty" => ResultClass::Success,
        "no_resource" | "no_target" | "skipped" => ResultClass::Resource,
        "window_closed" => ResultClass::Blocked,
        // "failed" / "exhausted" / unknown -> retryable
        _ => ResultClass::Retryable,
    }
}

/// A run signal reduced from `RunReport` (kept engine-neutral so this module
/// stays a pure data layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunSignal {
    /// The flow reached a declared `finish(exit)`.
    Exit(Exit),
    /// Hard runtime failure (missing target / guard failed / step error).
    RunFailed,
    /// Budget/loop exhaustion (`RunResult::Exhausted`).
    Exhausted,
}

impl RunSignal {
    /// Signal name used for reports/audit ("run_error" / "exhausted").
    pub fn name(self) -> &'static str {
        match self {
            RunSignal::Exit(e) => exit_name(e),
            RunSignal::RunFailed => "run_error",
            RunSignal::Exhausted => "exhausted",
        }
    }
}

/// Classify a run using `meta.exits` (name -> result), falling back to the
/// documented defaults. `meta.exits` always wins when the name is declared.
pub fn classify_signal(sig: RunSignal, meta_exits: &BTreeMap<String, ResultClass>) -> ResultClass {
    let name = sig.name();
    if let Some(c) = meta_exits.get(name) {
        return *c;
    }
    match sig {
        RunSignal::Exit(e) => default_exit_class(exit_name(e)),
        RunSignal::RunFailed => meta_exits
            .get("error")
            .or_else(|| meta_exits.get("failed"))
            .copied()
            .unwrap_or(ResultClass::Retryable),
        RunSignal::Exhausted => meta_exits
            .get("exhausted")
            .copied()
            .unwrap_or(ResultClass::Retryable),
    }
}

/// Parse the `exits` object out of a task's `meta.json` (tolerant M0 contract).
///
/// Accepted shapes per exit name:
/// `{"done": "success"}` or `{"done": {"result": "success"}}`.
/// A missing `exits` field yields an empty map (defaults apply); an invalid
/// result string is a loud error (config fail-fast).
pub fn parse_meta_exits(text: &str) -> Result<BTreeMap<String, ResultClass>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("meta.json: invalid JSON: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "meta.json: top-level must be an object".to_string())?;
    let Some(exits) = obj.get("exits") else {
        return Ok(BTreeMap::new());
    };
    let exits = exits
        .as_object()
        .ok_or_else(|| "meta.json: `exits` must be an object".to_string())?;
    let mut out = BTreeMap::new();
    for (name, val) in exits {
        let result = match val {
            Value::String(s) => s.clone(),
            Value::Object(m) => m
                .get("result")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("meta.json: exits.{name}.result must be a string"))?
                .to_string(),
            _ => {
                return Err(format!(
                    "meta.json: exits.{name} must be a result string or {{result: \"...\"}}"
                ))
            }
        };
        let class = ResultClass::parse(&result)
            .map_err(|e| format!("meta.json: exits.{name}: {e}"))?;
        out.insert(name.clone(), class);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Persistent state (serde → scheduler.state.json; pure ops below)
// ---------------------------------------------------------------------------

fn default_state_schema() -> u32 {
    STATE_SCHEMA_VERSION
}

/// Which entry was executing when the process died (crash-recovery marker).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RunningEntry {
    #[serde(default)]
    pub entry_id: String,
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub pass: u64,
    #[serde(default)]
    pub attempt: u32,
    #[serde(default)]
    pub since_ms: u64,
    #[serde(default)]
    pub resume: bool,
}

/// One quota counter for the cycle it belongs to. A counter whose `cycle`
/// differs from the current one reads as 0 and is reset on the next roll.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QuotaCounter {
    #[serde(default)]
    pub cycle: String,
    #[serde(default)]
    pub used: u32,
}

/// Per-entry runtime state. **No completion memory for temp entries** — this
/// struct is only counters/cooldown/quota, never "already ran this cycle".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryState {
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub runs: u64,
    #[serde(default)]
    pub successes: u64,
    #[serde(default)]
    pub resources: u64,
    #[serde(default)]
    pub blocked: u64,
    #[serde(default)]
    pub failures: u64,
    #[serde(default)]
    pub last_result: String,
    #[serde(default)]
    pub last_outcome: String,
    #[serde(default)]
    pub last_exit: String,
    #[serde(default)]
    pub last_attempts: u32,
    #[serde(default)]
    pub last_run_ms: u64,
    #[serde(default)]
    pub last_duration_ms: u64,
    #[serde(default)]
    pub cooldown_until_ms: u64,
    #[serde(default)]
    pub quota_day: QuotaCounter,
    #[serde(default)]
    pub quota_week: QuotaCounter,
}

impl Default for EntryState {
    fn default() -> Self {
        EntryState {
            task: String::new(),
            runs: 0,
            successes: 0,
            resources: 0,
            blocked: 0,
            failures: 0,
            last_result: String::new(),
            last_outcome: String::new(),
            last_exit: String::new(),
            last_attempts: 0,
            last_run_ms: 0,
            last_duration_ms: 0,
            cooldown_until_ms: 0,
            quota_day: QuotaCounter::default(),
            quota_week: QuotaCounter::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StateCycle {
    #[serde(default)]
    pub day: String,
    #[serde(default)]
    pub week: String,
}

/// `scheduler.state.json` — **runtime state only**, never "completed" flags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerState {
    #[serde(default = "default_state_schema")]
    pub schema_version: u32,
    #[serde(default)]
    pub created_ms: u64,
    #[serde(default)]
    pub last_pass_ms: u64,
    #[serde(default)]
    pub passes: u64,
    #[serde(default)]
    pub running: Option<RunningEntry>,
    #[serde(default)]
    pub cycle: StateCycle,
    #[serde(default)]
    pub last_stop: String,
    #[serde(default)]
    pub entries: BTreeMap<String, EntryState>,
}

impl Default for SchedulerState {
    fn default() -> Self {
        SchedulerState {
            schema_version: STATE_SCHEMA_VERSION,
            created_ms: 0,
            last_pass_ms: 0,
            passes: 0,
            running: None,
            cycle: StateCycle::default(),
            last_stop: String::new(),
            entries: BTreeMap::new(),
        }
    }
}

impl SchedulerState {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Pure state operations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleRoll {
    pub changed: bool,
    pub cycles: Cycles,
}

/// Roll quota counters over to the current day/week cycle.
///
/// Called at the start of every pass; idempotent within one cycle. This is the
/// pure half of "处理 day/week reset（periodic quota 清零）".
pub fn roll_cycles(state: &mut SchedulerState, cal: &Calendar, utc_ms: i64) -> CycleRoll {
    let cycles = cycle_ids(cal, utc_ms);
    let mut changed = state.cycle.day != cycles.day || state.cycle.week != cycles.week;
    state.cycle = StateCycle {
        day: cycles.day.clone(),
        week: cycles.week.clone(),
    };
    for st in state.entries.values_mut() {
        if st.quota_day.cycle != cycles.day {
            st.quota_day = QuotaCounter {
                cycle: cycles.day.clone(),
                used: 0,
            };
            changed = true;
        }
        if st.quota_week.cycle != cycles.week {
            st.quota_week = QuotaCounter {
                cycle: cycles.week.clone(),
                used: 0,
            };
            changed = true;
        }
    }
    CycleRoll { changed, cycles }
}

/// Quota consumed in the current cycle for this entry (0 when the counter
/// belongs to an older cycle).
pub fn quota_used(state: &SchedulerState, entry: &Entry, cycles: &Cycles) -> u32 {
    let Some(quota) = &entry.quota else {
        return 0;
    };
    let Some(st) = state.entries.get(&entry.entry_id) else {
        return 0;
    };
    match quota.per {
        QuotaPer::Day => {
            if st.quota_day.cycle == cycles.day {
                st.quota_day.used
            } else {
                0
            }
        }
        QuotaPer::Week => {
            if st.quota_week.cycle == cycles.week {
                st.quota_week.used
            } else {
                0
            }
        }
    }
}

/// Remaining quota, `None` when the entry has no quota (unlimited).
pub fn quota_remaining(state: &SchedulerState, entry: &Entry, cycles: &Cycles) -> Option<u32> {
    let quota = entry.quota.as_ref()?;
    Some(quota.times.saturating_sub(quota_used(state, entry, cycles)))
}

/// Mark the entry as running (crash-recovery marker). Written before execution.
pub fn mark_running(
    state: &mut SchedulerState,
    entry: &Entry,
    pass: u64,
    attempt: u32,
    now_utc_ms: u64,
    resume: bool,
) {
    state.running = Some(RunningEntry {
        entry_id: entry.entry_id.clone(),
        task: entry.task.clone(),
        pass,
        attempt,
        since_ms: now_utc_ms,
        resume,
    });
}

pub fn clear_running(state: &mut SchedulerState) {
    state.running = None;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordEffects {
    pub cooldown_until_ms: u64,
    pub quota_used: Option<u32>,
}

/// Retry policy (per pass execution): retry only `retryable`, at most
/// `entry.retry` extra attempts. `attempt` is 1-based.
pub fn should_retry(class: ResultClass, attempt: u32, entry: &Entry) -> bool {
    class == ResultClass::Retryable && attempt < entry.max_attempts()
}

/// Record one entry execution (one pass execution, all retry attempts included)
/// into the state: counters, last result, quota, cooldown.
pub fn record_result(
    state: &mut SchedulerState,
    entry: &Entry,
    cycles: &Cycles,
    now_utc_ms: i64,
    class: ResultClass,
    exit: &str,
    outcome: &str,
    attempts: u32,
    duration_ms: u64,
) -> RecordEffects {
    let now = now_utc_ms.max(0) as u64;
    let st = state
        .entries
        .entry(entry.entry_id.clone())
        .or_insert_with(|| EntryState {
            task: entry.task.clone(),
            ..EntryState::default()
        });
    if st.task.is_empty() {
        st.task = entry.task.clone();
    }
    st.runs = st.runs.saturating_add(1);
    match class {
        ResultClass::Success => st.successes = st.successes.saturating_add(1),
        ResultClass::Resource => st.resources = st.resources.saturating_add(1),
        ResultClass::Blocked => st.blocked = st.blocked.saturating_add(1),
        ResultClass::Retryable | ResultClass::Fatal => {
            st.failures = st.failures.saturating_add(1)
        }
    }
    st.last_result = class.as_str().to_string();
    st.last_outcome = outcome.to_string();
    st.last_exit = exit.to_string();
    st.last_attempts = attempts;
    st.last_run_ms = now;
    st.last_duration_ms = duration_ms;

    let mut quota_used_now = None;
    if class.consumes_quota() {
        if let Some(quota) = &entry.quota {
            match quota.per {
                QuotaPer::Day => {
                    if st.quota_day.cycle != cycles.day {
                        st.quota_day = QuotaCounter {
                            cycle: cycles.day.clone(),
                            used: 0,
                        };
                    }
                    st.quota_day.used = st.quota_day.used.saturating_add(1);
                    quota_used_now = Some(st.quota_day.used);
                }
                QuotaPer::Week => {
                    if st.quota_week.cycle != cycles.week {
                        st.quota_week = QuotaCounter {
                            cycle: cycles.week.clone(),
                            used: 0,
                        };
                    }
                    st.quota_week.used = st.quota_week.used.saturating_add(1);
                    quota_used_now = Some(st.quota_week.used);
                }
            }
        }
    }

    let cooldown_until_ms = if class.sets_cooldown() {
        now.saturating_add(entry.cooldown_s.saturating_mul(MS_PER_SEC as u64))
    } else {
        0
    };
    st.cooldown_until_ms = cooldown_until_ms;

    RecordEffects {
        cooldown_until_ms,
        quota_used: quota_used_now,
    }
}

// ---------------------------------------------------------------------------
// Guards
// ---------------------------------------------------------------------------

/// Evaluate the global guards at `utc_ms`. `None` = pass may run.
pub fn guard_status(sched: &Scheduler, utc_ms: i64) -> Option<GuardBlock> {
    let local = sched.calendar.to_local(utc_ms);
    if sched
        .blackouts
        .iter()
        .any(|b| b.window.contains(local))
    {
        return Some(GuardBlock::Blackout);
    }
    if !sched.active_hours.is_empty()
        && !sched.active_hours.iter().any(|w| w.contains(local))
    {
        return Some(GuardBlock::OutsideActiveHours);
    }
    None
}

/// Human reason for a guard block (audit/status).
pub fn guard_reason(sched: &Scheduler, utc_ms: i64, block: GuardBlock) -> String {
    let local = sched.calendar.to_local(utc_ms);
    match block {
        GuardBlock::Blackout => sched
            .blackouts
            .iter()
            .find(|b| b.window.contains(local))
            .map(|b| {
                if b.reason.is_empty() {
                    "blackout".to_string()
                } else {
                    format!("blackout:{}", b.reason)
                }
            })
            .unwrap_or_else(|| "blackout".to_string()),
        GuardBlock::OutsideActiveHours => "guards.active_hours".to_string(),
    }
}

/// If a blackout is active now, when does it end?
pub fn blackout_end_ms(sched: &Scheduler, utc_ms: i64) -> Option<i64> {
    let local = sched.calendar.to_local(utc_ms);
    sched
        .blackouts
        .iter()
        .filter_map(|b| b.window.current_end(local))
        .min()
        .map(|l| sched.calendar.from_local(l))
}

/// When do the active-hours open again? `None` when they are open now (or
/// unrestricted).
pub fn next_active_hours_start_ms(sched: &Scheduler, utc_ms: i64) -> Option<i64> {
    if sched.active_hours.is_empty() {
        return None;
    }
    let local = sched.calendar.to_local(utc_ms);
    if sched.active_hours.iter().any(|w| w.contains(local)) {
        return None;
    }
    sched
        .active_hours
        .iter()
        .filter_map(|w| w.next_start(local))
        .min()
        .map(|l| sched.calendar.from_local(l))
}

// ---------------------------------------------------------------------------
// Windows / entry filtering
// ---------------------------------------------------------------------------

/// periodic with no windows = always in window; temp is never window-gated.
pub fn in_window(entry: &Entry, local_ms: i64) -> bool {
    entry.lifecycle == Lifecycle::Temp
        || entry.windows.is_empty()
        || entry.windows.iter().any(|w| w.contains(local_ms))
}

/// Earliest instant one of the entry's windows opens (for waiting entries).
pub fn next_window_start_ms(entry: &Entry, local_ms: i64) -> Option<i64> {
    entry.windows.iter().filter_map(|w| w.next_start(local_ms)).min()
}

// ---------------------------------------------------------------------------
// Pass planning（entries order is the only authority）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRun {
    pub entry_index: usize,
    pub entry_id: String,
    pub task: String,
    pub lifecycle: Lifecycle,
    /// Start from the task checkpoint (crash recovery / retry after failure).
    pub resume: bool,
    /// Why it is runnable now: `periodic`, `temp`, `recover`, `recover_temp`.
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedEntry {
    pub entry_index: usize,
    pub entry_id: String,
    pub task: String,
    /// `disabled` | `outside_window` | `quota_exhausted` | `cooldown`.
    pub reason: String,
}

/// The result of one pass's candidate filtering. Execution order = `runnable`
/// order = `entries[]` order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassPlan {
    /// Set when the global guards block the whole pass (runnable is empty).
    pub guard: Option<GuardBlock>,
    pub runnable: Vec<PlannedRun>,
    pub skipped: Vec<SkippedEntry>,
    pub cycles: Cycles,
}

/// Plan one pass at `utc_ms`.
///
/// `recovering` is the persisted `state.running` marker (a crash left an entry
/// mid-run). The dirty entry is planned with `resume = true` (periodic) and is
/// not re-filtered by window/quota/cooldown — finishing an interrupted run
/// takes priority. Temp entries have no completion memory, so they are planned
/// fresh (the executor resets their `state/` first).
pub fn plan_pass(
    sched: &Scheduler,
    state: &SchedulerState,
    utc_ms: i64,
    recovering: Option<&RunningEntry>,
) -> PassPlan {
    let cycles = cycle_ids(&sched.calendar, utc_ms);
    let guard = guard_status(sched, utc_ms);
    let local = sched.calendar.to_local(utc_ms);
    let mut runnable = Vec::new();
    let mut skipped = Vec::new();
    if guard.is_some() {
        return PassPlan {
            guard,
            runnable,
            skipped,
            cycles,
        };
    }
    for (idx, entry) in sched.entries.iter().enumerate() {
        if !entry.enabled {
            skipped.push(SkippedEntry {
                entry_index: idx,
                entry_id: entry.entry_id.clone(),
                task: entry.task.clone(),
                reason: "disabled".to_string(),
            });
            continue;
        }
        if let Some(rec) = recovering {
            if rec.entry_id == entry.entry_id {
                let (reason, resume) = match entry.lifecycle {
                    Lifecycle::Periodic => ("recover", true),
                    Lifecycle::Temp => ("recover_temp", false),
                };
                runnable.push(PlannedRun {
                    entry_index: idx,
                    entry_id: entry.entry_id.clone(),
                    task: entry.task.clone(),
                    lifecycle: entry.lifecycle,
                    resume,
                    reason: reason.to_string(),
                });
                continue;
            }
        }
        match entry.lifecycle {
            Lifecycle::Temp => runnable.push(PlannedRun {
                entry_index: idx,
                entry_id: entry.entry_id.clone(),
                task: entry.task.clone(),
                lifecycle: entry.lifecycle,
                resume: false,
                reason: "temp".to_string(),
            }),
            Lifecycle::Periodic => {
                if !in_window(entry, local) {
                    skipped.push(SkippedEntry {
                        entry_index: idx,
                        entry_id: entry.entry_id.clone(),
                        task: entry.task.clone(),
                        reason: "outside_window".to_string(),
                    });
                    continue;
                }
                if let Some(remaining) = quota_remaining(state, entry, &cycles) {
                    if remaining == 0 {
                        skipped.push(SkippedEntry {
                            entry_index: idx,
                            entry_id: entry.entry_id.clone(),
                            task: entry.task.clone(),
                            reason: "quota_exhausted".to_string(),
                        });
                        continue;
                    }
                }
                let cooling = state
                    .entries
                    .get(&entry.entry_id)
                    .map(|st| st.cooldown_until_ms > utc_ms.max(0) as u64)
                    .unwrap_or(false);
                if cooling {
                    skipped.push(SkippedEntry {
                        entry_index: idx,
                        entry_id: entry.entry_id.clone(),
                        task: entry.task.clone(),
                        reason: "cooldown".to_string(),
                    });
                    continue;
                }
                runnable.push(PlannedRun {
                    entry_index: idx,
                    entry_id: entry.entry_id.clone(),
                    task: entry.task.clone(),
                    lifecycle: entry.lifecycle,
                    resume: false,
                    reason: "periodic".to_string(),
                });
            }
        }
    }
    PassPlan {
        guard,
        runnable,
        skipped,
        cycles,
    }
}

/// When can this entry run next? Used by `--status` and the panel's "今日清单".
pub fn entry_next_due_ms(
    sched: &Scheduler,
    state: &SchedulerState,
    entry: &Entry,
    utc_ms: i64,
) -> Option<i64> {
    if !entry.enabled {
        return None;
    }
    let cycles = cycle_ids(&sched.calendar, utc_ms);
    let local = sched.calendar.to_local(utc_ms);
    match entry.lifecycle {
        Lifecycle::Temp => Some(utc_ms),
        Lifecycle::Periodic => {
            if quota_remaining(state, entry, &cycles) == Some(0) {
                return Some(match entry.quota.as_ref().map(|q| q.per) {
                    Some(QuotaPer::Week) => next_week_reset_ms(&sched.calendar, utc_ms),
                    _ => next_day_reset_ms(&sched.calendar, utc_ms),
                });
            }
            if !in_window(entry, local) {
                return next_window_start_ms(entry, local).map(|l| sched.calendar.from_local(l));
            }
            if let Some(st) = state.entries.get(&entry.entry_id) {
                if st.cooldown_until_ms > utc_ms.max(0) as u64 {
                    return Some(st.cooldown_until_ms as i64);
                }
            }
            Some(utc_ms)
        }
    }
}

// ---------------------------------------------------------------------------
// Wake calculation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextWake {
    /// When the daemon should re-evaluate next (capped by [`MAX_SLEEP_MS`]).
    pub at_ms: i64,
    /// Why: `day_reset`, `week_reset`, `active_hours_start`, `blackout_end`,
    /// `window_start:<id>`, `quota_reset:<id>`, `cooldown:<id>`, `cap`.
    pub reasons: Vec<String>,
}

/// Earliest reason the daemon should wake up (§22.34.4 step 7): window start /
/// quota reset / blackout end / active-hours start / cooldown expiry / day-wet
/// reset, capped at 30s so a clock change or config edit is noticed quickly.
pub fn next_wake(sched: &Scheduler, state: &SchedulerState, utc_ms: i64) -> NextWake {
    let cal = &sched.calendar;
    let cycles = cycle_ids(cal, utc_ms);
    let local = cal.to_local(utc_ms);
    let cap = utc_ms + MAX_SLEEP_MS;
    let mut cands: Vec<(i64, String)> = Vec::new();

    cands.push((next_day_reset_ms(cal, utc_ms), "day_reset".to_string()));
    if has_weekly_quota(sched) {
        cands.push((next_week_reset_ms(cal, utc_ms), "week_reset".to_string()));
    }

    if let Some(block) = guard_status(sched, utc_ms) {
        match block {
            GuardBlock::Blackout => {
                if let Some(end) = blackout_end_ms(sched, utc_ms) {
                    cands.push((end, "blackout_end".to_string()));
                }
            }
            GuardBlock::OutsideActiveHours => {
                if let Some(start) = next_active_hours_start_ms(sched, utc_ms) {
                    cands.push((start, "active_hours_start".to_string()));
                }
            }
        }
    }

    for entry in &sched.entries {
        if !entry.enabled || entry.lifecycle != Lifecycle::Periodic {
            continue;
        }
        let st = state.entries.get(&entry.entry_id);
        let cooling_until = st.map(|s| s.cooldown_until_ms).unwrap_or(0) as i64;
        if cooling_until > utc_ms {
            cands.push((cooling_until, format!("cooldown:{}", entry.entry_id)));
            continue;
        }
        if let Some(remaining) = quota_remaining(state, entry, &cycles) {
            if remaining == 0 {
                let reset = match entry.quota.as_ref().map(|q| q.per) {
                    Some(QuotaPer::Week) => next_week_reset_ms(cal, utc_ms),
                    _ => next_day_reset_ms(cal, utc_ms),
                };
                cands.push((reset, format!("quota_reset:{}", entry.entry_id)));
                continue;
            }
        }
        if !in_window(entry, local) {
            if let Some(start) = next_window_start_ms(entry, local) {
                cands.push((cal.from_local(start), format!("window_start:{}", entry.entry_id)));
            }
        }
    }

    let mut best: Option<(i64, Vec<String>)> = None;
    for (at, reason) in cands {
        if at <= utc_ms {
            continue;
        }
        match &mut best {
            None => best = Some((at, vec![reason])),
            Some((b, reasons)) => {
                if at < *b {
                    *b = at;
                    reasons.clear();
                    reasons.push(reason);
                } else if at == *b {
                    reasons.push(reason);
                }
            }
        }
    }
    match best {
        None => NextWake {
            at_ms: cap,
            reasons: vec!["cap".to_string()],
        },
        Some((at, mut reasons)) => {
            if at > cap {
                NextWake {
                    at_ms: cap,
                    reasons: vec!["cap".to_string()],
                }
            } else {
                reasons.sort();
                reasons.dedup();
                NextWake {
                    at_ms: at,
                    reasons,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests（纯函数矩阵：cycle/窗口/quota/pass/wake/result/state）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sched(json: &str) -> Scheduler {
        validate_scheduler(json).expect("config must be valid")
    }

    fn minimal(entries: &str) -> Scheduler {
        sched(&format!(
            r#"{{"schema_version":3,"entries":[{entries}]}}"#
        ))
    }

    /// Local wall-clock -> UTC epoch ms, via the calendar's offset.
    fn at(cal: &Calendar, y: i64, mo: u32, d: u32, hh: i64, mm: i64) -> i64 {
        let day = days_from_civil(y, mo, d);
        let local = day * DAY_MS + (hh * 60 + mm) * MS_PER_MIN;
        cal.from_local(local)
    }

    fn cal_0500() -> Calendar {
        Calendar {
            day_reset_min: 300,
            week_reset_weekday: 0,
            week_reset_min: 300,
            utc_offset_min: 0,
        }
    }

    // -- time helpers -------------------------------------------------------

    #[test]
    fn hhmm_parsing_is_strict() {
        assert_eq!(parse_hhmm("05:00").unwrap(), 300);
        assert_eq!(parse_hhmm("5:00").unwrap(), 300);
        assert_eq!(parse_hhmm("23:59").unwrap(), 1439);
        assert_eq!(parse_hhmm("00:00").unwrap(), 0);
        assert_eq!(format_hhmm(300), "05:00");
        assert_eq!(format_hhmm(1439), "23:59");
        for bad in ["", "5", "24:00", "05:60", "05:00:30", "aa:bb", "05:00:00:00"] {
            assert!(parse_hhmm(bad).is_err(), "must reject `{bad}`");
        }
        // HH:MM:00 is accepted (minute resolution, trailing zero seconds).
        assert_eq!(parse_hhmm("05:00:00").unwrap(), 300);
    }

    #[test]
    fn civil_dates_roundtrip() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 1, 1), 10957);
        assert_eq!(days_from_civil(2026, 10, 23), 20749);
        assert_eq!(civil_from_days(days_from_civil(2026, 10, 23)), (2026, 10, 23));
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        // 2026-10-23 is a Friday (0 = Monday).
        assert_eq!(weekday_of_epoch_day(days_from_civil(2026, 10, 23)), 4);
        // 1970-01-05 was the first Monday after the epoch.
        assert_eq!(weekday_of_epoch_day(4), 0);
        assert_eq!(week_anchor_day(0), 4);
        assert_eq!(civil_from_days(week_anchor_day(0)), (1970, 1, 5));
        assert_eq!(format_date(20749), "2026-10-23");
        assert_eq!(
            format_local_datetime(at(&cal_0500(), 2026, 10, 23, 5, 0), 0),
            "2026-10-23T05:00:00"
        );
    }

    // -- cycle ids ----------------------------------------------------------

    #[test]
    fn day_cycle_respects_day_reset() {
        let cal = cal_0500();
        let before = cycle_ids(&cal, at(&cal, 2026, 10, 23, 4, 59));
        assert_eq!(before.day, "2026-10-22", "04:59 belongs to the previous cycle");
        let exact = cycle_ids(&cal, at(&cal, 2026, 10, 23, 5, 0));
        assert_eq!(exact.day, "2026-10-23");
        // Day cycle only changes at the reset instant.
        assert_eq!(
            cycle_ids(&cal, at(&cal, 2026, 10, 23, 23, 59)).day,
            "2026-10-23"
        );
        assert_eq!(
            cycle_ids(&cal, at(&cal, 2026, 10, 24, 0, 0)).day,
            "2026-10-23",
            "midnight is not a boundary when day_reset is 05:00"
        );
        assert_eq!(
            next_day_reset_ms(&cal, at(&cal, 2026, 10, 23, 6, 0)),
            at(&cal, 2026, 10, 24, 5, 0)
        );
    }

    #[test]
    fn week_cycle_respects_week_reset() {
        let cal = cal_0500();
        // 2026-10-19 is a Monday; 2026-10-25 is the following Sunday.
        assert_eq!(
            cycle_ids(&cal, at(&cal, 2026, 10, 19, 5, 0)).week,
            "2026-10-19"
        );
        assert_eq!(
            cycle_ids(&cal, at(&cal, 2026, 10, 25, 23, 0)).week,
            "2026-10-19",
            "Sunday night still belongs to the week that started Monday"
        );
        assert_eq!(
            cycle_ids(&cal, at(&cal, 2026, 10, 26, 4, 59)).week,
            "2026-10-19"
        );
        let next_week = cycle_ids(&cal, at(&cal, 2026, 10, 26, 5, 0));
        assert_eq!(next_week.week, "2026-10-26");
        assert_eq!(
            next_week_reset_ms(&cal, at(&cal, 2026, 10, 23, 12, 0)),
            at(&cal, 2026, 10, 26, 5, 0)
        );
    }

    #[test]
    fn week_reset_with_other_weekday_anchors_correctly() {
        let cal = Calendar {
            day_reset_min: 0,
            week_reset_weekday: 2, // Wednesday
            week_reset_min: 0,
            utc_offset_min: 0,
        };
        // 2026-10-21 is a Wednesday.
        assert_eq!(
            cycle_ids(&cal, at(&cal, 2026, 10, 21, 0, 0)).week,
            "2026-10-21"
        );
        assert_eq!(
            cycle_ids(&cal, at(&cal, 2026, 10, 20, 23, 59)).week,
            "2026-10-14"
        );
    }

    #[test]
    fn utc_offset_shifts_local_boundaries() {
        let cal = Calendar {
            day_reset_min: 300,
            week_reset_weekday: 0,
            week_reset_min: 300,
            utc_offset_min: 480, // +08:00
        };
        // 2026-10-22 21:05 UTC == 2026-10-23 05:05 local => new cycle.
        let utc = at(&cal, 2026, 10, 23, 5, 5);
        assert_eq!(cycle_ids(&cal, utc).day, "2026-10-23");
        assert_eq!(cycle_ids(&cal, utc - 10 * MS_PER_MIN).day, "2026-10-22");
    }

    // -- windows ------------------------------------------------------------

    fn window(json: &str) -> TimeWindow {
        let v: Value = serde_json::from_str(json).unwrap();
        let o = v.as_object().unwrap();
        let days: Vec<u8> = o
            .get("days")
            .and_then(|d| d.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u8)).collect())
            .unwrap_or_default();
        TimeWindow::from_parts(
            &days,
            o.get("start").and_then(|s| s.as_str()).unwrap(),
            o.get("end").and_then(|s| s.as_str()).unwrap(),
            "test",
        )
        .unwrap()
    }

    #[test]
    fn same_day_window_membership() {
        let w = window(r#"{"start":"08:00","end":"12:00"}"#);
        let local = |h: i64, m: i64| days_from_civil(2026, 10, 23) * DAY_MS + (h * 60 + m) * MS_PER_MIN;
        assert!(!w.contains(local(7, 59)));
        assert!(w.contains(local(8, 0)));
        assert!(w.contains(local(11, 59)));
        assert!(!w.contains(local(12, 0)), "end is exclusive");
    }

    #[test]
    fn cross_midnight_window_membership() {
        // Monday 22:00 -> Tuesday 02:00 (days = Monday only).
        let w = window(r#"{"days":[0],"start":"22:00","end":"02:00"}"#);
        let local = |y: i64, mo: u32, d: u32, h: i64, m: i64| days_from_civil(y, mo, d) * DAY_MS + (h * 60 + m) * MS_PER_MIN;
        assert!(w.contains(local(2026, 10, 19, 22, 30)), "Monday night");
        assert!(w.contains(local(2026, 10, 20, 1, 59)), "Tuesday early morning");
        assert!(!w.contains(local(2026, 10, 20, 2, 0)), "Tuesday 02:00 ends it");
        assert!(!w.contains(local(2026, 10, 19, 21, 59)));
        // Next opening after Friday is the next Monday at 22:00.
        let fri = local(2026, 10, 23, 12, 0);
        assert_eq!(w.next_start(fri), Some(local(2026, 10, 26, 22, 0)));
    }

    #[test]
    fn empty_window_is_always_open_and_full_day_works() {
        let all = window(r#"{"start":"00:00","end":"00:00"}"#);
        let local = days_from_civil(1980, 1, 1) * DAY_MS;
        assert!(all.contains(local));
        assert!(all.current_end(local).is_none());
        assert!(all.next_start(local).is_none());

        let w = window(r#"{"start":"08:00","end":"12:00"}"#);
        assert_eq!(
            w.next_start(local),
            Some(days_from_civil(1980, 1, 1) * DAY_MS + 8 * 60 * MS_PER_MIN)
        );
    }

    #[test]
    fn window_with_out_of_range_weekday_is_rejected() {
        let e = TimeWindow::from_parts(&[7], "08:00", "09:00", "w");
        assert!(e.unwrap_err().contains("0..=6"));
    }

    // -- guards -------------------------------------------------------------

    fn sched_with(hours: &str, blackouts: &str) -> Scheduler {
        sched(&format!(
            r#"{{"schema_version":3,
                 "config":{{"calendar":{{"day_reset":"05:00","week_reset":{{"weekday":0,"time":"05:00"}}}},
                            "guards":{{"active_hours":[{hours}],"blackouts":[{blackouts}]}},
                            "defaults":{{"retry":0,"cooldown":60}}}},
                 "entries":[{{"entry_id":"e1","task":"任务1","lifecycle":"periodic"}}]}}"#
        ))
    }

    #[test]
    fn active_hours_guard() {
        let s = sched_with(r#"["08:00","12:00"]"#, "");
        let cal = s.calendar;
        assert!(guard_status(&s, at(&cal, 2026, 10, 23, 9, 0)).is_none());
        assert_eq!(
            guard_status(&s, at(&cal, 2026, 10, 23, 7, 0)),
            Some(GuardBlock::OutsideActiveHours)
        );
        assert_eq!(
            next_active_hours_start_ms(&s, at(&cal, 2026, 10, 23, 7, 0)),
            Some(at(&cal, 2026, 10, 23, 8, 0))
        );
    }

    #[test]
    fn active_hours_can_cross_midnight() {
        let s = sched_with(r#"["22:00","06:00"]"#, "");
        let cal = s.calendar;
        assert!(guard_status(&s, at(&cal, 2026, 10, 23, 23, 0)).is_none());
        assert!(guard_status(&s, at(&cal, 2026, 10, 23, 5, 0)).is_none());
        assert_eq!(
            guard_status(&s, at(&cal, 2026, 10, 23, 12, 0)),
            Some(GuardBlock::OutsideActiveHours)
        );
        // From noon the next opening is the same day 22:00.
        assert_eq!(
            next_active_hours_start_ms(&s, at(&cal, 2026, 10, 23, 12, 0)),
            Some(at(&cal, 2026, 10, 23, 22, 0))
        );
    }

    #[test]
    fn blackout_guard_and_end() {
        let s = sched_with(
            "",
            r#"{"days":[],"start":"02:00","end":"02:10","reason":"维护"}"#,
        );
        let cal = s.calendar;
        assert_eq!(
            guard_status(&s, at(&cal, 2026, 10, 23, 2, 5)),
            Some(GuardBlock::Blackout)
        );
        assert_eq!(
            blackout_end_ms(&s, at(&cal, 2026, 10, 23, 2, 5)),
            Some(at(&cal, 2026, 10, 23, 2, 10))
        );
        assert_eq!(
            guard_reason(&s, at(&cal, 2026, 10, 23, 2, 5), GuardBlock::Blackout),
            "blackout:维护"
        );
        assert!(guard_status(&s, at(&cal, 2026, 10, 23, 12, 0)).is_none());
    }

    // -- quota / state ------------------------------------------------------

    fn quota_sched(per: &str, times: u32) -> Scheduler {
        sched(&format!(
            r#"{{"schema_version":3,"entries":[
                 {{"entry_id":"e1","task":"任务1","lifecycle":"periodic",
                   "quota":{{"per":"{per}","times":{times}}},
                   "windows":[{{"days":[],"start":"00:00","end":"00:00"}}]}}]}}"#
        ))
    }

    #[test]
    fn quota_rolls_at_day_and_week_boundaries() {
        let cal = cal_0500();
        let s = quota_sched("day", 2);
        let entry = &s.entries[0];
        let mut state = SchedulerState::new();

        // Day 1: two completed runs fill the quota.
        let day1 = at(&cal, 2026, 10, 23, 9, 0);
        let roll = roll_cycles(&mut state, &cal, day1);
        assert!(roll.changed, "first roll initialises counters");
        for _ in 0..2 {
            let cycles = cycle_ids(&cal, day1);
            record_result(
                &mut state,
                entry,
                &cycles,
                day1,
                ResultClass::Success,
                "done",
                "Done",
                1,
                1000,
            );
        }
        let cycles = cycle_ids(&cal, day1);
        assert_eq!(quota_used(&state, entry, &cycles), 2);
        assert_eq!(quota_remaining(&state, entry, &cycles), Some(0));

        // Still the same logical day => the used counter survives.
        let later = at(&cal, 2026, 10, 24, 4, 59);
        roll_cycles(&mut state, &cal, later);
        let cycles = cycle_ids(&cal, later);
        assert_eq!(quota_used(&state, entry, &cycles), 2);

        // Crossing 05:00 starts a new day cycle => 0 used.
        let next_day = at(&cal, 2026, 10, 24, 5, 0);
        assert!(roll_cycles(&mut state, &cal, next_day).changed);
        let cycles = cycle_ids(&cal, next_day);
        assert_eq!(quota_used(&state, entry, &cycles), 0);
        assert_eq!(quota_remaining(&state, entry, &cycles), Some(2));
    }

    #[test]
    fn weekly_quota_survives_weekdays_and_resets_on_monday() {
        let cal = cal_0500();
        let s = quota_sched("week", 3);
        let entry = &s.entries[0];
        let mut state = SchedulerState::new();

        let monday = at(&cal, 2026, 10, 19, 8, 0);
        roll_cycles(&mut state, &cal, monday);
        let cycles = cycle_ids(&cal, monday);
        record_result(
            &mut state,
            entry,
            &cycles,
            monday,
            ResultClass::Success,
            "done",
            "Done",
            1,
            10,
        );
        assert_eq!(quota_used(&state, entry, &cycles), 1);

        // Friday is the same week cycle.
        let friday = at(&cal, 2026, 10, 23, 8, 0);
        roll_cycles(&mut state, &cal, friday);
        let cycles = cycle_ids(&cal, friday);
        assert_eq!(quota_used(&state, entry, &cycles), 1);
        record_result(
            &mut state,
            entry,
            &cycles,
            friday,
            ResultClass::Success,
            "done",
            "Done",
            1,
            10,
        );

        // Next Monday 05:00 resets the weekly counter.
        let next_monday = at(&cal, 2026, 10, 26, 5, 0);
        assert!(roll_cycles(&mut state, &cal, next_monday).changed);
        let cycles = cycle_ids(&cal, next_monday);
        assert_eq!(quota_used(&state, entry, &cycles), 0);
    }

    #[test]
    fn quota_counts_completed_runs_only() {
        let cal = cal_0500();
        let s = quota_sched("day", 2);
        let entry = &s.entries[0];
        let mut state = SchedulerState::new();
        let now = at(&cal, 2026, 10, 23, 9, 0);
        let cycles = cycle_ids(&cal, now);
        roll_cycles(&mut state, &cal, now);

        let effects = record_result(
            &mut state,
            entry,
            &cycles,
            now,
            ResultClass::Blocked,
            "no_resource",
            "Failed",
            1,
            5,
        );
        assert_eq!(effects.quota_used, None, "blocked does not consume quota");
        assert!(effects.cooldown_until_ms > now as u64, "blocked => cooldown");
        assert_eq!(quota_used(&state, entry, &cycles), 0);

        let effects = record_result(
            &mut state,
            entry,
            &cycles,
            now,
            ResultClass::Success,
            "done",
            "Done",
            1,
            5,
        );
        assert_eq!(effects.quota_used, Some(1));
        assert_eq!(effects.cooldown_until_ms, 0, "success clears cooldown");
    }

    // -- pass planning ------------------------------------------------------

    fn plan_sched() -> Scheduler {
        sched(
            r#"{"schema_version":3,
                "config":{"calendar":{"day_reset":"00:00"},"defaults":{"retry":0,"cooldown":60}},
                "entries":[
                  {"entry_id":"e1","task":"任务1","lifecycle":"periodic",
                   "windows":[{"days":[],"start":"12:00","end":"13:00"}]},
                  {"entry_id":"e2","task":"任务2","lifecycle":"temp"},
                  {"entry_id":"e3","task":"任务3","lifecycle":"periodic",
                   "quota":{"per":"day","times":1},
                   "windows":[{"days":[],"start":"00:00","end":"00:00"}]},
                  {"entry_id":"e4","task":"任务4","lifecycle":"periodic",
                   "windows":[{"days":[],"start":"00:00","end":"00:00"}]},
                  {"entry_id":"e5","task":"任务5","lifecycle":"periodic","enabled":false,
                   "windows":[{"days":[],"start":"00:00","end":"00:00"}]}
                ]}"#,
        )
    }

    #[test]
    fn plan_pass_uses_entries_order_and_filters() {
        let s = plan_sched();
        let cal = s.calendar;
        let now = at(&cal, 2026, 10, 23, 9, 0);
        let mut state = SchedulerState::new();
        let cycles = cycle_ids(&cal, now);
        roll_cycles(&mut state, &cal, now);
        // Fill e3's daily quota.
        record_result(
            &mut state,
            &s.entries[2],
            &cycles,
            now,
            ResultClass::Success,
            "done",
            "Done",
            1,
            1,
        );
        // Put e4 in cooldown.
        let e4 = &mut state.entries.entry("e4".to_string()).or_default();
        e4.cooldown_until_ms = (now + 60_000) as u64;

        let plan = plan_pass(&s, &state, now, None);
        assert!(plan.guard.is_none());
        let ids: Vec<_> = plan.runnable.iter().map(|p| p.entry_id.as_str()).collect();
        assert_eq!(ids, vec!["e2"], "only the temp entry is runnable: {plan:?}");
        let skipped: Vec<_> = plan
            .skipped
            .iter()
            .map(|s| (s.entry_id.as_str(), s.reason.as_str()))
            .collect();
        assert_eq!(
            skipped,
            vec![
                ("e1", "outside_window"),
                ("e3", "quota_exhausted"),
                ("e4", "cooldown"),
                ("e5", "disabled"),
            ]
        );
    }

    #[test]
    fn plan_pass_keeps_order_when_all_runnable() {
        let s = sched(
            r#"{"schema_version":3,
                "config":{"calendar":{"day_reset":"00:00"}},
                "entries":[
                  {"entry_id":"a","task":"任务1","lifecycle":"periodic"},
                  {"entry_id":"b","task":"任务2","lifecycle":"temp"},
                  {"entry_id":"c","task":"任务3","lifecycle":"periodic"}
                ]}"#,
        );
        let now = at(&s.calendar, 2026, 10, 23, 9, 0);
        let plan = plan_pass(&s, &SchedulerState::new(), now, None);
        let ids: Vec<_> = plan.runnable.iter().map(|p| p.entry_id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"], "entries order is the only authority");
    }

    #[test]
    fn plan_pass_guard_blocks_whole_pass() {
        let s = sched_with(r#"["08:00","12:00"]"#, "");
        let cal = s.calendar;
        let now = at(&cal, 2026, 10, 23, 7, 0); // outside active hours
        let plan = plan_pass(&s, &SchedulerState::new(), now, None);
        assert_eq!(plan.guard, Some(GuardBlock::OutsideActiveHours));
        assert!(plan.runnable.is_empty());
        assert!(plan.skipped.is_empty(), "guard blocks before per-entry filtering");
    }

    #[test]
    fn plan_pass_dirty_recovery_marks_resume_for_periodic_only() {
        let s = sched(
            r#"{"schema_version":3,
                "config":{"calendar":{"day_reset":"00:00"}},
                "entries":[
                  {"entry_id":"p","task":"任务1","lifecycle":"periodic",
                   "windows":[{"days":[],"start":"12:00","end":"13:00"}]},
                  {"entry_id":"t","task":"任务2","lifecycle":"temp"}
                ]}"#,
        );
        let now = at(&s.calendar, 2026, 10, 23, 9, 0);
        // p is outside its window but was interrupted mid-run => still planned.
        let rec = RunningEntry {
            entry_id: "p".to_string(),
            task: "任务1".to_string(),
            pass: 3,
            attempt: 1,
            since_ms: (now - 5000) as u64,
            resume: true,
        };
        let plan = plan_pass(&s, &SchedulerState::new(), now, Some(&rec));
        assert_eq!(plan.runnable.len(), 2, "p (recover) + t (temp): {plan:?}");
        let p = plan
            .runnable
            .iter()
            .find(|r| r.entry_id == "p")
            .expect("p planned");
        assert!(p.resume, "periodic recovery resumes");
        assert_eq!(p.reason, "recover");

        let rec_temp = RunningEntry {
            entry_id: "t".to_string(),
            task: "任务2".to_string(),
            ..rec.clone()
        };
        let plan = plan_pass(&s, &SchedulerState::new(), now, Some(&rec_temp));
        assert_eq!(plan.runnable.len(), 1);
        assert!(!plan.runnable[0].resume, "temp has no completion memory");
        assert_eq!(plan.runnable[0].reason, "recover_temp");
    }

    // -- next_wake ----------------------------------------------------------

    #[test]
    fn next_wake_caps_at_30s() {
        let s = sched(
            r#"{"schema_version":3,
                "config":{"calendar":{"day_reset":"00:00"}},
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"periodic",
                            "windows":[{"days":[],"start":"12:00","end":"13:00"}]}]}"#,
        );
        let now = at(&s.calendar, 2026, 10, 23, 9, 0); // 3h before window
        let wake = next_wake(&s, &SchedulerState::new(), now);
        assert_eq!(wake.at_ms, now + MAX_SLEEP_MS);
        assert_eq!(wake.reasons, vec!["cap"]);
    }

    #[test]
    fn next_wake_uses_near_window_start() {
        let s = sched(
            r#"{"schema_version":3,
                "config":{"calendar":{"day_reset":"00:00"}},
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"periodic",
                            "windows":[{"days":[],"start":"09:00","end":"13:00"}]}]}"#,
        );
        let start = at(&s.calendar, 2026, 10, 23, 9, 0);
        // 10s before the window opens.
        let now = start - 10_000;
        let wake = next_wake(&s, &SchedulerState::new(), now);
        assert_eq!(wake.at_ms, start);
        assert_eq!(wake.reasons, vec!["window_start:e1"]);
    }

    #[test]
    fn next_wake_uses_quota_reset_and_cooldown() {
        let s = sched(
            r#"{"schema_version":3,
                "config":{"calendar":{"day_reset":"00:00"},"defaults":{"retry":0,"cooldown":60}},
                "entries":[
                  {"entry_id":"q","task":"任务1","lifecycle":"periodic",
                   "quota":{"per":"day","times":1}},
                  {"entry_id":"c","task":"任务2","lifecycle":"periodic"}
                ]}"#,
        );
        let cal = s.calendar;
        let now = at(&cal, 2026, 10, 23, 9, 0);
        let mut state = SchedulerState::new();
        let cycles = cycle_ids(&cal, now);
        roll_cycles(&mut state, &cal, now);
        record_result(
            &mut state,
            &s.entries[0],
            &cycles,
            now,
            ResultClass::Success,
            "done",
            "Done",
            1,
            1,
        );
        let c = &mut state.entries.entry("c".to_string()).or_default();
        c.cooldown_until_ms = (now + 20_000) as u64;
        let wake = next_wake(&s, &state, now);
        assert_eq!(wake.at_ms, now + 20_000, "cooldown expiry is the earliest");
        assert!(wake.reasons.iter().any(|r| r == "cooldown:c"));
    }

    // -- result classification ---------------------------------------------

    #[test]
    fn default_exit_classes_follow_the_contract() {
        assert_eq!(default_exit_class("done"), ResultClass::Success);
        assert_eq!(default_exit_class("resource_empty"), ResultClass::Success);
        assert_eq!(default_exit_class("no_resource"), ResultClass::Resource);
        assert_eq!(default_exit_class("no_target"), ResultClass::Resource);
        assert_eq!(default_exit_class("window_closed"), ResultClass::Blocked);
        assert_eq!(default_exit_class("failed"), ResultClass::Retryable);
        assert_eq!(default_exit_class("exhausted"), ResultClass::Retryable);
    }

    #[test]
    fn meta_exits_override_the_defaults() {
        let meta = parse_meta_exits(
            r#"{"exits":{"done":{"result":"blocked"},"failed":"fatal","error":"retryable"}}"#,
        )
        .expect("valid meta exits");
        assert_eq!(
            classify_signal(RunSignal::Exit(Exit::Done), &meta),
            ResultClass::Blocked
        );
        assert_eq!(
            classify_signal(RunSignal::Exit(Exit::Failed), &meta),
            ResultClass::Fatal
        );
        assert_eq!(
            classify_signal(RunSignal::RunFailed, &meta),
            ResultClass::Retryable
        );
        // Missing exits => defaults.
        let empty = BTreeMap::new();
        assert_eq!(
            classify_signal(RunSignal::Exit(Exit::Done), &empty),
            ResultClass::Success
        );
        assert_eq!(
            classify_signal(RunSignal::Exhausted, &empty),
            ResultClass::Retryable
        );
    }

    #[test]
    fn meta_exits_parser_rejects_bad_results() {
        assert!(parse_meta_exits(r#"{"exits":{"done":"exploded"}}"#).is_err());
        assert!(parse_meta_exits(r#"{"exits":[1,2]}"#).is_err());
        assert!(parse_meta_exits("not json").is_err());
        assert!(parse_meta_exits(r#"{"exits":{"done":{}}}"#).is_err());
        let none = parse_meta_exits(r#"{"schema_version":1,"name":"x"}"#).expect("old meta");
        assert!(none.is_empty());
    }

    #[test]
    fn retry_budget_comes_from_the_entry() {
        let s = minimal(
            r#"{"entry_id":"e","task":"任务1","lifecycle":"periodic","retry":2}"#,
        );
        let entry = &s.entries[0];
        assert_eq!(entry.max_attempts(), 3);
        assert!(should_retry(ResultClass::Retryable, 1, entry));
        assert!(should_retry(ResultClass::Retryable, 2, entry));
        assert!(!should_retry(ResultClass::Retryable, 3, entry));
        assert!(!should_retry(ResultClass::Blocked, 1, entry));
        assert!(!should_retry(ResultClass::Success, 1, entry));
    }

    // -- config validation matrix ------------------------------------------

    #[test]
    fn config_accepts_minimal_and_full_documents() {
        let s = minimal(r#"{"entry_id":"a","task":"任务1","lifecycle":"periodic"}"#);
        assert_eq!(s.mode, RunMode::Stop);
        assert_eq!(s.loop_interval_s, DEFAULT_LOOP_INTERVAL_S);
        assert_eq!(s.calendar.day_reset_min, 0);
        assert_eq!(s.calendar.week_reset_weekday, 0);
        assert_eq!(s.entries[0].retry, DEFAULT_RETRY);
        assert_eq!(s.entries[0].cooldown_s, DEFAULT_COOLDOWN_S);

        let full = sched(
            r#"{"schema_version":3,
                "config":{
                  "mode":"loop","loop_interval_s":5,"loop_rounds":3,"utc_offset_min":480,
                  "calendar":{"day_reset":"05:00","week_reset":{"weekday":6,"time":"04:30"}},
                  "guards":{"active_hours":[["08:00","23:00"]],
                            "blackouts":[{"days":[1],"start":"02:00","end":"02:10","reason":"维护"}]},
                  "defaults":{"retry":2,"cooldown":120}},
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"periodic",
                            "windows":[{"days":[0,1,2,3,4],"start":"08:00","end":"12:00"},
                                       {"days":[],"start":"22:00","end":"02:00"}],
                            "quota":{"per":"week","times":2},"params":{"max_rounds":3},
                            "retry":1,"cooldown":60}]}"#,
        );
        assert_eq!(full.mode, RunMode::Loop);
        assert_eq!(full.loop_rounds, 3);
        assert_eq!(full.calendar.day_reset_min, 300);
        assert_eq!(full.calendar.week_reset_weekday, 6);
        assert_eq!(full.calendar.week_reset_min, 270);
        assert_eq!(full.calendar.utc_offset_min, 480);
        assert_eq!(full.entries[0].windows.len(), 2);
        assert_eq!(full.entries[0].quota.as_ref().unwrap().times, 2);
        assert_eq!(
            full.entries[0].params.get("max_rounds"),
            Some(&Value::from(3))
        );
        assert_eq!(full.entries[0].retry, 1);
        assert_eq!(full.entries[0].cooldown_s, 60);
    }

    #[test]
    fn empty_entries_is_valid_after_deleting_all_tasks() {
        assert!(validate_scheduler(r#"{"schema_version":3,"entries":[]}"#).is_ok());
    }

    #[test]
    fn config_validation_rejects_bad_documents() {
        let cases: Vec<(&str, &str)> = vec![
            (r#"{"entries":[]}"#, "schema_version"),
            (r#"{"schema_version":2,"entries":[]}"#, "schema_version"),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"","task":"任务1"}]}"#,
                "entry_id",
            ),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"a","task":""}]}"#,
                "task",
            ),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"a","task":"../x"}]}"#,
                "task",
            ),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"a","task":"任务1"},
                     {"entry_id":"a","task":"任务2"}]}"#,
                "duplicate",
            ),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"a","task":"任务1",
                     "windows":[{"start":"25:00","end":"26:00"}]}]}"#,
                "hour",
            ),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"a","task":"任务1",
                     "lifecycle":"temp","quota":{"per":"day","times":1}}]}"#,
                "temp must not declare `quota`",
            ),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"a","task":"任务1",
                     "lifecycle":"temp","windows":[{"start":"08:00","end":"09:00"}]}]}"#,
                "temp must not declare `windows`",
            ),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"a","task":"任务1",
                     "quota":{"per":"day","times":0}}]}"#,
                "times must be >= 1",
            ),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"a","task":"任务1",
                     "quota":{"times":1}}]}"#,
                "quota.per is required",
            ),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"a","task":"任务1","retry":99}]}"#,
                "retry must be <=",
            ),
            (
                r#"{"schema_version":3,"entries":[{"entry_id":"a","task":"任务1","lifecycle":"cron"}]}"#,
                "unknown variant",
            ),
            (
                r#"{"schema_version":3,"config":{"calendar":{"week_reset":{"weekday":7}}},
                    "entries":[{"entry_id":"a","task":"任务1"}]}"#,
                "weekday must be 0..=6",
            ),
            (
                r#"{"schema_version":3,"config":{"guards":{"active_hours":[["08:00"]]}},
                    "entries":[{"entry_id":"a","task":"任务1"}]}"#,
                "invalid",
            ),
        ];
        for (json, needle) in cases {
            let err = validate_scheduler(json).expect_err(json);
            assert!(
                err.contains(needle),
                "`{json}` error should contain `{needle}`, got: {err}"
            );
        }
    }

    // -- next due -----------------------------------------------------------

    #[test]
    fn entry_next_due_reflects_window_quota_and_cooldown() {
        let s = sched(
            r#"{"schema_version":3,
                "config":{"calendar":{"day_reset":"00:00"},"defaults":{"retry":0,"cooldown":60}},
                "entries":[
                  {"entry_id":"w","task":"任务1","lifecycle":"periodic",
                   "windows":[{"days":[],"start":"12:00","end":"13:00"}]},
                  {"entry_id":"q","task":"任务2","lifecycle":"periodic",
                   "quota":{"per":"day","times":1}},
                  {"entry_id":"t","task":"任务3","lifecycle":"temp"}
                ]}"#,
        );
        let cal = s.calendar;
        let now = at(&cal, 2026, 10, 23, 9, 0);
        let mut state = SchedulerState::new();
        let cycles = cycle_ids(&cal, now);
        roll_cycles(&mut state, &cal, now);
        record_result(
            &mut state,
            &s.entries[1],
            &cycles,
            now,
            ResultClass::Success,
            "done",
            "Done",
            1,
            1,
        );
        assert_eq!(
            entry_next_due_ms(&s, &state, &s.entries[0], now),
            Some(at(&cal, 2026, 10, 23, 12, 0)),
            "waiting for the window"
        );
        assert_eq!(
            entry_next_due_ms(&s, &state, &s.entries[1], now),
            Some(at(&cal, 2026, 10, 24, 0, 0)),
            "daily quota resets at the logical day boundary"
        );
        assert_eq!(
            entry_next_due_ms(&s, &state, &s.entries[2], now),
            Some(now),
            "temp runs any pass"
        );
    }
}
