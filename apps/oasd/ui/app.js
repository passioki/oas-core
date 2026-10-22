// OAS engine editor - MVP front end.
// Plain JS on purpose: no build step, so the same files can be served by the
// Rust daemon today and wrapped by Tauri / an Android WebView later.

const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');
const statusEl = document.getElementById('status');
const roiEl = document.getElementById('roi');
const resultEl = document.getElementById('result');
const logEl = document.getElementById('log');
const btnCapture = document.getElementById('btn-capture');
const btnOcr = document.getElementById('btn-ocr');
const btnClear = document.getElementById('btn-clear');
const btnLines = document.getElementById('btn-lines');
const btnMatch = document.getElementById('btn-match');
const btnAsset = document.getElementById('btn-asset');
const assetId = document.getElementById('asset-id');
const assetKind = document.getElementById('asset-kind');
const taskName = document.getElementById('task-name');
const btnSaveAsset = document.getElementById('btn-save-asset');
const flowJson = document.getElementById('flow-json');
const btnSaveFlow = document.getElementById('btn-save-flow');
const assetsSummary = document.getElementById('assets-summary');
const btnClickStep = document.getElementById('btn-click-step');
const btnAppendStep = document.getElementById('btn-append-step');
const btnPoly = document.getElementById('btn-poly');
const runsSelect = document.getElementById('runs-select');
const btnRunsRefresh = document.getElementById('btn-runs-refresh');
const btnClicksOverlay = document.getElementById('btn-clicks-overlay');
const btnClicksClear = document.getElementById('btn-clicks-clear');
const clickStats = document.getElementById('click-stats');
const clicksStatus = document.getElementById('clicks-status');
// §22.18 visual flow editor
const flowOp = document.getElementById('flow-op');
const flowSteps = document.getElementById('flow-steps');
const flowInterrupts = document.getElementById('flow-interrupts');
const btnFlowAddStep = document.getElementById('flow-add-step');
const btnFlowFromJson = document.getElementById('flow-from-json');
const btnFlowToJson = document.getElementById('flow-to-json');
const btnFlowSaveViz = document.getElementById('flow-save-viz');
const btnFlowAddInterrupt = document.getElementById('flow-add-interrupt');
// §22.19 long-run stats
const btnStatsRefresh = document.getElementById('btn-stats-refresh');
const statsSummary = document.getElementById('stats-summary');

let img = null;
let roi = null; // {x,y,w,h} in frame coordinates
let dragStart = null;
let boxes = []; // detection boxes from the multi-line route
let selectedBox = null; // the detection block last clicked (N2, §19.2)
// N4 click-distribution state (§21): the run list, the loaded points + metrics,
// and a cached offscreen bitmap so draw() stays cheap.
let runs = [];
let clickData = null; // {file, clicks, distinct, points}
let clickOverlay = null;
// §22.18 visual flow editor model: {name, steps:[{op,argsText}], interrupts:[{detect,action}]}
let flowModel = { name: '', steps: [], interrupts: [] };
// §22.10 polygon selection: vertices in FRAME coordinates + closed flag.
// When closed, roi becomes the polygon bbox and assets are saved with `polygon`;
// the runtime clicks inside the polygon's largest inscribed rectangle.
let polygon = null;
let polyClosed = false;
let polyMode = false;

