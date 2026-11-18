//! Client diagnostics switch + sim-tick cost recorder.
//!
//! Verbose `[DIAG NET]`/`[DIAG SIM TICK]` logging is OFF by default in
//! production; enable with `?diag=1` in the URL. Tick durations are always
//! measured (two `Instant::now()` calls per tick) but only reported on anomaly — a slow frame is the actual
//! freeze signal, so it logs even without diag enabled.

use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};

static ENABLED: AtomicBool = AtomicBool::new(false);
static PERF_CAPTURE: AtomicBool = AtomicBool::new(cfg!(any(feature = "dev", debug_assertions)));

/// Ring buffer of the last N sim-tick durations in microseconds.
const CAP: usize = 128;
/// Anomaly threshold: one lockstep tick should cost far below its 100 ms slot.
const TICK_WARN_US: u64 = 80_000;

static RING: [AtomicUsize; CAP] = [const { AtomicUsize::new(0) }; CAP];
static IDX: AtomicUsize = AtomicUsize::new(0);

const STARTUP_WINDOW: Duration = Duration::from_secs(2);
const ROLLING_FRAME_CAP: usize = 120;
const ROLLING_TICK_CAP: usize = 60;
const METRIC_COUNT: usize = 14;

#[derive(Clone, Copy)]
pub(crate) enum StartupMetric {
    RenderFrame,
    UpdateSim,
    NameplateSample,
    LandCacheRebuild,
    SurfaceAcquire,
    GpuWait,
    GpuSubmit,
    WorldRender,
    MapUpdate,
    UiRender,
    WorldOverlays,
    NameplateRender,
    MenuPublish,
    TextDraw,
}

impl StartupMetric {
    const ALL_NAMES: [&'static str; METRIC_COUNT] = [
        "render_frame",
        "update_sim",
        "nameplate_sample",
        "land_cache_rebuild",
        "surface_acquire",
        "gpu_wait",
        "gpu_submit",
        "world_render",
        "map_update",
        "ui_render",
        "world_overlays",
        "nameplate_render",
        "menu_publish",
        "text_draw",
    ];

    const fn index(self) -> usize {
        match self {
            Self::RenderFrame => 0,
            Self::UpdateSim => 1,
            Self::NameplateSample => 2,
            Self::LandCacheRebuild => 3,
            Self::SurfaceAcquire => 4,
            Self::GpuWait => 5,
            Self::GpuSubmit => 6,
            Self::WorldRender => 7,
            Self::MapUpdate => 8,
            Self::UiRender => 9,
            Self::WorldOverlays => 10,
            Self::NameplateRender => 11,
            Self::MenuPublish => 12,
            Self::TextDraw => 13,
        }
    }
}

struct MetricRing {
    samples: [u64; ROLLING_FRAME_CAP],
    len: usize,
    cursor: usize,
}

impl Default for MetricRing {
    fn default() -> Self {
        Self {
            samples: [0; ROLLING_FRAME_CAP],
            len: 0,
            cursor: 0,
        }
    }
}

impl MetricRing {
    #[inline]
    fn push(&mut self, value_us: u64) {
        self.samples[self.cursor] = value_us;
        self.cursor = (self.cursor + 1) % ROLLING_FRAME_CAP;
        if self.len < ROLLING_FRAME_CAP {
            self.len += 1;
        }
    }

    fn summary_ms(&self) -> serde_json::Value {
        if self.len == 0 {
            return serde_json::json!({ "samples": 0, "p50_ms": 0.0, "p95_ms": 0.0, "max_ms": 0.0 });
        }
        let mut sorted = self.samples[..self.len].to_vec();
        let (p50, p95, max) = percentiles(&mut sorted);
        serde_json::json!({
            "samples": self.len,
            "p50_ms": round2_ms(p50),
            "p95_ms": round2_ms(p95),
            "max_ms": round2_ms(max),
        })
    }
}

struct RollingPerfState {
    frame_rings: [MetricRing; METRIC_COUNT],
    tick_profiles: [sow_core::engine::TickProfile; ROLLING_TICK_CAP],
    tick_len: usize,
    tick_cursor: usize,
    last_profile: Option<sow_core::engine::TickProfile>,
}

impl Default for RollingPerfState {
    fn default() -> Self {
        Self {
            frame_rings: std::array::from_fn(|_| MetricRing::default()),
            tick_profiles: [sow_core::engine::TickProfile::default(); ROLLING_TICK_CAP],
            tick_len: 0,
            tick_cursor: 0,
            last_profile: None,
        }
    }
}

#[derive(Default)]
struct StartupPerfWindow {
    started_at: Option<web_time::Instant>,
    samples_us: [Vec<u64>; METRIC_COUNT],
    cache_prepare_cpu_us: u64,
    cache_prepare_wall_us: u64,
    cache_prepare_step_max_us: u64,
    active: bool,
    reported: bool,
}

thread_local! {
    static STARTUP_PERF: RefCell<StartupPerfWindow> = RefCell::new(StartupPerfWindow::default());
    static ROLLING_PERF: RefCell<RollingPerfState> = RefCell::new(RollingPerfState::default());
}

