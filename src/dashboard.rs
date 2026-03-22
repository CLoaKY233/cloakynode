#[must_use]
pub fn index_html(hostname: &str) -> String {
    INDEX_HTML.replace("__HOSTNAME__", hostname)
}

const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Raspberry Pi Monitor</title>
  <style>
    :root {
      --bg: #09111d;
      --panel: rgba(10, 18, 31, 0.88);
      --panel-2: rgba(16, 28, 44, 0.9);
      --line: rgba(122, 162, 255, 0.22);
      --text: #edf4ff;
      --muted: #8ea7c4;
      --good: #3bd58f;
      --warn: #ffb54c;
      --bad: #ff6b6b;
      --accent: #63b3ff;
      --accent-2: #59f0c2;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
      color: var(--text);
      background:
        radial-gradient(circle at top left, rgba(99, 179, 255, 0.16), transparent 28%),
        radial-gradient(circle at right, rgba(89, 240, 194, 0.12), transparent 22%),
        linear-gradient(180deg, #0b1422 0%, #07101a 100%);
      min-height: 100vh;
    }
    .shell {
      max-width: 1160px;
      margin: 0 auto;
      padding: 24px;
    }
    .topbar {
      display: grid;
      gap: 16px;
      grid-template-columns: 1.8fr 1fr;
      margin-bottom: 20px;
    }
    .hero, .status {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 22px;
      box-shadow: 0 14px 44px rgba(0, 0, 0, 0.28);
      backdrop-filter: blur(14px);
    }
    .hero {
      padding: 22px;
    }
    .eyebrow {
      color: var(--accent-2);
      font-size: 12px;
      letter-spacing: 0.16em;
      text-transform: uppercase;
    }
    h1 {
      margin: 8px 0 6px;
      font-size: clamp(28px, 5vw, 42px);
      line-height: 1;
    }
    .subtitle, .meta, .status-text {
      color: var(--muted);
    }
    .meta {
      display: flex;
      gap: 16px;
      flex-wrap: wrap;
      margin-top: 14px;
      font-size: 14px;
    }
    .status {
      padding: 18px;
      display: grid;
      gap: 14px;
      align-content: start;
    }
    .pill-row, .warn-row {
      display: flex;
      flex-wrap: wrap;
      gap: 10px;
    }
    .pill, .warn-pill {
      border-radius: 999px;
      padding: 8px 12px;
      font-size: 13px;
      border: 1px solid transparent;
    }
    .pill {
      background: rgba(255, 255, 255, 0.06);
      color: var(--muted);
    }
    .pill.live {
      color: #dffef2;
      background: rgba(59, 213, 143, 0.12);
      border-color: rgba(59, 213, 143, 0.35);
    }
    .pill.offline {
      color: #ffe2e2;
      background: rgba(255, 107, 107, 0.12);
      border-color: rgba(255, 107, 107, 0.35);
    }
    .warn-pill.good {
      background: rgba(59, 213, 143, 0.12);
      border-color: rgba(59, 213, 143, 0.28);
    }
    .warn-pill.warn {
      background: rgba(255, 181, 76, 0.12);
      border-color: rgba(255, 181, 76, 0.28);
    }
    .warn-pill.bad {
      background: rgba(255, 107, 107, 0.12);
      border-color: rgba(255, 107, 107, 0.28);
    }
    .grid {
      display: grid;
      gap: 16px;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      margin-bottom: 16px;
    }
    .card, .charts {
      background: var(--panel-2);
      border: 1px solid var(--line);
      border-radius: 20px;
      box-shadow: 0 12px 34px rgba(0, 0, 0, 0.22);
    }
    .card {
      padding: 18px;
      min-height: 148px;
    }
    .label {
      color: var(--muted);
      font-size: 13px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
    }
    .value {
      margin-top: 14px;
      font-size: 34px;
      font-weight: 700;
      line-height: 1;
    }
    .detail {
      margin-top: 12px;
      color: var(--muted);
      font-size: 14px;
      min-height: 20px;
    }
    .charts {
      padding: 18px;
    }
    .chart-grid {
      display: grid;
      gap: 14px;
      grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    }
    .chart-panel {
      padding: 14px;
      border-radius: 16px;
      background: rgba(255, 255, 255, 0.03);
      border: 1px solid rgba(255, 255, 255, 0.04);
    }
    canvas {
      width: 100%;
      height: 100px;
      display: block;
      margin-top: 10px;
    }
    .chart-value {
      color: var(--text);
      font-size: 22px;
      font-weight: 700;
    }
    @media (max-width: 860px) {
      .topbar {
        grid-template-columns: 1fr;
      }
      .shell {
        padding: 16px;
      }
    }
  </style>
</head>
<body>
  <div class="shell">
    <section class="topbar">
      <div class="hero">
        <div class="eyebrow">Raspberry Pi Monitor</div>
        <h1>__HOSTNAME__</h1>
        <div class="subtitle">Low-overhead local dashboard for Pi thermals, power and system load.</div>
        <div class="meta">
          <span id="uptimeLabel">Uptime: --</span>
          <span id="loadLabel">Load: -- / --</span>
          <span id="clockLabel">ARM: -- GHz | GPU: -- MHz</span>
        </div>
      </div>
      <aside class="status">
        <div class="pill-row">
          <span id="connectionPill" class="pill">Connecting</span>
          <span class="pill">Polling: 5s</span>
          <span class="pill">History: 10 min</span>
        </div>
        <div class="status-text" id="statusText">Waiting for the first sample.</div>
        <div class="warn-row" id="warnRow"></div>
      </aside>
    </section>

    <section class="grid">
      <article class="card">
        <div class="label">CPU Usage</div>
        <div class="value" id="cpuValue">--</div>
        <div class="detail" id="cpuDetail">Per-core activity unavailable</div>
      </article>
      <article class="card">
        <div class="label">Memory</div>
        <div class="value" id="memoryValue">--</div>
        <div class="detail" id="memoryDetail">Used vs total</div>
      </article>
      <article class="card">
        <div class="label">Disk /</div>
        <div class="value" id="diskValue">--</div>
        <div class="detail" id="diskDetail">Filesystem usage</div>
      </article>
      <article class="card">
        <div class="label">CPU Temp</div>
        <div class="value" id="tempValue">--</div>
        <div class="detail" id="tempDetail">Thermal headroom</div>
      </article>
      <article class="card">
        <div class="label">Core Voltage</div>
        <div class="value" id="voltValue">--</div>
        <div class="detail" id="voltDetail">SDRAM rails hidden until detected</div>
      </article>
      <article class="card">
        <div class="label">Throttle State</div>
        <div class="value" id="throttleValue">--</div>
        <div class="detail" id="throttleDetail">Pi firmware flags</div>
      </article>
    </section>

    <section class="charts">
      <div class="chart-grid">
        <div class="chart-panel">
          <div class="label">CPU</div>
          <div class="chart-value" id="cpuChartValue">--</div>
          <canvas id="cpuChart" width="280" height="100"></canvas>
        </div>
        <div class="chart-panel">
          <div class="label">Memory</div>
          <div class="chart-value" id="memChartValue">--</div>
          <canvas id="memChart" width="280" height="100"></canvas>
        </div>
        <div class="chart-panel">
          <div class="label">Temperature</div>
          <div class="chart-value" id="tempChartValue">--</div>
          <canvas id="tempChart" width="280" height="100"></canvas>
        </div>
        <div class="chart-panel">
          <div class="label">Core Voltage</div>
          <div class="chart-value" id="voltChartValue">--</div>
          <canvas id="voltChart" width="280" height="100"></canvas>
        </div>
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
      el.textContent = ok ? "Connected" : "Disconnected";
      el.className = ok ? "pill live" : "pill offline";
    }

    function statusTone(value, warnAt, badAt) {
      if (value >= badAt) return "var(--bad)";
      if (value >= warnAt) return "var(--warn)";
      return "var(--good)";
    }

    function drawSpark(canvas, values, maxHint, color) {
      const ctx = canvas.getContext("2d");
      const { width, height } = canvas;
      ctx.clearRect(0, 0, width, height);

      ctx.strokeStyle = "rgba(255,255,255,0.08)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, height - 1);
      ctx.lineTo(width, height - 1);
      ctx.stroke();

      const filtered = values.filter((value) => Number.isFinite(value));
      if (filtered.length < 2) return;

      const max = Math.max(maxHint, ...filtered);
      const min = Math.min(...filtered);
      const span = Math.max(max - min, maxHint > 0 ? maxHint * 0.1 : 1);

      ctx.strokeStyle = color;
      ctx.lineWidth = 2.2;
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
      if (sample.under_voltage_now) warnings.push(["Under-voltage", "bad"]);
      if (sample.freq_capped_now) warnings.push(["Frequency capped", "warn"]);
      if (sample.throttled_now) warnings.push(["Throttled", "bad"]);
      if (sample.soft_temp_limit_now) warnings.push(["Soft temp limit", "warn"]);
      if (!warnings.length) warnings.push(["No active Pi firmware warnings", "good"]);
      warnings.forEach(([text, tone]) => {
        const pill = document.createElement("span");
        pill.className = `warn-pill ${tone}`;
        pill.textContent = text;
        row.appendChild(pill);
      });
    }

    function renderCurrent(sample) {
      const memPercent = pct(sample.memory_used_bytes, sample.memory_total_bytes);
      const diskPercent = pct(sample.disk_used_bytes, sample.disk_total_bytes);
      const cpuColor = statusTone(sample.cpu_usage_percent, 70, 90);
      const memColor = statusTone(memPercent, 70, 90);
      const tempColor = statusTone(sample.cpu_temp_c ?? 0, 60, 80);

      document.getElementById("cpuValue").textContent = `${sample.cpu_usage_percent.toFixed(1)}%`;
      document.getElementById("cpuValue").style.color = cpuColor;
      document.getElementById("cpuDetail").textContent = sample.cpu_per_core_percent.length
        ? sample.cpu_per_core_percent.map((value, index) => `c${index}: ${value.toFixed(0)}%`).join(" | ")
        : "Per-core counters pending";

      document.getElementById("memoryValue").textContent = `${memPercent.toFixed(1)}%`;
      document.getElementById("memoryValue").style.color = memColor;
      document.getElementById("memoryDetail").textContent =
        `${fmtBytes(sample.memory_used_bytes)} / ${fmtBytes(sample.memory_total_bytes)}`;

      document.getElementById("diskValue").textContent = `${diskPercent.toFixed(1)}%`;
      document.getElementById("diskDetail").textContent =
        `${fmtBytes(sample.disk_used_bytes)} / ${fmtBytes(sample.disk_total_bytes)}`;

      document.getElementById("tempValue").textContent = sample.cpu_temp_c == null ? "--" : `${sample.cpu_temp_c.toFixed(1)}°C`;
      document.getElementById("tempValue").style.color = tempColor;
      document.getElementById("tempDetail").textContent = sample.gpu_temp_c == null
        ? "GPU temp unavailable"
        : `GPU ${sample.gpu_temp_c.toFixed(1)}°C`;

      document.getElementById("voltValue").textContent = sample.core_volts == null ? "--" : `${sample.core_volts.toFixed(2)}V`;
      document.getElementById("voltDetail").textContent =
        `SDRAM C/I/P: ${sample.sdram_c_volts?.toFixed?.(2) ?? "--"} / ${sample.sdram_i_volts?.toFixed?.(2) ?? "--"} / ${sample.sdram_p_volts?.toFixed?.(2) ?? "--"} V`;

      document.getElementById("throttleValue").textContent = sample.throttled_now ? "Active" : "Clear";
      document.getElementById("throttleValue").style.color = sample.throttled_now || sample.under_voltage_now
        ? "var(--bad)"
        : "var(--good)";
      document.getElementById("throttleDetail").textContent =
        sample.throttled_raw == null ? "vcgencmd unavailable" : `Raw flags 0x${sample.throttled_raw.toString(16)}`;

      document.getElementById("uptimeLabel").textContent = `Uptime: ${fmtDuration(sample.uptime_seconds)}`;
      document.getElementById("loadLabel").textContent = `Load: ${sample.loadavg_1.toFixed(2)} / ${sample.loadavg_5.toFixed(2)}`;

      const armGHz = sample.arm_clock_hz == null ? "--" : (sample.arm_clock_hz / 1e9).toFixed(2);
      const gpuMHz = sample.gpu_clock_hz == null ? "--" : (sample.gpu_clock_hz / 1e6).toFixed(0);
      document.getElementById("clockLabel").textContent = `ARM: ${armGHz} GHz | GPU: ${gpuMHz} MHz`;
      document.getElementById("statusText").textContent = new Date(sample.timestamp_unix_ms).toLocaleTimeString();

      renderWarnings(sample);
    }

    function renderCharts(history) {
      const cpu = history.map((item) => item.cpu_usage_percent);
      const mem = history.map((item) => pct(item.memory_used_bytes, item.memory_total_bytes));
      const temp = history.map((item) => item.cpu_temp_c);
      const volt = history.map((item) => item.core_volts);

      drawSpark(charts.cpu, cpu, 100, "#63b3ff");
      drawSpark(charts.mem, mem, 100, "#59f0c2");
      drawSpark(charts.temp, temp, 90, "#ffb54c");
      drawSpark(charts.volt, volt, 2, "#ff8d73");

      const latest = history[history.length - 1];
      if (!latest) return;

      document.getElementById("cpuChartValue").textContent = `${latest.cpu_usage_percent.toFixed(1)}%`;
      document.getElementById("memChartValue").textContent = `${pct(latest.memory_used_bytes, latest.memory_total_bytes).toFixed(1)}%`;
      document.getElementById("tempChartValue").textContent = latest.cpu_temp_c == null ? "--" : `${latest.cpu_temp_c.toFixed(1)}°C`;
      document.getElementById("voltChartValue").textContent = latest.core_volts == null ? "--" : `${latest.core_volts.toFixed(2)}V`;
    }

    async function refresh() {
      try {
        const [currentResponse, historyResponse] = await Promise.all([
          fetch("/api/current", { cache: "no-store" }),
          fetch("/api/history?limit=120", { cache: "no-store" }),
        ]);
        if (!currentResponse.ok || !historyResponse.ok) throw new Error("request failed");

        const current = await currentResponse.json();
        const history = await historyResponse.json();
        setConnection(true);
        renderCurrent(current);
        renderCharts(history);
      } catch (_) {
        setConnection(false);
        document.getElementById("statusText").textContent = "Unable to reach API.";
      }
    }

    refresh();
    setInterval(refresh, 5000);
  </script>
</body>
</html>
"##;