// --- N4: pure click-distribution helpers (§21 CD1~CD5) -----------------------
// Kept as plain top-level code on purpose: no build step, no CDN, no module
// loader. Exposed on window.OAS_HEATMAP so they can be checked from the browser
// console / a headless run without a node toolchain.
window.OAS_HEATMAP = {
  // CD4: distinct = how many different integer pixels were hit. A small count
  // means every run re-taps the same spot - the anti-ban signal is broken.
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
  // Frame-absolute point -> canvas pixel. The canvas is created at the native
  // frame size so this is normally 1:1; the ratios keep it correct if a frame
  // is ever drawn scaled.
  mapPoint(p, canvasW, canvasH, frameW, frameH) {
    const fw = frameW || canvasW || 1;
    const fh = frameH || canvasH || 1;
    return { x: Number(p.x) * (canvasW / fw), y: Number(p.y) * (canvasH / fh) };
  },
  // CD3 heat colour ramp: sparse = cool blue, dense = bright red.
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

function log(msg) {
  const t = new Date().toLocaleTimeString();
  logEl.textContent = `[${t}] ${msg}\n` + logEl.textContent;
}

function setStatus(text) { statusEl.textContent = text; }

// Map a mouse event to frame coordinates (canvas is scaled by CSS width).
function toFrame(ev) {
  const r = canvas.getBoundingClientRect();
  const sx = canvas.width / r.width;
  const sy = canvas.height / r.height;
  return {
    x: Math.round((ev.clientX - r.left) * sx),
    y: Math.round((ev.clientY - r.top) * sy),
  };
}

function normalizeRect(a, b) {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  const w = Math.abs(a.x - b.x);
  const h = Math.abs(a.y - b.y);
  return { x, y, w, h };
}

function draw() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (img) ctx.drawImage(img, 0, 0);
  if (roi && roi.w > 0 && roi.h > 0) {
    ctx.save();
    ctx.strokeStyle = '#31d0aa';
    ctx.lineWidth = 2;
    ctx.strokeRect(roi.x + 0.5, roi.y + 0.5, roi.w, roi.h);
    ctx.fillStyle = 'rgba(49,208,170,0.12)';
    ctx.fillRect(roi.x, roi.y, roi.w, roi.h);
    ctx.fillStyle = '#31d0aa';
    ctx.font = '14px Consolas, monospace';
    ctx.fillText(`${roi.w}x${roi.h} @ ${roi.x},${roi.y}`, roi.x + 4, Math.max(14, roi.y - 6));
    ctx.restore();
  }
  // §22.10 polygon overlay (orange; closed => translucent fill + bbox roi)
  if (polygon && polygon.length) {
    ctx.save();
    ctx.beginPath();
    ctx.moveTo(polygon[0].x, polygon[0].y);
    for (let i = 1; i < polygon.length; i++) ctx.lineTo(polygon[i].x, polygon[i].y);
    if (polyClosed) ctx.closePath();
    if (polyClosed) {
      ctx.fillStyle = 'rgba(255,140,0,0.12)';
      ctx.fill();
    }
    ctx.strokeStyle = '#ff8c00';
    ctx.lineWidth = 2;
    ctx.stroke();
    for (const p of polygon) {
      ctx.beginPath();
      ctx.arc(p.x, p.y, 4, 0, Math.PI * 2);
      ctx.fillStyle = '#ff8c00';
      ctx.fill();
    }
    ctx.restore();
  }
  // detection boxes (multi-line route)
  // High-contrast strategy: every outline is drawn twice - a dark halo first,
  // then the bright line on top - so it stays readable on both light and dark
  // game art. Labels get a dark plate for the same reason.
  for (const b of boxes) {
    const [x, y, w, h] = b.box;
    ctx.save();
    ctx.strokeStyle = 'rgba(0,0,0,0.85)';
    ctx.lineWidth = 5;
    ctx.strokeRect(x + 0.5, y + 0.5, w, h);
    ctx.strokeStyle = '#ffd54a';
    ctx.lineWidth = 2;
    ctx.strokeRect(x + 0.5, y + 0.5, w, h);
    // tiny corner ticks make the exact edges readable on busy art
    ctx.strokeStyle = 'rgba(0,0,0,0.85)';
    ctx.lineWidth = 4;
    const tick = 6;
    ctx.beginPath();
    for (const [cx, cy, dx, dy] of [[x, y, 1, 1], [x + w, y, -1, 1], [x, y + h, 1, -1], [x + w, y + h, -1, -1]]) {
      ctx.moveTo(cx, cy);
      ctx.lineTo(cx + dx * tick, cy);
      ctx.moveTo(cx, cy);
      ctx.lineTo(cx, cy + dy * tick);
    }
    ctx.stroke();
    ctx.fillStyle = 'rgba(232,180,49,0.08)';
    ctx.fillRect(x, y, w, h);
    // label plate
    const label = `${b.text} ${b.score.toFixed(2)}`;
    ctx.font = '13px Consolas, monospace';
    const tw = ctx.measureText(label).width;
    const ly = y >= 16 ? y - 15 : y + 1;
    ctx.fillStyle = 'rgba(0,0,0,0.72)';
    ctx.fillRect(x, ly, tw + 8, 15);
    ctx.fillStyle = '#ffd54a';
    ctx.fillText(label, x + 4, ly + 12);
    ctx.restore();
  }
  // Draw order (§21): frame -> ROI/boxes -> click heatmap + scatter, then the
  // on-canvas metrics label and legend.
  drawClickOverlay();
}

function polygonBBox(poly) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const p of poly) {
    x0 = Math.min(x0, p.x); y0 = Math.min(y0, p.y);
    x1 = Math.max(x1, p.x); y1 = Math.max(y1, p.y);
  }
  return {
    x: Math.round(x0),
    y: Math.round(y0),
    w: Math.round(x1 - x0 + 1),
    h: Math.round(y1 - y0 + 1),
  };
}

function updateRoiLabel() {
  if (!roi) {
    roiEl.textContent = polygon ? `多边形：${polygon.length} 个顶点（未闭合）` : '未选择';
    btnOcr.disabled = true;
    btnLines.disabled = true;
    if (btnMatch) btnMatch.disabled = true;
    if (btnAsset) btnAsset.disabled = true;
    if (btnSaveAsset) btnSaveAsset.disabled = true;
    return;
  }
  const polyInfo =
    polygon && polyClosed ? ` · 多边形 ${polygon.length} 顶点（点击区=匹配外接矩形）` : '';
  roiEl.textContent = `x=${roi.x} y=${roi.y} w=${roi.w} h=${roi.h}${polyInfo}`;
  btnOcr.disabled = roi.w < 2 || roi.h < 2;
  btnLines.disabled = roi.w < 2 || roi.h < 2;
  if (btnMatch) btnMatch.disabled = roi.w < 2 || roi.h < 2;
  if (btnAsset) btnAsset.disabled = roi.w < 2 || roi.h < 2;
  if (btnSaveAsset) btnSaveAsset.disabled = roi.w < 2 || roi.h < 2;
}

// --- N2: 生成点击指令 (§19.2) ------------------------------------------------
// A selected detection block already carries its box in FRAME coordinates: the
// /lines route returns ROI-local boxes and the UI shifts them by the ROI origin.
// The generated step is therefore frame-absolute by construction.
function boxCenter(b) {
  const [x, y, w, h] = b.box;
  return { x: Math.round(x + w / 2), y: Math.round(y + h / 2) };
}

function clickStepForBox(b) {
  const c = boxCenter(b);
  return { op: 'click_point', args: { x: c.x, y: c.y } };
}

function updateClickStepButtons() {
  const on = !!selectedBox;
  if (btnClickStep) btnClickStep.disabled = !on;
  if (btnAppendStep) btnAppendStep.disabled = !on;
}

async function doGenerateClickStep() {
  if (!selectedBox) return;
  const c = boxCenter(selectedBox);
  const step = clickStepForBox(selectedBox);
  const json = JSON.stringify(step); // exact §19.2 style: compact one-liner
  resultEl.textContent =
    `${json}\n\n` +
    `来源区块：「${selectedBox.text}」score=${selectedBox.score.toFixed(4)}\n` +
    `框 = [${selectedBox.box.join(', ')}] · 框中心（帧绝对）= x=${c.x}, y=${c.y}\n` +
    '（已含 ROI 偏移；可直接粘进 flow.json 的 steps）';
  log(`生成点击指令 (${c.x},${c.y}) ← 区块「${selectedBox.text}」`);
  try {
    await navigator.clipboard.writeText(json);
    log('点击指令已复制到剪贴板');
  } catch (e) {
    log('点击指令已生成（复制失败，请手动选中）');
  }
}

