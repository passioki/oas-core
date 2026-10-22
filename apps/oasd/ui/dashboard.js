// OAS 调试面板（ENGINE §22.24）：点击分布 + 长跑统计独立页面。
// 编辑器侧只保留“编辑/采集/flow”，调试数据搬到这里，页面不再过长。
(function () {
  'use strict';

  const statusEl = document.getElementById('status');
  const canvas = document.getElementById('screen');
  const ctx = canvas.getContext('2d');
  const hintEl = document.getElementById('hint');
  const runsSelect = document.getElementById('runs-select');
  const btnRunsRefresh = document.getElementById('btn-runs-refresh');
  const btnClicksOverlay = document.getElementById('btn-clicks-overlay');
  const btnClicksClear = document.getElementById('btn-clicks-clear');
  const clickStats = document.getElementById('click-stats');
  const clicksStatus = document.getElementById('clicks-status');
  const btnStatsRefresh = document.getElementById('btn-stats-refresh');
  const statsSummary = document.getElementById('stats-summary');
  const logEl = document.getElementById('log');
  const btnCapture = document.getElementById('btn-capture');

  let img = null;
  let runs = [];
  let clickData = null; // {file, clicks, distinct, points}
  let clickOverlay = null;

  function log(msg) {
    const t = new Date().toLocaleTimeString();
    logEl.textContent = `[${t}] ${msg}\n` + logEl.textContent;
  }

  function setStatus(text) {
    statusEl.textContent = text;
  }

  function setClicksStatus(text) {
    clicksStatus.textContent = text;
  }

  // --- 点击分布辅助（与编辑器共用同一套颜色语义） ---
  const HEATMAP = {
    countDistinct(points) {
      const seen = new Set();
      for (const p of points || []) {
        const x = Number(p && p.x);
        const y = Number(p && p.y);
        if (!isFinite(x) || !isFinite(y)) continue;
        seen.add(Math.round(x) + ',' + Math.round(y));
      }
      return seen.size;
    },
    colorAt(t) {
      const v = Math.max(0, Math.min(1, Number(t) || 0));
      if (v < 0.2) return 'rgba(0,120,255,0.30)';
      if (v < 0.45) return 'rgba(0,220,190,0.40)';
      if (v < 0.7) return 'rgba(255,220,40,0.55)';
      return 'rgba(255,48,0,0.75)';
    },
    heatRadius(w, h) {
      return Math.max(12, Math.min(48, Math.round(Math.min(w, h) * 0.045)));
    },
  };

  function draw() {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (img) ctx.drawImage(img, 0, 0);
    if (!clickData || !clickData.points || !clickData.points.length) {
      hintEl.textContent = '暖色 = 落点密集 · 白点 = 单次点击';
      return;
    }
    if (
      !clickOverlay ||
      clickOverlay.width !== canvas.width ||
      clickOverlay.height !== canvas.height
    ) {
      buildClickOverlay();
    }
    if (clickOverlay) ctx.drawImage(clickOverlay, 0, 0);
    hintEl.textContent = `${clickData.file} · clicks=${clickData.clicks} distinct=${clickData.distinct}`;
  }

  function buildClickOverlay() {
    if (!clickData || !clickData.points.length) {
      clickOverlay = null;
      return;
    }
    const w = canvas.width || 1280;
    const h = canvas.height || 720;
    const overlay = document.createElement('canvas');
    overlay.width = w;
    overlay.height = h;
    const octx = overlay.getContext('2d');
    const heat = document.createElement('canvas');
    heat.width = w;
    heat.height = h;
    const hctx = heat.getContext('2d');
    const radius = HEATMAP.heatRadius(w, h);
    hctx.save();
    hctx.globalCompositeOperation = 'lighter';
    for (const p of clickData.points) {
      const x = Number(p && p.x);
      const y = Number(p && p.y);
      if (!isFinite(x) || !isFinite(y)) continue;
      const g = hctx.createRadialGradient(x, y, 0, x, y, radius);
      g.addColorStop(0, HEATMAP.colorAt(1));
      g.addColorStop(0.5, HEATMAP.colorAt(0.55));
      g.addColorStop(1, HEATMAP.colorAt(0.05));
      hctx.fillStyle = g;
      hctx.beginPath();
      hctx.arc(x, y, radius, 0, Math.PI * 2);
      hctx.fill();
    }
    hctx.restore();
    octx.save();
    try {
      octx.filter = 'blur(2px)';
    } catch (e) {
      /* ctx.filter unsupported - blobs are already soft */
    }
    octx.drawImage(heat, 0, 0);
    octx.restore();
    for (const p of clickData.points) {
      const x = Number(p && p.x);
      const y = Number(p && p.y);
      if (!isFinite(x) || !isFinite(y)) continue;
      octx.beginPath();
      octx.arc(x, y, 3.2, 0, Math.PI * 2);
      octx.fillStyle = 'rgba(0,0,0,0.85)';
      octx.fill();
      octx.beginPath();
      octx.arc(x, y, 1.7, 0, Math.PI * 2);
      octx.fillStyle = '#f2f7ff';
      octx.fill();
    }
    clickOverlay = overlay;
  }

  async function loadFrame() {
    const r = await fetch('/frame.png');
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    const blob = await r.blob();
    const url = URL.createObjectURL(blob);
    try {
      img = await new Promise((resolve, reject) => {
        const im = new Image();
        im.onload = () => resolve(im);
        im.onerror = () => reject(new Error('image decode failed'));
        im.src = url;
      });
      canvas.width = img.width;
      canvas.height = img.height;
      draw();
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  async function refreshState() {
    const r = await fetch('/state');
    const s = await r.json();
    if (!s.captured) {
      setStatus(s.error ? `未截到帧：${s.error}` : '没有帧');
      return;
    }
    const caps = `单行识别=${s.ocr !== false ? '可用' : '不可用'} · 多行识别=${s.det !== false ? '可用' : '不可用'}`;
    setStatus(`${s.device} · ${s.width}x${s.height} · ${caps}`);
  }

  function updateClickStatsText() {
    clickStats.textContent = clickData
      ? `clicks=${clickData.clicks} distinct=${clickData.distinct}`
      : 'clicks=— distinct=—';
  }

  function populateRunsSelect(wanted) {
    const keep = wanted || runsSelect.value || '';
    runsSelect.innerHTML = '';
    if (!runs.length) {
      const o = document.createElement('option');
      o.value = '';
      o.textContent = '（没有运行记录）';
      runsSelect.appendChild(o);
      runsSelect.disabled = true;
      return;
    }
    runsSelect.disabled = false;
    for (const r of runs) {
      const o = document.createElement('option');
      o.value = r.file;
      const extra = r.clicks != null ? ` · clicks=${r.clicks} distinct=${r.distinct}` : '';
      o.textContent = `${r.file}${extra}`;
      runsSelect.appendChild(o);
    }
    if (keep) {
      if (!runs.some((r) => r.file === keep)) {
        const o = document.createElement('option');
        o.value = keep;
        o.textContent = keep;
        runsSelect.appendChild(o);
      }
      runsSelect.value = keep;
    }
  }

  async function refreshRuns() {
    btnRunsRefresh.disabled = true;
    try {
      const r = await fetch('/runs');
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const s = await r.json();
      if (!s || !s.ok) throw new Error(((s && s.errors) || []).join('; ') || 'ok=false');
      runs = Array.isArray(s.runs) ? s.runs : [];
      populateRunsSelect();
      if (!runs.length) {
        setClicksStatus('没有运行记录（先跑一次 oasd run --clicks-out <file>）');
        log('点击分布：没有运行记录');
      } else {
        setClicksStatus(`已发现 ${runs.length} 条运行记录，选一条后点「叠加分布」`);
      }
    } catch (e) {
      runs = [];
      populateRunsSelect('');
      setClicksStatus(`读取运行记录失败：${e && e.message ? e.message : e}`);
      log(`点击分布：读取 /runs 失败：${e && e.message ? e.message : e}`);
    } finally {
      btnRunsRefresh.disabled = false;
    }
  }

  async function loadClicks(file) {
    if (!file) {
      setClicksStatus('请先选择一条运行记录');
      return;
    }
    btnClicksOverlay.disabled = true;
    setClicksStatus(`正在读取 ${file} …`);
    try {
      const r = await fetch('/runs/clicks?file=' + encodeURIComponent(file));
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const s = await r.json();
      if (!s || !s.ok) throw new Error(((s && s.errors) || []).join('; ') || 'ok=false');
      const points = Array.isArray(s.points) ? s.points : [];
      const clicks = Number.isFinite(Number(s.clicks)) ? Number(s.clicks) : points.length;
      const distinct = Number.isFinite(Number(s.distinct))
        ? Number(s.distinct)
        : HEATMAP.countDistinct(points);
      clickData = { file: s.file || file, clicks, distinct, points };
      clickOverlay = null;
      draw();
      updateClickStatsText();
      setClicksStatus(
        `已叠加 ${clickData.file} · clicks=${clicks} distinct=${distinct}（${points.length} 个点）`
      );
      log(`点击分布已叠加：${clickData.file} clicks=${clicks} distinct=${distinct}`);
    } catch (e) {
      clickData = null;
      clickOverlay = null;
      draw();
      updateClickStatsText();
      const msg = e && e.message ? e.message : String(e);
      setClicksStatus(`读取点击分布失败：${msg}`);
      log(`点击分布：读取 ${file} 失败：${msg}`);
    } finally {
      btnClicksOverlay.disabled = false;
    }
  }

  function clearClicks() {
    clickData = null;
    clickOverlay = null;
    updateClickStatsText();
    setClicksStatus('分布已清除（画面未变）');
    draw();
    log('点击分布已清除');
  }

  async function loadStats() {
    btnStatsRefresh.disabled = true;
    try {
      const r = await fetch('/stats');
      const s = await r.json();
      if (!s.ok) {
        statsSummary.textContent = '统计失败：' + (s.errors || []).join('; ');
        return;
      }
      const m = s.summary;
      const lines = [
        `点击 ${m.clicks.clicks}（miss ${m.clicks.misses} · distinct ${m.clicks.distinct} · 文件 ${m.clicks.files}）`,
        `队列 done/failed ${m.queue.done}/${m.queue.failed}（attempts ${m.queue.attempts}）`,
        `步骤 failed ${m.events.failed_steps} · retry ${m.events.retries} · 弹窗 ${m.events.popup_interrupts}`,
        `失败截图 ${m.shots.count}`,
      ];
      const locs = m.clicks.locators || {};
      for (const k of Object.keys(locs)) lines.push(`locator ${k}=${locs[k]}`);
      if (m.alerts && m.alerts.length) {
        lines.push('', '告警：');
        for (const a of m.alerts) lines.push('· ' + a);
      } else {
        lines.push('', '告警：无');
      }
      statsSummary.textContent = lines.join('\n');
    } catch (e) {
      statsSummary.textContent = '统计异常：' + e;
    } finally {
      btnStatsRefresh.disabled = false;
    }
  }

  async function doCapture() {
    btnCapture.disabled = true;
    setStatus('截图中…');
    try {
      const r = await fetch('/capture', { method: 'POST' });
      const s = await r.json();
      if (!s.ok) throw new Error(s.error || 'capture failed');
      await loadFrame();
      await refreshState();
      log(`已截图 ${s.width}x${s.height}`);
    } catch (e) {
      setStatus('截图异常');
      log('截图异常：' + e);
    } finally {
      btnCapture.disabled = false;
    }
  }

  btnRunsRefresh.addEventListener('click', () => refreshRuns());
  btnClicksOverlay.addEventListener('click', () => loadClicks(runsSelect.value));
  btnClicksClear.addEventListener('click', clearClicks);
  btnStatsRefresh.addEventListener('click', () => loadStats());
  btnCapture.addEventListener('click', doCapture);
  runsSelect.addEventListener('change', () =>
    setClicksStatus(`已选择 ${runsSelect.value || '（无）'}，点「叠加分布」查看`)
  );

  (async function boot() {
    try {
      await loadFrame();
    } catch (e) {
      setStatus('没有可用帧，点「截图」');
      log('帧加载失败：' + e);
    }
    try {
      await refreshState();
    } catch (e) {
      log('状态读取失败：' + e);
    }
    try {
      await refreshRuns();
    } catch (e) {
      log('运行记录读取失败：' + e);
    }
    await loadStats();
    updateClickStatsText();
    let deepRun = '';
    try {
      deepRun = new URLSearchParams(location.search).get('run') || '';
    } catch (e) {
      deepRun = '';
    }
    if (deepRun) {
      populateRunsSelect(deepRun);
      runsSelect.value = deepRun;
      setClicksStatus(`深链 ?run=${deepRun}，正在加载…`);
      await loadClicks(deepRun);
    }
  })();
})();
