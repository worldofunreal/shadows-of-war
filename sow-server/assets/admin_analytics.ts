// Operator dashboard for /admin/analytics. Loads the admin analytics report with the
// operator's access key and renders it as tables. Every value is written as text.

interface SourceStatus {
  status: string;
  source: string;
}

interface DailyTotals {
  active_sessions?: number;
  active_accounts?: number;
}

type Counts = Record<string, number>;

interface TutorialStep {
  episode_id: string;
  step_id?: string;
  started?: number;
  completed?: number;
  exited?: number;
  completion_rate?: number | null;
}

interface RetentionWindow {
  status: string;
  cohort_cutoff_date?: string | null;
  eligible_accounts?: number;
  returned_accounts?: number;
  eligible_browsers?: number;
  returned_browsers?: number;
  rate?: number;
}

interface MissionDimensionChurn {
  dimension: string;
  axis: string;
  tag: string;
  offered: number;
  completed: number;
  swapped_out_total: number;
  swap_rate?: number | null;
  completion_rate?: number | null;
}

interface MissionItemChurn {
  mission_id: string;
  axis: string;
  tag: string;
  offered: number;
  completed: number;
  swapped_out_total: number;
  claimed_ad_2x?: number;
  swap_rate?: number | null;
}

interface MenuGuideSummaryRow {
  guide_id: string;
  completed: number;
  skipped_step: number;
  disabled_all: number;
}

interface AnalyticsReport {
  generated_at?: string;
  daily?: DailyTotals[];
  funnel?: Counts;
  funnel_sessions?: Counts;
  sources?: Record<string, SourceStatus>;
  dimensions?: {
    build?: Record<string, Counts>;
    device_class?: Record<string, Counts>;
    source?: Record<string, Counts>;
  };
  tutorial_steps?: TutorialStep[];
  match_flow?: MatchFlow[];
  retention?: {
    site?: Record<string, RetentionWindow>;
    site_browser?: Record<string, RetentionWindow>;
  };
  mission_churn_by_dimension?: MissionDimensionChurn[];
  mission_churn_by_mission?: MissionItemChurn[];
  menu_guide_summary?: MenuGuideSummaryRow[];
}

interface MatchFlow {
  event: string;
  mode?: string;
  outcome?: string;
  wait_bucket?: string;
  duration_bucket?: string;
  reason_category?: string;
  count: number;
}

const DEVICE_EVENTS = ["matchmaking_joined", "match_started_client", "match_ended_client", "match_exit"];
const SOURCE_EVENTS = ["landing_visit", "play_now_click", "match_started_client", "match_ended_client"];

function $<T extends HTMLElement = HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

function esc(value: unknown): string {
  return String(value ?? "");
}

function rows<T>(target: HTMLElement, values: T[], render: (row: HTMLTableRowElement, value: T) => void): void {
  target.replaceChildren(
    ...values.map((value) => {
      const row = document.createElement("tr");
      render(row, value);
      return row;
    }),
  );
}

function cell(row: HTMLTableRowElement, value: unknown, className?: string): void {
  const td = document.createElement("td");
  td.textContent = esc(value);
  if (className) td.className = className;
  row.append(td);
}

function renderMetrics(funnel: Counts, today: DailyTotals): void {
  const metrics: [string, number | undefined][] = [
    ["Active sessions today", today.active_sessions || 0],
    ["Legacy account pseudonyms today", today.active_accounts || 0],
    ["Site visits", funnel.landing_visit || 0],
    ["Game shells loaded", funnel.shell_loaded || 0],
    ["Quick match attempts", funnel.menu_quick_match || 0],
    ["Matchmaking joins", funnel.matchmaking_joined || 0],
    ["Queue cancellations", funnel.matchmaking_cancelled || 0],
    ["Lobby join failures", funnel.lobby_join_failed || 0],
    ["Match starts", funnel.match_started_client || 0],
    ["Match ends", funnel.match_ended_client || 0],
    ["Explicit early exits", funnel.match_exit || 0],
  ];
  $("metrics").replaceChildren(
    ...metrics.map(([label, value]) => {
      const box = document.createElement("div");
      box.className = "metric";
      const small = document.createElement("small");
      small.textContent = label;
      const strong = document.createElement("strong");
      strong.textContent = Number(value).toLocaleString();
      box.append(small, strong);
      return box;
    }),
  );
}

function renderDimensions(
  id: string,
  dimensions: Record<string, Counts> | undefined,
  columns: string[] = DEVICE_EVENTS,
): void {
  const total = (counts: Counts) => Object.values(counts).reduce((sum, value) => sum + value, 0);
  rows(
    $(id),
    Object.entries(dimensions || {}).sort((a, b) => total(b[1]) - total(a[1])),
    (row, [label, counts]) => {
      cell(row, label);
      cell(row, total(counts).toLocaleString());
      for (const name of columns) cell(row, Number(counts[name] || 0).toLocaleString());
    },
  );
}