// Append the generated step to the flow textarea. A parse failure (or a
// non-array `steps`) must leave the textarea untouched - never corrupt user
// input.
function doAppendStepToFlow() {
  if (!selectedBox) return;
  const step = clickStepForBox(selectedBox);
  const raw = (flowJson.value || '').trim();
  if (!raw) {
    resultEl.textContent = '追加失败：flow 为空，请先粘贴/保存一个 flow.json（textarea 未改动）';
    return;
  }
  let flow;
  try {
    flow = JSON.parse(raw);
  } catch (e) {
    resultEl.textContent = `追加失败：现有 flow 不是合法 JSON（${e.message}）。textarea 未改动。`;
    return;
  }
  if (!Array.isArray(flow.steps)) {
    resultEl.textContent = '追加失败：flow.steps 不是数组。textarea 未改动。';
    return;
  }
  flow.steps.push(step);
  flowJson.value = JSON.stringify(flow, null, 2);
  resultEl.textContent =
    `已追加到 flow.steps（共 ${flow.steps.length} 步）\n` + JSON.stringify(step);
  log(`已追加点击指令 (${step.args.x},${step.args.y}) 到 flow（${flow.steps.length} 步）`);
}

// --- N4: 点击分布视图 (§21 CD1~CD5) ------------------------------------------
// Backend contract (impl-n4-core):
//   GET /runs              -> {ok, runs:[{file,clicks,distinct,mtime_ms}]}
//   GET /runs/clicks?file= -> {ok, file, clicks, distinct, points:[{ts_ms,x,y,
//                              target_x,target_y,sel,seed}]}
// Every route call is defensive: a missing route (404) or bad JSON shows a
// message in the panel/log and never breaks boot.

function setClicksStatus(text) {
  if (clicksStatus) clicksStatus.textContent = text;
}

function updateClickStatsText() {
  if (!clickStats) return;
  clickStats.textContent = clickData
    ? `clicks=${clickData.clicks} distinct=${clickData.distinct}`
    : 'clicks=— distinct=—';
}

function populateRunsSelect(wanted) {
  if (!runsSelect) return;
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
    // A deep link may name a file the list has not picked up yet (mtime race):
    // still try to load it instead of silently doing nothing.
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
  if (!btnRunsRefresh) return;
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

// Build the cached overlay bitmap: additive radial gradients (heat), a soft
// blur when the browser supports ctx.filter, then small high-contrast scatter
// dots. Rebuilt only when a new run is loaded or the canvas size changes.
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
  const radius = window.OAS_HEATMAP.heatRadius(w, h);
  hctx.save();
  hctx.globalCompositeOperation = 'lighter';
  for (const p of clickData.points) {
    const x = Number(p && p.x);
    const y = Number(p && p.y);
    if (!isFinite(x) || !isFinite(y)) continue;
    const q = window.OAS_HEATMAP.mapPoint({ x, y }, w, h);
    if (q.x < -radius || q.y < -radius || q.x > w + radius || q.y > h + radius) continue;
    const g = hctx.createRadialGradient(q.x, q.y, 0, q.x, q.y, radius);
    g.addColorStop(0, window.OAS_HEATMAP.colorAt(1));
    g.addColorStop(0.5, window.OAS_HEATMAP.colorAt(0.55));
    g.addColorStop(1, window.OAS_HEATMAP.colorAt(0.05));
    hctx.fillStyle = g;
    hctx.beginPath();
    hctx.arc(q.x, q.y, radius, 0, Math.PI * 2);
    hctx.fill();
  }
  hctx.restore();
  octx.save();
  try {
    octx.filter = 'blur(2px)';
  } catch (e) {
    /* ctx.filter unsupported - still fine, the blobs are already soft */
  }
  octx.drawImage(heat, 0, 0);
  octx.restore();
  // scatter dots: dark halo + near-white core, readable on any game art
  for (const p of clickData.points) {
    const x = Number(p && p.x);
    const y = Number(p && p.y);
    if (!isFinite(x) || !isFinite(y)) continue;
    const q = window.OAS_HEATMAP.mapPoint({ x, y }, w, h);
    octx.beginPath();
    octx.arc(q.x, q.y, 3.2, 0, Math.PI * 2);
    octx.fillStyle = 'rgba(0,0,0,0.85)';
    octx.fill();
    octx.beginPath();
    octx.arc(q.x, q.y, 1.7, 0, Math.PI * 2);
    octx.fillStyle = '#f2f7ff';
    octx.fill();
  }
  clickOverlay = overlay;
}