#[inline]
fn round2_ms(us: u64) -> f64 {
    ((us as f64) / 10.0).round() / 100.0
}

#[inline]
pub fn perf_capture_enabled() -> bool {
    PERF_CAPTURE.load(Ordering::Relaxed)
}

pub fn set_perf_capture(on: bool) {
    PERF_CAPTURE.store(on, Ordering::Relaxed);
    if !on {
        return;
    }
    ROLLING_PERF.with(|state| {
        *state.borrow_mut() = RollingPerfState::default();
    });
}

pub(crate) fn record_tick_profile(profile: sow_core::engine::TickProfile) {
    if !perf_capture_enabled() {
        return;
    }
    ROLLING_PERF.with(|state| {
        let mut state = state.borrow_mut();
        let idx = state.tick_cursor;
        state.tick_profiles[idx] = profile;
        state.tick_cursor = (idx + 1) % ROLLING_TICK_CAP;
        if state.tick_len < ROLLING_TICK_CAP {
            state.tick_len += 1;
        }
        state.last_profile = Some(profile);
    });
}

pub fn format_perf_report_json() -> String {
    let capture_enabled = perf_capture_enabled();
    ROLLING_PERF.with(|state| {
        let state = state.borrow();
        let mut frame = serde_json::Map::with_capacity(METRIC_COUNT);
        for (i, &name) in StartupMetric::ALL_NAMES.iter().enumerate() {
            frame.insert(name.to_string(), state.frame_rings[i].summary_ms());
        }

        let slice = &state.tick_profiles[..state.tick_len];
        let phase_summary = |extract: fn(&sow_core::engine::TickProfile) -> u64| {
            if slice.is_empty() {
                return serde_json::json!({ "samples": 0, "p50_ms": 0.0, "p95_ms": 0.0, "max_ms": 0.0 });
            }
            let mut vals: Vec<u64> = slice.iter().map(extract).collect();
            let (p50, p95, max) = percentiles(&mut vals);
            serde_json::json!({
                "samples": slice.len(),
                "p50_ms": round2_ms(p50),
                "p95_ms": round2_ms(p95),
                "max_ms": round2_ms(max),
            })
        };

        let sim = serde_json::json!({
            "total": phase_summary(|p| p.total_us),
            "income": phase_summary(|p| p.income_us),
            "ai_think": phase_summary(|p| p.ai_think_us),
            "construction": phase_summary(|p| p.construction_us),
            "projectiles": phase_summary(|p| p.projectiles_us),
            "sam": phase_summary(|p| p.sam_us),
            "combat": phase_summary(|p| p.combat_us),
            "sync_buildings": phase_summary(|p| p.sync_buildings_us),
            "trade_ships": phase_summary(|p| p.trade_ships_us),
            "fleets": phase_summary(|p| p.fleets_us),
            "snapshot": phase_summary(|p| p.snapshot_us),
        });

        let counts = state.last_profile.map_or_else(
            || serde_json::json!(null),
            |p| {
                serde_json::json!({
                    "tick": p.tick,
                    "alive_players": p.alive_players,
                    "buildings": p.buildings,
                    "ports": p.ports,
                    "trade_ships": p.trade_ships,
                    "warships": p.warships,
                    "transports": p.transports,
                    "attacks": p.attacks,
                    "total_border_tiles": p.total_border_tiles,
                })
            },
        );

        serde_json::json!({
            "capture_enabled": capture_enabled,
            "frame": frame,
            "sim": sim,
            "counts": counts,
        })
        .to_string()
    })
}

pub(crate) fn start_startup_perf_window(cache_build: Option<(u64, u64, u64)>) {
    if !enabled() {
        return;
    }
    STARTUP_PERF.with(|window| {
        let mut window = window.borrow_mut();
        *window = StartupPerfWindow::default();
        window.started_at = Some(web_time::Instant::now());
        window.active = true;
        if let Some((cpu_us, wall_us, max_step_us)) = cache_build {
            window.cache_prepare_cpu_us = cpu_us;
            window.cache_prepare_wall_us = wall_us;
            window.cache_prepare_step_max_us = max_step_us;
        }
        for samples in &mut window.samples_us {
            samples.reserve(120);
        }
    });
}

#[inline]
pub(crate) fn startup_perf_timer() -> Option<web_time::Instant> {
    if perf_capture_enabled() {
        return Some(web_time::Instant::now());
    }
    if !enabled() {
        return None;
    }
    STARTUP_PERF.with(|window| {
        let window = window.borrow();
        if !window.active {
            return None;
        }
        let started_at = window.started_at?;
        let now = web_time::Instant::now();
        (!window.reported && now.duration_since(started_at) < STARTUP_WINDOW).then_some(now)
    })
}

