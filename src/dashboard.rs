#[must_use]
pub fn index_html(hostname: &str) -> String {
    INDEX_HTML.replace("__HOSTNAME__", hostname)
}

const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Pi Monitor</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;700&family=Manrope:wght@300;400;500;700&display=swap" rel="stylesheet">
  <style>
    :root {
      --bg: #0a0a0a;
      --card: #111111;
      --border: #1f1f1f;
      --fg: #ffffff;
      --muted: #707070;
      --faint: #404040;
      --accent: #ffffff;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    html { font-size: 16px; }
    body {
      font-family: "Manrope", -apple-system, sans-serif;
      font-weight: 400;
      color: var(--fg);
      background: var(--bg);
      min-height: 100vh;
      line-height: 1.5;
      -webkit-font-smoothing: antialiased;
    }
    .shell {
      max-width: 1200px;
      margin: 0 auto;
      padding: 32px;
    }
    header {
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 24px;
      align-items: start;
      margin-bottom: 32px;
      padding-bottom: 24px;
      border-bottom: 1px solid var(--border);
    }
    .hostname {
      font-size: 13px;
      font-weight: 500;
      letter-spacing: 0.12em;
      text-transform: uppercase;
      color: var(--muted);
      margin-bottom: 4px;
    }
    h1 {
      font-size: clamp(32px, 6vw, 48px);
      font-weight: 300;
      letter-spacing: -0.02em;
      line-height: 1.1;
    }
    .status-bar {
      display: flex;
      gap: 16px;
      align-items: center;
      flex-wrap: wrap;
    }
    .pill {
      font-family: "JetBrains Mono", monospace;
      font-size: 11px;
      font-weight: 500;
      letter-spacing: 0.05em;
      text-transform: uppercase;
      padding: 6px 12px;
      border: 1px solid var(--border);
      color: var(--muted);
    }
    .pill.live { border-color: var(--fg); color: var(--fg); }
    .pill.offline { border-color: var(--faint); color: var(--faint); font-style: italic; }
    .meta-row {
      display: flex;
      gap: 32px;
      margin-top: 16px;
      flex-wrap: wrap;
    }
    .meta-item {
      font-family: "JetBrains Mono", monospace;
      font-size: 12px;
      color: var(--muted);
    }
    .meta-item span { color: var(--fg); }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 1px;
      background: var(--border);
      margin-bottom: 1px;
    }
    .card {
      background: var(--card);
      padding: 24px;
      min-height: 140px;
      display: flex;
      flex-direction: column;
    }
    .label {
      font-size: 11px;
      font-weight: 500;
      letter-spacing: 0.1em;
      text-transform: uppercase;
      color: var(--muted);
      margin-bottom: 16px;
    }
    .value {
      font-family: "JetBrains Mono", monospace;
      font-size: 32px;
      font-weight: 500;
      line-height: 1;
      margin-bottom: 12px;
    }
    .value.warn { text-decoration: underline; font-style: italic; }
    .value.critical { font-weight: 700; }
    .detail {
      font-family: "JetBrains Mono", monospace;
      font-size: 11px;
      color: var(--faint);
      margin-top: auto;
    }
    .detail span { color: var(--muted); }
    .charts {
      background: var(--card);
      border-top: 1px solid var(--border);
    }
    .chart-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
      gap: 1px;
      background: var(--border);
    }
    .chart-panel {
      background: var(--card);
      padding: 24px;
    }
    .chart-header {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      margin-bottom: 16px;
    }
    .chart-value {
      font-family: "JetBrains Mono", monospace;
      font-size: 20px;
      font-weight: 500;
    }
    canvas {
      width: 100%;
      height: 80px;
      display: block;
    }
    .warnings {
      display: flex;
      gap: 12px;
      flex-wrap: wrap;
      margin-top: 16px;
    }
    .warn-pill {
      font-family: "JetBrains Mono", monospace;
      font-size: 10px;
      font-weight: 500;
      letter-spacing: 0.05em;
      text-transform: uppercase;
      padding: 4px 10px;
      border: 1px solid var(--muted);
      color: var(--muted);
    }
    .warn-pill.critical { border-color: var(--fg); color: var(--fg); font-weight: 700; }
    .warn-pill.warn { font-style: italic; text-decoration: underline; }
    @media (max-width: 768px) {
      .shell { padding: 16px; }
      header { grid-template-columns: 1fr; }
      .meta-row { gap: 16px; }
      .card { padding: 20px; min-height: 120px; }
      .value { font-size: 28px; }
    }
  </style>