// On-canvas `clicks=N distinct=K` label + tiny legend for screenshot evidence.
function drawClickStats() {
  if (!clickData) return;
  const label = `clicks=${clickData.clicks} distinct=${clickData.distinct}`;
  ctx.save();
  ctx.font = 'bold 15px Consolas, monospace';
  const pad = 10;
  const plateW = ctx.measureText(label).width + pad * 2;
  const plateH = 26;
  const x0 = 10;
  const y0 = 10;
  ctx.fillStyle = 'rgba(0,0,0,0.75)';
  ctx.fillRect(x0, y0, plateW, plateH);
  ctx.strokeStyle = 'rgba(255,255,255,0.28)';
  ctx.lineWidth = 1;
  ctx.strokeRect(x0 + 0.5, y0 + 0.5, plateW - 1, plateH - 1);
  ctx.fillStyle = '#ffd54a';
  ctx.fillText(label, x0 + pad, y0 + 19);
  // legend: cool -> hot ramp + the single-click dot marker
  const lw = 110;
  const lh = 8;
  const lx = x0 + 8;
  const ly = y0 + plateH + 12;
  ctx.font = '12px Consolas, monospace';
  const caption = '稀疏 → 密集    ● 单次点击';
  const capW = ctx.measureText(caption).width;
  ctx.fillStyle = 'rgba(0,0,0,0.65)';
  ctx.fillRect(x0, ly - 8, Math.max(lw + 48, capW + 16), lh + 32);
  const grad = ctx.createLinearGradient(lx, 0, lx + lw, 0);
  grad.addColorStop(0, 'rgba(0,120,255,0.55)');
  grad.addColorStop(0.5, 'rgba(255,220,40,0.7)');
  grad.addColorStop(1, 'rgba(255,48,0,0.9)');
  ctx.fillStyle = grad;
  ctx.fillRect(lx, ly, lw, lh);
  ctx.beginPath();
  ctx.arc(lx + lw + 20, ly + lh / 2, 3.2, 0, Math.PI * 2);
  ctx.fillStyle = 'rgba(0,0,0,0.85)';
  ctx.fill();
  ctx.beginPath();
  ctx.arc(lx + lw + 20, ly + lh / 2, 1.7, 0, Math.PI * 2);
  ctx.fillStyle = '#f2f7ff';
  ctx.fill();
  ctx.fillStyle = '#cbd5e1';
  ctx.fillText(caption, lx, ly + lh + 14);
  ctx.restore();
}

function drawClickOverlay() {
  if (!clickData) return;
  if (clickData.points && clickData.points.length) {
    if (
      !clickOverlay ||
      clickOverlay.width !== canvas.width ||
      clickOverlay.height !== canvas.height
    ) {
      buildClickOverlay();
    }
    if (clickOverlay) ctx.drawImage(clickOverlay, 0, 0);
  }
  drawClickStats();
}