function renderRetention(
  id: string,
  windows: Record<string, RetentionWindow>,
  subject: "accounts" | "browsers",
): void {
  rows(
    $(id),
    Object.entries(windows),
    (row, [windowName, data]) => {
      const ready = data.status === "ready";
      const eligible = subject === "browsers" ? data.eligible_browsers : data.eligible_accounts;
      const returned = subject === "browsers" ? data.returned_browsers : data.returned_accounts;
      cell(row, windowName.toUpperCase());
      cell(
        row,
        data.status === "ready"
          ? "Matured"
          : data.status === "not_matured"
            ? "Pending"
            : `No eligible ${subject}`,
      );
      cell(row, data.cohort_cutoff_date || "—");
      cell(row, ready ? Number(eligible || 0).toLocaleString() : "—");
      cell(row, ready ? Number(returned || 0).toLocaleString() : "—");
      cell(row, ready ? (100 * Number(data.rate)).toFixed(1) + "%" : "—");
    },
  );
}

function renderReport(report: AnalyticsReport): void {
  const today = report.daily?.[0] || {};
  const funnel = report.funnel || {};
  const funnelSessions = report.funnel_sessions || {};
  $("generated").textContent = "Updated " + esc(report.generated_at) + " · UTC";
  renderMetrics(funnel, today);
  rows($("sources"), Object.entries(report.sources || {}), (row, [name, source]) => {
    cell(row, name);
    cell(row, source.status, source.status === "connected" ? "status" : "pending");
    cell(row, source.source);
  });
  rows(
    $("funnel"),
    Object.entries(funnel).sort((a, b) => b[1] - a[1]),
    (row, [name, count]) => {
      cell(row, name.replaceAll("_", " "));
      cell(row, Number(count).toLocaleString());
      cell(row, Number(funnelSessions[name] || 0).toLocaleString());
    },
  );
  rows($("match-flow"), report.match_flow || [], (row, flow) => {
    cell(row, flow.event.replaceAll("_", " "));
    cell(row, flow.mode?.replaceAll("_", " ") || "—");
    cell(row, flow.outcome || "—");
    cell(row, flow.wait_bucket || "—");
    cell(row, flow.duration_bucket || "—");
    cell(row, flow.reason_category?.replaceAll("_", " ") || "—");
    cell(row, Number(flow.count).toLocaleString());
  });
  renderDimensions("by-build", report.dimensions?.build);
  renderDimensions("by-device", report.dimensions?.device_class);
  renderDimensions("by-source", report.dimensions?.source, SOURCE_EVENTS);
  rows($("tutorial-steps"), report.tutorial_steps || [], (row, step) => {
    cell(row, step.episode_id);
    cell(row, String(step.step_id || "").replaceAll("_", " "));
    cell(row, Number(step.started || 0).toLocaleString());
    cell(row, Number(step.completed || 0).toLocaleString());
    cell(row, Number(step.exited || 0).toLocaleString());
    cell(
      row,
      step.completion_rate == null ? "—" : (100 * Number(step.completion_rate)).toFixed(1) + "%",
    );
  });
  rows($("mission-dimensions"), report.mission_churn_by_dimension || [], (row, dim) => {
    cell(row, String(dim.dimension || `${dim.axis}:${dim.tag}`));
    cell(row, Number(dim.offered || 0).toLocaleString());
    cell(row, Number(dim.swapped_out_total || 0).toLocaleString());
    cell(row, Number(dim.completed || 0).toLocaleString());
    cell(row, dim.swap_rate == null ? "—" : (100 * Number(dim.swap_rate)).toFixed(1) + "%");
    cell(
      row,
      dim.completion_rate == null ? "—" : (100 * Number(dim.completion_rate)).toFixed(1) + "%",
    );
  });
  rows($("mission-items"), report.mission_churn_by_mission || [], (row, item) => {
    cell(row, item.mission_id);
    cell(row, `${item.axis}:${item.tag}`);
    cell(row, Number(item.offered || 0).toLocaleString());
    cell(row, Number(item.swapped_out_total || 0).toLocaleString());
    cell(row, Number(item.completed || 0).toLocaleString());
    cell(row, item.swap_rate == null ? "—" : (100 * Number(item.swap_rate)).toFixed(1) + "%");
    cell(row, Number(item.claimed_ad_2x || 0).toLocaleString());
  });
  rows($("menu-guide-summary"), report.menu_guide_summary || [], (row, guide) => {
    cell(row, guide.guide_id);
    cell(row, Number(guide.completed || 0).toLocaleString());
    cell(row, Number(guide.skipped_step || 0).toLocaleString());
    cell(row, Number(guide.disabled_all || 0).toLocaleString());
  });
  renderRetention("site-browser-retention", report.retention?.site_browser || {}, "browsers");
  renderRetention("retention", report.retention?.site || {}, "accounts");
  $("notice").textContent = "Report loaded. Other channels are shown as separate platform sources, not as zero.";
}

$<HTMLFormElement>("controls").addEventListener("submit", async (event) => {
  event.preventDefault();
  const token = $<HTMLInputElement>("token").value.trim();
  const days = $<HTMLSelectElement>("days").value;
  $("notice").className = "notice";
  $("notice").textContent = "Loading…";
  try {
    const response = await fetch("/admin/api/analytics?days=" + encodeURIComponent(days), {
      headers: { Authorization: "Bearer " + token },
      cache: "no-store",
    });
    if (!response.ok)
      throw new Error(
        response.status === 401 ? "Access key was not accepted." : "Analytics service is unavailable.",
      );
    renderReport((await response.json()) as AnalyticsReport);
  } catch (error) {
    $("notice").className = "notice error";
    $("notice").textContent = (error as Error).message;
  }
});