</head>
<body>
  <div class="shell">
    <header>
      <div>
        <div class="hostname">Raspberry Pi</div>
        <h1>__HOSTNAME__</h1>
        <div class="meta-row">
          <div class="meta-item">Up <span id="uptimeLabel">--</span></div>
          <div class="meta-item">Load <span id="loadLabel">-- / --</span></div>
          <div class="meta-item">Clock <span id="clockLabel">-- / --</span></div>
        </div>
      </div>
      <div class="status-bar">
        <span id="connectionPill" class="pill">Connecting</span>
        <span class="pill">5s Poll</span>
      </div>
    </header>

    <section class="grid">
      <article class="card">
        <div class="label">CPU</div>
        <div class="value" id="cpuValue">--</div>
        <div class="detail" id="cpuDetail">--</div>
      </article>
      <article class="card">
        <div class="label">Memory</div>
        <div class="value" id="memoryValue">--</div>
        <div class="detail" id="memoryDetail">--</div>
      </article>
      <article class="card">
        <div class="label">Disk</div>
        <div class="value" id="diskValue">--</div>
        <div class="detail" id="diskDetail">--</div>
      </article>
      <article class="card">
        <div class="label">Temperature</div>
        <div class="value" id="tempValue">--</div>
        <div class="detail" id="tempDetail">--</div>
      </article>
      <article class="card">
        <div class="label">Voltage</div>
        <div class="value" id="voltValue">--</div>
        <div class="detail" id="voltDetail">--</div>
      </article>
      <article class="card">
        <div class="label">Throttle</div>
        <div class="value" id="throttleValue">--</div>
        <div class="detail" id="throttleDetail">--</div>
      </article>
    </section>

    <section class="charts">
      <div class="chart-grid">
        <div class="chart-panel">
          <div class="chart-header">
            <div class="label">CPU History</div>
            <div class="chart-value" id="cpuChartValue">--</div>
          </div>
          <canvas id="cpuChart"></canvas>
        </div>
        <div class="chart-panel">
          <div class="chart-header">
            <div class="label">Memory History</div>
            <div class="chart-value" id="memChartValue">--</div>
          </div>
          <canvas id="memChart"></canvas>
        </div>
        <div class="chart-panel">
          <div class="chart-header">
            <div class="label">Temperature History</div>
            <div class="chart-value" id="tempChartValue">--</div>
          </div>
          <canvas id="tempChart"></canvas>
        </div>
        <div class="chart-panel">
          <div class="chart-header">
            <div class="label">Voltage History</div>
            <div class="chart-value" id="voltChartValue">--</div>
          </div>
          <canvas id="voltChart"></canvas>
        </div>
      </div>
    </section>

    <section style="padding: 24px 0; border-top: 1px solid var(--border); margin-top: 1px;">
      <div class="label" style="margin-bottom: 12px;">System Status</div>
      <div class="warnings" id="warnRow"></div>
      <div style="font-family: 'JetBrains Mono', monospace; font-size: 11px; color: var(--faint); margin-top: 16px;">
        Last update: <span id="statusText" style="color: var(--muted);">--</span>
      </div>
    </section>
  </div>

  <script>
    const charts = {
      cpu: document.getElementById("cpuChart"),
      mem: document.getElementById("memChart"),
      temp: document.getElementById("tempChart"),
      volt: document.getElementById("voltChart"),
    };

    function pct(used, total) {
      if (!total) return 0;
      return (used / total) * 100;
    }

    function fmtBytes(bytes) {
      if (!bytes) return "0 B";
      const units = ["B", "KB", "MB", "GB", "TB"];
      let value = bytes;
      let idx = 0;
      while (value >= 1024 && idx < units.length - 1) {
        value /= 1024;
        idx += 1;
      }
      return `${value.toFixed(value >= 10 || idx === 0 ? 0 : 1)} ${units[idx]}`;
    }

    function fmtDuration(seconds) {
      const s = Math.floor(seconds || 0);
      const days = Math.floor(s / 86400);
      const hours = Math.floor((s % 86400) / 3600);
      const mins = Math.floor((s % 3600) / 60);
      return `${days}d ${hours}h ${mins}m`;
    }

    function setConnection(ok) {
      const el = document.getElementById("connectionPill");
      el.textContent = ok ? "Connected" : "Offline";
      el.className = ok ? "pill live" : "pill offline";
    }

    function statusClass(value, warnAt, criticalAt) {
      if (value >= criticalAt) return "critical";
      if (value >= warnAt) return "warn";
      return "";
    }

    function drawSpark(canvas, values, maxHint) {
      const ctx = canvas.getContext("2d");
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      ctx.scale(dpr, dpr);
      const width = rect.width;
      const height = rect.height;

      ctx.clearRect(0, 0, width, height);

      ctx.strokeStyle = "#1f1f1f";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, height - 0.5);
      ctx.lineTo(width, height - 0.5);
      ctx.stroke();

      const filtered = values.filter((v) => Number.isFinite(v));
      if (filtered.length < 2) return;

      const max = Math.max(maxHint, ...filtered);
      const min = Math.min(...filtered);
      const span = Math.max(max - min, maxHint > 0 ? maxHint * 0.1 : 1);

      ctx.strokeStyle = "#ffffff";
      ctx.lineWidth = 1.5;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.beginPath();
      filtered.forEach((value, index) => {
        const x = (index / (filtered.length - 1)) * width;
        const normalized = (value - min) / span;
        const y = height - normalized * (height - 8) - 4;
        if (index === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.stroke();
    }

    function renderWarnings(sample) {
      const row = document.getElementById("warnRow");
      row.innerHTML = "";
      const warnings = [];
      if (sample.under_voltage_now) warnings.push(["Under-voltage", "critical"]);
      if (sample.freq_capped_now) warnings.push(["Freq capped", "warn"]);
      if (sample.throttled_now) warnings.push(["Throttled", "critical"]);
      if (sample.soft_temp_limit_now) warnings.push(["Temp limit", "warn"]);
      if (!warnings.length) warnings.push(["Nominal", ""]);
      warnings.forEach(([text, cls]) => {
        const pill = document.createElement("span");
        pill.className = `warn-pill ${cls}`;
        pill.textContent = text;
        row.appendChild(pill);
      });
    }

    function renderCurrent(sample) {
      const memPercent = pct(sample.memory_used_bytes, sample.memory_total_bytes);
      const diskPercent = pct(sample.disk_used_bytes, sample.disk_total_bytes);

      const cpuEl = document.getElementById("cpuValue");
      cpuEl.textContent = `${sample.cpu_usage_percent.toFixed(1)}%`;
      cpuEl.className = "value " + statusClass(sample.cpu_usage_percent, 70, 90);
      document.getElementById("cpuDetail").innerHTML = sample.cpu_per_core_percent.length
        ? sample.cpu_per_core_percent.map((v, i) => `<span>c${i} ${v.toFixed(0)}%</span>`).join(" ")
        : "<span>--</span>";

      const memEl = document.getElementById("memoryValue");
      memEl.textContent = `${memPercent.toFixed(1)}%`;
      memEl.className = "value " + statusClass(memPercent, 70, 90);
      document.getElementById("memoryDetail").innerHTML =
        `<span>${fmtBytes(sample.memory_used_bytes)}</span> / ${fmtBytes(sample.memory_total_bytes)}`;

      document.getElementById("diskValue").textContent = `${diskPercent.toFixed(1)}%`;
      document.getElementById("diskDetail").innerHTML =
        `<span>${fmtBytes(sample.disk_used_bytes)}</span> / ${fmtBytes(sample.disk_total_bytes)}`;

      const tempEl = document.getElementById("tempValue");
      tempEl.textContent = sample.cpu_temp_c == null ? "--" : `${sample.cpu_temp_c.toFixed(1)}°`;
      tempEl.className = "value " + statusClass(sample.cpu_temp_c ?? 0, 60, 80);
      document.getElementById("tempDetail").innerHTML = sample.gpu_temp_c == null
        ? "<span>GPU --</span>"
        : `<span>GPU ${sample.gpu_temp_c.toFixed(1)}°</span>`;

      const voltEl = document.getElementById("voltValue");
      voltEl.textContent = sample.core_volts == null ? "--" : `${sample.core_volts.toFixed(2)}V`;
      document.getElementById("voltDetail").innerHTML =
        `SDRAM <span>${sample.sdram_c_volts?.toFixed?.(2) ?? "--"}</span> / <span>${sample.sdram_i_volts?.toFixed?.(2) ?? "--"}</span> / <span>${sample.sdram_p_volts?.toFixed?.(2) ?? "--"}</span>`;

      const throttleEl = document.getElementById("throttleValue");
      throttleEl.textContent = sample.throttled_now ? "Active" : "Clear";
      throttleEl.className = "value" + (sample.throttled_now || sample.under_voltage_now ? " critical" : "");
      document.getElementById("throttleDetail").innerHTML =
        sample.throttled_raw == null ? "<span>--</span>" : `<span>0x${sample.throttled_raw.toString(16).toUpperCase()}</span>`;

      document.getElementById("uptimeLabel").textContent = fmtDuration(sample.uptime_seconds);
      document.getElementById("loadLabel").textContent = `${sample.loadavg_1.toFixed(2)} / ${sample.loadavg_5.toFixed(2)}`;

      const armGHz = sample.arm_clock_hz == null ? "--" : (sample.arm_clock_hz / 1e9).toFixed(2);
      const gpuMHz = sample.gpu_clock_hz == null ? "--" : (sample.gpu_clock_hz / 1e6).toFixed(0);
      document.getElementById("clockLabel").textContent = `${armGHz}GHz / ${gpuMHz}MHz`;
      document.getElementById("statusText").textContent = new Date(sample.timestamp_unix_ms).toLocaleTimeString();

      renderWarnings(sample);
    }

    function renderCharts(history) {
      const cpu = history.map((i) => i.cpu_usage_percent);
      const mem = history.map((i) => pct(i.memory_used_bytes, i.memory_total_bytes));
      const temp = history.map((i) => i.cpu_temp_c);
      const volt = history.map((i) => i.core_volts);

      drawSpark(charts.cpu, cpu, 100);
      drawSpark(charts.mem, mem, 100);
      drawSpark(charts.temp, temp, 90);
      drawSpark(charts.volt, volt, 2);

      const latest = history[history.length - 1];
      if (!latest) return;

      document.getElementById("cpuChartValue").textContent = `${latest.cpu_usage_percent.toFixed(1)}%`;
      document.getElementById("memChartValue").textContent = `${pct(latest.memory_used_bytes, latest.memory_total_bytes).toFixed(1)}%`;
      document.getElementById("tempChartValue").textContent = latest.cpu_temp_c == null ? "--" : `${latest.cpu_temp_c.toFixed(1)}°`;
      document.getElementById("voltChartValue").textContent = latest.core_volts == null ? "--" : `${latest.core_volts.toFixed(2)}V`;
    }

    async function refresh() {
      try {
        const [currentRes, historyRes] = await Promise.all([
          fetch("/api/current", { cache: "no-store" }),
          fetch("/api/history?limit=120", { cache: "no-store" }),
        ]);
        if (!currentRes.ok || !historyRes.ok) throw new Error("request failed");
        const current = await currentRes.json();
        const history = await historyRes.json();
        setConnection(true);
        renderCurrent(current);
        renderCharts(history);
      } catch (_) {
        setConnection(false);
        document.getElementById("statusText").textContent = "Connection failed";
      }
    }

    refresh();
    setInterval(refresh, 5000);
  </script>
</body>
</html>
"##;