async function loadClicks(file) {
  if (!file) {
    setClicksStatus('请先选择一条运行记录');
    return;
  }
  if (btnClicksOverlay) btnClicksOverlay.disabled = true;
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
      : window.OAS_HEATMAP.countDistinct(points);
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
    resultEl.textContent = `点击分布读取失败：${msg}`;
    log(`点击分布：读取 ${file} 失败：${msg}`);
  } finally {
    if (btnClicksOverlay) btnClicksOverlay.disabled = false;
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

// --- N3: task bundle save helpers -----------------------------------------

function currentTask() {
  const t = (taskName.value || '').trim();
  if (!t) {
    resultEl.textContent = '请先填任务名';
    taskName.focus();
    return null;
  }
  return t;
}

function isValidAssetId(id) {
  return /^[\u4e00-\u9fa5A-Za-z_][\u4e00-\u9fa5A-Za-z0-9_]*$/.test(id);
}

function showErrors(prefix, errors) {
  const list = (errors || []).map((e) => '· ' + e).join('\n');
  resultEl.textContent = `${prefix}\n${list || '（无错误信息）'}`;
}

async function postJson(url, payload) {
  const r = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  return r.json();
}

// Show what is actually on disk for this task (/task/assets summary).
async function refreshTaskAssets(task) {
  if (!task) task = (taskName.value || '').trim();
  if (!task) return;
  try {
    const r = await fetch('/task/assets?task=' + encodeURIComponent(task));
    const s = await r.json();
    if (!s.ok) {
      assetsSummary.textContent = '读取失败：\n' + (s.errors || []).join('\n');
      return;
    }
    const res = s.design_resolution ? s.design_resolution.join('x') : '未设置';
    const lines = [`任务 ${s.task} · design ${res} · ${s.count} 个素材`];
    for (const it of s.items || []) {
      const roi = it.roi ? `[${it.roi.join(', ')}]` : '—';
      const thr = it.threshold != null ? ` thr=${Number(it.threshold.toFixed(3))}` : '';
      lines.push(`· ${it.id}  [${it.kind}]  ${it.file || '（无图）'}  roi=${roi}${thr}`);
    }
    assetsSummary.textContent = lines.join('\n');
  } catch (e) {
    assetsSummary.textContent = '读取异常：' + e;
  }
}

// Persist the current ROI as an asset in the task bundle. The server crops the
// LAST captured frame and assigns images/aNNN.png; the (Chinese) id never
// becomes a path.
async function doSaveAsset() {
  if (!roi) return;
  const task = currentTask();
  if (!task) return;
  const id = (assetId.value || '').trim();
  if (!id) {
    resultEl.textContent = '请先填素材 id';
    assetId.focus();
    return;
  }
  if (!isValidAssetId(id)) {
    resultEl.textContent = 'id 可用中文/字母/数字/下划线，但不能以数字开头、不能含空格或标点';
    return;
  }
  const kind = assetKind.value;
  const payload = { task, id, kind, roi: [roi.x, roi.y, roi.w, roi.h] };
  if (kind === 'template') payload.threshold = 0.85;
  // §22.10: a closed polygon restricts the runtime click area to its largest
  // inscribed rectangle; roi is already the polygon bbox.
  if (polygon && polyClosed && polygon.length >= 3) {
    payload.polygon = polygon.map((p) => [p.x, p.y]);
  }
  btnSaveAsset.disabled = true;
  try {
    let s = await postJson('/task/save_asset', payload);
    // §4 rev: an existing id is a *re-crop*, not an error — ask once, then
    // overwrite with rev+1. Cancelling writes nothing (old asset stays).
    if (!s.ok && (s.errors || []).some((e) => e.includes('already exists'))) {
      const ok = window.confirm(
        `素材 id「${id}」已存在。\n\n覆盖它并升版（rev+1）吗？\n` +
          '· 确定 = 用当前截图的 ROI 覆盖\n' +
          '· 取消 = 不写入任何文件（想保留旧素材请换个 id）'
      );
      if (!ok) {
        resultEl.textContent = '已取消：未写入任何文件（旧素材保留）';
        log(`保存素材取消：${id} 已存在`);
        return;
      }
      s = await postJson('/task/save_asset', { ...payload, replace: true });
    }
    if (!s.ok) {
      showErrors('保存失败（未写入任何文件）：', s.errors);
      log(`保存素材失败：${id}`);
      return;
    }
    resultEl.textContent = JSON.stringify(s.item, null, 2);
    log(`已保存素材：${s.item.id} rev=${s.item.rev} → ${s.item.file || '（无图）'}`);
    await refreshTaskAssets(task);
  } catch (e) {
    resultEl.textContent = '保存异常：' + e;
  } finally {
    updateRoiLabel();
  }
}

async function doSaveFlow() {
  const task = currentTask();
  if (!task) return;
  const text = (flowJson.value || '').trim();
  if (!text) {
    resultEl.textContent = '请先粘贴 flow.json';
    return;
  }
  let flow;
  try {
    flow = JSON.parse(text);
  } catch (e) {
    resultEl.textContent = 'flow.json 不是合法 JSON：' + e.message;
    return;
  }
  btnSaveFlow.disabled = true;
  try {
    const s = await postJson('/task/save_flow', { task, flow });
    if (!s.ok) {
      showErrors('保存 flow 失败（未写入任何文件）：', s.errors);
      log('保存 flow 失败');
      return;
    }
    resultEl.textContent = `flow.json 已保存到任务「${s.task}」（引用校验通过）`;
    log(`flow.json 已保存：${s.task}`);
    await refreshTaskAssets(task);
  } catch (e) {
    resultEl.textContent = '保存 flow 异常：' + e;
  } finally {
    btnSaveFlow.disabled = false;
  }
}

// --- §22.18 流程编辑器（可视化） --------------------------------------------
function flowEnsureOps() {
  if (!flowOp || flowOp.options.length) return;
  const ops = (window.OAS_FLOW && window.OAS_FLOW.OPS) || [];
  for (const op of ops) {
    const o = document.createElement('option');
    o.value = op;
    o.textContent = op;
    flowOp.appendChild(o);
  }
}

function flowRender() {
  if (!flowSteps) return;
  flowEnsureOps();
  flowSteps.textContent = '';
  if (!flowModel.steps.length) flowSteps.textContent = '（无步骤）';
  flowModel.steps.forEach((step, i) => {
    const row = document.createElement('div');
    row.className = 'flow-step';
    const op = document.createElement('span');
    op.className = 'flow-op';
    op.textContent = step.op;
    const args = document.createElement('input');
    args.className = 'flow-args';
    args.spellcheck = false;
    args.value = step.argsText;
    args.title = step.argsText;
    args.addEventListener('input', () => { step.argsText = args.value; });
    const up = document.createElement('button');
    up.textContent = '↑';
    up.disabled = i === 0;
    up.addEventListener('click', () => {
      if (i === 0) return;
      [flowModel.steps[i - 1], flowModel.steps[i]] = [flowModel.steps[i], flowModel.steps[i - 1]];
      flowRender();
    });
    const down = document.createElement('button');
    down.textContent = '↓';
    down.disabled = i === flowModel.steps.length - 1;
    down.addEventListener('click', () => {
      if (i >= flowModel.steps.length - 1) return;
      [flowModel.steps[i + 1], flowModel.steps[i]] = [flowModel.steps[i], flowModel.steps[i + 1]];
      flowRender();
    });
    const del = document.createElement('button');
    del.textContent = '✕';
    del.title = '删除该步骤';
    del.addEventListener('click', () => {
      flowModel.steps.splice(i, 1);
      flowRender();
    });
    row.append(op, args, up, down, del);
    flowSteps.appendChild(row);
  });

  flowInterrupts.textContent = '';
  if (!flowModel.interrupts.length) flowInterrupts.textContent = '（无）';
  flowModel.interrupts.forEach((rule, i) => {
    const row = document.createElement('div');
    row.className = 'flow-step';
    const detect = document.createElement('input');
    detect.className = 'flow-args';
    detect.placeholder = 'detect（弹窗模板 id）';
    detect.value = rule.detect || '';
    detect.addEventListener('input', () => { rule.detect = detect.value; });
    const action = document.createElement('input');
    action.className = 'flow-args';
    action.placeholder = 'action（要点的按钮 id，空=点 detect）';
    action.value = rule.action || '';
    action.addEventListener('input', () => { rule.action = action.value; });
    const del = document.createElement('button');
    del.textContent = '✕';
    del.addEventListener('click', () => {
      flowModel.interrupts.splice(i, 1);
      flowRender();
    });
    row.append(detect, action, del);
    flowInterrupts.appendChild(row);
  });
}

function flowLoadFromJson() {
  if (!window.OAS_FLOW) {
    resultEl.textContent = 'flow_model.js 未加载（换页/清缓存后重试）';
    return;
  }
  try {
    const text = (flowJson.value || '').trim();
    if (!text) {
      resultEl.textContent = 'flow.json 为空：先载入任务流程或粘贴 JSON';
      return;
    }
    flowModel = window.OAS_FLOW.parseFlow(text);
    flowRender();
    resultEl.textContent =
      `流程编辑器已载入：${flowModel.steps.length} 个步骤 / ${flowModel.interrupts.length} 条弹窗规则`;
    log(`流程编辑器：载入 ${flowModel.steps.length} 步`);
  } catch (e) {
    resultEl.textContent = '可视化载入失败：' + (e && e.message ? e.message : e);
    log('流程可视化载入失败：' + (e && e.message ? e.message : e));
  }
}

function flowBuildToJson() {
  if (!window.OAS_FLOW) {
    resultEl.textContent = 'flow_model.js 未加载';
    return null;
  }
  try {
    const text = window.OAS_FLOW.buildFlow(flowModel);
    flowJson.value = text;
    resultEl.textContent = '已生成 flow.json（保存前仍会做服务端校验）';
    return text;
  } catch (e) {
    resultEl.textContent = '生成 flow.json 失败：' + (e && e.message ? e.message : e);
    return null;
  }
}

async function flowSaveVisual() {
  const errs = window.OAS_FLOW ? window.OAS_FLOW.validateModel(flowModel) : [];
  if (errs.length) {
    resultEl.textContent = '流程检查未通过：\n· ' + errs.join('\n· ');
    return;
  }
  if (!flowBuildToJson()) return;
  await doSaveFlow();
}

async function loadTaskFlow(task) {
  if (!task) return;
  try {
    const r = await fetch('/task/flow?task=' + encodeURIComponent(task));
    const s = await r.json();
    if (!s.ok) {
      log('读取任务流程失败：' + (s.errors || []).join('; '));
      return;
    }
    if (!s.flow) {
      log(`任务「${task}」还没有 flow.json`);
      return;
    }
    flowJson.value = JSON.stringify(s.flow, null, 2);
    flowModel = window.OAS_FLOW ? window.OAS_FLOW.parseFlow(flowJson.value) : flowModel;
    flowModel.name = s.flow.name || task;
    flowRender();
    log(`已载入任务「${task}」的 flow（${flowModel.steps.length} 步）`);
  } catch (e) {
    log('读取任务流程异常：' + e);
  }
}

async function loadStats() {
  if (!statsSummary) return;
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
  }
}

