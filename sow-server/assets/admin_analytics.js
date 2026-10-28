(() => {
  // sow-server/assets/admin_analytics.ts
  var DEVICE_EVENTS = ["matchmaking_joined", "match_started_client", "match_ended_client", "match_exit"];
  function $(id) {
    return document.getElementById(id);
  }
  function esc(value) {
    return String(value ?? "");
  }
  function rows(target, values, render) {
    target.replaceChildren(...values.map((value) => {
      const row = document.createElement("tr");
      render(row, value);
      return row;
    }));
  }
  function cell(row, value, className) {
    const td = document.createElement("td");
    td.textContent = esc(value);
    if (className)
      td.className = className;
    row.append(td);
  }
  function renderMetrics(funnel, today) {
    const metrics = [
      ["Active sessions today", today.active_sessions || 0],
      ["Consenting accounts today", today.active_accounts || 0],
      ["Site visits", funnel.landing_visit || 0],
      ["Game shells loaded", funnel.shell_loaded || 0],
      ["Matchmaking joins", funnel.matchmaking_joined || 0],
      ["Match starts", funnel.match_started_client || 0],
      ["Match ends", funnel.match_ended_client || 0],
      ["Explicit early exits", funnel.match_exit || 0]
    ];
    $("metrics").replaceChildren(...metrics.map(([label, value]) => {
      const box = document.createElement("div");
      box.className = "metric";
      const small = document.createElement("small");
      small.textContent = label;
      const strong = document.createElement("strong");
      strong.textContent = Number(value).toLocaleString();
      box.append(small, strong);
      return box;
    }));
  }
  function renderDimensions(id, dimensions) {
    const total = (counts) => Object.values(counts).reduce((sum, value) => sum + value, 0);
    rows($(id), Object.entries(dimensions || {}).sort((a, b) => total(b[1]) - total(a[1])), (row, [label, events]) => {
      cell(row, label);
      cell(row, total(events).toLocaleString());
      for (const name of DEVICE_EVENTS)
        cell(row, Number(events[name] || 0).toLocaleString());
    });
  }
  function renderRetention(windows) {
    rows($("retention"), Object.entries(windows), (row, [windowName, data]) => {
      const ready = data.status === "ready";
      cell(row, windowName.toUpperCase());
      cell(row, data.status === "ready" ? "Matured" : data.status === "not_matured" ? "Still maturing" : "No eligible accounts");
      cell(row, data.cohort_cutoff_date || "—");
      cell(row, ready ? Number(data.eligible_accounts || 0).toLocaleString() : "—");
      cell(row, ready ? Number(data.returned_accounts || 0).toLocaleString() : "—");
      cell(row, ready ? (100 * Number(data.rate)).toFixed(1) + "%" : "—");
    });
  }
  function renderReport(report) {
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
    rows($("funnel"), Object.entries(funnel).sort((a, b) => b[1] - a[1]), (row, [name, count]) => {
      cell(row, name.replaceAll("_", " "));
      cell(row, Number(count).toLocaleString());
      cell(row, Number(funnelSessions[name] || 0).toLocaleString());
    });
    renderDimensions("by-build", report.dimensions?.build);
    renderDimensions("by-device", report.dimensions?.device_class);
    rows($("tutorial-steps"), report.tutorial_steps || [], (row, step) => {
      cell(row, step.episode_id);
      cell(row, String(step.step_id || "").replaceAll("_", " "));
      cell(row, Number(step.started || 0).toLocaleString());
      cell(row, Number(step.completed || 0).toLocaleString());
      cell(row, Number(step.exited || 0).toLocaleString());
      cell(row, step.completion_rate == null ? "—" : (100 * Number(step.completion_rate)).toFixed(1) + "%");
    });
    renderRetention(report.retention?.site || {});
    $("notice").textContent = "Report loaded. Other channels are shown as separate platform sources, not as zero.";
  }
  $("controls").addEventListener("submit", async (event) => {
    event.preventDefault();
    const token = $("token").value.trim();
    const days = $("days").value;
    $("notice").className = "notice";
    $("notice").textContent = "Loading…";
    try {
      const response = await fetch("/admin/api/analytics?days=" + encodeURIComponent(days), {
        headers: { Authorization: "Bearer " + token },
        cache: "no-store"
      });
      if (!response.ok)
        throw new Error(response.status === 401 ? "Access key was not accepted." : "Analytics service is unavailable.");
      renderReport(await response.json());
    } catch (error) {
      $("notice").className = "notice error";
      $("notice").textContent = error.message;
    }
  });
})();
