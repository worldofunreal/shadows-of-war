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

/// Ring buffer of the last N sim-tick durations in microseconds.
const CAP: usize = 128;
/// Anomaly threshold: one lockstep tick should cost far below its 100 ms slot.
const TICK_WARN_US: u64 = 80_000;
/// Seconds between perf summaries while anomalies keep occurring.
const SUMMARY_SECS: u64 = 30;

static RING: [AtomicUsize; CAP] = [const { AtomicUsize::new(0) }; CAP];
static IDX: AtomicUsize = AtomicUsize::new(0);
static ANOMALIES: AtomicUsize = AtomicUsize::new(0);
static LAST_SUMMARY_MS: AtomicUsize = AtomicUsize::new(0);

const STARTUP_WINDOW: Duration = Duration::from_secs(2);

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

#[derive(Default)]
struct StartupPerfWindow {
    started_at: Option<web_time::Instant>,
    samples_us: [Vec<u64>; 14],
    cache_prepare_cpu_us: u64,
    cache_prepare_wall_us: u64,
    cache_prepare_step_max_us: u64,
    active: bool,
    reported: bool,
}

thread_local! {
    static STARTUP_PERF: RefCell<StartupPerfWindow> = RefCell::new(StartupPerfWindow::default());
}

pub(crate) fn start_startup_perf_window(cache_build: Option<(u64, u64, u64)>) {
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

pub(crate) fn startup_perf_timer() -> Option<web_time::Instant> {
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
            window.samples_us[metric.index()]
                .push(now.duration_since(started_at).as_micros() as u64);
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
    if duration_us > TICK_WARN_US {
        ANOMALIES.fetch_add(1, Ordering::Relaxed);
        Some(duration_us)
    } else {
        None
    }
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

/// Emits the periodic summary line when anomalies occurred and `SUMMARY_SECS`
/// elapsed since the last one. `now_ms`: monotonic wall clock in milliseconds.
pub fn maybe_summary(now_ms: u64) {
    if ANOMALIES.load(Ordering::Relaxed) == 0 {
        return;
    }
    let last = LAST_SUMMARY_MS.load(Ordering::Relaxed) as u64;
    if last != 0 && now_ms.saturating_sub(last) < SUMMARY_SECS * 1000 {
        return;
    }
    if LAST_SUMMARY_MS
        .compare_exchange(
            last as usize,
            now_ms as usize,
            Ordering::Relaxed,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return;
    }
    let (p50, p95, max) = percentiles_recent();
    log::warn!(
        "[SIM PERF] last {}s: anomalies={} tick_ms p50={} p95={} max={} (budget 100ms)",
        SUMMARY_SECS,
        ANOMALIES.load(Ordering::Relaxed),
        p50 / 1000,
        p95 / 1000,
        max / 1000,
    );
}