async function loadFrame() {
  return new Promise((resolve, reject) => {
    const im = new Image();
    im.onload = () => {
      img = im;
      canvas.width = im.width;
      canvas.height = im.height;
      draw();
      resolve(im);
    };
    im.onerror = () => reject(new Error('帧加载失败'));
    im.src = '/frame.png?t=' + Date.now();
  });
}

async function refreshState() {
  const r = await fetch('/state');
  const s = await r.json();
  if (!s.captured) {
    setStatus(s.error ? `未截到帧：${s.error}` : '没有帧');
    return;
  }
  setStatus(`${s.device} · ${s.width}x${s.height}`);
}

async function doCapture() {
  btnCapture.disabled = true;
  setStatus('截图中…');
  try {
    const r = await fetch('/capture', { method: 'POST' });
    const s = await r.json();
    if (!s.ok) {
      setStatus(`截图失败：${s.error}`);
      log(`截图失败：${s.error}`);
      return;
    }
    // A new capture invalidates detection boxes (their frame coordinates came
    // from the old frame); the click distribution overlay is kept but its
    // cached bitmap is dropped so draw() rebuilds it over the fresh frame.
    boxes = [];
    polygon = null;
    polyClosed = false;
    selectedBox = null;
    clickOverlay = null;
    updateClickStepButtons();
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

canvas.addEventListener('mousedown', (ev) => {
  // P1-d2 step 3: every detected block is individually operable. Clicking inside
  // a box selects that block (ROI := box) so 生成素材 JSON applies to it, and
  // shows only that block's text.
  const p0 = toFrame(ev);
  // §22.10 polygon mode: each click appends a vertex; double-click closes.
  if (polyMode) {
    if (ev.detail >= 2) {
      ev.preventDefault();
      return; // the closing dblclick handles this; avoid duplicate vertices
    }
    if (!polygon || polyClosed) {
      polygon = [];
      polyClosed = false;
      roi = null;
    }
    polygon.push(p0);
    if (polygon.length >= 3) roi = polygonBBox(polygon);
    updateRoiLabel();
    draw();
    return;
  }
  if (boxes.length) {
    for (let i = boxes.length - 1; i >= 0; i--) {
      const b = boxes[i];
      const [bx, by, bw, bh] = b.box;
      if (p0.x >= bx && p0.x <= bx + bw && p0.y >= by && p0.y <= by + bh) {
        roi = { x: bx, y: by, w: bw, h: bh };
        polygon = null;
        polyClosed = false;
        selectedBox = b;
        dragStart = null;
        updateRoiLabel();
        updateClickStepButtons();
        draw();
        resultEl.textContent =
          `选中区块：${b.text}\nscore = ${b.score.toFixed(4)}\nroi = [${b.box.join(', ')}]\n` +
          '（点「生成素材 JSON」为这一块建素材；点「生成点击指令」把它变成 click_point 步骤）';
        log(`选中区块「${b.text}」(${b.score.toFixed(3)})`);
        return;
      }
    }
  }
  dragStart = p0;
  roi = null;
  boxes = [];
  polygon = null;
  polyClosed = false;
  selectedBox = null;
  updateClickStepButtons();
  draw();
});

canvas.addEventListener('mousemove', (ev) => {
  if (polyMode || !dragStart) return;
  roi = normalizeRect(dragStart, toFrame(ev));
  updateRoiLabel();
  draw();
});

canvas.addEventListener('mouseup', () => {
  if (polyMode || !dragStart) return;
  dragStart = null;
  updateRoiLabel();
  draw();
  if (roi) log(`选中 ROI ${roi.w}x${roi.h} @ ${roi.x},${roi.y}`);
});

// §22.10: double-click closes the polygon. The double-click's first mousedown may
// have appended a duplicate of the last vertex — drop consecutive duplicates.
canvas.addEventListener('dblclick', (ev) => {
  if (!polyMode || !polygon || polygon.length < 3) return;
  ev.preventDefault();
  while (polygon.length > 3) {
    const a = polygon[polygon.length - 1];
    const b = polygon[polygon.length - 2];
    if (Math.abs(a.x - b.x) <= 2 && Math.abs(a.y - b.y) <= 2) polygon.pop();
    else break;
  }
  polyClosed = true;
  roi = polygonBBox(polygon);
  updateRoiLabel();
  draw();
  log(
    `多边形已闭合：${polygon.length} 顶点，bbox ${roi.w}x${roi.h} @ ${roi.x},${roi.y}` +
      '（运行时点击区 = 最大内接矩形）'
  );
});

btnPoly.addEventListener('click', () => {
  polyMode = !polyMode;
  if (polyMode) {
    polygon = null;
    polyClosed = false;
    roi = null;
    dragStart = null;
    btnPoly.textContent = '退出多边形模式（点顶点，双击闭合）';
    log('多边形模式：逐点点击，双击闭合；闭合后点击区 = 多边形最大内接矩形');
  } else {
    btnPoly.textContent = '多边形框选（点顶点，双击闭合）';
  }
  updateRoiLabel();
  draw();
});

btnClear.addEventListener('click', () => {
  roi = null;
  polygon = null;
  polyClosed = false;
  selectedBox = null;
  updateRoiLabel();
  updateClickStepButtons();
  draw();
  resultEl.textContent = '—';
});

btnOcr.addEventListener('click', async () => {
  if (!roi) return;
  btnOcr.disabled = true;
  resultEl.textContent = '识别中…';
  try {
    // Wired up in the next step; the route already exists on the backend side.
    const q = `x=${roi.x}&y=${roi.y}&w=${roi.w}&h=${roi.h}`;
    const r = await fetch('/ocr?' + q);
    const s = await r.json();
    if (s.error) {
      resultEl.textContent = '错误：' + s.error;
    } else {
      resultEl.textContent = `text = ${JSON.stringify(s.text)}\nscore = ${s.score.toFixed(4)}\nelapsed = ${s.elapsed_ms.toFixed(1)} ms`;
      log(`OCR → ${s.text} (${s.score.toFixed(3)})`);
    }
  } catch (e) {
    resultEl.textContent = '识别异常：' + e;
  } finally {
    btnOcr.disabled = false;
  }
});

// §22.21: image-recognition test for an ROI — crop it as a template and search
// the WHOLE frame (the same fallback the runtime uses for shifted UIs).
async function doMatch(r) {
  if (!r) return null;
  const q = `x=${r.x}&y=${r.y}&w=${r.w}&h=${r.h}`;
  const resp = await fetch('/match?' + q);
  const s = await resp.json();
  if (s.error) {
    resultEl.textContent = '匹配错误：' + s.error;
    log('图识别匹配错误：' + s.error);
    return s;
  }
  const roiScore = typeof s.roi_score === 'number' ? s.roi_score : -2;
  resultEl.textContent =
    `图识别 score = ${Number(s.score).toFixed(4)} @ (${s.x}, ${s.y})\n` +
    `ROI内 score = ${roiScore.toFixed(4)}\n` +
    `elapsed = ${Number(s.elapsed_ms || 0).toFixed(1)} ms（全帧金字塔搜索）`;
  log(`图识别 → score=${Number(s.score).toFixed(3)} @ (${s.x},${s.y})`);
  return s;
}

if (btnMatch) {
  btnMatch.addEventListener('click', async () => {
    if (!roi) return;
    btnMatch.disabled = true;
    resultEl.textContent = '图识别匹配中（全帧搜索）…';
    try {
      await doMatch(roi);
    } catch (e) {
      resultEl.textContent = '图识别异常：' + e;
    } finally {
      btnMatch.disabled = false;
    }
  });
}

btnCapture.addEventListener('click', doCapture);

// Generate the declaration JSON for copy/paste. `file` is intentionally NOT
// generated: when saved, the engine assigns images/aNNN.png, so the (Chinese)
// id never lands on disk as a path (ENGINE-DESIGN §18.1).
btnAsset.addEventListener('click', async () => {
  if (!roi) return;
  const id = (assetId.value || '').trim();
  if (!id) {
    resultEl.textContent = '请先填素材 id';
    assetId.focus();
    return;
  }
  if (!isValidAssetId(id)) {
    resultEl.textContent = 'id 可用中文/字母/数字/下划线，但不能以数字开头、不能含空格或标点';
    return;
  }
  const kind = assetKind.value;
  const task = (taskName.value || '').trim();
  const decl = {
    ...(task ? { task } : {}),
    id,
    kind,
    roi: [roi.x, roi.y, roi.w, roi.h],
    ...(kind === 'template' ? { threshold: 0.85 } : {}),
    ...(polygon && polyClosed && polygon.length >= 3
      ? { polygon: polygon.map((p) => [p.x, p.y]) }
      : {}),
  };
  const json = JSON.stringify(decl, null, 2);
  resultEl.textContent = `${json}\n\n（保存时由引擎分配 images/aNNN.png；id 不会变成路径）`;
  try {
    await navigator.clipboard.writeText(json);
    log(`素材 JSON 已复制：${id} (${kind})`);
  } catch (e) {
    log(`素材 JSON 已生成（复制失败，请手动选中）：${id}`);
  }
});

btnLines.addEventListener('click', async () => {
  if (!roi) return;
  btnLines.disabled = true;
  resultEl.textContent = '多行识别中…';
  try {
    const q = `x=${roi.x}&y=${roi.y}&w=${roi.w}&h=${roi.h}`;
    const r = await fetch('/lines?' + q);
    const s = await r.json();
    if (s.error) {
      resultEl.textContent = '错误：' + s.error;
      log('多行识别错误：' + s.error);
    } else {
      // /lines returns boxes in ROI-LOCAL coordinates; the canvas draws in
      // frame coordinates, so shift by the ROI origin (this was the visible
      // "boxes drift to the upper-left" bug).
      boxes = (s.lines || []).map((b) => ({
        ...b,
        box: [roi.x + b.box[0], roi.y + b.box[1], b.box[2], b.box[3]],
      }));
      // New boxes = previous block selection is stale.
      selectedBox = null;
      updateClickStepButtons();
      draw();
      const txt = boxes.map((b, i) => `${i + 1}. ${b.text} (${b.score.toFixed(3)})`).join('\n');
      resultEl.textContent = `${boxes.length} 行 / ${s.elapsed_ms.toFixed(0)} ms\n` + txt;
      log(`多行识别 → ${boxes.length} 行`);
    }
  } catch (e) {
    resultEl.textContent = '多行识别异常：' + e;
  } finally {
    btnLines.disabled = false;
  }
});

// --- boot -------------------------------------------------------------------
// Each independent concern is isolated so a missing frame or a not-yet-served
// backend route (/runs) can never blank-screen the editor.
(async function boot() {
  btnSaveAsset.addEventListener('click', doSaveAsset);
  btnSaveFlow.addEventListener('click', doSaveFlow);
  taskName.addEventListener('change', async () => {
    await refreshTaskAssets();
    await loadTaskFlow((taskName.value || '').trim());
  });
  // §22.18 可视化流程编辑器
  if (btnFlowAddStep) {
    btnFlowAddStep.addEventListener('click', () => {
      if (!flowOp) return;
      const defaults = window.OAS_FLOW ? window.OAS_FLOW.defaultArgs(flowOp.value) : {};
      flowModel.steps.push({ op: flowOp.value, argsText: JSON.stringify(defaults, null, 2) });
      flowRender();
    });
  }
  if (btnFlowFromJson) btnFlowFromJson.addEventListener('click', flowLoadFromJson);
  if (btnFlowToJson) btnFlowToJson.addEventListener('click', () => { flowBuildToJson(); });
  if (btnFlowSaveViz) btnFlowSaveViz.addEventListener('click', flowSaveVisual);
  if (btnFlowAddInterrupt) {
    btnFlowAddInterrupt.addEventListener('click', () => {
      flowModel.interrupts.push({ detect: '', action: '' });
      flowRender();
    });
  }
  if (flowSteps) flowRender();
  if (btnStatsRefresh) btnStatsRefresh.addEventListener('click', () => loadStats());
  await loadStats();
  // N2: 生成点击指令 / 追加到 flow
  if (btnClickStep) btnClickStep.addEventListener('click', doGenerateClickStep);
  if (btnAppendStep) btnAppendStep.addEventListener('click', doAppendStepToFlow);
  updateClickStepButtons();
  // N4: 点击分布 controls
  if (btnRunsRefresh) btnRunsRefresh.addEventListener('click', () => refreshRuns());
  if (btnClicksOverlay) btnClicksOverlay.addEventListener('click', () => loadClicks(runsSelect.value));
  if (btnClicksClear) btnClicksClear.addEventListener('click', clearClicks);
  if (runsSelect) {
    runsSelect.addEventListener('change', () =>
      setClicksStatus(`已选择 ${runsSelect.value || '（无）'}，点「叠加分布」查看`)
    );
  }
  updateClickStatsText();

  // Frame is optional: the click-distribution view must work even with no adb
  // device attached (canvas then keeps its default 1280x720 frame size).
  try {
    await loadFrame();
    await refreshState();
    btnCapture.disabled = false;
    log('已连接，画面已加载');
  } catch (e) {
    setStatus('没有可用帧，点「截图」');
    btnCapture.disabled = false;
    log('帧加载失败（点击分布仍可用）：' + (e && e.message ? e.message : e));
  }

  try {
    await refreshTaskAssets();
  } catch (e) {
    log('任务素材读取失败：' + e);
  }

  // §22.18: load the task's existing flow into the visual editor; `?task=<name>`
  // is a deep link used by the headless acceptance run.
  let deepTask = '';
  try {
    deepTask = new URLSearchParams(location.search).get('task') || '';
  } catch (e) {
    deepTask = '';
  }
  if (deepTask) taskName.value = deepTask;
  await loadTaskFlow((taskName.value || '').trim());

  // §22.21 deep link for the headless acceptance:
  //   /?match=1&x=330&y=69&w=95&h=55  -> select that ROI and run the image test.
  try {
    const dp = new URLSearchParams(location.search);
    if (dp.get('match') === '1') {
      const x = Number(dp.get('x'));
      const y = Number(dp.get('y'));
      const w = Number(dp.get('w'));
      const h = Number(dp.get('h'));
      if (w > 1 && h > 1) {
        roi = { x, y, w, h };
        updateRoiLabel();
        if (img) draw();
        await doMatch(roi);
      }
    }
  } catch (e) {
    log('图识别深链失败：' + e);
  }
  window.OAS_FLOW_UI = {
    getModel: () => flowModel,
    render: flowRender,
    load: loadTaskFlow,
    build: flowBuildToJson,
  };

  // N4: refresh /runs once on boot, then honour the `?run=<file>` deep link so
  // a headless screenshot can land directly on the rendered distribution.
  let deepRun = '';
  try {
    deepRun = new URLSearchParams(location.search).get('run') || '';
  } catch (e) {
    deepRun = '';
  }
  try {
    await refreshRuns();
    if (deepRun) {
      populateRunsSelect(deepRun);
      if (runsSelect) runsSelect.value = deepRun;
      setClicksStatus(`深链 ?run=${deepRun}，正在加载…`);
      log(`点击分布：深链加载 ${deepRun}`);
      await loadClicks(deepRun);
    }
  } catch (e) {
    setClicksStatus('点击分布初始化失败：' + (e && e.message ? e.message : e));
    log('点击分布初始化失败：' + e);
  }
})();