pub(crate) fn record_startup_perf(metric: StartupMetric, started_at: web_time::Instant) {
    let now = web_time::Instant::now();
    let elapsed_us = now.duration_since(started_at).as_micros() as u64;
    if perf_capture_enabled() {
        ROLLING_PERF.with(|state| {
            state.borrow_mut().frame_rings[metric.index()].push(elapsed_us);
        });
    }
    if !enabled() {
        return;
    }
    STARTUP_PERF.with(|window| {
        let mut window = window.borrow_mut();
        let Some(window_started_at) = window.started_at else {
            return;
        };
        if window.reported {
            return;
        }
        if started_at >= window_started_at
            && started_at.duration_since(window_started_at) < STARTUP_WINDOW
        {
            window.samples_us[metric.index()].push(elapsed_us);
        }
        if now.duration_since(window_started_at) >= STARTUP_WINDOW {
            report_startup_perf(&mut window);
        }
    });
}

pub(crate) fn finish_startup_perf_window_if_due(now: web_time::Instant) {
    STARTUP_PERF.with(|window| {
        let mut window = window.borrow_mut();
        if window
            .started_at
            .is_some_and(|started_at| {
                now.checked_duration_since(started_at)
                    .is_some_and(|elapsed| elapsed >= STARTUP_WINDOW)
            })
            && !window.reported
        {
            report_startup_perf(&mut window);
        }
    });
}

fn report_startup_perf(window: &mut StartupPerfWindow) {
    if window.reported {
        return;
    }
    window.reported = true;
    window.active = false;
    let [
        frame,
        update,
        sample,
        rebuild,
        acquire,
        wait,
        submit,
        world,
        map_update,
        ui,
        overlays,
        nameplates,
        publish,
        text,
    ] = &mut window.samples_us;
    let frame = percentiles(frame);
    let update = percentiles(update);
    let sample = percentiles(sample);
    let rebuild = percentiles(rebuild);
    let acquire = percentiles(acquire);
    let wait = percentiles(wait);
    let submit = percentiles(submit);
    let world = percentiles(world);
    let map_update = percentiles(map_update);
    let ui = percentiles(ui);
    let overlays = percentiles(overlays);
    let nameplates = percentiles(nameplates);
    let publish = percentiles(publish);
    let text = percentiles(text);
    log::info!(
        "[STARTUP PERF] window_ms=2000 render_frame_us={}/{}/{} update_sim_us={}/{}/{} sample_nameplates_us={}/{}/{} land_cache_rebuild_us={}/{}/{} cache_prepare_cpu_us={} cache_prepare_wall_us={} cache_prepare_step_max_us={} surface_acquire_us={}/{}/{} gpu_wait_us={}/{}/{} gpu_submit_us={}/{}/{} world_render_us={}/{}/{} map_update_us={}/{}/{} ui_render_us={}/{}/{} world_overlays_us={}/{}/{} nameplate_render_us={}/{}/{} menu_publish_us={}/{}/{} text_draw_us={}/{}/{} (p50/p95/max)",
        frame.0, frame.1, frame.2,
        update.0, update.1, update.2,
        sample.0, sample.1, sample.2,
        rebuild.0, rebuild.1, rebuild.2,
        window.cache_prepare_cpu_us,
        window.cache_prepare_wall_us,
        window.cache_prepare_step_max_us,
        acquire.0, acquire.1, acquire.2,
        wait.0, wait.1, wait.2,
        submit.0, submit.1, submit.2,
        world.0, world.1, world.2,
        map_update.0, map_update.1, map_update.2,
        ui.0, ui.1, ui.2,
        overlays.0, overlays.1, overlays.2,
        nameplates.0, nameplates.1, nameplates.2,
        publish.0, publish.1, publish.2,
        text.0, text.1, text.2,
    );
}

fn percentiles(samples: &mut Vec<u64>) -> (u64, u64, u64) {
    if samples.is_empty() {
        return (0, 0, 0);
    }
    samples.sort_unstable();
    let at = |q: f64| samples[((samples.len() as f64 * q).ceil() as usize).saturating_sub(1)];
    (at(0.50), at(0.95), *samples.last().unwrap_or(&0))
}

pub fn init_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    if on {
        PERF_CAPTURE.store(true, Ordering::Relaxed);
    }
}

pub fn init_from_url() {
    let on = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .map(|s| s.contains("diag=1"))
        .unwrap_or(false);
    init_enabled(on);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Record one sim-tick duration. Returns `Some(dt_us)` when the tick was an
/// anomaly (slow enough to threaten the frame budget).
pub fn record_tick(duration_us: u64) -> Option<u64> {
    let i = IDX.fetch_add(1, Ordering::Relaxed) % CAP;
    RING[i].store(duration_us as usize, Ordering::Relaxed);
    (duration_us > TICK_WARN_US).then_some(duration_us)
}

fn snapshot_sorted() -> Vec<u64> {
    let mut v: Vec<u64> = RING
        .iter()
        .map(|a| a.load(Ordering::Relaxed) as u64)
        .filter(|&us| us > 0)
        .collect();
    v.sort_unstable();
    v
}

/// `(min_us, p95_us, max_us)` over the recent window.
pub fn percentiles_recent() -> (u64, u64, u64) {
    let v = snapshot_sorted();
    if v.is_empty() {
        return (0, 0, 0);
    }
    let p = |q: f64| v[((v.len() as f64 * q).ceil() as usize).saturating_sub(1)];
    (*v.first().unwrap_or(&0), p(0.95), *v.last().unwrap_or(&0))
}

