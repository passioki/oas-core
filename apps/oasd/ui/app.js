// OAS engine editor - MVP front end.
// Plain JS on purpose: no build step, so the same files can be served by the
// Rust daemon today and wrapped by Tauri / an Android WebView later.

const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');
const hintEl = document.getElementById('hint');
const statusEl = document.getElementById('status');
const roiEl = document.getElementById('roi');
const resultEl = document.getElementById('result');
const logEl = document.getElementById('log');
const btnLivePreview = document.getElementById('btn-live-preview');
const btnOcr = document.getElementById('btn-ocr');
const btnClear = document.getElementById('btn-clear');
const btnLines = document.getElementById('btn-lines');
const linesList = document.getElementById('lines-list');
const btnMatch = document.getElementById('btn-match');
const btnMatchSave = document.getElementById('btn-match-save');
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
const btnRegionClick = document.getElementById('btn-region-click');
const btnRegionDrag = document.getElementById('btn-region-drag');
const btnRegionSave = document.getElementById('btn-region-save');
const btnPoly = document.getElementById('btn-poly');
const btnBackMode = document.getElementById('btn-back-mode');
const btnActionPoly = document.getElementById('btn-action-poly');
const btnActionInvert = document.getElementById('btn-action-invert');
// §22.27 会话图片列表（多帧）
const frameFile = document.getElementById('frame-file');
const btnFrameUpload = document.getElementById('btn-frame-upload');
const btnFrameCapture = document.getElementById('btn-frame-capture');
const btnFrameRelations = document.getElementById('btn-frame-relations');
const btnFramesRefresh = document.getElementById('btn-frames-refresh');
const btnFramesClear = document.getElementById('btn-frames-clear');
const framesList = document.getElementById('frames-list');
const frameDetail = document.getElementById('frame-detail');
const canvasStage = document.getElementById('canvas-stage');
const canvasWindows = document.getElementById('canvas-windows');
const btnCanvasPanels = document.getElementById('btn-canvas-panels');
const frameCategoryFilter = document.getElementById('frame-category-filter');
const frameCategorySet = document.getElementById('frame-category-set');
let currentFrameId = null;
let framesCache = [];
let categoryFilterValue = '';
let frameCategoryPicker = null;
const framesStatus = document.getElementById('frames-status');
// Unified capture dialog (image page only).
const captureDialog = document.getElementById('capture-frame-dialog');
const captureNameHost = document.getElementById('capture-name-host');
const captureCategoryHost = document.getElementById('capture-category-host');
const capturePreviewImg = document.getElementById('capture-preview-img');
const captureError = document.getElementById('capture-error');
const captureCancel = document.getElementById('capture-cancel');
const captureSubmit = document.getElementById('capture-submit');
const relationsDialog = document.getElementById('relations-dialog');
const relationsStatus = document.getElementById('relations-status');
const relationsView = document.getElementById('relations-view');
const relationRecordDialog = document.getElementById('relation-record-dialog');
const relationRecordTask = document.getElementById('relation-record-task');
const relationRecordAssetHost = document.getElementById('relation-record-asset-host');
const relationRecordResult = document.getElementById('relation-record-result');
const relationRecordSubmit = document.getElementById('relation-record-submit');
const annotationDialog = document.getElementById('annotation-dialog');
const annotationType = document.getElementById('annotation-type');
const annotationA = document.getElementById('annotation-a');
const annotationB = document.getElementById('annotation-b');
const annotationNote = document.getElementById('annotation-note');
const annotationError = document.getElementById('annotation-error');
let relationRecordAssetPicker = null;
let relationRecordAssetId = '';
let lastRelationsSnapshot = null;
let lastRelationsTask = '';
let captureNamePicker = null;
let captureCategoryPicker = null;
let captureBusy = false;
// §22.28 顶部标题行：结果/日志抽屉 + 任务素材列表
const btnResultToggle = document.getElementById('btn-result-toggle');
const btnLogToggle = document.getElementById('btn-log-toggle');
const hdrDrawers = document.getElementById('hdr-drawers');
const resultPop = document.getElementById('result-pop');
const logPop = document.getElementById('log-pop');
const assetSearch = document.getElementById('asset-search');
const assetCategoryFilter = document.getElementById('asset-category-filter');
const assetsList = document.getElementById('assets-list');
const taskRefAssets = document.getElementById('task-ref-assets');
const taskRefAssetsTask = document.getElementById('task-ref-assets-task');
const assetPositionPicker = document.getElementById('asset-position-picker');
const assetPositionInheritHost = document.getElementById('asset-position-inherit-host');
const assetPositionList = document.getElementById('asset-position-list');
const assetPositionStatus = document.getElementById('asset-position-status');
let assetFramesCache = [];
let pendingAssetPositions = [];
let assetPositionInheritPicker = null;
const btnAssetsRefresh = document.getElementById('assets-refresh');
const btnAssetsBatchTest = document.getElementById('btn-assets-batch-test');
const runsSelect = document.getElementById('runs-select');
const clickScope = document.getElementById('click-scope');
const clickTaskFilter = document.getElementById('click-task-filter');
const btnClicksAggregate = document.getElementById('btn-clicks-aggregate');
const btnRunDelete = document.getElementById('btn-run-delete');
const btnRunsTrashAll = document.getElementById('btn-runs-trash-all');
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
// §22.22 全局素材库
const libraryId = document.getElementById('library-id');
const btnLibrarySave = document.getElementById('btn-library-save');
const librarySelect = document.getElementById('library-select');
const btnLibraryRefresh = document.getElementById('btn-library-refresh');
const btnLibraryMatch = document.getElementById('btn-library-match');
const libraryStatus = document.getElementById('library-status');
// N38/M1: 任务列表 / 新建任务对话框 / 运行任务
const taskSelect = document.getElementById('task-select');
const btnTaskRefresh = document.getElementById('btn-task-refresh');
const btnTaskCreate = document.getElementById('btn-task-create');
const btnTaskRun = document.getElementById('btn-task-run');
const btnTaskStop = document.getElementById('btn-task-stop');
// v1.1 P2: 任务工作台 + 完成条件
const goalMode = document.getElementById('goal-mode');
const goalCount = document.getElementById('goal-count');
const btnGoalApply = document.getElementById('btn-goal-apply');
const goalStatus = document.getElementById('goal-status');
const btnWbAssets = document.getElementById('btn-wb-assets');
const btnWbFlow = document.getElementById('btn-wb-flow');
// v1.1 P3: app shell pages
const appNav = document.getElementById('app-nav');
const appPages = Array.from(document.querySelectorAll('.app-page'));
const navDevice = document.getElementById('nav-device');
const libList = document.getElementById('lib-list');
const libRefresh = document.getElementById('lib-refresh');
const libNew = document.getElementById('lib-new');
const instCard = document.getElementById('inst-card');
const instRefresh = document.getElementById('inst-refresh');
const instSerial = document.getElementById('inst-serial');
const instAdb = document.getElementById('inst-adb');
const instSave = document.getElementById('inst-save');
const instMsg = document.getElementById('inst-msg');
const instDevicesSelect = document.getElementById('inst-device-select');
const instDevicesRefresh = document.getElementById('inst-devices-refresh');
const instDevicesMsg = document.getElementById('inst-devices-msg');
const pageSchedSummary = document.getElementById('page-sched-summary');
const pageSchedEntries = document.getElementById('page-sched-entries');
const pageSchedRefresh = document.getElementById('page-sched-refresh');
const pageSchedReset = document.getElementById('page-sched-reset');
const settingsConfig = document.getElementById('settings-config');
const settingsRefresh = document.getElementById('settings-refresh');
const runDry = document.getElementById('run-dry');
const runReplay = document.getElementById('run-replay');
const runStatusEl = document.getElementById('run-status');
const createTaskDialog = document.getElementById('create-task-dialog');
const createName = document.getElementById('create-name');
const createTemplate = document.getElementById('create-template');
const createDesc = document.getElementById('create-desc');
const createError = document.getElementById('create-error');
const btnCreateSubmit = document.getElementById('btn-create-submit');
const btnCreateCancel = document.getElementById('btn-create-cancel');
// M2/N37：调度面板 + 重置记忆对话框
const btnSchedRefresh = document.getElementById('btn-sched-refresh');
const btnSchedReset = document.getElementById('btn-sched-reset');
const schedSummary = document.getElementById('sched-summary');
const schedEntries = document.getElementById('sched-entries');
const resetMemoryDialog = document.getElementById('reset-memory-dialog');const resetTaskList = document.getElementById('reset-task-list');
const resetIncludeCheckpoint = document.getElementById('reset-include-checkpoint');
const resetError = document.getElementById('reset-error');
const btnResetAll = document.getElementById('btn-reset-all');
const btnResetInvert = document.getElementById('btn-reset-invert');
const btnResetNone = document.getElementById('btn-reset-none');
const btnResetSubmit = document.getElementById('btn-reset-submit');
const btnResetCancel = document.getElementById('btn-reset-cancel');
let schedulerStatus = null;
// M1 运行状态轮询：已消费的日志行数 / 是否已有轮询在跑（避免重复轮询）
let runLogSeen = 0;
let runPolling = false;

let img = null;
let livePreview = true;
let liveTimer = null;
let liveBusy = false;
let liveSocket = null;
let liveObjectUrl = null;
let liveReconnectTimer = null;
// Live-preview passthrough: a pending mouse gesture on the live canvas.
let liveInputDrag = null;
let roi = null; // {x,y,w,h} 识别框（前端识别/裁剪区域）
let roiBack = null; // v1.2 操作区域（action_roi）；null = 默认识别区域
let lastMatch = null; // v1.2.2: last image-match result; save-with-action-region source
let backMode = false; // 正在绘制操作区域
let actionPoly = null; // 操作区域多边形顶点
let actionPolyClosed = false;
let actionPolyMode = false;
let actionInvert = false;
let dragTarget = 'roi'; // 'roi' | 'back'
let dragStart = null;
let boxes = []; // detection boxes from the multi-line route
let selectedBox = null; // the detection block last clicked (N2, §19.2)
// v1.1: ROI used by the latest multi-line OCR; each line can be turned into an
// inline-OCR flow step (roi + text) without saving per-line assets.
let lastLinesRoi = null;
// v1.2: multi-select save candidates + their recognition / operation regions.
// `actionRoi` defaults to the recognition bbox; adjusted on canvas.
let saveCandidates = [];
let activeCandidate = -1;
let pendingActionAsset = null; // existing asset whose operation region is being edited
const btnBatchSave = document.getElementById('btn-batch-save');
const btnAssetConflicts = document.getElementById('btn-asset-conflicts');
const assetConflicts = document.getElementById('asset-conflicts');
const btnAssetsMigrate = document.getElementById('btn-assets-migrate');
const assetsMigrateStatus = document.getElementById('assets-migrate-status');
const batchStatus = document.getElementById('batch-status');
// N4 click-distribution state (§21): the run list, the loaded points + metrics,
// and a cached offscreen bitmap so draw() stays cheap.
let runs = [];
let clickData = null; // {file, clicks, distinct, points}
let clickOverlay = null;
// §22.18 visual flow editor model: {name, steps:[{op,argsText}], interrupts:[{detect,action}]}
let flowModel = { name: '', steps: [], interrupts: [] };
let taskFlowRaw = null;
// §22.28 任务素材列表（分组/搜索/行内测试）
let taskAssets = [];
let taskAssetsMeta = {};
let resultOpen = false;
let logOpen = false;
// §22.22: serve 能力位（OCR 模型是否就绪），避免"无模型/无 feature 构建"静默 501
let ocrReady = true;
let detReady = true;
// §22.10 多边形框选：顶点为画面坐标；闭合后 roi = 多边形外接矩形。
// 运行时：点击区 = 匹配外接矩形；匹配/OCR 使用多边形掩膜（N10b）。
let polygon = null;
let polyClosed = false;
let polyMode = false;
let polyDrag = null; // 正在拖动的顶点下标（闭合后编辑）

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

function log(msg, level, detail) {
  const now = new Date();
  const pad = (n, w = 2) => String(n).padStart(w, '0');
  const t = `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}.${pad(now.getMilliseconds(), 3)}`;
  const lv = String(level || 'INFO').toUpperCase();
  let line = `[${t}] [${lv}] ${msg}`;
  if (detail !== undefined) {
    line += ' | ' + (typeof detail === 'string' ? detail : JSON.stringify(detail));
  }
  logEl.textContent = line + '\n' + logEl.textContent;
  if (!window.__oasLogs) window.__oasLogs = [];
  window.__oasLogs.push(line);
  if (window.__oasLogs.length > 2000) window.__oasLogs.shift();
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

// Live passthrough: only while the live preview is current. Click -> tap,
// drag -> swipe. ROI/polygon editing is intentionally paused in this mode.
const LIVE_TAP_SLOP = 6;
function clampToFrame(p) {
  return {
    x: Math.max(0, Math.min(Math.max(0, canvas.width - 1), p.x)),
    y: Math.max(0, Math.min(Math.max(0, canvas.height - 1), p.y)),
  };
}
function liveInputAllowed() {
  return livePreview === true;
}
async function sendLiveTap(p) {
  const r = await postJson('/input/tap', { x: p.x, y: p.y });
  if (!r || !r.ok) {
    throw new Error((r && r.errors || []).join('; ') || 'tap failed');
  }
  resultEl.textContent = `实时操作：点击 (${p.x}, ${p.y})\n设备：${r.device}`;
  log(`[实时] tap (${p.x}, ${p.y}) device=${r.device}`);
}
async function sendLiveSwipe(from, to, ms) {
  const r = await postJson('/input/swipe', { from: [from.x, from.y], to: [to.x, to.y], ms });
  if (!r || !r.ok) {
    throw new Error((r && r.errors || []).join('; ') || 'swipe failed');
  }
  resultEl.textContent = `实时操作：滑动 (${from.x}, ${from.y}) → (${to.x}, ${to.y}) ${ms}ms\n设备：${r.device}`;
  log(`[实时] swipe (${from.x}, ${from.y}) -> (${to.x}, ${to.y}) ${ms}ms device=${r.device}`);
}
function finishLiveInput(ev) {
  if (!liveInputDrag || !liveInputAllowed()) return;
  const d = liveInputDrag;
  liveInputDrag = null;
  const dx = ev.clientX - d.clientX;
  const dy = ev.clientY - d.clientY;
  if (Math.abs(dx) <= LIVE_TAP_SLOP && Math.abs(dy) <= LIVE_TAP_SLOP) {
    void sendLiveTap(d.start).catch((e) => {
      resultEl.textContent = '实时操作失败：' + (e && e.message ? e.message : e);
      log('[实时] tap 失败：' + (e && e.message ? e.message : e));
    });
    return;
  }
  const end = clampToFrame(toFrame(ev));
  const ms = Math.max(50, Math.min(2000, Math.round(performance.now() - d.t)));
  void sendLiveSwipe(d.start, end, ms).catch((e) => {
    resultEl.textContent = '实时操作失败：' + (e && e.message ? e.message : e);
    log('[实时] swipe 失败：' + (e && e.message ? e.message : e));
  });
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
    ctx.fillText(`识别 ${roi.w}x${roi.h} @ ${roi.x},${roi.y}`, roi.x + 4, Math.max(14, roi.y - 6));
    ctx.restore();
  }
  // §22.25 / v1.2 操作区域（action_roi）：橙色虚线；未设置时运行期用识别结果。
  if (roiBack && roiBack.w > 0 && roiBack.h > 0) {
    ctx.save();
    ctx.setLineDash([8, 5]);
    ctx.strokeStyle = '#ff9f40';
    ctx.lineWidth = 2;
    ctx.strokeRect(roiBack.x + 0.5, roiBack.y + 0.5, roiBack.w, roiBack.h);
    ctx.fillStyle = 'rgba(255,159,64,0.10)';
    ctx.fillRect(roiBack.x, roiBack.y, roiBack.w, roiBack.h);
    ctx.fillStyle = '#ff9f40';
    ctx.font = '14px Consolas, monospace';
    ctx.fillText(
      `操作区域 ${roiBack.w}x${roiBack.h} @ ${roiBack.x},${roiBack.y}`,
      roiBack.x + 4,
      Math.max(14, roiBack.y - 6)
    );
    ctx.restore();
  }
  // Operation polygon (action_shape): orange solid; invert means screen minus shape.
  if (actionPoly && actionPoly.length) {
    ctx.save();
    ctx.beginPath();
    ctx.moveTo(actionPoly[0].x, actionPoly[0].y);
    for (let i = 1; i < actionPoly.length; i++) ctx.lineTo(actionPoly[i].x, actionPoly[i].y);
    if (actionPolyClosed) ctx.closePath();
    ctx.strokeStyle = '#ffb020';
    ctx.lineWidth = 2;
    ctx.stroke();
    if (actionPolyClosed) {
      ctx.fillStyle = 'rgba(255,176,32,0.12)';
      ctx.fill();
    }
    if (actionInvert) {
      ctx.setLineDash([10, 6]);
      ctx.strokeStyle = '#ffb020';
      ctx.strokeRect(0.5, 0.5, canvas.width - 1, canvas.height - 1);
    }
    ctx.restore();
  }
  saveCandidates.forEach((c, idx) => {
    if (!c.action || c.action.w <= 0 || c.action.h <= 0) return;
    const active = idx === activeCandidate;
    ctx.save();
    ctx.setLineDash([5, 4]);
    ctx.strokeStyle = active ? 'rgba(255,159,64,0.95)' : 'rgba(255,159,64,0.45)';
    ctx.lineWidth = active ? 2 : 1;
    ctx.strokeRect(c.action.x + 0.5, c.action.y + 0.5, c.action.w, c.action.h);
    ctx.restore();
  });
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
  const aShape = currentActionShape();
  let backInfo = ' · 操作区域未设置（=真实识别结果）';
  if (aShape) {
    const b = actionShapeBBox(aShape);
    const geom = aShape.kind === 'polygon' ? `多边形 ${aShape.points.length} 顶点` : `矩形 ${b ? `${b[2]}x${b[3]} @ ${b[0]},${b[1]}` : ''}`;
    backInfo = ` · 操作区域 ${geom}${aShape.invert ? '（反选：屏幕−区域）' : ''}`;
  }
  if (!roi) {
    roiEl.textContent = polygon
      ? `多边形：${polygon.length} 个顶点（未闭合）`
      : '未选择';
    btnOcr.disabled = true;
    btnLines.disabled = true;
    if (btnMatch) btnMatch.disabled = true;
    if (btnLibrarySave) btnLibrarySave.disabled = true;
    if (btnAsset) btnAsset.disabled = true;
    if (btnRegionClick) btnRegionClick.disabled = true;
    if (btnRegionDrag) btnRegionDrag.disabled = true;
    if (btnRegionSave) btnRegionSave.disabled = true;
    return;
  }
  const polyInfo =
    polygon && polyClosed ? ` · 多边形 ${polygon.length} 顶点（识别掩膜）` : '';
  roiEl.textContent =
    `识别框 x=${roi.x} y=${roi.y} w=${roi.w} h=${roi.h}${polyInfo}${backInfo}`;
  btnOcr.disabled = roi.w < 2 || roi.h < 2;
  btnLines.disabled = roi.w < 2 || roi.h < 2;
  if (btnRegionClick) btnRegionClick.disabled = roi.w < 2 || roi.h < 2;
  if (btnRegionDrag) btnRegionDrag.disabled = roi.w < 2 || roi.h < 2;
  if (btnRegionSave) btnRegionSave.disabled = roi.w < 2 || roi.h < 2;
  if (btnMatch) btnMatch.disabled = roi.w < 2 || roi.h < 2;
  if (btnLibrarySave) btnLibrarySave.disabled = roi.w < 2 || roi.h < 2;
  if (btnAsset) btnAsset.disabled = roi.w < 2 || roi.h < 2;
  updateSaveButtonState();
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

// Append one generated step to the flow textarea. A parse failure (or a
// non-array `steps`) must leave the textarea untouched - never corrupt user
// input. Shared by the local-point path and the v1.1 inline-OCR path.
function appendStepToFlow(step, label) {
  // v1.2.2: workbench flow panel is removed; flow editing lives in 任务构建.
  // Copy the generated step instead of mutating a hidden textarea.
  if (!flowJson) {
    const json = JSON.stringify(step, null, 2);
    resultEl.textContent = `已复制${label || '步骤'}到剪贴板（到「任务构建」粘贴/添加）：\n${json}`;
    log(`已复制${label || '步骤'}到剪贴板（任务构建编辑）`);
    if (navigator.clipboard) navigator.clipboard.writeText(json).catch(() => {});
    return true;
  }
  const raw = (flowJson.value || '').trim();
  if (!raw) {
    resultEl.textContent = '追加失败：flow 为空，请先粘贴/保存一个 flow.json（textarea 未改动）';
    return false;
  }
  let flow;
  try {
    flow = JSON.parse(raw);
  } catch (e) {
    resultEl.textContent = `追加失败：现有 flow 不是合法 JSON（${e.message}）。textarea 未改动。`;
    return false;
  }
  if (!Array.isArray(flow.steps)) {
    resultEl.textContent = '追加失败：flow.steps 不是数组。textarea 未改动。';
    return false;
  }
  flow.steps.push(step);
  flowJson.value = JSON.stringify(flow, null, 2);
  resultEl.textContent = `已追加到流程（共 ${flow.steps.length} 步）\n` + JSON.stringify(step, null, 2);
  log(`已追加${label || '步骤'}到流程（${flow.steps.length} 步）`);
  return true;
}

// Append the generated step to the flow textarea. A parse failure (or a
// non-array `steps`) must leave the textarea untouched - never corrupt user
// input.
function doAppendStepToFlow() {
  if (!selectedBox) return;
  const step = clickStepForBox(selectedBox);
  appendStepToFlow(step, '点击指令');
}

// --- v1.1 P2: 任务级完成条件 ------------------------------------------------
async function applyTaskGoal() {
  const task = (taskName.value || '').trim();
  if (!task) {
    resultEl.textContent = '请先选择/新建任务';
    return;
  }
  const mode = goalMode ? goalMode.value : 'battles';
  const goal = { mode };
  if (mode === 'battles') {
    const n = Number(goalCount && goalCount.value);
    if (!Number.isInteger(n) || n < 1 || n > 999) {
      resultEl.textContent = '完成次数必须是 1~999 的整数';
      return;
    }
    goal.count = n;
  }
  const r = await postJson('/task/goal', { task, goal });
  if (!r.ok) {
    showErrors('写入完成条件失败：', r.errors);
    return;
  }
  if (goalStatus) goalStatus.textContent = `已写入 meta.goal：${JSON.stringify(goal)}`;
  if (mode !== 'battles') {
    resultEl.textContent =
      '完成条件已写入 meta.goal。\n' +
      '「直到资源耗尽 / 直到条件出现」需要 flow 自己用 while/识别/出现 判断，编辑器只记录目标。';
    return;
  }
  // battles: wrap existing actions in loop(param('max_battles')), preserve the
  // original finish exit. This is what makes "打 N 场算完成" executable.
  let flow;
  try {
    const fr = await fetch('/task/flow?task=' + encodeURIComponent(task));
    const fs = await fr.json();
    flow = fs.flow;
  } catch (e) {
    showErrors('读取 flow 失败：', [String(e)]);
    return;
  }
  if (!flow || !Array.isArray(flow.steps)) {
    flow = { schema_version: 1, name: task, steps: [] };
  }
  const already = flow.steps.length === 2 &&
    flow.steps[0].op === 'loop' &&
    JSON.stringify(flow.steps[0].args || {}).includes('max_battles');
  if (!already) {
    let exit = 'done';
    const actions = [];
    for (const s of flow.steps) {
      if (s.op === 'finish') exit = (s.args && s.args.exit) || exit;
      else actions.push(s);
    }
    if (!actions.length) actions.push({ op: 'log', args: { level: 'info', text: "'完成条件循环'" } });
    flow.steps = [
      { op: 'loop', args: { n: "param('max_battles')" }, body: actions },
      { op: 'finish', args: { exit } },
    ];
  }
  const sr = await postJson('/task/save_flow', { task, flow });
  if (!sr.ok) {
    showErrors('生成完成条件流程失败：', sr.errors);
    return;
  }
  resultEl.textContent =
    `完成条件已应用：一次运行打 ${goal.count} 场。\n` +
    `flow 已生成 loop n=param('max_battles')，并保留出口 ${flow.steps[1].args.exit}。`;
  if (typeof loadTaskFlow === 'function') await loadTaskFlow(task);
  log(`完成条件：${task} 打 ${goal.count} 场`);
}

// --- v1.1 P2: 任务工作台导航 -----------------------------------------------
// --- v1.1 P3: 应用级页面（工作台/任务库/实例/调度/设置） --------------------
function activatePage(name) {
  for (const p of appPages) p.classList.toggle('active', p.id === 'page-' + name);
  if (appNav) {
    for (const b of appNav.querySelectorAll('.app-nav-btn')) {
      b.classList.toggle('active', b.dataset.page === name);
    }
  }
  if (name === 'tasks') refreshLibrary();
  else if (name === 'builder' && window.OAS_BUILDER) {
    // R14: builder must open the same active task as the workbench.
    window.OAS_BUILDER.activate((taskName && taskName.value || '').trim());
  }
  else if (name === 'instances') refreshInstance();
  else if (name === 'schedule') refreshSchedulePage();
  else if (name === 'settings') refreshSettings();
}

async function refreshNavDevice() {
  try {
    const r = await fetch('/instance');
    const s = await r.json();
    if (navDevice) {
      navDevice.textContent = s.ok
        ? `实例：${s.device_serial || '未配置'} · ${s.state && s.state.device ? s.state.device : '未连接'}`
        : '实例：未配置';
    }
  } catch (e) {
    if (navDevice) navDevice.textContent = '实例：不可用';
  }
}

async function refreshLibrary() {
  if (!libList) return;
  libList.textContent = '加载中…';
  try {
    const r = await fetch('/task/library');
    const s = await r.json();
    if (!s.ok) {
      libList.textContent = '加载失败：' + ((s.errors || []).join('; ') || '未知错误');
      return;
    }
    libList.textContent = '';
    if (!s.tasks.length) {
      libList.textContent = '（还没有任务；点「新建任务…」）';
      return;
    }
    for (const t of s.tasks) {
      const card = document.createElement('div');
      card.className = 'task-card';
      const meta = document.createElement('div');
      meta.className = 'task-meta';
      const title = document.createElement('div');
      title.className = 'task-title';
      title.textContent = `${t.display_name || t.name}  (${t.name})`;
      const sub = document.createElement('div');
      sub.className = 'task-sub';
      const goal = t.goal && t.goal.mode ? ` · 完成条件=${t.goal.mode}${t.goal.count ? '/' + t.goal.count : ''}` : '';
      sub.textContent = `${t.category || '—'} · 素材 ${t.assets ?? '?'} · ${t.description || '（无描述）'}${goal}`;
      meta.append(title, sub);
      const open = document.createElement('button');
      open.type = 'button';
      open.textContent = '打开';
      open.addEventListener('click', () => openTaskInWorkbench(t.name));
      const del = document.createElement('button');
      del.type = 'button';
      del.textContent = '删除';
      del.addEventListener('click', () => deleteTaskFromLibrary(t.name));
      card.append(meta, open, del);
      libList.appendChild(card);
    }
  } catch (e) {
    libList.textContent = '加载异常：' + e;
  }
}

async function openTaskInWorkbench(name) {
  taskName.value = name;
  if (taskSelect) syncTaskSelect(name);
  activatePage('workbench');
  if (window.__oasActivatePanel) window.__oasActivatePanel('panel-task');
  await refreshTaskAssets(name);
  await loadTaskFlow(name);
}

async function deleteTaskFromLibrary(name) {
  if (!window.confirm(`删除任务「${name}」？\n\n任务目录（flow/assets/images）会被删除，不可恢复。`)) return;
  const r = await postJson('/task/delete', { task: name });
  if (!r.ok) {
    window.alert('删除失败：' + (r.errors || []).join('\n'));
    return;
  }
  log(`已删除任务：${name}`);
  await refreshLibrary();
}

async function refreshInstanceDevices() {
  if (!instDevicesSelect) return;
  if (instDevicesMsg) instDevicesMsg.textContent = '检测中…';
  try {
    const r = await fetch('/instance/devices');
    const s = await r.json();
    if (!s.ok) {
      if (instDevicesMsg) instDevicesMsg.textContent = '检测失败：' + ((s.errors || []).join('; ') || '未知错误');
      return;
    }
    const devices = s.devices || [];
    instDevicesSelect.textContent = '';
    const blank = document.createElement('option');
    blank.value = '';
    blank.textContent = devices.length ? '（选择检测到的实例）' : '（没有检测到 adb 实例）';
    instDevicesSelect.appendChild(blank);
    for (const d of devices) {
      const aliases = (d.aliases || []).join(' / ');
      const opt = document.createElement('option');
      opt.value = d.serial;
      opt.textContent = d.label || `${d.serial}${aliases ? ' (' + aliases + ')' : ''}`;
      instDevicesSelect.appendChild(opt);
    }
    // Match the configured serial against every alias of the physical device.
    const configured = (s.configured || '').trim();
    let match = null;
    if (configured) {
      match = devices.find((d) => (d.aliases || []).includes(configured) || d.serial === configured) || null;
    }
    if (!match && devices.length === 1) match = devices[0];
    if (match) {
      instDevicesSelect.value = match.serial;
      if (instSerial) instSerial.value = match.serial;
    }
    if (instDevicesMsg) {
      instDevicesMsg.textContent = devices.length
        ? `检测到 ${devices.length} 个物理实例；选择后保存到 config/live.json。`
        : '没有检测到 adb 实例；请先启动模拟器，再点「检测实例」。';
    }
  } catch (e) {
    if (instDevicesMsg) instDevicesMsg.textContent = '检测异常：' + e;
  }
}

async function refreshInstance() {
  if (!instCard) return;
  try {
    const r = await fetch('/instance');
    const s = await r.json();
    if (!s.ok) {
      instCard.textContent = '加载失败：' + ((s.errors || []).join('; ') || '未知错误');
      return;
    }
    instCard.textContent = '';
    const rows = [
      ['活动实例', s.name],
      ['设备 serial', s.device_serial || '（未配置）'],
      ['adb 路径', s.adb_path || '（PATH/ADB_PATH）'],
      ['serve 端口', String(s.serve_port || '—')],
      ['绑定地址', s.bind || '—'],
      ['设备状态', s.state && s.state.device ? s.state.device : '未连接'],
      ['错误', (s.state && s.state.error) || '无'],
    ];
    for (const [k, v] of rows) {
      const row = document.createElement('div');
      row.className = 'row';
      row.innerHTML = `<span class="muted small" style="width:110px">${k}</span><span>${String(v)}</span>`;
      instCard.appendChild(row);
    }
    if (navDevice) navDevice.textContent = `实例：${s.device_serial || '未配置'}`;
    if (instSerial) instSerial.value = s.device_serial || '';
    if (instAdb) instAdb.value = s.adb_path || '';
    await refreshInstanceDevices();
  } catch (e) {
    instCard.textContent = '加载异常：' + e;
  }
}

async function saveInstance() {
  if (!instSerial) return;
  if (instMsg) instMsg.textContent = '保存中…';
  const r = await postJson('/instance', {
    device_serial: instSerial.value.trim(),
    adb_path: instAdb ? instAdb.value.trim() : '',
  });
  if (!r.ok) {
    if (instMsg) instMsg.textContent = '保存失败：' + (r.errors || []).join('; ');
    return;
  }
  if (instMsg) instMsg.textContent = '已保存 config/live.json；重启 serve 后生效。';
  log('实例配置已保存；重启 serve 生效');
  await refreshInstance();
}

async function refreshSchedulePage() {
  if (!pageSchedSummary) return;
  try {
    const r = await fetch('/scheduler/status');
    const s = await r.json();
    if (s.ok === false) {
      pageSchedSummary.textContent = '读取失败：' + ((s.errors || []).join('; ') || '未知');
      return;
    }
    schedulerStatus = s;
    const guards = s.guards || {};
    pageSchedSummary.textContent =
      `模式=${s.mode || '—'} · 轮次=${s.passes ?? 0} · 下一唤醒=${s.next_wake_local || '—'} · ` +
      `护栏=${guards.blocked ? '已阻断' : '正常'} · 计划=${s.config_path || '—'}`;
    pageSchedEntries.textContent = '';
    for (const e of s.entries || []) {
      const card = document.createElement('div');
      card.className = 'task-card';
      const meta = document.createElement('div');
      meta.className = 'task-meta';
      const title = document.createElement('div');
      title.className = 'task-title';
      title.textContent = `${e.entry_id} · ${e.task}`;
      const sub = document.createElement('div');
      sub.className = 'task-sub';
      const q = e.quota || {};
      sub.textContent =
        `${e.lifecycle} · ${e.runnable_now ? '现在可跑' : (e.blocked_reason || '等待')}` +
        (q.per ? ` · quota ${q.used ?? 0}/${q.times ?? '?'}` : '') +
        (e.next_due_local ? ` · 下次 ${e.next_due_local}` : '');
      meta.append(title, sub);
      card.append(meta);
      pageSchedEntries.appendChild(card);
    }
  } catch (e) {
    pageSchedSummary.textContent = '读取异常：' + e;
  }
}

async function refreshSettings() {
  if (!settingsConfig) return;
  try {
    const r = await fetch('/config');
    const s = await r.json();
    settingsConfig.textContent = s.ok
      ? JSON.stringify(s.config, null, 2)
      : '读取失败：' + ((s.errors || []).join('; ') || '未知错误');
  } catch (e) {
    settingsConfig.textContent = '读取异常：' + e;
  }
}

function regionRoiArray() {
  return roi ? [roi.x, roi.y, roi.w, roi.h] : null;
}

function promptDragDirection() {
  const raw = (window.prompt('拖动方向：up / down / left / right（必填）', 'up') || '').trim().toLowerCase();
  if (!['up', 'down', 'left', 'right'].includes(raw)) {
    resultEl.textContent = '拖动方向必须是 up / down / left / right';
    return null;
  }
  return raw;
}

function generateRegionClick() {
  const r = regionRoiArray();
  if (!r) return;
  const step = { op: 'click_target', args: { find: [{ kind: 'region', roi: r }] } };
  appendStepToFlow(step, '区域点击');
}

function generateRegionDrag() {
  const r = regionRoiArray();
  if (!r) return;
  const direction = promptDragDirection();
  if (!direction) return;
  const step = {
    op: 'drag',
    args: {
      from: { kind: 'region', roi: r },
      direction,
      distance: [150, 500],
      duration_ms: [300, 650],
      margin: 0.15,
    },
  };
  appendStepToFlow(step, '区域拖动');
}

// --- v1.1: 多行 OCR 行级操作（内联 ROI，不强制建素材） --------------------
function inlineOcrClickStep(text, roi) {
  return {
    op: 'click_target',
    args: {
      find: [
        {
          kind: 'ocr_text',
          roi: [roi.x, roi.y, roi.w, roi.h],
          text,
        },
      ],
    },
  };
}

function makeResultRow(kind, text, name, recognize, action, sourceRoi, selected, fromLines) {
  return {
    kind: kind || 'ocr_line',
    text: text || name || '',
    name: name || text || '识别结果',
    recognize: { ...recognize },
    action: action ? { ...action } : { ...recognize },
    sourceRoi: sourceRoi ? { ...sourceRoi } : null,
    selected: selected !== false && candidateNameValid(name || text || ''),
    fromLines: !!fromLines,
  };
}

function replaceResults(rows, sourceLabel) {
  saveCandidates = rows || [];
  activeCandidate = -1;
  pendingActionAsset = null;
  renderCandidateList(sourceLabel);
  updateSaveButtonState();
}

function updateSaveButtonState() {
  if (!btnSaveAsset) return;
  // Having result rows is enough to make the button clickable; "none checked"
  // is a user prompt, not a disabled state.
  btnSaveAsset.disabled = saveCandidates.length === 0;
}

function renderOcrLines(lines, roi, sourceLabel) {
  lastLinesRoi = roi;
  if (!linesList) return;
  // v1.2 multi-select candidates. Every recognized line starts with:
  //   recognize = the OCR line bbox
  //   action    = the same rect (default operation region)
  // Both are absolute frame coords; action can be adjusted on the canvas.
  replaceResults(
    lines.map((b) => {
      const rec = { x: b.box[0], y: b.box[1], w: b.box[2], h: b.box[3] };
      return makeResultRow(
        'ocr_line',
        b.text,
        b.text,
        rec,
        { ...rec },
        { x: roi.x, y: roi.y, w: roi.w, h: roi.h },
        candidateNameValid(b.text),
        true
      );
    }),
    sourceLabel
  );
}

function candidateNameValid(name) {
  return /^[A-Za-z_\u4e00-\u9fa5][A-Za-z0-9_\u4e00-\u9fa5]*$/.test((name || '').trim());
}

function normalizeOcrText(s) {
  return String(s || '').replace(/[\s，。！？、：；,.!?:;'"“”‘’（）()\[\]【】{}<>]/g, '').toLowerCase();
}

// Detailed match result: exact / normalized / fuzzy (one CJK substitution).
// `ocrTextMatches` stays as the boolean convenience wrapper.
function ocrTextMatchInfo(expected, actual) {
  const rawExpected = String(expected ?? '');
  const rawActual = String(actual ?? '');
  const a = normalizeOcrText(rawExpected);
  const b = normalizeOcrText(rawActual);
  const out = {
    expected: rawExpected,
    actual: rawActual,
    normExpected: a,
    normActual: b,
    match: false,
    mode: 'none',
    distance: null,
  };
  if (rawExpected.trim() === rawActual.trim()) {
    out.match = true;
    out.mode = 'exact';
    out.distance = 0;
    return out;
  }
  if (a === b) {
    out.match = true;
    out.mode = 'normalized';
    out.distance = 0;
    return out;
  }
  if (a.length < 2 || b.length < 2 || Math.abs(a.length - b.length) > 1) return out;
  const dp = Array.from({ length: a.length + 1 }, () => new Array(b.length + 1).fill(0));
  for (let i = 0; i <= a.length; i++) dp[i][0] = i;
  for (let j = 0; j <= b.length; j++) dp[0][j] = j;
  for (let i = 1; i <= a.length; i++) {
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      dp[i][j] = Math.min(dp[i - 1][j] + 1, dp[i][j - 1] + 1, dp[i - 1][j - 1] + cost);
    }
  }
  out.distance = dp[a.length][b.length];
  if (out.distance <= 1) {
    out.match = true;
    out.mode = 'fuzzy';
  }
  return out;
}

function ocrTextMatches(expected, actual) {
  return ocrTextMatchInfo(expected, actual).match;
}

function candidateStatusText(sourceLabel) {
  if (!batchStatus) return;
  const sel = saveCandidates.filter((c) => c.selected);
  const invalid = sel.filter((c) => !candidateNameValid(c.name));
  if (!saveCandidates.length) {
    batchStatus.textContent = '—';
  } else if (invalid.length) {
    batchStatus.textContent = `${sel.length} 个选中；${invalid.length} 个名称非法（需字母/中文/数字/下划线，不能以数字开头）`;
    batchStatus.style.color = '#ff8a80';
  } else {
    batchStatus.textContent = `${sel.length} 个选中 / 共 ${saveCandidates.length} 行 · ${sourceLabel || ''}`;
    batchStatus.style.color = '';
  }
  if (btnBatchSave) btnBatchSave.disabled = !sel.length || invalid.length > 0;
  updateSaveButtonState();
}

function renderCandidateList(sourceLabel) {
  if (!linesList) return;
  linesList.textContent = '';
  const head = document.createElement('div');
  head.className = 'muted small candidate-select-all';
  const selectAll = document.createElement('input');
  selectAll.type = 'checkbox';
  const selectedCount = saveCandidates.filter((c) => c.selected).length;
  selectAll.checked = saveCandidates.length > 0 && selectedCount === saveCandidates.length;
  selectAll.indeterminate = selectedCount > 0 && selectedCount < saveCandidates.length;
  selectAll.title = '全选 / 全不选';
  selectAll.addEventListener('change', () => {
    saveCandidates.forEach((c) => { c.selected = selectAll.checked; });
    renderCandidateList(sourceLabel);
    draw();
  });
  const headText = document.createElement('span');
  headText.textContent =
    `全选 · ${saveCandidates.length} 项 · ${sourceLabel || '识别结果'} · 勾选后点「保存素材」`;
  head.append(selectAll, headText);
  linesList.appendChild(head);
  if (!saveCandidates.length) {
    const empty = document.createElement('div');
    empty.className = 'muted small';
    empty.textContent = '（没有识别结果）';
    linesList.appendChild(empty);
    batchStatus.textContent = '—';
    if (btnBatchSave) btnBatchSave.disabled = true;
    updateSaveButtonState();
    return;
  }
  saveCandidates.forEach((c, idx) => {
    const row = document.createElement('div');
    row.className = 'ocr-line-row';
    const headRow = document.createElement('div');
    headRow.className = 'candidate-head';
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.checked = c.selected;
    cb.title = '勾选后批量保存';
    cb.addEventListener('change', () => {
      c.selected = cb.checked;
      candidateStatusText(sourceLabel);
    });
    const name = document.createElement('input');
    name.className = 'candidate-name';
    name.value = c.name;
    name.placeholder = '素材名称（全局唯一）';
    name.addEventListener('input', () => {
      c.name = name.value;
      name.classList.toggle('invalid', !candidateNameValid(c.name) && c.selected);
      candidateStatusText(sourceLabel);
    });
    name.classList.toggle('invalid', !candidateNameValid(c.name));
    headRow.append(cb, name);

    const info = document.createElement('div');
    info.className = 'candidate-info muted small';
    const r = c.recognize;
    const a = c.action;
    info.textContent =
      `识别 [${r.x},${r.y},${r.w},${r.h}] · 操作 [${a.x},${a.y},${a.w},${a.h}]`;

    const actions = document.createElement('div');
    actions.className = 'candidate-actions';
    const adjust = document.createElement('button');
    adjust.type = 'button';
    adjust.textContent = '调操作区域';
    adjust.title = '在画布上拖拽画出该素材的操作区域（默认=识别区域）';
    adjust.addEventListener('click', () => {
      activeCandidate = idx;
      roiBack = { ...c.action };
      backMode = true;
      pendingActionAsset = null;
      if (btnBackMode) btnBackMode.textContent = '退出操作区域编辑（拖拽画操作区域）';
      updateRoiLabel();
      draw();
      log(`编辑「${c.name || c.text}」的操作区域：在画布上拖拽`);
    });
    const reset = document.createElement('button');
    reset.type = 'button';
    reset.textContent = '重置';
    reset.title = '操作区域恢复为识别区域';
    reset.addEventListener('click', () => {
      c.action = { ...c.recognize };
      if (activeCandidate === idx) roiBack = { ...c.action };
      renderCandidateList(sourceLabel);
      draw();
    });
    const gen = document.createElement('button');
    gen.type = 'button';
    gen.textContent = '生成步骤';
    gen.title = '生成 click_target（该行识别区域+文字）';
    gen.addEventListener('click', () => {
      const rr = lastLinesRoi || roi;
      const step = inlineOcrClickStep(c.text, rr);
      resultEl.textContent = JSON.stringify(step, null, 2);
      log(`生成 OCR 点击步骤：「${c.text}」`);
    });
    const append = document.createElement('button');
    append.type = 'button';
    append.textContent = '复制步骤';
    append.title = '复制该行点击步骤到剪贴板（到任务构建使用）';
    append.addEventListener('click', () => {
      const rr = lastLinesRoi || roi;
      appendStepToFlow(inlineOcrClickStep(c.text, rr), 'OCR 点击');
    });
    actions.append(adjust, reset, gen, append);
    row.append(headRow, info, actions);
    linesList.appendChild(row);
  });
  candidateStatusText(sourceLabel);
}

async function saveSelectedCandidates() {
  const task = (taskName.value || '').trim();
  if (!task) {
    resultEl.textContent = '请先选择/新建任务';
    return;
  }
  const chosen = saveCandidates.filter((c) => c.selected);
  if (!chosen.length) {
    resultEl.textContent = '请先勾选要保存的素材';
    return;
  }
  const bad = chosen.filter((c) => !candidateNameValid(c.name));
  if (bad.length) {
    resultEl.textContent = '名称非法（需字母/中文开头，仅字母/数字/下划线/中文）：\n' + bad.map((c) => c.name).join('\n');
    return;
  }
  const assets = chosen.map((c) => ({
    id: c.name.trim(),
    kind: 'ocr_line',
    // v1.2.3: single recognition ROI = the multi-line region the user tested.
    roi: [c.sourceRoi.x, c.sourceRoi.y, c.sourceRoi.w, c.sourceRoi.h],
    action_shape: { kind: 'rect', roi: [c.action.x, c.action.y, c.action.w, c.action.h] },
    action_source: 'fixed',
    ocr_mode: 'lines',
    ocr_text: c.text,
  }));
  if (btnBatchSave) btnBatchSave.disabled = true;
  if (batchStatus) batchStatus.textContent = '保存中…';
  try {
    const r = await postJson('/task/save_assets', { task, assets });
    if (!r.ok) {
      if (batchStatus) {
        batchStatus.textContent = '保存失败：' + (r.errors || []).join('; ');
        batchStatus.style.color = '#ff8a80';
      }
      resultEl.textContent = '批量保存失败：\n' + (r.errors || []).join('\n');
      return;
    }
    resultEl.textContent = `已批量保存 ${r.saved.length} 个素材：\n` + r.saved.map((a) => a.id).join('\n');
    log(`批量保存 ${r.saved.length} 个素材`);
    await refreshTaskAssets(task);
  } catch (e) {
    resultEl.textContent = '批量保存异常：' + e;
  } finally {
    candidateStatusText('已保存');
  }
}

// v1.2.3 (R19): per-asset click/drag dry-run tests + step generation.
function actionShapeBBox(shape) {
  if (!shape) return null;
  if (Array.isArray(shape.roi) && shape.roi.length === 4) return shape.roi;
  const pts = shape.points || [];
  if (!pts.length) return null;
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const p of pts) {
    x0 = Math.min(x0, p[0]); y0 = Math.min(y0, p[1]);
    x1 = Math.max(x1, p[0]); y1 = Math.max(y1, p[1]);
  }
  return [x0, y0, x1 - x0 + 1, y1 - y0 + 1];
}

function actionShapeContains(shape, x, y) {
  if (!shape) return true;
  if (shape.kind === 'rect') {
    const r = shape.roi;
    if (!r) return false;
    return x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3];
  }
  const pts = shape.points || [];
  if (pts.length < 3) return false;
  let inside = false;
  let j = pts.length - 1;
  for (let i = 0; i < pts.length; i++) {
    const [xi, yi] = pts[i];
    const [xj, yj] = pts[j];
    if ((yi > y) !== (yj > y) && x < ((xj - xi) * (y - yi)) / (yj - yi || 1) + xi) inside = !inside;
    j = i;
  }
  return inside;
}

function actionShapeEffectiveContains(shape, x, y) {
  if (!shape) return true;
  const inside = actionShapeContains(shape, x, y);
  return shape.invert ? !inside : inside;
}

function currentActionShape() {
  if (actionPoly && actionPolyClosed && actionPoly.length >= 3) {
    return {
      kind: 'polygon',
      points: actionPoly.map((p) => [p.x, p.y]),
      invert: actionInvert,
    };
  }
  if (roiBack && roiBack.w > 0 && roiBack.h > 0) {
    return {
      kind: 'rect',
      roi: [roiBack.x, roiBack.y, roiBack.w, roiBack.h],
      invert: actionInvert,
    };
  }
  return null;
}

function loadActionShapeForEdit(shape) {
  actionPoly = null;
  actionPolyClosed = false;
  actionInvert = !!(shape && shape.invert);
  if (shape && shape.kind === 'polygon' && Array.isArray(shape.points)) {
    actionPoly = shape.points.map((p) => ({ x: p[0], y: p[1] }));
    actionPolyClosed = actionPoly.length >= 3;
    roiBack = actionShapeBBox(shape)
      ? { x: actionShapeBBox(shape)[0], y: actionShapeBBox(shape)[1], w: actionShapeBBox(shape)[2], h: actionShapeBBox(shape)[3] }
      : null;
  } else if (shape && shape.kind === 'rect' && Array.isArray(shape.roi)) {
    roiBack = { x: shape.roi[0], y: shape.roi[1], w: shape.roi[2], h: shape.roi[3] };
  }
  if (btnActionInvert) btnActionInvert.textContent = `操作区域反选：${actionInvert ? '开' : '关'}`;
}

function rectsOverlap(a, b) {
  return a[0] < b[0] + b[2] && a[0] + a[2] > b[0] && a[1] < b[1] + b[3] && a[1] + a[3] > b[1];
}

function actionValidity(it, recognizedBox) {
  if (!recognizedBox) return { valid: null, text: '无法计算：没有真实识别框' };
  const shape = it.action_shape;
  const source = it.action_source || 'recognition';
  if (!shape) {
    if (source === 'recognition') return { valid: true, text: '有效（操作区=真实识别框）' };
    return { valid: false, text: '无效：action_source=fixed 但没有 action_shape' };
  }
  let effective = [0, 0, canvas.width || 1280, canvas.height || 720];
  if (shape.kind === 'rect' && Array.isArray(shape.roi)) effective = shape.roi;
  if (!shape.invert) {
    const hit = rectsOverlap(recognizedBox, effective);
    if (source === 'fixed') {
      return { valid: hit, text: hit ? '有效（固定操作区与识别框重叠）' : '独立操作区域（fixed，允许不重叠）', independent: !hit };
    }
    return { valid: hit, text: hit ? '有效（识别框在操作区内）' : '无效：真实识别框与操作区不重叠' };
  }
  // inverted: effective region is screen minus shape; overlap if any corner of
  // the recognized box is outside the shape.
  const corners = [
    [recognizedBox[0], recognizedBox[1]],
    [recognizedBox[0] + recognizedBox[2] - 1, recognizedBox[1]],
    [recognizedBox[0], recognizedBox[1] + recognizedBox[3] - 1],
    [recognizedBox[0] + recognizedBox[2] - 1, recognizedBox[1] + recognizedBox[3] - 1],
  ];
  const outside = corners.some(([x, y]) => !actionShapeContains(shape, x, y));
  if (source === 'fixed') return { valid: outside, text: outside ? '有效（反选固定操作区与识别框相交）' : '独立操作区域（反选；识别框完全在排除区内）', independent: !outside };
  return { valid: outside, text: outside ? '有效（反选操作区与识别框相交）' : '无效：真实识别框完全落在反选排除区内' };
}

function randomPointInAction(it) {
  const shape = it && it.action_shape;
  const fw = canvas.width || 1280;
  const fh = canvas.height || 720;
  if (!shape) {
    const box = (it && it.roi) || [0, 0, fw, fh];
    return randomPointInBox(box, 0.12);
  }
  const bbox = actionShapeBBox(shape);
  for (let attempt = 0; attempt < 4000; attempt++) {
    let x, y;
    if (shape.invert) {
      x = randomInt(0, fw - 1);
      y = randomInt(0, fh - 1);
    } else if (shape.kind === 'rect' && bbox) {
      return randomPointInBox(bbox, 0.12);
    } else if (bbox) {
      x = randomInt(bbox[0], bbox[0] + bbox[2] - 1);
      y = randomInt(bbox[1], bbox[1] + bbox[3] - 1);
    } else {
      break;
    }
    if (actionShapeEffectiveContains(shape, x, y)) return { x, y };
  }
  return { x: Math.floor(fw / 2), y: Math.floor(fh / 2) };
}

function assetActionBox(it) {
  const shape = it.action_shape;
  if (shape) {
    const b = actionShapeBBox(shape);
    if (b) return b;
  }
  return it.roi || null;
}

function boxCenter(box) {
  return { x: box[0] + Math.floor(box[2] / 2), y: box[1] + Math.floor(box[3] / 2) };
}

function randomInt(min, max) {
  return min + Math.floor(Math.random() * Math.max(1, max - min + 1));
}

function randomPointInBox(box, margin = 0.15) {
  const insetX = Math.floor(box[2] * Math.max(0, Math.min(0.45, margin)));
  const insetY = Math.floor(box[3] * Math.max(0, Math.min(0.45, margin)));
  const x0 = box[0] + insetX;
  const y0 = box[1] + insetY;
  const x1 = Math.max(x0, box[0] + box[2] - 1 - insetX);
  const y1 = Math.max(y0, box[1] + box[3] - 1 - insetY);
  return { x: randomInt(x0, x1), y: randomInt(y0, y1) };
}

function testFrameQuery(it) {
  if (livePreview) return '&live=1';
  const fid = Array.isArray(it && it.positions) && it.positions.length ? it.positions[0] : null;
  return fid != null ? `&frame_id=${fid}` : '';
}

async function testAssetClick(it) {
  const task = currentTask();
  const action = assetActionBox(it);
  if (!action) {
    resultEl.textContent = `素材「${it.id}」没有识别/操作区域`;
    return;
  }
  try {
    let point = randomPointInAction(it);
    let detail = '';
    let recognizedBox = null;
    if (it.kind === 'template') {
      const r = await fetch(
        `/task/asset/match?task=${encodeURIComponent(task)}&id=${encodeURIComponent(it.id)}${testFrameQuery(it)}`
      );
      const s = await r.json();
      if (s.error) {
        resultEl.textContent = `点击测试失败：${s.error}`;
        return;
      }
      if (!it.roi) {
        resultEl.textContent = `素材「${it.id}」没有 roi`;
        return;
      }
      point = randomPointInAction(it);
      boxes = [{ text: `点测 ${s.id}`, score: Number(s.score), box: [s.x, s.y, s.w, s.h] }];
      recognizedBox = [s.x, s.y, s.w, s.h];
      draw();
      detail = `匹配得分 ${Number(s.score).toFixed(4)} @ (${s.x},${s.y})\n`;
    } else if (it.kind === 'ocr_line' || it.kind === 'ocr_lines') {
      const derived = it.kind === 'ocr_line' && it.ocr_mode === 'lines' && !!it.ocr_text;
      const src = it.roi;
      const q = `x=${src[0]}&y=${src[1]}&w=${src[2]}&h=${src[3]}` +
        `&task=${encodeURIComponent(task)}&id=${encodeURIComponent(it.id)}` +
        testFrameQuery(it);
      const r = await fetch((it.kind === 'ocr_lines' || derived) ? '/lines?' + q : '/ocr?' + q);
      const s = await r.json();
      if (s.error) {
        resultEl.textContent = `点击测试失败：${s.error}`;
        return;
      }
      detail = (it.kind === 'ocr_lines' || derived)
        ? `多行识别 ${s.lines ? s.lines.length : 0} 行\n`
        : `识别文字 ${JSON.stringify(s.text)} 得分 ${Number(s.score).toFixed(4)}\n`;
      if (Array.isArray(s.lines) && s.lines.length) {
        const line = (it.ocr_text && s.lines.find((l) => (l.text || '').trim() === it.ocr_text.trim())) || s.lines[0];
        if (line && Array.isArray(line.box) && line.box.length === 4) {
          recognizedBox = [src[0] + line.box[0], src[1] + line.box[1], line.box[2], line.box[3]];
          detail += `真实识别框：${JSON.stringify(recognizedBox)}（${JSON.stringify(line.text || '')}）\n`;
        }
      } else if (!derived && it.kind === 'ocr_line') {
        recognizedBox = [src[0], src[1], src[2], src[3]];
        detail += `单行 OCR 无独立 bbox，按识别 ROI 计算：${JSON.stringify(recognizedBox)}\n`;
      }
    }
    const validity = actionValidity(it, recognizedBox);
    if (validity.text) detail += `区域有效性：${validity.text}\n`;
    const frameSource = (typeof s !== 'undefined' && s.frame_source)
      ? s.frame_source
      : (livePreview ? 'live' : (it.positions && it.positions.length ? `position:#${it.positions[0]}` : 'current'));
    detail += `帧来源：${frameSource}\n`;
    resultEl.textContent =
      `点击测试（dry-run，不实际点击）「${it.id}」\n` +
      detail +
      `操作区域 [${action.join(',')}] → 目标点 (${point.x}, ${point.y})\n` +
      '注意：这里只验证定位/目标点；实际点击请在 flow 中运行。';
    log(`素材点击测试（dry-run）：${it.id} → (${point.x},${point.y})`, 'INFO', {
      frame: frameSource,
      kind: it.kind,
      action_box: action,
      point,
      detail,
    });
  } catch (e) {
    resultEl.textContent = '点击测试异常：' + e;
  }
}

function testAssetDrag(it) {
  const action = assetActionBox(it);
  if (!action) {
    resultEl.textContent = `素材「${it.id}」没有识别/操作区域`;
    return;
  }
  const direction = (window.prompt('拖动方向（up/down/left/right）', 'up') || '').trim();
  if (!['up', 'down', 'left', 'right'].includes(direction)) {
    resultEl.textContent = '拖动方向必须是 up/down/left/right';
    return;
  }
  const from = randomPointInAction(it);
  const dist = 150 + Math.floor(Math.random() * 200);
  const lateralMargin = Math.max(4, Math.floor((direction === 'up' || direction === 'down' ? action[2] : action[3]) * 0.3));
  const jitter = randomInt(-lateralMargin, lateralMargin);
  const fw = canvas.width || 1280;
  const fh = canvas.height || 720;
  const to = {
    x: from.x + (direction === 'left' ? -dist : direction === 'right' ? dist : jitter),
    y: from.y + (direction === 'up' ? -dist : direction === 'down' ? dist : jitter),
  };
  to.x = Math.max(0, Math.min(fw - 1, to.x));
  to.y = Math.max(0, Math.min(fh - 1, to.y));
  if (direction === 'up' || direction === 'down') to.x = Math.max(0, Math.min(fw - 1, from.x + jitter));
  if (direction === 'left' || direction === 'right') to.y = Math.max(0, Math.min(fh - 1, from.y + jitter));
  const step = {
    op: 'drag',
    args: {
      from: { kind: 'region', roi: action },
      direction,
      distance: [150, 350],
      duration_ms: [300, 650],
      margin: 0.15,
    },
  };
  resultEl.textContent =
    `拖动测试（dry-run，不实际滑动）「${it.id}」\n` +
    `方向 ${direction} · 起点 (${from.x},${from.y}) → 目标 (${to.x},${to.y})\n\n` +
    JSON.stringify(step, null, 2);
  log(`素材拖动测试（dry-run）：${it.id} ${direction} (${from.x},${from.y})→(${to.x},${to.y})`);
}

async function generateAssetStep(it) {
  let step;
  if (it.kind === 'template') {
    step = { op: 'click_target', args: { find: [{ kind: 'template', sel: it.id }] } };
  } else if (it.kind === 'region') {
    step = { op: 'click_target', args: { find: [{ kind: 'region', sel: it.id }] } };
  } else {
    const text = (it.ocr_text || window.prompt('OCR 文字（用于 ocr_text 定位）', it.id) || '').trim();
    if (!text) {
      resultEl.textContent = '生成步骤取消：OCR 文字不能为空';
      return;
    }
    step = {
      op: 'click_target',
      args: { find: [{ kind: 'ocr_text', sel: it.id, text }] },
    };
  }
  const json = JSON.stringify(step, null, 2);
  resultEl.textContent = `已生成「${it.id}」步骤（复制到剪贴板，到「任务构建」使用）：\n${json}`;
  try {
    await navigator.clipboard.writeText(json);
    log(`已生成素材步骤并复制：${it.id}`);
  } catch (e) {
    log(`已生成素材步骤（复制失败，请手动选中）`);
  }
}

// v1.1: asset-list OCR action — ocr_lines uses /lines (not single-line /ocr);
// v1.2.3: exactly one recognition ROI; legacy source ROI is migrated server-side.
function assetRecognitionBox(it) {
  // v1.2.3: exactly one recognition ROI. Legacy source ROI is migrated by the
  // server at asset-load time, so the editor never picks a second region.
  return it && Array.isArray(it.roi) ? it.roi : null;
}

async function testAssetOcr(it) {
  const rec = assetRecognitionBox(it);
  if (!rec) {
    resultEl.textContent = `素材「${it.id}」没有识别区域（roi）`;
    return;
  }
  const derived = it.kind === 'ocr_line' && it.ocr_mode === 'lines' && !!it.ocr_text;
  const roi = { x: rec[0], y: rec[1], w: rec[2], h: rec[3] };
  const action = assetActionBox(it);
  const task = (taskName.value || '').trim();
  const q = `x=${roi.x}&y=${roi.y}&w=${roi.w}&h=${roi.h}` +
    `&task=${encodeURIComponent(task)}&id=${encodeURIComponent(it.id)}` +
    testFrameQuery(it);
  const frameMeta = currentFrameId
    ? (assetFramesCache.find((f) => f.id === currentFrameId) || {})
    : null;
  const frameLine = currentFrameId
    ? `当前帧 #${currentFrameId}${frameMeta.name ? ` ${frameMeta.name}` : ''}`
    : '当前帧：实时画面';
  const areaLine =
    `识别区域 [${rec.join(',')}]` +
    (action ? `；操作区域 [${action.join(',')}]（仅点击，不参与识别）` : '');
  const withFrame = `${areaLine}\n${frameLine}`;
  resultEl.textContent = `素材「${it.id}」识别中…\n${withFrame}`;
  log(`OCR 测试开始 asset=${it.id}`, 'INFO', {
    frame: currentFrameId ? `#${currentFrameId} ${frameMeta?.name || ''}`.trim() : 'live',
    kind: it.kind,
    mode: it.ocr_mode || 'single',
    roi,
    action,
    expected: it.ocr_text || null,
    task,
  });
  try {
    if (it.kind === 'ocr_lines' || derived) {
      if (!detReady) {
        resultEl.textContent = '多行识别不可用：serve 需 --features ocr 且带 --det-model';
        return;
      }
      const r = await fetch('/lines?' + q);
      const s = await r.json();
      if (s.error) {
        log(`OCR 测试 /lines 失败 asset=${it.id}`, 'ERROR', {
          roi, frame: currentFrameId ? `#${currentFrameId}` : 'live', error: s.error,
        });
        if (await fallbackFullscreenLines(`识别区域错误：${s.error}`)) {
          resultEl.textContent = withFrame + '\n' + resultEl.textContent;
        }
        return;
      }
      const lines = (s.lines || []).map((b) => ({
        ...b,
        box: [roi.x + b.box[0], roi.y + b.box[1], b.box[2], b.box[3]],
      }));
      if (!lines.length) {
        log(`OCR 测试 /lines 无文字 asset=${it.id}`, 'WARN', {
          roi, frame: currentFrameId ? `#${currentFrameId}` : 'live',
        });
        if (await fallbackFullscreenLines('识别区域内没有文字')) {
          resultEl.textContent = withFrame + '\n' + resultEl.textContent;
        }
        return;
      }
      log(`OCR 测试 /lines 返回 asset=${it.id}`, 'INFO', {
        frame: currentFrameId ? `#${currentFrameId}` : 'live',
        roi,
        count: lines.length,
        elapsed_ms: Number(s.elapsed_ms || 0).toFixed(1),
        lines: lines.map((l) => ({ text: l.text, score: Number(l.score), box: l.box })),
      });
      let matched = null;
      let matchInfo = null;
      if (derived) {
        const rank = { exact: 0, normalized: 1, fuzzy: 2 };
        for (const line of lines) {
          const info = ocrTextMatchInfo(it.ocr_text, line.text);
          if (!info.match) continue;
          if (
            !matchInfo ||
            rank[info.mode] < rank[matchInfo.mode] ||
            (info.mode === matchInfo.mode && Number(line.score) > Number(matched.score))
          ) {
            matched = line;
            matchInfo = info;
          }
        }
        if (!matchInfo) {
          const raw = lines
            .map((l) => `${l.text}(${Number(l.score).toFixed(3)})`)
            .join(', ');
          log(`OCR 测试未匹配 asset=${it.id}`, 'WARN', {
            expected: it.ocr_text,
            frame: currentFrameId ? `#${currentFrameId}` : 'live',
            roi,
            lines: lines.map((l) => ({ text: l.text, score: l.score })),
          });
          const reason = `识别区域未找到期望文字 ${JSON.stringify(it.ocr_text)}：${raw}`;
          if (await fallbackFullscreenLines(reason)) {
            resultEl.textContent = withFrame + '\n' + resultEl.textContent;
          } else {
            resultEl.textContent = withFrame + '\n' + reason;
          }
          return;
        }
        log(`OCR 测试匹配 asset=${it.id}`, 'INFO', {
          expected: it.ocr_text,
          actual: matched.text,
          mode: matchInfo.mode,
          distance: matchInfo.distance,
          score: Number(matched.score),
          frame: currentFrameId ? `#${currentFrameId}` : 'live',
          roi,
        });
      }
      resultEl.textContent =
        withFrame + '\n' +
        `${lines.length} 行 / ${Number(s.elapsed_ms || 0).toFixed(0)} ms\n` +
        lines.map((b, i) => `${i + 1}. ${b.text} (${Number(b.score).toFixed(3)})`).join('\n') +
        (derived && matchInfo
          ? `\n期望文字：${JSON.stringify(it.ocr_text)} → 匹配（${matchInfo.mode}, distance=${matchInfo.distance}）`
          : '');
      renderOcrLines(lines, roi, `素材「${it.id}」`);
      log(`素材多行识别「${it.id}」→ ${lines.length} 行`, 'INFO', {
        roi,
        frame: currentFrameId ? `#${currentFrameId}` : 'live',
      });
      return;
    }
    const r = await fetch('/ocr?' + q);
    const s = await r.json();
    if (s.error || !String(s.text || '').trim()) {
      const reason = s.error || '识别区域内没有文字';
      log(`OCR 测试单行失败 asset=${it.id}`, 'WARN', {
        roi, frame: currentFrameId ? `#${currentFrameId}` : 'live',
        text: s.text, score: s.score, error: s.error || '',
      });
      if (await fallbackFullscreenLines(reason)) {
        resultEl.textContent = withFrame + '\n' + resultEl.textContent;
      }
      return;
    }
    log(`OCR 测试单行成功 asset=${it.id}`, 'INFO', {
      roi, frame: currentFrameId ? `#${currentFrameId}` : 'live',
      text: s.text, score: s.score, elapsed_ms: s.elapsed_ms,
    });
    resultEl.textContent =
      withFrame + '\n' +
      `文字 = ${JSON.stringify(s.text)}\n得分 = ${Number(s.score).toFixed(4)}\n` +
      `耗时 = ${Number(s.elapsed_ms).toFixed(1)} ms`;
    log(`素材识别：${it.id} → ${s.text}`, 'INFO', { roi });
  } catch (e) {
    log(`OCR 测试异常 asset=${it.id}`, 'ERROR', { error: String(e) });
    resultEl.textContent = '识别异常：' + e;
  }
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
      setClicksStatus(`已发现 ${runs.length} 条运行记录，选一条后点「叠加本次分布」`);
      // v1.1 P4: default view is aggregated over all runs (no per-run clutter).
      if (btnClicksAggregate) await loadAggregate();
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
  // The distribution overlay is only for the click-distribution panel. Drawing
  // it during normal素材采集 interferes with ROI/frame selection and hides the
  // game screen behind heat colors.
  const clickPanel = document.getElementById('panel-click');
  if (!clickPanel || !clickPanel.classList.contains('active')) return;
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

// v1.1 P4: aggregate view (all/recent/latest, optional task filter). Raw per-run
// JSONL files stay the source of truth; aggregation is computed on demand.
async function loadAggregate() {
  const scope = clickScope ? clickScope.value : 'all';
  const task = clickTaskFilter ? clickTaskFilter.value.trim() : '';
  if (btnClicksAggregate) btnClicksAggregate.disabled = true;
  setClicksStatus('正在聚合运行记录…');
  try {
    const qs = new URLSearchParams({ scope });
    if (task) qs.set('task', task);
    const r = await fetch('/runs/aggregate?' + qs.toString());
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    const s = await r.json();
    if (!s || !s.ok) throw new Error(((s && s.errors) || []).join('; ') || 'ok=false');
    const points = Array.isArray(s.points) ? s.points : [];
    clickData = {
      file: `聚合(${s.scope}${task ? ' · ' + task : ''})`,
      clicks: Number(s.clicks) || points.length,
      distinct: Number(s.distinct) || window.OAS_HEATMAP.countDistinct(points),
      points,
    };
    clickOverlay = null;
    draw();
    updateClickStatsText();
    const files = Array.isArray(s.files) ? s.files : [];
    setClicksStatus(
      `聚合 ${s.runs} 次运行 / ${files.length} 个文件 · clicks=${clickData.clicks} distinct=${clickData.distinct}` +
        `（${points.length} 个点）`
    );
    log(`点击分布聚合：${clickData.file} runs=${s.runs} clicks=${clickData.clicks}`);
  } catch (e) {
    setClicksStatus('聚合失败：' + (e && e.message ? e.message : e));
  } finally {
    if (btnClicksAggregate) btnClicksAggregate.disabled = false;
  }
}

async function deleteRunFiles(files, label) {
  if (!files.length) {
    setClicksStatus('没有可删除的运行记录');
    return;
  }
  if (!window.confirm(`把 ${files.length} 个运行记录移到 runs/_trash？\n\n原始文件不会永久删除。`)) return;
  const r = await postJson('/runs/delete', { files });
  if (!r.ok) {
    window.alert('删除失败：' + (r.errors || []).join('\n'));
    return;
  }
  log(`已移动 ${r.moved.length} 个运行记录到 _trash（${label}）`);
  await refreshRuns();
  if (clickData && String(clickData.file || '').startsWith('聚合')) await loadAggregate();
}

async function deleteSelectedRun() {
  const file = runsSelect ? runsSelect.value : '';
  if (!file) {
    setClicksStatus('请先选择一条运行记录');
    return;
  }
  await deleteRunFiles([file], '选中运行');
}

async function trashAllRuns() {
  const files = runs.map((r) => r.file).filter(Boolean);
  await deleteRunFiles(files, '全部运行');
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
function collectFlowAssetIds(flow) {
  const ids = new Set();
  const textRe = /(?:appear|ocr|ocr_digit|require_template)\(\s*['"]([^'"]+)['"]/g;
  function scan(v, key) {
    if (typeof v === 'string') {
      if (key === 'sel' || key === 'require_template') ids.add(v);
      let m;
      textRe.lastIndex = 0;
      while ((m = textRe.exec(v))) ids.add(m[1]);
      return;
    }
    if (Array.isArray(v)) {
      for (const x of v) scan(x, key);
      return;
    }
    if (v && typeof v === 'object') {
      for (const [k, x] of Object.entries(v)) scan(x, k);
    }
  }
  scan(flow || {}, '');
  return ids;
}

function runAssetTest(it) {
  if (it.kind === 'template') testAssetMatch(it);
  else if (it.kind === 'region') {
    pickAssetRoi(it);
    resultEl.textContent =
      `操作区域「${it.id}」[${(it.roi || []).join(',')}]\n` +
      `流程引用：click_target/find {kind:"region", sel:"${it.id}"}`;
  } else testAssetOcr(it);
}

function renderTaskReferencedAssets() {
  const targets = [taskRefAssets, taskRefAssetsTask].filter(Boolean);
  if (!targets.length) return;
  const ids = collectFlowAssetIds(taskFlowRaw || flowModel || {});
  if (!ids.size) {
    for (const el of targets) el.textContent = '（当前任务 flow 未引用素材）';
    return;
  }
  const rows = (taskAssets || []).filter((a) => ids.has(a.id));
  if (!rows.length) {
    for (const el of targets) el.textContent = '（flow 引用的素材在素材库中未找到）';
    return;
  }
  for (const el of targets) {
    window.OASAssetUI.renderRows(el, rows, {
      task: (taskName.value || '').trim(),
      pick: false,
      manage: false,
      actions: true,
      group: true,
      groupBy: 'category',
      onPick: (it) => pickAssetRoi(it),
      onTest: runAssetTest,
      onClickTest: (it) => testAssetClick(it),
      onDragTest: (it) => testAssetDrag(it),
      onStep: (it) => generateAssetStep(it),
      onCollectionTest: (it) => testAssetCollection(it),
    });
  }
}

async function refreshTaskAssets(task) {
  if (!task) task = (taskName.value || '').trim();
  if (!task) return;
  try {
    const r = await fetch('/task/assets?task=' + encodeURIComponent(task));
    const s = await r.json();
    if (!s.ok) {
      if (assetsList) assetsList.textContent = '读取失败：' + (s.errors || []).join('; ');
      return;
    }
    taskAssets = s.items || [];
    for (const c of s.collections || []) {
      taskAssets.push({
        id: c.id,
        kind: 'template_set',
        members: c.members || [],
        threshold: c.threshold,
        action_shape: c.action_shape,
        action_source: c.action_source,
        category: c.category || '素材集合',
        _collection: true,
      });
    }
    taskAssetsMeta = { task: s.task, design: s.design_resolution, count: s.count };
    refreshAssetCategoryFilter();
    renderAssetList();
    renderTaskReferencedAssets();
    renderAssetPositionInheritPicker();
    // Frame rows show how many current-task assets reference each frame; keep
    // that derived relation in sync after asset CRUD/load.
    if (typeof refreshFrames === 'function') {
      try { await refreshFrames(); } catch (e) { /* frame panel may be hidden */ }
    }
  } catch (e) {
    if (assetsList) assetsList.textContent = '读取异常：' + e;
  }
}

const ASSET_KIND_LABELS = {
  template: '模板图（template）',
  ocr_line: '单行文字（ocr_line）',
  ocr_lines: '多行文字（ocr_lines）',
  region: '操作区域（region）',
  template_set: '素材集合（template_set）',
};

function makeAssetThumb(it) {
  return window.OASAssetUI.makeThumb(it, (taskName.value || '').trim());
}

function assetCategory(it) {
  return ((it && it.category) || '').trim() || '未分类';
}

function refreshAssetCategoryFilter() {
  if (!assetCategoryFilter) return;
  const keep = assetCategoryFilter.value;
  const cats = [...new Set((taskAssets || []).map(assetCategory))].sort((a, b) => {
    if (a === '未分类') return 1;
    if (b === '未分类') return -1;
    return a.localeCompare(b, 'zh-CN');
  });
  assetCategoryFilter.textContent = '';
  const all = document.createElement('option');
  all.value = '';
  all.textContent = '全部分类';
  assetCategoryFilter.appendChild(all);
  for (const cat of cats) {
    const o = document.createElement('option');
    o.value = cat;
    o.textContent = cat;
    assetCategoryFilter.appendChild(o);
  }
  assetCategoryFilter.value = cats.includes(keep) ? keep : '';
}

function renderAssetList() {
  if (!assetsList) return;
  const q = ((assetSearch && assetSearch.value) || '').trim().toLowerCase();
  const cat = assetCategoryFilter ? assetCategoryFilter.value : '';
  const items = (taskAssets || []).filter(
    (it) => (!q || String(it.id).toLowerCase().includes(q)) &&
      (!cat || assetCategory(it) === cat)
  );
  assetsList.textContent = '';
  if (taskAssetsMeta.task) {
    const res = taskAssetsMeta.design ? taskAssetsMeta.design.join('x') : '未设置';
    const head = document.createElement('div');
    head.className = 'muted small';
    head.textContent = `素材库 ${taskAssetsMeta.task} · design ${res} · ${taskAssets.length} 个素材` +
      (cat ? ` · 分类：${cat}` : '');
    assetsList.appendChild(head);
  }
  if (!items.length) {
    const empty = document.createElement('div');
    empty.className = 'muted small';
    empty.textContent = (q || cat) ? '（没有匹配的素材）' : '（任务还没有素材）';
    assetsList.appendChild(empty);
    return;
  }
  window.OASAssetUI.renderRows(assetsList, items, {
    task: (taskName.value || '').trim(),
    pick: false,
    manage: true,
    actions: true,
    group: true,
    groupBy: 'category',
    clear: false,
    emptyText: (q || cat) ? '（没有匹配的素材）' : '（任务还没有素材）',
    onPick: (it) => pickAssetRoi(it),
    onTest: runAssetTest,
    onEdit: (it) => editAsset(it),
    onPosition: (it) => editAssetPositions(it),
    onDelete: (it) => deleteAsset(it),
    onAction: (it) => editAssetActionRoi(it),
    onClickTest: (it) => testAssetClick(it),
    onDragTest: (it) => testAssetDrag(it),
    onStep: (it) => generateAssetStep(it),
    onCollectionTest: (it) => testAssetCollection(it),
    onCollectionDelete: (it) => deleteCollection(it),
  });
  attachAssetDragReorder();
}

function attachAssetDragReorder() {
  if (!assetsList) return;
  let draggedId = '';
  const rows = Array.from(assetsList.querySelectorAll('.asset-row[data-asset-id][draggable="true"]'));
  const itemIds = (taskAssets || [])
    .filter((a) => a.kind !== 'template_set')
    .map((a) => a.id);
  for (const row of rows) {
    row.addEventListener('dragstart', (e) => {
      draggedId = row.dataset.assetId || '';
      row.classList.add('asset-row-dragging');
      if (e.dataTransfer) {
        e.dataTransfer.effectAllowed = 'move';
        e.dataTransfer.setData('text/plain', draggedId);
      }
    });
    row.addEventListener('dragend', () => {
      draggedId = '';
      row.classList.remove('asset-row-dragging');
      for (const r of rows) r.classList.remove('asset-drag-over');
    });
    row.addEventListener('dragover', (e) => {
      if (!draggedId || row.dataset.assetId === draggedId) return;
      e.preventDefault();
      if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
      row.classList.add('asset-drag-over');
    });
    row.addEventListener('dragleave', () => row.classList.remove('asset-drag-over'));
    row.addEventListener('drop', async (e) => {
      e.preventDefault();
      row.classList.remove('asset-drag-over');
      const targetId = row.dataset.assetId || '';
      const fromId = draggedId || (e.dataTransfer ? e.dataTransfer.getData('text/plain') : '');
      if (!fromId || !targetId || fromId === targetId) return;
      const order = itemIds.filter((id) => id !== fromId);
      const at = order.indexOf(targetId);
      if (at < 0) return;
      order.splice(at, 0, fromId);
      await saveAssetOrder(order);
    });
  }
}

async function saveAssetOrder(ids) {
  const task = currentTask();
  const body = { ids };
  if (task) body.task = task;
  const r = await postJson('/task/assets/reorder', body);
  if (!r.ok) {
    resultEl.textContent = '素材排序失败：' + (r.errors || []).join('; ');
    return;
  }
  log(`素材顺序已保存（${r.ids.length} 个）`);
  await refreshTaskAssets(task);
}

async function testAssetCollection(it) {
  const task = (taskName.value || '').trim();
  if (!task || !Array.isArray(it.members) || !it.members.length) {
    resultEl.textContent = `集合「${it.id}」没有成员`;
    return;
  }
  try {
    const r = await fetch(
      `/task/asset/collection/match?task=${encodeURIComponent(task)}&id=${encodeURIComponent(it.id)}`
    );
    const s = await r.json();
    if (!s.ok) {
      resultEl.textContent = `集合「${it.id}」测试失败：` + (s.errors || []).join('; ');
      return;
    }
    resultEl.textContent =
      `素材集合「${it.id}」测试（dry，与 runtime 同源）：\n` +
      `最佳成员：${s.member}\n` +
      `score=${Number(s.score).toFixed(4)} · threshold=${Number(s.threshold).toFixed(3)} · pass=${s.pass}\n` +
      `位置：(${s.x},${s.y})` +
      (s.action_box ? ` · 操作区域[${s.action_box.join(',')}]` : '');
    log(`集合测试「${it.id}」`, 'INFO', {
      frame: currentFrameId ? `#${currentFrameId}` : 'live',
      member: s.member,
      score: Number(s.score),
      threshold: Number(s.threshold),
      pass: s.pass,
      action_box: s.action_box || null,
    });
  } catch (e) {
    resultEl.textContent = `集合「${it.id}」测试异常：${e}`;
  }
}

async function deleteCollection(it) {
  const task = (taskName.value || '').trim();
  if (!task) return;
  if (!window.confirm(`删除素材集合「${it.id}」？\n\nflow 仍引用该集合时会被阻止。`)) return;
  const r = await postJson('/task/asset/collection/delete', { task, id: it.id });
  if (!r.ok) {
    const msg = (r.errors || []).join('\n') || '未知错误';
    resultEl.textContent = `无法删除集合「${it.id}」：\n${msg}`;
    log(`删除集合被阻止：${it.id} — ${msg}`);
    window.alert(`无法删除集合「${it.id}」\n\n${msg}`);
    return;
  }
  log(`已删除素材集合：${it.id}`);
  await refreshTaskAssets(task);
}

async function batchTestAssets() {
  const task = (taskName.value || '').trim();
  resultEl.textContent = livePreview ? '批量实时帧测试中…' : '批量位置帧测试中…';
  try {
    const r = await postJson('/task/assets/batch_test', { task, live: !!livePreview });
    if (!r.ok) {
      resultEl.textContent = '批量帧测试失败：' + (r.errors || []).join('; ');
      return;
    }
    const rows = r.results || [];
    const lines = rows.map((x) => {
      const where = x.frame_source === 'live' ? '实时帧' : (x.frame_id == null ? '无帧' : `#${x.frame_id}`);
      return `${x.pass ? '✅' : '❌'} ${x.id} @ ${where}` +
        (x.score != null ? ` score=${Number(x.score).toFixed(3)}` : '') +
        (x.text ? ` text=${JSON.stringify(x.text)}` : '') +
        (x.error ? ` · ${x.error}` : '');
    });
    resultEl.textContent =
      `批量帧测试：${r.passed}/${r.total} 通过\n` + lines.join('\n');
    log(`批量帧测试：${r.passed}/${r.total}（${livePreview ? '实时帧' : '位置帧'}）`);
  } catch (e) {
    resultEl.textContent = '批量帧测试异常：' + e;
  }
}

async function deleteAsset(it) {
  const task = (taskName.value || '').trim();
  if (!task) return;
  if (!window.confirm(`删除素材「${it.id}」？\n\nflow 仍在引用该素材时会被阻止。`)) return;
  const r = await postJson('/task/asset/delete', { task, id: it.id });
  if (!r.ok) {
    const msg = (r.errors || []).join('\n') || '未知错误';
    resultEl.textContent = `无法删除素材「${it.id}」：\n${msg}`;
    log(`删除素材被阻止：${it.id} — ${msg}`);
    showErrors('删除失败：', r.errors);
    window.alert(`无法删除素材「${it.id}」\n\n${msg}`);
    return;
  }
  log(`已删除素材：${it.id}`);
  await refreshTaskAssets(task);
}

async function editAssetPositions(it) {
  const task = (taskName.value || '').trim();
  if (!task) return;
  if (!assetFramesCache.length) {
    await refreshAssetPositionFrames();
  }
  let chosen = (it.positions || []).slice();
  const overlay = document.createElement('div');
  overlay.className = 'asset-modal-overlay';
  const box = document.createElement('div');
  box.className = 'asset-modal';
  const title = document.createElement('h3');
  title.textContent = `设置素材位置帧：${it.id}`;
  const hint = document.createElement('p');
  hint.className = 'muted small';
  hint.textContent = '搜索分类/帧名编号，点候选加入；也可以从其他素材继承位置帧；保存后会重新读取素材分类。';
  const pickerHost = document.createElement('div');
  const inheritHost = document.createElement('div');
  const list = document.createElement('div');
  list.className = 'asset-list';
  const msg = document.createElement('div');
  msg.className = 'muted small';
  msg.textContent = '—';

  function renderChosen() {
    list.textContent = '';
    if (!chosen.length) {
      list.textContent = '（暂无位置）';
      return;
    }
    for (const id of chosen) {
      const f = assetFramesCache.find((x) => x.id === id);
      const row = document.createElement('div');
      row.className = 'asset-position-row';
      const label = document.createElement('span');
      label.textContent = f ? assetFrameLabel(f) : `#${id}`;
      const del = document.createElement('button');
      del.type = 'button';
      del.className = 'asset-test';
      del.textContent = '删';
      del.addEventListener('click', () => {
        chosen = chosen.filter((x) => x !== id);
        renderChosen();
      });
      row.append(label, del);
      list.appendChild(row);
    }
  }

  const picker = window.OASAssetUI.createSearchPicker({
    items: assetFramesCache,
    placeholder: '搜索分类 / 帧名 / #id，选择位置候选…',
    className: 'asset-position-search',
    menuClass: 'asset-position-menu',
    label: (f) => `${(f.category || '').trim() || '未分类'} / #${f.id} ${f.name || '帧'}`,
    onChoose: (f, input) => {
      if (!chosen.includes(f.id)) chosen.push(f.id);
      input.value = '';
      renderChosen();
      msg.textContent = `已加入 #${f.id}；当前 ${chosen.length} 个位置`;
    },
  });
  pickerHost.appendChild(picker);

  const inheritLabel = document.createElement('div');
  inheritLabel.className = 'muted small';
  inheritLabel.textContent = '从其他素材继承位置帧（只追加）：';
  inheritHost.appendChild(inheritLabel);
  const inheritItems = (taskAssets || [])
    .filter((a) => a.id !== it.id && a.kind !== 'template_set' && Array.isArray(a.positions) && a.positions.length)
    .map((a) => ({
      id: a.id,
      label: `${a.id}（${a.positions.length} 个位置帧）`,
      positions: a.positions.slice(),
    }));
  const inheritPicker = window.OASAssetUI.createSearchPicker({
    items: inheritItems,
    value: '',
    placeholder: '搜索其他素材，选择后追加它的位置帧…',
    label: (item) => item.label,
    onChoose: (item, input) => {
      for (const id of item.positions) {
        if (!chosen.includes(id)) chosen.push(id);
      }
      input.value = '';
      renderChosen();
      msg.textContent = `已继承「${item.id}」的 ${item.positions.length} 个位置帧；当前 ${chosen.length} 个`;
    },
  });
  inheritHost.appendChild(inheritPicker);

  const actions = document.createElement('div');
  actions.className = 'asset-modal-actions';
  const cancel = document.createElement('button');
  cancel.type = 'button';
  cancel.textContent = '取消';
  cancel.addEventListener('click', () => overlay.remove());
  const save = document.createElement('button');
  save.type = 'button';
  save.textContent = '保存位置';
  save.addEventListener('click', async () => {
    save.disabled = true;
    msg.textContent = '保存中…';
    const r = await postJson('/task/asset/positions', { task, id: it.id, positions: chosen });
    if (!r.ok) {
      msg.textContent = '保存失败：' + (r.errors || []).join('; ');
      resultEl.textContent = `设置素材「${it.id}」位置失败：` + (r.errors || []).join('; ');
      save.disabled = false;
      return;
    }
    const text = `素材「${it.id}」位置帧：${chosen.length ? chosen.join(',') : '（无）'}`;
    resultEl.textContent = text;
    log(text);
    overlay.remove();
    await refreshTaskAssets(task);
    await refreshAssetPositionFrames();
  });
  actions.append(cancel, save);
  box.append(title, hint, pickerHost, inheritHost, list, msg, actions);
  overlay.appendChild(box);
  document.body.appendChild(overlay);
  renderChosen();
  picker.input.focus();
}

async function editAsset(it) {
  const task = (taskName.value || '').trim();
  if (!task) return;
  const newId = (window.prompt('素材 id（留空 = 不重命名）', it.id) || '').trim();
  if (newId && newId !== it.id) {
    const r = await postJson('/task/asset/rename', { task, from: it.id, to: newId });
    if (!r.ok) {
      showErrors('重命名失败：', r.errors);
      return;
    }
    log(`素材重命名：${it.id} → ${newId}`);
  }
  if (it.kind === 'template') {
    const t = window.prompt('模板阈值 0~1（留空 = 不改）', String(it.threshold ?? 0.85));
    if (t != null && t.trim() !== '') {
      const tv = Number(t);
      const r = await postJson('/task/asset/update', { task, id: newId || it.id, threshold: tv });
      if (!r.ok) {
        showErrors('修改阈值失败：', r.errors);
        return;
      }
    }
  } else if (roi && window.confirm('用当前画布识别框更新该 OCR 素材的 ROI？')) {
    const r = await postJson('/task/asset/update', {
      task,
      id: newId || it.id,
      roi: [roi.x, roi.y, roi.w, roi.h],
    });
    if (!r.ok) {
      showErrors('更新 ROI 失败：', r.errors);
      return;
    }
  }
  await refreshTaskAssets(task);
}

function editAssetActionRoi(it) {
  if (!it.roi) {
    resultEl.textContent = `素材「${it.id}」没有识别区域`;
    return;
  }
  pendingActionAsset = it.id;
  activeCandidate = -1;
  backMode = true;
  roi = { x: it.roi[0], y: it.roi[1], w: it.roi[2], h: it.roi[3] };
  const actionBox = actionShapeBBox(it.action_shape);
  roiBack = actionBox
    ? { x: actionBox[0], y: actionBox[1], w: actionBox[2], h: actionBox[3] }
    : { ...roi };
  loadActionShapeForEdit(it.action_shape);
  if (!actionBox) roiBack = { ...roi };
  if (btnBackMode) btnBackMode.textContent = '退出操作区域编辑（拖拽画操作区域）';
  updateRoiLabel();
  draw();
  log(`编辑素材「${it.id}」操作区域：在画布上拖拽画框`);
}

async function updateAssetActionRoi(id) {
  const task = (taskName.value || '').trim();
  const shape = currentActionShape();
  if (!task || !shape) {
    pendingActionAsset = null;
    return;
  }
  const r = await postJson('/task/asset/update', {
    task,
    id,
    action_shape: shape,
    action_source: 'fixed',
  });
  pendingActionAsset = null;
  backMode = false;
  if (btnBackMode) btnBackMode.textContent = '绘制操作区域（默认=识别区域）';
  if (!r.ok) {
    showErrors('更新操作区域失败：', r.errors);
    return;
  }
  log(`素材「${id}」操作区域已更新`);
  await refreshTaskAssets(task);
}

function pickAssetRoi(it) {
  if (!it.roi) {
    resultEl.textContent = `素材「${it.id}」没有 roi`;
    return;
  }
  const shapeBox = actionShapeBBox(it.action_shape);
  roi = { x: it.roi[0], y: it.roi[1], w: it.roi[2], h: it.roi[3] };
  roiBack = shapeBox
    ? { x: shapeBox[0], y: shapeBox[1], w: shapeBox[2], h: shapeBox[3] }
    : null;
  loadActionShapeForEdit(it.action_shape);
  polygon = null;
  polyClosed = false;
  updateRoiLabel();
  draw();
  log(`已用素材「${it.id}」设置识别区域${shapeBox ? '（含操作区域）' : ''}`);
}

async function testAssetMatch(it) {
  const task = (taskName.value || '').trim();
  if (!task) return;
  resultEl.textContent = `素材「${it.id}」全屏匹配中…`;
  try {
    const r = await fetch(
      `/task/asset/match?task=${encodeURIComponent(task)}&id=${encodeURIComponent(it.id)}${testFrameQuery(it)}`
    );
    const s = await r.json();
    if (s.error) {
      resultEl.textContent = '测试失败：' + s.error;
      return;
    }
    const pass = s.pass ? '通过' : '未过阈值';
    resultEl.textContent =
      `素材「${s.id}」测试：${pass}\n得分 = ${Number(s.score).toFixed(4)}（阈值 ${Number(s.threshold).toFixed(2)}）\n` +
      `位置 = (${s.x}, ${s.y})  尺寸 = ${s.w}x${s.h}\n耗时 = ${Number(s.elapsed_ms).toFixed(1)} ms\n` +
      `帧来源 = ${s.frame_source || (livePreview ? 'live' : 'current')}`;
    boxes = [{ text: `测试 ${s.id} ${Number(s.score).toFixed(2)}`, score: s.score, box: [s.x, s.y, s.w, s.h] }];
    draw();
    log(`素材测试：${s.id}`, 'INFO', {
      frame: currentFrameId ? `#${currentFrameId}` : 'live',
      score: Number(s.score),
      threshold: Number(s.threshold),
      pass: s.pass,
      point: [s.x, s.y],
      size: [s.w, s.h],
      elapsed_ms: s.elapsed_ms,
    });
  } catch (e) {
    resultEl.textContent = '测试异常：' + e;
  }
}

// Persist the current ROI as an asset in the task bundle. The server crops the
// LAST captured frame and assigns images/aNNN.png; the (Chinese) id never
// becomes a path.
// R23: one save entry for every recognition result kind.
function assetFrameLabel(f) {
  return `#${f.id} ${f.name || '帧'} ${f.width}x${f.height}${f.current ? ' · 当前' : ''}`;
}

function renderPendingPositions() {
  if (!assetPositionList) return;
  assetPositionList.textContent = '';
  // Drop ids that disappeared with a deleted frame.
  const known = new Set(assetFramesCache.map((f) => f.id));
  pendingAssetPositions = pendingAssetPositions.filter((id) => known.has(id));
  if (!pendingAssetPositions.length) {
    assetPositionList.textContent = '（还没有添加位置）';
    return;
  }
  for (const id of pendingAssetPositions) {
    const f = assetFramesCache.find((x) => x.id === id);
    const row = document.createElement('div');
    row.className = 'asset-position-row';
    const label = document.createElement('span');
    label.textContent = f ? assetFrameLabel(f) : `#${id}`;
    const del = document.createElement('button');
    del.type = 'button';
    del.textContent = '删';
    del.className = 'asset-test';
    del.title = '移除该位置';
    del.addEventListener('click', () => {
      pendingAssetPositions = pendingAssetPositions.filter((x) => x !== id);
      renderPendingPositions();
    });
    row.append(label, del);
    assetPositionList.appendChild(row);
  }
}

function addAssetPosition(id) {
  const n = Number(id);
  if (!Number.isInteger(n) || n <= 0) return;
  if (!pendingAssetPositions.includes(n)) pendingAssetPositions.push(n);
  renderPendingPositions();
}

function renderAssetPositionInheritPicker() {
  if (!assetPositionInheritHost || !window.OASAssetUI || !window.OASAssetUI.createSearchPicker) return;
  if (!assetPositionInheritPicker) {
    assetPositionInheritPicker = window.OASAssetUI.createSearchPicker({
      items: [],
      value: '',
      placeholder: '从其他素材继承位置帧…',
      label: (item) => item.label,
      onChoose: (item, input) => {
        const positions = Array.isArray(item.positions) ? item.positions : [];
        for (const id of positions) {
          if (!pendingAssetPositions.includes(id)) pendingAssetPositions.push(id);
        }
        input.value = '';
        renderPendingPositions();
        if (assetPositionStatus) {
          assetPositionStatus.textContent = `已继承「${item.id}」的 ${positions.length} 个位置帧；当前 ${pendingAssetPositions.length} 个`;
        }
      },
    });
    assetPositionInheritHost.textContent = '';
    const label = document.createElement('div');
    label.className = 'muted small';
    label.textContent = '继承其他素材的位置帧（只追加，不会删除已选帧）：';
    assetPositionInheritHost.appendChild(label);
    assetPositionInheritHost.appendChild(assetPositionInheritPicker);
  }
  const items = (taskAssets || [])
    .filter((a) => a.kind !== 'template_set' && Array.isArray(a.positions) && a.positions.length)
    .map((a) => ({
      id: a.id,
      label: `${a.id}（${a.positions.length} 个位置帧）`,
      positions: a.positions.slice(),
    }));
  assetPositionInheritPicker.setItems(items);
}

function renderAssetPositionPicker() {
  if (!assetPositionPicker) return;
  assetPositionPicker.textContent = '';
  const picker = window.OASAssetUI.createSearchPicker({
    items: assetFramesCache,
    placeholder: '搜索分类 / 帧名 / #id，选择候选位置…',
    className: 'asset-position-search',
    menuClass: 'asset-position-menu',
    label: (f) => `${(f.category || '').trim() || '未分类'} / #${f.id} ${f.name || '帧'}`,
    onChoose: (f, input) => {
      addAssetPosition(f.id);
      input.value = '';
      input.focus();
    },
  });
  assetPositionPicker.appendChild(picker);
  renderAssetPositionInheritPicker();
}

async function refreshAssetPositionFrames() {
  if (!assetPositionPicker) return;
  try {
    const r = await fetch('/frames');
    const s = await r.json();
    assetFramesCache = s.frames || [];
    renderAssetPositionPicker();
    renderPendingPositions();
    if (assetPositionStatus) {
      assetPositionStatus.textContent = assetFramesCache.length
        ? `共 ${assetFramesCache.length} 帧；搜索并选择候选即可添加位置（可多个）`
        : '（还没有帧：先到「图片」面板截图/上传，才能保存素材）';
    }
  } catch (e) {
    if (assetPositionStatus) assetPositionStatus.textContent = '帧列表读取失败：' + e;
  }
}

function selectedAssetPositions() {
  const chosen = pendingAssetPositions.slice();
  // First save convenience: attach the current frame automatically. After the
  // asset exists, its positions are explicit and thumbnails only use those.
  if (!chosen.length && currentFrameId != null) return [currentFrameId];
  return chosen;
}

async function saveAssetUnified() {
  const task = currentTask();
  if (!task) {
    resultEl.textContent = '请先选择/新建任务';
    return;
  }
  const chosen = saveCandidates.filter((c) => c.selected);
  if (!chosen.length) {
    resultEl.textContent = '请先在「识别结果」中勾选要保存的项';
    window.alert('请先勾选要保存的识别结果。');
    return;
  }
  const bad = chosen.filter((c) => !candidateNameValid(c.name));
  if (bad.length) {
    resultEl.textContent = '名称非法（需字母/中文/数字/下划线，不能以数字开头）：\n' +
      bad.map((c) => c.name).join('\n');
    return;
  }
  const ocrRows = chosen.filter((c) => c.kind === 'ocr_line' || c.kind === 'ocr_lines');
  const otherRows = chosen.filter((c) => c.kind !== 'ocr_line' && c.kind !== 'ocr_lines');
  const positions = selectedAssetPositions();
  if (!positions.length) {
    resultEl.textContent = '请先选择素材位置帧（可多选；没有帧就先到「图片」面板截图/上传）';
    window.alert('添加素材必须选择至少一个位置帧。');
    return;
  }
  const errors = [];
  let saved = 0;
  try {
    if (ocrRows.length) {
      const assets = ocrRows.map((c) => {
        // v1.2.3: one recognition ROI. For a line saved from multi-line OCR the
        // recognition ROI is the multi-line region the user actually tested;
        // action_roi stays the tight line box used for clicking.
        const rec = (c.fromLines && c.sourceRoi) ? c.sourceRoi : c.recognize;
        const row = {
          id: c.name.trim(),
          kind: 'ocr_line',
          roi: [rec.x, rec.y, rec.w, rec.h],
          action_shape: { kind: 'rect', roi: [c.action.x, c.action.y, c.action.w, c.action.h] },
          action_source: 'fixed',
          positions,
        };
        if (c.fromLines && c.sourceRoi) {
          row.ocr_mode = 'lines';
          row.ocr_text = c.text;
        }
        return row;
      });
      const r = await postJson('/task/save_assets', { task, assets });
      if (!r.ok) {
        errors.push(...(r.errors || ['批量保存失败']));
      } else {
        saved += (r.saved || []).length;
      }
    }
    for (const c of otherRows) {
      const payload = {
        task,
        id: c.name.trim(),
        kind: c.kind,
        roi: [c.recognize.x, c.recognize.y, c.recognize.w, c.recognize.h],
        positions,
      };
      if (c.kind === 'template') payload.threshold = 0.85;
      if (c.kind !== 'region' && c.action) {
        payload.action_shape = { kind: 'rect', roi: [c.action.x, c.action.y, c.action.w, c.action.h] };
        payload.action_source = 'fixed';
      }
      const r = await postJson('/task/save_asset', payload);
      if (!r.ok) {
        errors.push(...(r.errors || [`保存 ${c.name} 失败`]));
      } else {
        saved += 1;
      }
    }
  } catch (e) {
    errors.push(String(e));
  }
  if (errors.length) {
    resultEl.textContent = `保存结果：成功 ${saved}，失败 ${errors.length}\n` + errors.join('\n');
    return;
  }
  resultEl.textContent = `已保存 ${saved} 个素材`;
  log(`统一保存素材：${saved} 个`);
  replaceResults([], '');
  await refreshTaskAssets(task);
  pendingAssetPositions = [];
  await refreshAssetPositionFrames();
  renderPendingPositions();
}


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
  // v1.2: 显式操作区域（action_roi）；不设时运行期用识别结果。
  if (kind !== 'region') {
    const shape = currentActionShape();
    if (shape) {
      payload.action_shape = shape;
      payload.action_source = 'fixed';
    }
  }
  // §22.10: a closed polygon is an identification mask (match/OCR); the click
  // area stays the matched bbox unless roi_back is set above.
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
  if (!flowJson || !btnSaveFlow) return;
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
  const labels = (window.OAS_FLOW && window.OAS_FLOW.OP_LABELS) || {};
  for (const op of ops) {
    const o = document.createElement('option');
    o.value = op;
    o.textContent = `${labels[op] || op}（${op}）`;
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
    const labels = (window.OAS_FLOW && window.OAS_FLOW.OP_LABELS) || {};
    op.textContent = step.op;
    op.title = labels[step.op] ? `${labels[step.op]}（${step.op}）` : step.op;
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
      taskFlowRaw = null;
      flowModel = { name: task, steps: [], interrupts: [] };
      renderTaskReferencedAssets();
      return;
    }
    taskFlowRaw = s.flow;
    if (flowJson) flowJson.value = JSON.stringify(s.flow, null, 2);
    try {
      flowModel = window.OAS_FLOW ? window.OAS_FLOW.parseFlow(JSON.stringify(s.flow)) : flowModel;
      flowModel.name = s.flow.name || task;
      flowRender();
      if (flowModel.nested) {
        log(`已载入任务「${task}」的 flow：含 ${flowModel.nestedBlocks.length} 个 if/loop/while 嵌套块，简化流程面板只读；请用任务构建或 JSON 文本编辑`, 'INFO', {
          nested_blocks: flowModel.nestedBlocks,
          steps: flowModel.steps.length,
        });
      } else {
        log(`已载入任务「${task}」的 flow（${flowModel.steps.length} 步）`);
      }
    } catch (e) {
      // Nested control-flow is edited in builder/JSON; the workbench still
      // needs the raw flow for referenced-asset display.
      log('流程可视化解析失败（仍显示引用素材）：' + e.message, 'ERROR');
    }
    renderTaskReferencedAssets();
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

async function loadLibrary() {
  if (!librarySelect) return;
  try {
    const r = await fetch('/library');
    const s = await r.json();
    if (!s.ok) {
      libraryStatus.textContent = '素材库读取失败：' + (s.errors || []).join('; ');
      return;
    }
    const items = s.items || [];
    librarySelect.textContent = '';
    const opt0 = document.createElement('option');
    opt0.value = '';
    opt0.textContent = items.length ? '（选择库素材）' : '（素材库为空）';
    librarySelect.appendChild(opt0);
    for (const it of items) {
      const o = document.createElement('option');
      o.value = it.id;
      o.textContent = `${it.id}（阈值 ${Number(it.threshold ?? 0.85).toFixed(2)}）`;
      librarySelect.appendChild(o);
    }
    libraryStatus.textContent = `素材库：${items.length} 个素材`;
    if (btnLibraryMatch) btnLibraryMatch.disabled = items.length === 0;
  } catch (e) {
    libraryStatus.textContent = '素材库读取异常：' + e;
  }
}

async function saveLibraryAsset() {
  if (!roi) return;
  const id = (libraryId.value || '').trim();
  if (!id) {
    libraryStatus.textContent = '请先填写库素材 id';
    return;
  }
  btnLibrarySave.disabled = true;
  try {
    const r = await fetch('/library/save_asset', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        id,
        roi: [roi.x, roi.y, roi.w, roi.h],
        threshold: 0.85,
      }),
    });
    const s = await r.json();
    if (!s.ok) {
      libraryStatus.textContent = '保存到素材库失败：' + (s.errors || []).join('; ');
      return;
    }
    libraryStatus.textContent = `已存入素材库：${s.asset.id}（rev ${s.asset.rev}）`;
    log(`素材库：已保存「${s.asset.id}」（${roi.w}x${roi.h}）`);
    await loadLibrary();
  } catch (e) {
    libraryStatus.textContent = '保存到素材库异常：' + e;
  } finally {
    btnLibrarySave.disabled = !roi;
  }
}

async function matchLibrary() {
  const id = (librarySelect.value || '').trim();
  if (!id) {
    libraryStatus.textContent = '先选择一个库素材';
    return;
  }
  btnLibraryMatch.disabled = true;
  libraryStatus.textContent = `库素材「${id}」全屏匹配中…`;
  try {
    const r = await fetch('/match?lib=' + encodeURIComponent(id));
    const s = await r.json();
    if (s.error) {
      libraryStatus.textContent = '库素材匹配失败：' + s.error;
      return;
    }
    libraryStatus.textContent =
      `库素材「${id}」匹配：得分 ${Number(s.score).toFixed(4)} @ (${s.x}, ${s.y})` +
      ` · 耗时 ${Number(s.elapsed_ms || 0).toFixed(1)} ms`;
    log(`库素材匹配：${id} → 得分 ${Number(s.score).toFixed(3)} @ (${s.x},${s.y})`);
  } catch (e) {
    libraryStatus.textContent = '库素材匹配异常：' + e;
  } finally {
    btnLibraryMatch.disabled = false;
  }
}

// --- §22.27 会话图片列表（多帧，参考旧 annotator「图片」面板） ---
function ensureFrameCategoryPicker() {
  if (!frameCategoryFilter || frameCategoryPicker) return frameCategoryPicker;
  if (!window.OASAssetUI || !window.OASAssetUI.createSearchPicker) return null;
  frameCategoryPicker = window.OASAssetUI.createSearchPicker({
    items: [],
    value: '全部分类',
    placeholder: '搜索分类…',
    label: (item) => item.label || item.id || '',
    renderOption: (row, item) => {
      if (item.unseen > 0) {
        const badge = document.createElement('span');
        badge.className = 'filter-new-badge';
        badge.textContent = String(item.unseen);
        row.appendChild(badge);
      }
    },
    onChoose: (item, input) => {
      categoryFilterValue = item.id || '';
      input.value = item.label || '全部分类';
      refreshFrames();
    },
  });
  frameCategoryFilter.textContent = '';
  frameCategoryFilter.appendChild(frameCategoryPicker);
  return frameCategoryPicker;
}

function updateFrameCategoryPicker(items) {
  const picker = ensureFrameCategoryPicker();
  if (!picker) return;
  picker.setItems(items);
  const chosen = items.find((it) => it.id === categoryFilterValue) || items[0];
  categoryFilterValue = chosen ? chosen.id : '';
  picker.setValue(chosen ? chosen.label : '全部分类');
}

function renderFrameDetail(f, assetIds) {
  if (!frameDetail) return;
  if (!f) {
    frameDetail.textContent = '（选择一帧查看详情）';
    return;
  }
  const cats = (taskAssets || [])
    .filter((a) => (a.positions || []).includes(f.id))
    .map((a) => a.id);
  const ids = assetIds || cats;
  const when = f.ts_ms ? new Date(Number(f.ts_ms)).toLocaleString() : '—';
  const size = f.bytes ? `${(Number(f.bytes) / 1024).toFixed(1)} KiB` : '—';
  frameDetail.textContent =
    `#${f.id} ${f.name || '帧'}\n` +
    `分类：${(f.category || '').trim() || '未分类'}\n` +
    `尺寸：${f.width || '?'}x${f.height || '?'}\n` +
    `来源：${f.source || '—'}\n` +
    `时间：${when}\n` +
    `大小：${size}\n` +
    `状态：${f.current ? '当前帧' : '非当前'}${f.seen === false ? ' · 未看过' : ''}\n` +
    `关联素材：${ids && ids.length ? ids.join('、') : '（无）'}`;
}

async function refreshFrames() {
  if (!framesList) return;
  try {
    const r = await fetch('/frames');
    const s = await r.json();
    if (!s.ok) {
      framesStatus.textContent = '图片列表读取失败：' + (s.errors || []).join('; ');
      return;
    }
    const frames = s.frames || [];
    framesCache = frames;
    framesList.textContent = '';
    if (!frames.length) {
      framesList.textContent = '（暂无图片，点「截取当前帧入库」或上传）';
      framesStatus.textContent = '—';
      updateFrameCategoryPicker([{ id: '', label: '全部分类', unseen: 0, count: 0 }]);
      if (frameCategorySet) {
        frameCategorySet.textContent = '';
        const o = document.createElement('option');
        o.value = '';
        o.textContent = '（先选择当前帧）';
        frameCategorySet.appendChild(o);
      }
      currentFrameId = null;
      renderFrameDetail(null);
      return;
    }
    const groups = new Map();
    const unseenByCat = new Map();
    const assetsByFrame = new Map();
    for (const a of (taskAssets || [])) {
      const positions = Array.isArray(a.positions) ? a.positions : [];
      for (const fid of positions) {
        if (!assetsByFrame.has(fid)) assetsByFrame.set(fid, []);
        assetsByFrame.get(fid).push(a.id);
      }
    }
    let totalUnseen = 0;
    let currentFrame = null;
    for (const f of frames) {
      if (f.current) currentFrame = f;
      const cat = (f.category || '').trim() || '未分类';
      if (!groups.has(cat)) groups.set(cat, []);
      groups.get(cat).push(f);
      if (f.seen === false) {
        unseenByCat.set(cat, (unseenByCat.get(cat) || 0) + 1);
        totalUnseen += 1;
      }
    }
    currentFrameId = currentFrame ? currentFrame.id : null;
    renderFrameDetail(currentFrame, currentFrame ? assetsByFrame.get(currentFrame.id) : null);
    const orderedCats = Array.from(groups.keys()).sort((a, b) => {
      if (a === '未分类') return 1;
      if (b === '未分类') return -1;
      return 0;
    });
    const filterItems = [
      { id: '', label: `全部分类（${frames.length}）`, unseen: totalUnseen, count: frames.length },
      ...orderedCats.map((cat) => ({
        id: cat,
        label: `${cat}（${(groups.get(cat) || []).length}）`,
        unseen: unseenByCat.get(cat) || 0,
        count: (groups.get(cat) || []).length,
      })),
    ];
    updateFrameCategoryPicker(filterItems);
    if (frameCategorySet) {
      const keep = frameCategorySet.value;
      frameCategorySet.textContent = '';
      const blank = document.createElement('option');
      blank.value = '';
      blank.textContent = currentFrame ? '（未分类）' : '（先选择当前帧）';
      frameCategorySet.appendChild(blank);
      for (const cat of orderedCats) {
        if (cat === '未分类') continue;
        const o = document.createElement('option');
        o.value = cat;
        o.textContent = cat;
        frameCategorySet.appendChild(o);
      }
      const fresh = document.createElement('option');
      fresh.value = '__new__';
      fresh.textContent = '+ 新建分类…';
      frameCategorySet.appendChild(fresh);
      const curCat = currentFrame ? ((currentFrame.category || '').trim() || '') : '';
      frameCategorySet.value = currentFrame && orderedCats.includes(curCat) ? curCat : (keep || '');
    }
    // Only render the selected category, not every group expanded.
    const selectedCat = categoryFilterValue || '';
    const visibleCats = selectedCat ? [selectedCat] : orderedCats;
    for (const cat of visibleCats) {
      if (!groups.has(cat)) continue;
      const group = document.createElement('div');
      group.className = 'frame-group';
      const head = document.createElement('div');
      head.className = 'frame-group-head';
      head.textContent = `${cat}（${groups.get(cat).length}）`;
      head.title = '把帧拖到分组标题上可改分类';
      head.addEventListener('dragover', (e) => e.preventDefault());
      head.addEventListener('drop', (e) => {
        e.preventDefault();
        const id = Number(e.dataTransfer.getData('text/plain'));
        if (id) moveFrameToCategory(id, cat === '未分类' ? '' : cat);
      });
      group.appendChild(head);
      for (const f of groups.get(cat)) {
        const row = document.createElement('div');
        row.className = 'frame-row' + (f.current ? ' current' : '');
        if (f.seen === false) {
          row.classList.add('frame-row-new');
          const newBadge = document.createElement('span');
          newBadge.className = 'frame-new-badge';
          newBadge.textContent = '新';
          newBadge.title = '尚未点击或编辑过该帧';
          row.appendChild(newBadge);
        }
        row.draggable = true;
        row.dataset.id = String(f.id);
        row.addEventListener('dragstart', (e) => {
          e.dataTransfer.setData('text/plain', String(f.id));
          e.dataTransfer.effectAllowed = 'move';
          row.classList.add('dragging');
        });
        row.addEventListener('dragend', () => row.classList.remove('dragging'));
        row.addEventListener('dragover', (e) => e.preventDefault());
        row.addEventListener('drop', (e) => {
          e.preventDefault();
          const dragged = Number(e.dataTransfer.getData('text/plain'));
          if (dragged && dragged !== f.id) {
            moveFrameBefore(dragged, f.id, cat === '未分类' ? '' : cat);
          }
        });
        const linked = assetsByFrame.get(f.id) || [];
        const pick = document.createElement('button');
        pick.type = 'button';
        pick.className = 'frame-pick';
        pick.textContent = `${f.name || '帧'} · #${f.id} ${f.width}x${f.height}${f.current ? ' · 当前' : ''} · 素材 ${linked.length}`;
        pick.title = `来源 ${f.source} · 时间 ${f.ts_ms} · ${f.bytes} 字节 · 关联素材：${linked.join('、') || '无'}`;
        pick.addEventListener('click', () => selectFrame(f.id));
        const rename = document.createElement('button');
        rename.type = 'button';
        rename.className = 'frame-del';
        rename.textContent = '名';
        rename.title = '重命名该帧';
        rename.addEventListener('click', () => renameFrame(f));
        const catBtn = document.createElement('button');
        catBtn.type = 'button';
        catBtn.className = 'frame-del';
        catBtn.textContent = '类';
        catBtn.title = '设置分类（自由文本）';
        catBtn.addEventListener('click', () => setFrameCategory(f));
        const del = document.createElement('button');
        del.type = 'button';
        del.className = 'frame-del';
        del.textContent = '删';
        del.title = '删除该帧';
        del.addEventListener('click', () => deleteFrame(f.id));
        row.append(pick, rename, catBtn, del);
        group.appendChild(row);
      }
      framesList.appendChild(group);
    }
    framesStatus.textContent = `共 ${frames.length} 帧；拖动排序/换分组，点行切换当前帧`;
    await refreshAssetPositionFrames();
  } catch (e) {
    framesStatus.textContent = '图片列表异常：' + e;
  }
}

async function renameFrame(f) {
  const name = window.prompt('帧名称', f.name || `帧 ${f.id}`);
  if (name == null) return;
  const r = await postJson('/frames/rename', { id: f.id, name: name.trim() });
  if (!r.ok) {
    framesStatus.textContent = '重命名失败：' + (r.errors || []).join('; ');
    return;
  }
  await refreshFrames();
}

async function renameFrameCategory() {
  const from = window.prompt('要重命名的分类（整类改名，不能为空）', '');
  if (from == null || !from.trim()) return;
  const to = window.prompt(`把分类「${from.trim()}」改为（不能为空）`, '');
  if (to == null || !to.trim()) {
    framesStatus.textContent = '分类重命名取消：新分类不能为空';
    return;
  }
  const r = await postJson('/frames/category/rename', {
    from: from.trim(),
    to: to.trim(),
  });
  if (!r.ok) {
    framesStatus.textContent = '分类重命名失败：' + (r.errors || []).join('; ');
    return;
  }
  log(`分类重命名：${r.from} → ${r.to}（${r.changed} 帧）`);
  await refreshFrames();
}

async function applyFrameCategoryFromSelect() {
  if (!currentFrameId || !frameCategorySet) {
    framesStatus.textContent = '请先点一帧设为当前帧，再应用分类';
    return;
  }
  let cat = frameCategorySet.value;
  if (cat === '__new__') {
    cat = (window.prompt('新分类名称', '') || '').trim();
    if (!cat) return;
  }
  const r = await postJson('/frames/category', {
    id: currentFrameId,
    category: cat === '（未分类）' ? '' : cat,
  });
  if (!r.ok) {
    framesStatus.textContent = '设置分类失败：' + (r.errors || []).join('; ');
    return;
  }
  await refreshFrames();
}

function relationTaskName() {
  const direct = (taskName && taskName.value || '').trim();
  if (direct) return direct;
  const builder = document.getElementById('builder-task');
  return builder && builder.value ? builder.value.trim() : '';
}

async function loadFrameAssetRelations() {
  if (!relationsView || !relationsDialog) return;
  const task = relationTaskName();
  relationsView.textContent = '';
  if (relationsStatus) relationsStatus.textContent = '加载中…';
  if (!task) {
    if (relationsStatus) relationsStatus.textContent = '请先选择任务（任务页或任务构建页）';
    return;
  }
  try {
    const r = await fetch('/relations/frame-assets?task=' + encodeURIComponent(task));
    const s = await r.json();
    if (!s.ok) {
      if (relationsStatus) relationsStatus.textContent = '加载失败：' + (s.errors || []).join('; ');
      return;
    }
    const rel = s.relations || {};
    lastRelationsSnapshot = rel;
    lastRelationsTask = task;
    if (relationsStatus) {
      relationsStatus.textContent = `${rel.task || task} · ${(rel.frames || []).length} 帧 / ${(rel.assets || []).length} 素材（只读，派生自 Asset.positions）`;
    }

    function section(title) {
      const box = document.createElement('div');
      box.className = 'relations-section';
      const h = document.createElement('div');
      h.className = 'relations-section-title';
      h.textContent = title;
      box.appendChild(h);
      relationsView.appendChild(box);
      return box;
    }
    function line(box, text, cls) {
      const d = document.createElement('div');
      d.className = 'relations-line' + (cls ? ' ' + cls : '');
      d.textContent = text;
      box.appendChild(d);
    }

    const cov = rel.coverage || {};
    const covBox = section('覆盖概览');
    line(covBox, `帧：${cov.frames_with_assets || 0}/${cov.frames_total || 0} 有素材；素材：${cov.assets_with_positions || 0}/${cov.assets_total || 0} 有位置帧；transition：${cov.transitions || 0}；标注：${cov.annotations || 0}`);

    const frameBox = section('帧 → 素材');
    for (const f of (rel.frames || [])) {
      const assets = (f.assets || []).join('、') || '（无）';
      line(frameBox, `#${f.id} ${f.name || '帧'} [${f.category || '未分类'}] → ${assets}`);
    }

    const assetBox = section('素材 → 位置帧');
    for (const a of (rel.assets || [])) {
      const pos = (a.positions || []).map((id) => '#' + id).join('、') || '（无位置帧）';
      line(assetBox, `${a.id} (${a.kind || '?'}) → ${pos}`);
    }

    const coBox = section('素材 ↔ 素材共现');
    if (!(rel.asset_cooccurrence || []).length) line(coBox, '（暂无可共现素材）', 'muted');
    for (const p of (rel.asset_cooccurrence || [])) {
      const frames = (p.frames || []).map((id) => '#' + id).join('、');
      line(coBox, `${p.a} ↔ ${p.b} · ${p.count} 次 · ${frames}`);
    }

    const sharedBox = section('帧 ↔ 帧共享素材');
    if (!(rel.frame_shared_assets || []).length) line(sharedBox, '（暂无共享素材的帧对）', 'muted');
    for (const p of (rel.frame_shared_assets || [])) {
      line(sharedBox, `#${p.a} ↔ #${p.b} · 共享 ${p.assets.length} 个：${(p.assets || []).join('、')}`);
    }

    const transBox = section('帧 → 帧 transition（录制证据）');
    const knownFrames = new Set((rel.frames || []).map((f) => f.id));
    if (!(rel.transitions || []).length) line(transBox, '（尚无录制证据）', 'muted');
    for (const t of (rel.transitions || [])) {
      const task = t.task ? ` · ${t.task}` : '';
      const asset = t.asset || '?';
      const missing = !knownFrames.has(t.from_frame) || !knownFrames.has(t.to_frame);
      line(
        transBox,
        `#${t.from_frame} → #${t.to_frame} · 点击 ${asset}${task}${missing ? ' · 帧缺失（历史证据）' : ''}`
      );
    }

    const annBox = section('人工标注');
    if (!(rel.annotations || []).length) line(annBox, '（暂无标注）', 'muted');
    for (const ann of (rel.annotations || [])) {
      const row = document.createElement('div');
      row.className = 'relations-line annotation-row';
      const text = document.createElement('span');
      text.textContent = `${ann.type} · ${ann.a} ↔ ${ann.b}${ann.note ? ' · ' + ann.note : ''}`;
      const del = document.createElement('button');
      del.type = 'button';
      del.className = 'asset-test';
      del.textContent = '删除';
      del.addEventListener('click', () => deleteRelationAnnotation(ann.id));
      row.append(text, del);
      annBox.appendChild(row);
    }
  } catch (e) {
    if (relationsStatus) relationsStatus.textContent = '加载异常：' + e;
  }
}

async function openFrameAssetRelations() {
  if (!relationsDialog) return;
  await loadFrameAssetRelations();
  relationsDialog.showModal();
}

function relationAssetScope(rel, task) {
  if (rel && rel.task === '（全局素材库）') return 'global';
  return task || lastRelationsTask || '';
}

function fillAnnotationOptions() {
  if (!annotationA || !annotationB || !lastRelationsSnapshot) return;
  const rel = lastRelationsSnapshot;
  const scope = relationAssetScope(rel, lastRelationsTask);
  const kind = annotationType ? annotationType.value : 'alternative';
  const frames = (rel.frames || []).map((f) => ({
    id: `frame:${f.id}`,
    label: `帧 #${f.id} ${f.name || ''}`,
  }));
  const assets = (rel.assets || []).map((a) => ({
    id: `asset:${scope}/${a.id}`,
    label: `素材 ${a.id}`,
  }));
  const opts = (kind === 'similar' || kind === 'overlay') ? frames : [...frames, ...assets];
  for (const sel of [annotationA, annotationB]) {
    sel.textContent = '';
    for (const o of opts) {
      const el = document.createElement('option');
      el.value = o.id;
      el.textContent = o.label;
      sel.appendChild(el);
    }
    if (opts.length > 1 && sel === annotationB) sel.selectedIndex = 1;
  }
}

async function openAnnotationDialog() {
  if (!annotationDialog) return;
  if (!lastRelationsSnapshot) await loadFrameAssetRelations();
  if (!lastRelationsSnapshot) return;
  if (annotationError) annotationError.textContent = '';
  if (annotationNote) annotationNote.value = '';
  fillAnnotationOptions();
  annotationDialog.showModal();
}

async function submitRelationAnnotation() {
  if (!annotationDialog || !annotationA || !annotationB) return;
  const task = lastRelationsTask || relationTaskName();
  if (!task && lastRelationsSnapshot?.task !== '（全局素材库）') {
    if (annotationError) annotationError.textContent = '缺少任务上下文';
    return;
  }
  const payload = {
    task,
    kind: annotationType ? annotationType.value : 'alternative',
    a: annotationA.value,
    b: annotationB.value,
    note: annotationNote ? annotationNote.value.trim() : '',
  };
  const btn = document.getElementById('annotation-submit');
  if (btn) btn.disabled = true;
  try {
    const r = await postJson('/relations/annotate', payload);
    if (!r.ok) {
      if (annotationError) annotationError.textContent = '保存失败：' + (r.errors || []).join('; ');
      return;
    }
    annotationDialog.close();
    await loadFrameAssetRelations();
  } catch (e) {
    if (annotationError) annotationError.textContent = '保存异常：' + e;
  } finally {
    if (btn) btn.disabled = false;
  }
}

async function deleteRelationAnnotation(id) {
  if (!id) return;
  try {
    const r = await postJson('/relations/annotate/delete', { id });
    if (!r.ok) {
      if (relationsStatus) relationsStatus.textContent = '删除标注失败：' + (r.errors || []).join('; ');
      return;
    }
    await loadFrameAssetRelations();
  } catch (e) {
    if (relationsStatus) relationsStatus.textContent = '删除标注异常：' + e;
  }
}

function ensureRelationRecordPicker() {
  if (!relationRecordAssetHost || !window.OASAssetUI || !window.OASAssetUI.createSearchPicker) return;
  if (!relationRecordAssetPicker) {
    relationRecordAssetPicker = window.OASAssetUI.createSearchPicker({
      items: [],
      value: '',
      placeholder: '搜索/选择要点击的素材…',
      label: (item) => item.label,
      onChoose: (item, input) => {
        relationRecordAssetId = item.id || '';
        input.value = item.label;
      },
    });
    relationRecordAssetHost.textContent = '';
    relationRecordAssetHost.appendChild(relationRecordAssetPicker);
  }
}

async function openRelationRecordDialog() {
  if (!relationRecordDialog) return;
  const task = relationTaskName();
  if (!task) {
    window.alert('请先选择任务');
    return;
  }
  await refreshTaskAssets(task);
  ensureRelationRecordPicker();
  const items = (taskAssets || [])
    .filter((a) => a.kind !== 'template_set')
    .map((a) => ({ id: a.id, label: `${a.id} (${a.kind || '?'})` }));
  relationRecordAssetPicker.setItems(items);
  relationRecordAssetId = items.length ? items[0].id : '';
  relationRecordAssetPicker.setValue(relationRecordAssetId);
  if (relationRecordTask) relationRecordTask.textContent = `任务：${task} · 先停留在动作前的画面`;
  if (relationRecordResult) relationRecordResult.textContent = '';
  relationRecordDialog.showModal();
}

async function submitRelationRecord() {
  if (!relationRecordSubmit || relationRecordSubmit.disabled) return;
  const task = relationTaskName();
  if (!task || !relationRecordAssetId) {
    if (relationRecordResult) relationRecordResult.textContent = '缺少任务或动作素材';
    return;
  }
  const wait = Number(document.getElementById('relation-record-wait')?.value || 900);
  relationRecordSubmit.disabled = true;
  if (relationRecordResult) relationRecordResult.textContent = '录制中：抓前帧 → 点击 → 等待 → 抓后帧…';
  try {
    const r = await postJson('/relations/record', {
      task,
      asset: relationRecordAssetId,
      wait_ms: Number.isFinite(wait) ? wait : 900,
    });
    if (!r.ok) {
      if (relationRecordResult) relationRecordResult.textContent = '录制失败：' + (r.errors || []).join('; ');
      return;
    }
    const obs = r.observation || {};
    if (relationRecordResult) {
      relationRecordResult.textContent =
        `已录制：#${obs.from_frame} → #${obs.to_frame} · 点击 ${obs.asset}` +
        `（${r.before?.name || ''} → ${r.after?.name || ''}）`;
    }
    await refreshFrames();
    if (relationsDialog && relationsDialog.open) await loadFrameAssetRelations();
  } catch (e) {
    if (relationRecordResult) relationRecordResult.textContent = '录制异常：' + e;
  } finally {
    relationRecordSubmit.disabled = false;
  }
}

async function setFrameCategory(f) {
  const cat = window.prompt('分类（自由文本，留空=未分类）', f.category || '');
  if (cat == null) return;
  const r = await postJson('/frames/category', { id: f.id, category: cat.trim() });
  if (!r.ok) {
    framesStatus.textContent = '设置分类失败：' + (r.errors || []).join('; ');
    return;
  }
  await refreshFrames();
}

async function moveFrameToCategory(id, category) {
  const r = await postJson('/frames/category', { id, category });
  if (!r.ok) {
    framesStatus.textContent = '改分类失败：' + (r.errors || []).join('; ');
    return;
  }
  await refreshFrames();
}

async function moveFrameBefore(dragged, target, category) {
  // Update category first (if dropped into another group), then reorder.
  const cat = await postJson('/frames/category', { id: dragged, category });
  if (!cat.ok) {
    framesStatus.textContent = '改分类失败：' + (cat.errors || []).join('; ');
    return;
  }
  const s = await (await fetch('/frames')).json();
  const ids = (s.frames || []).map((f) => f.id);
  const from = ids.indexOf(dragged);
  const to = ids.indexOf(target);
  if (from >= 0 && to >= 0) {
    ids.splice(from, 1);
    const insert = Math.max(0, ids.indexOf(target) >= 0 ? ids.indexOf(target) : to);
    ids.splice(insert, 0, dragged);
  }
  const r = await postJson('/frames/reorder', { ids });
  if (!r.ok) {
    framesStatus.textContent = '排序失败：' + (r.errors || []).join('; ');
    return;
  }
  await refreshFrames();
}

async function selectFrame(id) {
  try {
    pauseLivePreview();
    const r = await fetch('/frames/select', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id }),
    });
    const s = await r.json();
    if (s.error) {
      framesStatus.textContent = '切换失败：' + s.error;
      return;
    }
    await loadFrame();
    await refreshState();
    draw();
    log(`已切换当前帧 #${id}（${s.width}x${s.height}）`);
    await refreshFrames();
  } catch (e) {
    framesStatus.textContent = '切换异常：' + e;
  }
}

async function deleteFrame(id) {
  try {
    const r = await fetch('/frames/delete', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id }),
    });
    const s = await r.json();
    if (!r.ok || s.error) {
      const msg = s.error || (s.errors || []).join('; ') || '未知错误';
      framesStatus.textContent = '删除失败：' + msg;
      window.alert('无法删除该帧：\n' + msg);
      return;
    }
    log(`已删除图片 #${id}（剩 ${s.remaining} 帧）`);
    await refreshFrames();
    const st = await (await fetch('/state')).json();
    if (st.captured) {
      await loadFrame();
      draw();
    }
  } catch (e) {
    framesStatus.textContent = '删除异常：' + e;
  }
}

async function clearFrames() {
  const count = (framesCache || []).length;
  if (!count) {
    framesStatus.textContent = '列表已经是空的';
    return;
  }
  const ok = window.confirm(
    `确认清空全部 ${count} 帧？\n\n会先备份到 frames/.backup/clear-<时间>/，但仍会从当前列表移除。`
  );
  if (!ok) return;
  try {
    const r = await fetch('/frames/clear', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ confirm: true }),
    });
    const s = await r.json();
    if (!s.ok) {
      framesStatus.textContent = '清空失败：' + (s.errors || []).join('; ');
      return;
    }
    const backup = s.backup_dir ? `；备份：${s.backup_dir}` : '';
    log(`已清空会话图片列表 ${s.frames || count} 帧${backup}`);
    await refreshFrames();
  } catch (e) {
    framesStatus.textContent = '清空异常：' + e;
  }
}

// 任意图片 -> PNG 字节（浏览器 canvas 转换；/frames 只接受 PNG）
async function fileToPngBytes(file) {
  const bmp = await createImageBitmap(file);
  const c = document.createElement('canvas');
  c.width = bmp.width;
  c.height = bmp.height;
  c.getContext('2d').drawImage(bmp, 0, 0);
  const blob = await new Promise((res) => c.toBlob(res, 'image/png'));
  return new Uint8Array(await blob.arrayBuffer());
}

async function uploadFrames() {
  pauseLivePreview();
  const files = Array.from(frameFile.files || []);
  if (!files.length) {
    framesStatus.textContent = '先选择图片文件（可多选）';
    return;
  }
  btnFrameUpload.disabled = true;
  framesStatus.textContent = `上传中（${files.length} 张）…`;
  let okCount = 0;
  try {
    for (const file of files) {
      const bytes = await fileToPngBytes(file);
      const r = await fetch('/frames', {
        method: 'POST',
        headers: { 'Content-Type': 'image/png' },
        body: bytes,
      });
      const s = await r.json();
      if (!s.ok) {
        log(`上传失败 ${file.name}：${(s.errors || []).join('; ')}`);
        continue;
      }
      okCount += 1;
      log(`已上传图片 #${s.id}（${s.width}x${s.height}）`);
    }
    if (okCount) {
      await loadFrame();
      await refreshState();
      draw();
    }
    framesStatus.textContent = `已上传 ${okCount}/${files.length} 张`;
    await refreshFrames();
  } catch (e) {
    framesStatus.textContent = '上传异常：' + e;
  } finally {
    btnFrameUpload.disabled = false;
  }
}

function updateLiveButton() {
  if (!btnLivePreview) return;
  btnLivePreview.textContent = livePreview ? '实时：开' : '实时：暂停';
  btnLivePreview.classList.toggle('live-on', livePreview);
  if (hintEl) {
    hintEl.textContent = livePreview
      ? '实时：开 · 画布直接操作设备（单击=点击，拖拽=滑动）；暂停实时可框选 ROI'
      : '实时：暂停 · 画布用于 ROI/多边形编辑；点「实时：开」恢复设备操作';
  }
}

function fitCanvasToStage() {
  if (!canvasStage) return;
  const sw = canvasStage.clientWidth;
  const sh = canvasStage.clientHeight;
  if (sw < 16 || sh < 16) return;
  const ar = (canvas.width || 1280) / (canvas.height || 720);
  let w = sw;
  let h = w / ar;
  if (h > sh) {
    h = sh;
    w = h * ar;
  }
  canvas.style.width = `${Math.max(1, Math.floor(w))}px`;
  canvas.style.height = `${Math.max(1, Math.floor(h))}px`;
}

async function loadFrame(live = false) {
  return new Promise((resolve, reject) => {
    const im = new Image();
    im.onload = () => {
      img = im;
      canvas.width = im.width;
      canvas.height = im.height;
      fitCanvasToStage();
      draw();
      resolve(im);
    };
    im.onerror = () => reject(new Error('帧加载失败'));
    // `live=1` makes serve grab a fresh device frame for this request. The
    // request never appends to the frame gallery; only 截图/入库 does that.
    im.src = live
      ? '/frame.png?live=1&t=' + Date.now()
      : '/frame.png?t=' + Date.now();
  });
}

function applyLiveBlob(blob) {
  if (!livePreview) return;
  const url = URL.createObjectURL(blob);
  const im = new Image();
  im.onload = () => {
    if (!livePreview) {
      URL.revokeObjectURL(url);
      return;
    }
    img = im;
    canvas.width = im.width;
    canvas.height = im.height;
    fitCanvasToStage();
    draw();
    if (liveObjectUrl) URL.revokeObjectURL(liveObjectUrl);
    liveObjectUrl = url;
  };
  im.onerror = () => URL.revokeObjectURL(url);
  im.src = url;
}

function openLiveSocket() {
  if (!livePreview) return;
  if (liveSocket && (
    liveSocket.readyState === WebSocket.OPEN ||
    liveSocket.readyState === WebSocket.CONNECTING
  )) return;
  try {
    const proto = window.location.protocol === 'https:' ? 'wss' : 'ws';
    const ws = new WebSocket(`${proto}://${window.location.host}/ws/live`);
    ws.binaryType = 'blob';
    liveSocket = ws;
    ws.onmessage = (ev) => {
      if (ev.data instanceof Blob) applyLiveBlob(ev.data);
    };
    ws.onerror = () => {
      try { ws.close(); } catch (e) { /* ignore */ }
    };
    ws.onclose = () => {
      if (liveSocket === ws) liveSocket = null;
      if (livePreview && !liveReconnectTimer) {
        liveReconnectTimer = setTimeout(() => {
          liveReconnectTimer = null;
          openLiveSocket();
        }, 1000);
      }
    };
  } catch (e) {
    startLivePollingFallback();
  }
}

function startLivePollingFallback() {
  if (liveTimer) return;
  liveTimer = setInterval(async () => {
    if (!livePreview || liveBusy) return;
    liveBusy = true;
    try {
      await loadFrame(true);
    } catch (e) {
      // device offline; keep polling
    } finally {
      liveBusy = false;
    }
  }, 1200);
}

function startLivePreview() {
  livePreview = true;
  liveInputDrag = null;
  updateLiveButton();
  if (liveTimer) {
    clearInterval(liveTimer);
    liveTimer = null;
  }
  openLiveSocket();
  // If the browser (or an older server) cannot upgrade, use polling fallback.
  setTimeout(() => {
    if (livePreview && !(liveSocket && liveSocket.readyState === WebSocket.OPEN)) {
      startLivePollingFallback();
    }
  }, 1500);
}

function pauseLivePreview() {
  livePreview = false;
  liveInputDrag = null;
  if (liveTimer) {
    clearInterval(liveTimer);
    liveTimer = null;
  }
  if (liveReconnectTimer) {
    clearTimeout(liveReconnectTimer);
    liveReconnectTimer = null;
  }
  if (liveSocket) {
    try { liveSocket.close(); } catch (e) { /* ignore */ }
    liveSocket = null;
  }
  if (liveObjectUrl) {
    URL.revokeObjectURL(liveObjectUrl);
    liveObjectUrl = null;
  }
  updateLiveButton();
}

async function refreshState() {
  const r = await fetch('/state');
  const s = await r.json();
  ocrReady = s.ocr !== false;
  detReady = s.det !== false;
  if (!s.captured) {
    setStatus(s.error ? `未截到帧：${s.error}` : '没有帧');
    return;
  }
  const caps = `单行识别=${ocrReady ? '可用' : '不可用'} · 多行识别=${detReady ? '可用' : '不可用'}`;
  setStatus(`${s.device} · ${s.width}x${s.height} · ${caps}`);
}

function ensureCapturePickers() {
  if (!window.OASAssetUI || !window.OASAssetUI.createSearchPicker) return;
  if (!captureNamePicker && captureNameHost) {
    captureNamePicker = window.OASAssetUI.createSearchPicker({
      items: [],
      value: '',
      placeholder: '搜索已有名称，或输入新名称…',
      enterChooses: false,
      label: (item) => item.label,
      onChoose: (item, input) => {
        input.value = item.label;
      },
      onEnter: () => confirmCaptureInput(captureNamePicker),
    });
    captureNameHost.textContent = '';
    captureNameHost.appendChild(captureNamePicker);
  }
  if (!captureCategoryPicker && captureCategoryHost) {
    captureCategoryPicker = window.OASAssetUI.createSearchPicker({
      items: [],
      value: '',
      placeholder: '搜索已有分类，或输入新分类…',
      enterChooses: false,
      label: (item) => item.label,
      onChoose: (item, input) => {
        input.value = item.id || '';
      },
      onEnter: () => confirmCaptureInput(captureCategoryPicker),
    });
    captureCategoryHost.textContent = '';
    captureCategoryHost.appendChild(captureCategoryPicker);
  }
}

// Enter inside an input confirms that field only (closes the dropdown), then
// moves focus to the save button. A subsequent Enter on the dialog saves.
function confirmCaptureInput(picker) {
  if (picker && picker.close) picker.close();
  if (picker && picker.input) picker.input.blur();
  setTimeout(() => {
    if (captureDialog && captureDialog.open && captureSubmit) captureSubmit.focus();
  }, 0);
}

async function openCaptureFrameDialog() {
  if (captureBusy) return;
  ensureCapturePickers();
  if (!captureDialog || !captureNamePicker || !captureCategoryPicker) return;
  captureBusy = true;
  if (btnFrameCapture) btnFrameCapture.disabled = true;
  try {
    const r = await postJson('/capture/preview', {});
    if (!r || !r.ok) {
      const msg = (r && r.errors || []).join('; ') || '截取失败';
      resultEl.textContent = '截取失败：' + msg;
      log('截取失败：' + msg);
      return;
    }
    const names = Array.from(new Set((framesCache || []).map((f) => (f.name || '').trim()).filter(Boolean)));
    const cats = Array.from(new Set((framesCache || []).map((f) => (f.category || '').trim()).filter(Boolean)));
    captureNamePicker.setItems(names.map((name) => ({ id: name, label: name })));
    captureCategoryPicker.setItems([
      { id: '', label: '未分类' },
      ...cats.map((cat) => ({ id: cat, label: cat })),
    ]);
    const now = new Date();
    const stamp = [now.getHours(), now.getMinutes(), now.getSeconds()]
      .map((n) => String(n).padStart(2, '0'))
      .join('');
    captureNamePicker.setValue(`帧 ${stamp}`);
    captureCategoryPicker.setValue(categoryFilterValue || '');
    if (capturePreviewImg) {
      capturePreviewImg.src = (r.preview_url || '/capture/preview.png') + '?t=' + Date.now();
    }
    if (captureError) captureError.textContent = '';
    captureDialog.showModal();
    setTimeout(() => captureNamePicker.input.focus(), 0);
  } catch (e) {
    const msg = e && e.message ? e.message : String(e);
    resultEl.textContent = '截取异常：' + msg;
    log('截取异常：' + msg);
  } finally {
    captureBusy = false;
    if (btnFrameCapture) btnFrameCapture.disabled = false;
  }
}

async function submitCaptureFrame() {
  if (captureBusy) return;
  const name = (captureNamePicker && captureNamePicker.input.value || '').trim();
  const category = (captureCategoryPicker && captureCategoryPicker.input.value || '').trim();
  if (!name) {
    if (captureError) captureError.textContent = '名称不能为空';
    return;
  }
  captureBusy = true;
  if (captureSubmit) captureSubmit.disabled = true;
  try {
    const r = await postJson('/capture/commit', { name, category });
    if (!r || !r.ok) {
      const msg = (r && r.errors || []).join('; ') || '保存截图失败';
      if (captureError) captureError.textContent = msg;
      return;
    }
    log(`已保存帧 #${r.id}：${r.name} / ${(r.category || '').trim() || '未分类'}（实时/当前帧未改变）`);
    if (captureDialog) captureDialog.close();
    if (capturePreviewImg) capturePreviewImg.removeAttribute('src');
    await refreshFrames();
    // 保存后回到实时画面；截图过程中从未暂停实时。
    if (!livePreview) startLivePreview();
    try {
      await loadFrame(true);
    } catch (e) {
      // 设备离线时保留当前画面
    }
  } catch (e) {
    if (captureError) captureError.textContent = '保存异常：' + (e && e.message ? e.message : e);
  } finally {
    captureBusy = false;
    if (captureSubmit) captureSubmit.disabled = false;
  }
}

canvas.addEventListener('mousedown', (ev) => {
  // Live passthrough: current screen is the device, so a click/drag is sent to
  // the emulator. Pause 实时 to get the ROI/polygon editing behavior back.
  if (liveInputAllowed()) {
    if (ev.button !== 0) return;
    ev.preventDefault();
    liveInputDrag = {
      start: clampToFrame(toFrame(ev)),
      clientX: ev.clientX,
      clientY: ev.clientY,
      t: performance.now(),
    };
    return;
  }
  // P1-d2 step 3: every detected block is individually operable. Clicking inside
  // a box selects that block (ROI := box) so 生成素材 JSON applies to it, and
  // shows only that block's text.
  const p0 = toFrame(ev);
  if (actionPolyMode) {
    if (ev.detail >= 2) {
      ev.preventDefault();
      return;
    }
    if (!actionPoly || actionPolyClosed) {
      actionPoly = [];
      actionPolyClosed = false;
    }
    actionPoly.push(p0);
    updateRoiLabel();
    draw();
    return;
  }
  // §22.10 多边形模式：未闭合时逐点点击、双击闭合；闭合后可拖动顶点、双击删顶点。
  if (polyMode) {
    if (ev.detail >= 2) {
      ev.preventDefault();
      return; // 交给 dblclick 处理（闭合/删顶点），避免多加一个重复点
    }
    if (polygon && polygon.length) {
      const vi = nearestVertex(p0);
      if (vi >= 0) {
        polyDrag = vi; // 闭合/未闭合都允许拖动已有顶点
        log(`拖动顶点 ${vi + 1}/${polygon.length}`);
        return;
      }
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
  // §22.25 操作区域模式：拖出一个 ROI 作为「操作区域」（roi_back），识别框保持不变。
  if (backMode) {
    dragStart = p0;
    dragTarget = 'back';
    draw();
    return;
  }
  if (boxes.length) {
    for (let i = boxes.length - 1; i >= 0; i--) {
      const b = boxes[i];
      const [bx, by, bw, bh] = b.box;
      if (p0.x >= bx && p0.x <= bx + bw && p0.y >= by && p0.y <= by + bh) {
        roi = { x: bx, y: by, w: bw, h: bh };
        roiBack = null;
        polygon = null;
        polyClosed = false;
        selectedBox = b;
        dragStart = null;
        updateRoiLabel();
        updateClickStepButtons();
        draw();
        resultEl.textContent =
          `选中区块：${b.text}\n得分 = ${b.score.toFixed(4)}\nROI = [${b.box.join(', ')}]\n` +
          '（点「生成素材 JSON」为这一块建素材；点「生成点击指令」把它变成 click_point 步骤）';
        log(`选中区块「${b.text}」(${b.score.toFixed(3)})`);
        return;
      }
    }
  }
  dragStart = p0;
  dragTarget = 'roi';
  roi = null;
  roiBack = null;
  lastMatch = null;
  if (btnMatchSave) btnMatchSave.disabled = true;
  boxes = [];
  polygon = null;
  polyClosed = false;
  selectedBox = null;
  updateClickStepButtons();
  draw();
});

canvas.addEventListener('mousemove', (ev) => {
  if (liveInputAllowed()) {
    if (liveInputDrag) ev.preventDefault();
    return;
  }
  if (polyMode) {
    if (polyDrag === null || !polygon) return;
    polygon[polyDrag] = toFrame(ev);
    if (polygon.length >= 3) roi = polygonBBox(polygon);
    updateRoiLabel();
    draw();
    return;
  }
  if (!dragStart) return;
  if (dragTarget === 'back') {
    roiBack = normalizeRect(dragStart, toFrame(ev));
    updateRoiLabel();
    draw();
    return;
  }
  roi = normalizeRect(dragStart, toFrame(ev));
  updateRoiLabel();
  draw();
});

canvas.addEventListener('mouseup', (ev) => {
  if (liveInputAllowed() && liveInputDrag) {
    finishLiveInput(ev);
    return;
  }
  if (polyMode) {
    if (polyDrag === null) return;
    polyDrag = null;
    if (polygon && polygon.length >= 3) roi = polygonBBox(polygon);
    updateRoiLabel();
    draw();
    if (roi) log(`顶点已调整：${polygon.length} 顶点，bbox ${roi.w}x${roi.h} @ ${roi.x},${roi.y}`);
    return;
  }
  if (!dragStart) return;
  dragStart = null;
  if (dragTarget === 'back') {
    dragTarget = 'roi';
    const finished = roiBack ? { ...roiBack } : null;
    if (activeCandidate >= 0 && saveCandidates[activeCandidate]) {
      saveCandidates[activeCandidate].action = finished || { ...saveCandidates[activeCandidate].recognize };
      activeCandidate = -1;
      backMode = false;
      if (btnBackMode) btnBackMode.textContent = '绘制操作区域（默认=识别区域）';
      renderCandidateList(lastLinesRoi ? '本次识别' : '识别结果');
      log(`操作区域已更新：${finished ? `${finished.w}x${finished.h} @ ${finished.x},${finished.y}` : '默认=识别区域'}`);
    } else if (pendingActionAsset) {
      updateAssetActionRoi(pendingActionAsset);
    }
    updateRoiLabel();
    draw();
    if (roiBack) log(`操作区域 ${roiBack.w}x${roiBack.h} @ ${roiBack.x},${roiBack.y}`);
    return;
  }
  updateRoiLabel();
  draw();
  if (roi) log(`选中识别框 ${roi.w}x${roi.h} @ ${roi.x},${roi.y}`);
});

// Mouse release outside the canvas still ends a live gesture; otherwise the
// browser leaves liveInputDrag set until the next click.
window.addEventListener('mouseup', (ev) => {
  if (liveInputAllowed() && liveInputDrag) finishLiveInput(ev);
});

// 最近顶点（8px 内）——用于闭合后编辑顶点
function nearestVertex(p) {
  if (!polygon) return -1;
  let best = -1;
  let bestD = 8 * 8;
  for (let i = 0; i < polygon.length; i++) {
    const dx = polygon[i].x - p.x;
    const dy = polygon[i].y - p.y;
    const d = dx * dx + dy * dy;
    if (d <= bestD) {
      bestD = d;
      best = i;
    }
  }
  return best;
}

// §22.10：双击行为 —— 未闭合则闭合；已闭合且双击顶点则删除该顶点（保持 >=3）。
canvas.addEventListener('dblclick', (ev) => {
  if (liveInputAllowed()) return;
  if (actionPolyMode) {
    if (!actionPoly || actionPoly.length < 3) return;
    ev.preventDefault();
    actionPolyClosed = true;
    actionPolyMode = false;
    if (btnActionPoly) btnActionPoly.textContent = '操作区域多边形（点顶点，双击闭合）';
    log(`操作区域多边形已闭合：${actionPoly.length} 顶点${actionInvert ? '（反选：屏幕−区域）' : ''}`);
    updateRoiLabel();
    draw();
    return;
  }
  if (!polyMode || !polygon || polygon.length < 3) return;
  ev.preventDefault();
  const p = toFrame(ev);
  if (polyClosed) {
    const vi = nearestVertex(p);
    if (vi >= 0 && polygon.length > 3) {
      polygon.splice(vi, 1);
      roi = polygonBBox(polygon);
      updateRoiLabel();
      draw();
      log(`已删除顶点 ${vi + 1}，剩余 ${polygon.length} 个；bbox ${roi.w}x${roi.h} @ ${roi.x},${roi.y}`);
    }
    return;
  }
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
      '（点击区=匹配外接矩形；多边形用于匹配/OCR 掩膜）'
  );
});

btnPoly.addEventListener('click', () => {
  polyMode = !polyMode;
  if (polyMode) {
    polygon = null;
    polyClosed = false;
    polyDrag = null;
    roi = null;
    roiBack = null;
    dragStart = null;
    btnPoly.textContent = '退出多边形模式（点顶点，双击闭合；闭合后可拖/双击删顶点）';
    log('多边形模式：逐点点击，双击闭合；闭合后可拖动顶点、双击删除顶点');
  } else {
    btnPoly.textContent = '多边形框选（点顶点，双击闭合）';
  }
  updateRoiLabel();
  draw();
});

btnActionPoly.addEventListener('click', () => {
  actionPolyMode = !actionPolyMode;
  if (actionPolyMode) {
    actionPoly = null;
    actionPolyClosed = false;
    backMode = false;
    polyMode = false;
    if (btnBackMode) btnBackMode.textContent = '绘制操作区域（默认=识别区域）';
    btnActionPoly.textContent = '退出操作区域多边形（点顶点，双击闭合）';
    log(`操作区域多边形模式：逐点点击，双击闭合${actionInvert ? '（反选=整个屏幕−区域）' : ''}`);
  } else {
    btnActionPoly.textContent = '操作区域多边形（点顶点，双击闭合）';
  }
  updateRoiLabel();
  draw();
});

btnActionInvert.addEventListener('click', () => {
  actionInvert = !actionInvert;
  btnActionInvert.textContent = `操作区域反选：${actionInvert ? '开' : '关'}`;
  log(`操作区域反选：${actionInvert ? '开（有效区=整个屏幕−区域）' : '关'}`);
  updateRoiLabel();
  draw();
});

btnBackMode.addEventListener('click', () => {
  backMode = !backMode;
  btnBackMode.textContent = backMode
    ? '退出操作区域编辑（拖拽画操作区域）'
    : '绘制操作区域（默认=识别区域）';
  if (!backMode) {
    activeCandidate = -1;
    pendingActionAsset = null;
  }
  log(
    backMode
      ? '操作区域模式：在画面上拖拽画出「操作区域」（action_roi，可大于识别区域）'
      : '已退出操作区域模式'
  );
  draw();
});

btnClear.addEventListener('click', () => {
  roi = null;
  roiBack = null;
  lastMatch = null;
  if (btnMatchSave) btnMatchSave.disabled = true;
  polygon = null;
  polyClosed = false;
  selectedBox = null;
  activeCandidate = -1;
  pendingActionAsset = null;
  replaceResults([], '');
  updateRoiLabel();
  updateClickStepButtons();
  draw();
  resultEl.textContent = '—';
});

async function fallbackFullscreenLines(reason) {
  if (!detReady) {
    log('OCR 全屏回退不可用：无 det model', 'ERROR', { reason });
    resultEl.textContent = `单行识别失败（${reason}）；多行识别不可用（无 det model）`;
    return false;
  }
  const fw = canvas.width || (img && img.naturalWidth) || (roi && roi.x + roi.w) || 1280;
  const fh = canvas.height || (img && img.naturalHeight) || (roi && roi.y + roi.h) || 720;
  const full = { x: 0, y: 0, w: fw, h: fh };
  resultEl.textContent = `单行识别失败（${reason}），改用全屏多行识别…`;
  log(`OCR 全屏回退开始`, 'WARN', { reason, full_roi: full });
  const r = await fetch(`/lines?x=0&y=0&w=${fw}&h=${fh}`);
  const s = await r.json();
  if (s.error || !Array.isArray(s.lines)) {
    log('OCR 全屏回退失败', 'ERROR', { reason, error: s.error || '无结果' });
    resultEl.textContent = `全屏多行回退失败：${s.error || '无结果'}`;
    return false;
  }
  const lines = s.lines.map((b) => ({
    ...b,
    box: [b.box[0], b.box[1], b.box[2], b.box[3]],
  }));
  log('OCR 全屏回退成功', 'INFO', {
    reason, full_roi: full, count: lines.length,
    elapsed_ms: s.elapsed_ms,
    lines: lines.map((l) => ({ text: l.text, score: l.score })),
  });
  renderOcrLines(lines, full, '全屏多行回退');
  resultEl.textContent =
    `单行识别失败（${reason}）→ 全屏多行识别 ${lines.length} 行；` +
    '请在「识别结果」勾选需要的行。';
  return true;
}

btnOcr.addEventListener('click', async () => {
  if (!roi) return;
  if (!ocrReady) {
    resultEl.textContent =
      'OCR 不可用：serve 未加载 OCR（需 `cargo build -p oasd --features ocr` 构建，并带 --model 启动）';
    return;
  }
  btnOcr.disabled = true;
  resultEl.textContent = '识别中…';
  try {
    // Wired up in the next step; the route already exists on the backend side.
    const q = `x=${roi.x}&y=${roi.y}&w=${roi.w}&h=${roi.h}`;
    const r = await fetch('/ocr?' + q);
    const s = await r.json();
    if (s.error || !String(s.text || '').trim()) {
      if (await fallbackFullscreenLines(s.error || 'ROI 内没有文字')) return;
      resultEl.textContent = '错误：' + (s.error || 'ROI 内没有文字');
    } else {
      const rec = { x: roi.x, y: roi.y, w: roi.w, h: roi.h };
      replaceResults([
        makeResultRow(
          'ocr_line',
          s.text,
          s.text,
          rec,
          roiBack ? { ...roiBack } : { ...rec },
          rec,
          candidateNameValid(s.text)
        ),
      ], '单行识别');
      resultEl.textContent = `文字 = ${JSON.stringify(s.text)}\n得分 = ${s.score.toFixed(4)}\n耗时 = ${s.elapsed_ms.toFixed(1)} ms`;
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
  lastMatch = s;
  if (btnMatchSave) btnMatchSave.disabled = false;
  const rec = { x: s.x, y: s.y, w: s.w, h: s.h };
  let action = { ...rec };
  if (roiBack && r) {
    action = {
      x: rec.x + (roiBack.x - r.x),
      y: rec.y + (roiBack.y - r.y),
      w: roiBack.w,
      h: roiBack.h,
    };
  }
  const matchName = `模板_${Math.max(1, Math.round(Number(s.score) * 100))}_${Date.now() % 10000}`;
  replaceResults([
    makeResultRow('template', `图匹配 ${Number(s.score).toFixed(3)}`, matchName, rec, action, rec, true),
  ], '图片匹配');
  boxes = [{ text: `图匹配 ${Number(s.score).toFixed(2)}`, score: Number(s.score), box: [s.x, s.y, s.w, s.h] }];
  draw();
  resultEl.textContent =
    `图识别得分 = ${Number(s.score).toFixed(4)} @ (${s.x}, ${s.y})\n` +
    `识别区域 = [${s.x}, ${s.y}, ${s.w}, ${s.h}]\n` +
    `ROI 内得分 = ${roiScore.toFixed(4)}\n` +
    `耗时 = ${Number(s.elapsed_ms || 0).toFixed(1)} ms（全屏金字塔搜索）\n` +
    '（点「用匹配结果建素材」可把该识别区域作为素材，再拖出操作区域）';
  log(`图识别 → 得分 ${Number(s.score).toFixed(3)} @ (${s.x},${s.y})`);
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

if (btnMatchSave) {
  btnMatchSave.addEventListener('click', () => {
    if (!lastMatch) return;
    roi = { x: lastMatch.x, y: lastMatch.y, w: lastMatch.w, h: lastMatch.h };
    // Image-match result: recognition region = matched bbox; operation region
    // defaults to it and can be enlarged/moved via 「绘制操作区域」.
    roiBack = null;
    polygon = null;
    polyClosed = false;
    selectedBox = null;
    boxes = [{ text: `图匹配 ${Number(lastMatch.score).toFixed(2)}`, score: Number(lastMatch.score), box: [roi.x, roi.y, roi.w, roi.h] }];
    if (assetKind && assetKind.value !== 'template') assetKind.value = 'template';
    updateRoiLabel();
    draw();
    resultEl.textContent =
      `已用图匹配结果设置识别区域：[${roi.x}, ${roi.y}, ${roi.w}, ${roi.h}]\n` +
      '1) 填素材 id → 保存素材（模板图）；2) 需要更大点击范围时点「绘制操作区域」再拖拽；\n' +
      '3) 保存时 action_roi 会一并写入。';
    log('图匹配结果 → 识别区域已设置；可调整操作区域后保存模板素材');
  });
}

if (btnLivePreview) {
  btnLivePreview.addEventListener('click', async () => {
    if (livePreview) {
      pauseLivePreview();
      try {
        await loadFrame(false);
      } catch (e) {
        // keep the last rendered frame
      }
    } else {
      startLivePreview();
    }
  });
}

// Generate the declaration JSON for copy/paste. `file` is intentionally NOT
// generated: when saved, the engine assigns images/aNNN.png, so the (Chinese)
// id never lands on disk as a path (ENGINE-DESIGN §18.1).
if (btnAsset) btnAsset.addEventListener('click', async () => {
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
  if (!ocrReady || !detReady) {
    resultEl.textContent =
      '多行识别不可用：serve 需 `--features ocr` 构建且带 --model 与 --det-model 启动';
    return;
  }
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
      lastLinesRoi = { x: roi.x, y: roi.y, w: roi.w, h: roi.h };
      const txt = boxes.map((b, i) => `${i + 1}. ${b.text} (${b.score.toFixed(3)})`).join('\n');
      resultEl.textContent = `${boxes.length} 行 / ${s.elapsed_ms.toFixed(0)} ms\n` + txt;
      renderOcrLines(boxes, lastLinesRoi, '本次多行识别');
      log(`多行识别 → ${boxes.length} 行（可直接点击任一行）`);
    }
  } catch (e) {
    resultEl.textContent = '多行识别异常：' + e;
  } finally {
    btnLines.disabled = false;
  }
});

// --- N38/M1: 任务创建 + 编辑器「运行任务」 ------------------------------------
// 创建走 POST /task/create（服务端原子创建 + 校验），运行走 POST /task/run
// （后台线程跑 run::execute），状态由 GET /task/run/status 轮询到结果/日志窗口。

function setRunStatus(text) {
  if (runStatusEl) runStatusEl.textContent = text || '—';
}

// 与服务端一致的前置检查（服务端仍是权威）；错误信息用中文，直接显示。
function validateTaskNameClient(name) {
  if (!name) return '任务名称不能为空';
  if (name.length > 120) return '任务名称不能超过 120 个字符';
  if (/[\\/:*?"<>|]/.test(name)) return '任务名称不能包含 \\ / : * ? " < > | 这些字符';
  if (name.includes('..')) return '任务名称不能包含 `..`（防止路径穿越）';
  if (name === '.' || name.endsWith('.')) return '任务名称不能是 .，也不能以 . 结尾';
  if (name.trim() !== name) return '任务名称首尾不能有空格';
  const reserved = ['CON', 'PRN', 'AUX', 'NUL'];
  for (let i = 1; i <= 9; i++) {
    reserved.push('COM' + i);
    reserved.push('LPT' + i);
  }
  const stem = name.split('.')[0].toUpperCase();
  if (reserved.includes(stem)) return `「${name}」是 Windows 保留设备名，请换一个`;
  return '';
}

// 把当前任务名同步到任务列表下拉框（名称不在列表里就保持原样）。
function syncTaskSelect(name) {
  if (!taskSelect) return;
  const want = (name || '').trim();
  const has = Array.from(taskSelect.options).some((o) => o.value === want);
  if (has) taskSelect.value = want;
}

// GET /task/list -> 下拉框；selectName 是想选中的任务（创建后/深链）。
async function refreshTaskList(selectName) {
  if (!taskSelect) return;
  try {
    const r = await fetch('/task/list');
    const s = await r.json();
    if (!s.ok) {
      log('任务列表读取失败：' + (s.errors || []).join('; '));
      return;
    }
    const tasks = s.tasks || [];
    const want = (selectName || (taskName.value || '')).trim();
    taskSelect.textContent = '';
    if (!tasks.length) {
      const opt = document.createElement('option');
      opt.value = '';
      opt.textContent = '（还没有任务，点「新建任务」）';
      taskSelect.appendChild(opt);
      return;
    }
    for (const t of tasks) {
      const opt = document.createElement('option');
      opt.value = t;
      opt.textContent = t;
      taskSelect.appendChild(opt);
    }
    // Single source of truth: the task name input mirrors the selected task.
    // Without this, batch save could write to the default "任务1" while the
    // user is looking at another selected task.
    const selected = want && tasks.includes(want) ? want : tasks[0];
    taskSelect.value = selected;
    if (taskName) taskName.value = selected;
  } catch (e) {
    log('任务列表读取异常：' + e);
  }
}

function openCreateTaskDialog() {
  if (!createTaskDialog) return;
  if (createError) createError.textContent = '';
  if (createName) createName.value = '';
  if (createTemplate) createTemplate.value = 'daily';
  if (createDesc) createDesc.value = '';
  if (typeof createTaskDialog.showModal === 'function') {
    if (!createTaskDialog.open) createTaskDialog.showModal();
  } else {
    createTaskDialog.setAttribute('open', '');
  }
  if (createName) createName.focus();
}

function closeCreateTaskDialog() {
  if (!createTaskDialog) return;
  if (typeof createTaskDialog.close === 'function') {
    if (createTaskDialog.open) createTaskDialog.close();
  } else {
    createTaskDialog.removeAttribute('open');
  }
}

// 对话框「创建」：POST /task/create -> 刷新任务列表并选中 -> 刷新素材/flow。
async function submitCreateTask() {
  const name = ((createName && createName.value) || '').trim();
  const template = (createTemplate && createTemplate.value) || 'daily';
  const description = ((createDesc && createDesc.value) || '').trim();
  const bad = validateTaskNameClient(name);
  if (bad) {
    if (createError) createError.textContent = bad;
    if (createName) createName.focus();
    return;
  }
  if (btnCreateSubmit) btnCreateSubmit.disabled = true;
  if (createError) createError.textContent = '正在创建任务包…';
  try {
    const s = await postJson('/task/create', { name, template, description });
    if (!s.ok) {
      const errs = (s.errors || []).map((e) => '· ' + e).join('\n');
      if (createError) createError.textContent = '创建失败：\n' + (errs || '（未知错误）');
      return;
    }
    const t = s.task || {};
    if (createError) createError.textContent = '';
    closeCreateTaskDialog();
    taskName.value = t.name || name;
    await refreshTaskList(taskName.value);
    syncTaskSelect(taskName.value);
    await refreshTaskAssets(taskName.value);
    await loadTaskFlow(taskName.value);
    const metaText = t.meta ? JSON.stringify(t.meta, null, 2) : '';
    resultEl.textContent =
      `已创建任务「${taskName.value}」\n模板：${t.template || template} · 分类：${t.category || '—'}\n` +
      `路径：${t.path || ''}\n\n${metaText}`;
    log(`已创建任务「${taskName.value}」（模板 ${t.template || template}）`);
    setStatus(`已创建任务：${taskName.value}`);
    setRunStatus(`任务「${taskName.value}」已就绪：点「运行任务」开始一次运行。`);
  } catch (e) {
    if (createError) createError.textContent = '创建异常：' + (e && e.message ? e.message : e);
  } finally {
    if (btnCreateSubmit) btnCreateSubmit.disabled = false;
  }
}

function formatRunSummary(s) {
  const lines = [];
  lines.push(`任务「${s.task || '—'}」运行结束：${s.outcome || '未知'}`);
  lines.push(`退出码 = ${s.exit == null ? '—' : s.exit}`);
  if (s.mode) lines.push(`模式 = ${s.mode}`);
  if (s.seed != null) lines.push(`seed = ${s.seed}`);
  if (s.clicks != null) lines.push(`实际点击 = ${s.clicks} 次`);
  if (s.started_ms && s.finished_ms) {
    lines.push(`耗时 = ${((s.finished_ms - s.started_ms) / 1000).toFixed(1)} 秒`);
  }
  if (s.error) lines.push(`错误 = ${s.error}`);
  return lines.join('\n');
}

// 把一次状态快照写到任务面板状态行 + 增量日志写到已有「日志窗口」。
function renderRunStatus(s) {
  const mode = s.mode || (s.replay ? 'replay' : (s.dry_run ? 'dry-run' : 'live'));
  let text;
  if (s.running) {
    text = `运行中：任务「${s.task || '—'}」（${mode}）…`;
  } else if (s.task) {
    text = `上次运行：任务「${s.task}」→ ${s.outcome || '—'}（exit ${s.exit == null ? '—' : s.exit}）`;
    if (s.error) text += `\n错误：${s.error}`;
  } else {
    text = '尚未运行任务（先选择任务，再点「运行任务」）';
  }
  setRunStatus(text);
  const tail = Array.isArray(s.log_tail) ? s.log_tail : [];
  const count = Number(s.log_count || 0);
  if (count > runLogSeen) {
    const fresh = Math.min(count - runLogSeen, tail.length);
    const start = Math.max(0, tail.length - fresh);
    // log() 把新行加在最上面：倒序写入才能让日志窗口保持时间顺序。
    for (let i = tail.length - 1; i >= start; i--) log('[run] ' + tail[i]);
    runLogSeen = count;
  }
}

// 轮询 GET /task/run/status 直到 !running；结束后把结果写到「结果窗口」。
async function pollRunStatus() {
  if (runPolling) return;
  runPolling = true;
  try {
    while (true) {
      const r = await fetch('/task/run/status');
      const s = await r.json();
      if (!s.ok) {
        setRunStatus('运行状态读取失败：' + (s.error || '未知错误'));
        break;
      }
      renderRunStatus(s);
      if (!s.running) {
        resultEl.textContent = formatRunSummary(s);
        log(`任务运行结束：${s.outcome || '—'}（exit ${s.exit == null ? '—' : s.exit}）`);
        try {
          await refreshRuns();
        } catch (e) {
          /* 点击分布刷新失败不影响运行结果 */
        }
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
  } catch (e) {
    setRunStatus('运行状态轮询异常：' + (e && e.message ? e.message : e));
  } finally {
    runPolling = false;
    if (btnTaskRun) btnTaskRun.disabled = false;
    if (btnTaskStop) btnTaskStop.disabled = true;
  }
}

function syncRunButtonLabel() {
  if (!btnTaskRun) return;
  btnTaskRun.textContent = runDry && runDry.checked
    ? '测试运行（dry-run，不点击设备）'
    : '运行任务';
}

async function stopRunTask() {
  if (btnTaskStop) btnTaskStop.disabled = true;
  setRunStatus('正在请求停止当前任务…');
  try {
    const s = await postJson('/task/run/stop', {});
    if (!s.ok) {
      setRunStatus('停止失败：' + (s.error || (s.errors || []).join('; ') || '未知错误'));
      log('停止任务失败', 'ERROR', s);
      if (btnTaskStop) btnTaskStop.disabled = false;
      return;
    }
    log('已请求停止当前任务；等待运行线程结束…', 'INFO', { message: s.message });
    setRunStatus('已请求停止；等待当前动作完成后结束…');
  } catch (e) {
    setRunStatus('停止异常：' + (e && e.message ? e.message : e));
    if (btnTaskStop) btnTaskStop.disabled = false;
  }
}

// 任务面板「运行任务」：POST /task/run -> 轮询状态 -> 结果/日志窗口。
async function doRunTask() {
  const task = currentTask();
  if (!task) return;
  const payload = { task };
  if (runDry && runDry.checked) payload.dry_run = true;
  const replay = ((runReplay && runReplay.value) || '').trim();
  if (replay) payload.replay = replay;
  const modeLabel = replay
    ? `回放（${replay}）`
    : (payload.dry_run ? 'dry-run（不点击设备）' : '实机');
  if (btnTaskRun) btnTaskRun.disabled = true;
  if (btnTaskStop) btnTaskStop.disabled = false;
  runLogSeen = 0;
  setRunStatus(`正在启动任务「${task}」（${modeLabel}）…`);
  try {
    const s = await postJson('/task/run', payload);
    if (!s.ok) {
      const errs = (s.errors || []).map((e) => '· ' + e).join('\n');
      setRunStatus('启动失败：\n' + (errs || '未知错误'));
      showErrors('运行任务失败：', s.errors);
      log('运行任务失败：' + (errs || '未知错误'));
      if (btnTaskRun) btnTaskRun.disabled = false;
      return;
    }
    log(`运行任务：${s.task}（${s.mode || modeLabel}）`);
    resultEl.textContent =
      `任务「${s.task}」运行中…\n模式 = ${s.mode || modeLabel}\n` +
      '实时日志写入「日志窗口」；结束后这里显示结果。';
    await pollRunStatus();
  } catch (e) {
    setRunStatus('运行异常：' + (e && e.message ? e.message : e));
    log('运行异常：' + e);
    if (btnTaskRun) btnTaskRun.disabled = false;
  }
}

// --- M2/N37: 调度面板 + 重置记忆 -------------------------------------------
function fmtLocal(ms) {
  if (!ms) return '—';
  try {
    return new Date(Number(ms)).toLocaleString('zh-CN', { hour12: false });
  } catch (e) {
    return String(ms);
  }
}

function schedBadge(text, cls) {
  const s = document.createElement('span');
  s.className = 'badge ' + (cls || '');
  s.textContent = text;
  return s;
}

function renderScheduler(s) {
  if (!schedSummary || !schedEntries) return;
  schedulerStatus = s;
  const guards = s.guards || {};
  const parts = [
    `模式=${s.mode || '—'}`,
    `轮次=${s.passes ?? 0}`,
    `下一唤醒=${s.next_wake_local || fmtLocal(s.next_wake_ms)}`,
    `护栏=${guards.blocked ? '已阻断' : '正常'}`,
  ];
  if (guards.blackout_reason) parts.push(`原因=${guards.blackout_reason}`);
  if (s.last_stop) parts.push(`上次停止=${typeof s.last_stop === 'string' ? s.last_stop : JSON.stringify(s.last_stop)}`);
  schedSummary.textContent = parts.join(' · ');

  schedEntries.textContent = '';
  const entries = Array.isArray(s.entries) ? s.entries : [];
  if (!entries.length) {
    schedEntries.textContent = '（无调度条目：编辑 scheduler.json 后刷新）';
    return;
  }
  for (const e of entries) {
    const row = document.createElement('div');
    row.className = 'sched-entry';
    const head = document.createElement('div');
    head.className = 'row';
    const name = document.createElement('strong');
    name.textContent = `${e.entry_id || '?'} · ${e.task || '?'}`;
    head.appendChild(name);
    head.appendChild(schedBadge(e.lifecycle || '?', 'kind'));
    if (e.enabled === false) head.appendChild(schedBadge('已禁用', 'warn'));
    if (e.runnable_now) head.appendChild(schedBadge('现在可跑', 'ok'));
    if (e.blocked_reason) head.appendChild(schedBadge('等待：' + e.blocked_reason, 'warn'));
    row.appendChild(head);

    const meta = document.createElement('div');
    meta.className = 'muted small';
    const quota = e.quota || {};
    const bits = [];
    if (quota.per) bits.push(`quota ${quota.used ?? 0}/${quota.times ?? '?'}（${quota.per}）`);
    if (e.next_due_local) bits.push(`下次=${e.next_due_local}`);
    if (e.cooldown_until_ms) bits.push(`冷却至=${fmtLocal(e.cooldown_until_ms)}`);
    if (e.last_result) bits.push(`上次=${e.last_result}`);
    meta.textContent = bits.join(' · ') || '（无窗口限制 / 每轮执行）';
    row.appendChild(meta);
    schedEntries.appendChild(row);
  }
}

async function refreshScheduler() {
  if (!schedSummary) return;
  try {
    const r = await fetch('/scheduler/status');
    const s = await r.json();
    if (s.ok === false) {
      schedSummary.textContent = '读取调度状态失败：' + ((s.errors || []).join('; ') || '未知错误');
      return;
    }
    renderScheduler(s);
  } catch (e) {
    schedSummary.textContent = '读取调度状态失败：' + e;
  }
}

function resetTaskChecks(mode) {
  if (!resetTaskList) return;
  for (const cb of resetTaskList.querySelectorAll('input[type=checkbox][data-task]')) {
    if (mode === 'all') cb.checked = true;
    else if (mode === 'none') cb.checked = false;
    else cb.checked = !cb.checked;
  }
}

function openResetMemoryDialog() {
  if (!resetMemoryDialog) return;
  if (resetError) resetError.textContent = '';
  if (!schedulerStatus) {
    refreshScheduler().then(() => {
      populateResetTaskList();
      resetMemoryDialog.showModal();
    });
    return;
  }
  populateResetTaskList();
  resetMemoryDialog.showModal();
}

function populateResetTaskList() {
  if (!resetTaskList) return;
  resetTaskList.textContent = '';
  const choices = (schedulerStatus && schedulerStatus.task_choices) || [];
  const seen = new Set();
  const rows = [];
  for (const c of choices) {
    const task = c.task || '';
    if (!task || seen.has(task)) continue;
    seen.add(task);
    rows.push(c);
  }
  if (!rows.length && schedulerStatus && Array.isArray(schedulerStatus.entries)) {
    for (const e of schedulerStatus.entries) {
      if (e.task && !seen.has(e.task)) {
        seen.add(e.task);
        rows.push({ task: e.task });
      }
    }
  }
  if (!rows.length) {
    resetTaskList.textContent = '（没有可重置的任务：先创建任务/加入调度）';
    return;
  }
  for (const c of rows) {
    const label = document.createElement('label');
    label.className = 'opt reset-task-row';
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.dataset.task = c.task;
    label.appendChild(cb);
    const text = document.createElement('span');
    let note = '';
    if (c.bundle_exists === false) note += ' · 任务目录不存在';
    if (c.state_dir_exists) note += ' · 有 state';
    text.textContent = `${c.task}${note}`;
    label.appendChild(text);
    resetTaskList.appendChild(label);
  }
}

async function submitResetMemory() {
  if (!resetTaskList) return;
  const tasks = Array.from(resetTaskList.querySelectorAll('input[type=checkbox][data-task]'))
    .filter((cb) => cb.checked)
    .map((cb) => cb.dataset.task);
  if (!tasks.length) {
    if (resetError) resetError.textContent = '请至少选择一个任务（可用「全选」）。';
    return;
  }
  const include_checkpoint = resetIncludeCheckpoint ? resetIncludeCheckpoint.checked : true;
  try {
    const r = await postJson('/scheduler/reset', { tasks, include_checkpoint });
    if (!r.ok) {
      if (resetError) resetError.textContent = (r.errors || ['重置失败']).join('\n');
      return;
    }
    const rep = r.report || {};
    resultEl.textContent =
      `重置记忆完成：任务=[${(rep.tasks || tasks).join(', ')}] · ` +
      `删除文件=${rep.files_removed ?? '?'} · 状态=${rep.state_path || ''}`;
    if (logEl) logEl.textContent += '[reset-memory] ' + (r.log || []).join('\n') + '\n';


    if (resetMemoryDialog) resetMemoryDialog.close();
    await refreshScheduler();
  } catch (e) {
    if (resetError) resetError.textContent = '请求失败：' + e;
  }
}

// --- boot -------------------------------------------------------------------// Each independent concern is isolated so a missing frame or a not-yet-served
// backend route (/runs) can never blank-screen the editor.
(async function boot() {
  // §22.26 三栏面板切换（素材 / 任务 / 流程）—— 参考旧 OAS annotator 的 rail。
  const railButtons = Array.from(document.querySelectorAll('.rail-btn'));
  function activatePanel(panelId) {
    for (const b of railButtons) b.classList.toggle('active', b.dataset.panel === panelId);
    for (const p of document.querySelectorAll('.rail-panels .panel')) {
      p.classList.toggle('active', p.id === panelId);
    }
    if (panelId === 'panel-schedule') refreshScheduler();
    // Repaint so a cached click-distribution overlay disappears immediately
    // when leaving that panel (and appears when entering it).
    if (typeof draw === 'function') draw();
  }
  window.__oasActivatePanel = activatePanel;
  for (const b of railButtons) {
    b.addEventListener('click', () => {
      if (b.dataset.href) {
        window.location.href = b.dataset.href;
        return;
      }
      activatePanel(b.dataset.panel);
    });
  }
  // §22.28 顶部标题行：结果 / 日志 抽屉
  function syncDrawers() {
    if (hdrDrawers) hdrDrawers.classList.toggle('hidden', !(resultOpen || logOpen));
    if (resultPop) resultPop.classList.toggle('open', resultOpen);
    if (logPop) logPop.classList.toggle('open', logOpen);
    if (btnResultToggle) btnResultToggle.classList.toggle('active', resultOpen);
    if (btnLogToggle) btnLogToggle.classList.toggle('active', logOpen);
  }
  if (btnResultToggle) {
    btnResultToggle.addEventListener('click', () => {
      resultOpen = !resultOpen;
      syncDrawers();
    });
  }
  if (btnLogToggle) {
    btnLogToggle.addEventListener('click', () => {
      logOpen = !logOpen;
      syncDrawers();
    });
  }
  syncDrawers();
  if (btnAssetsRefresh) btnAssetsRefresh.addEventListener('click', () => refreshTaskAssets());
  if (btnAssetsBatchTest) btnAssetsBatchTest.addEventListener('click', batchTestAssets);
  if (assetSearch) assetSearch.addEventListener('input', renderAssetList);
  if (assetCategoryFilter) assetCategoryFilter.addEventListener('change', renderAssetList);
  if (railButtons.length) activatePanel(railButtons[0].dataset.panel);
  // 深链 ?panel=flow / ?panel=task 供验收/直达使用
  try {
    const wantPanel = new URLSearchParams(location.search).get('panel');
    const ok = ['panel-assets', 'panel-images', 'panel-click', 'panel-task', 'panel-schedule'];
    if (wantPanel && ok.includes(wantPanel)) activatePanel(wantPanel);
  } catch (e) {
    /* ignore */
  }
  // 深链 ?new=1：直接打开「新建任务」对话框（headless 截图验收用）
  try {
    if (new URLSearchParams(location.search).get('new') === '1') openCreateTaskDialog();
  } catch (e) {
    /* ignore */
  }
  // 深链 ?reset=1：打开「重置记忆」多选框（headless 截图验收用）
  try {
    if (new URLSearchParams(location.search).get('reset') === '1') openResetMemoryDialog();
  } catch (e) {
    /* ignore */
  }

  btnSaveAsset.addEventListener('click', saveAssetUnified);
  if (btnSaveFlow) btnSaveFlow.addEventListener('click', doSaveFlow);
  taskName.addEventListener('change', async () => {
    syncTaskSelect(taskName.value);
    await refreshTaskAssets();
    await loadTaskFlow((taskName.value || '').trim());
  });
  // --- N38/M1: 任务列表 / 新建任务 / 运行任务 ---
  if (btnTaskRefresh) btnTaskRefresh.addEventListener('click', () => refreshTaskList());
  if (taskSelect) {
    taskSelect.addEventListener('change', async () => {
      const name = (taskSelect.value || '').trim();
      if (!name) return;
      taskName.value = name;
      await refreshTaskAssets(name);
      await loadTaskFlow(name);
      setRunStatus(`已选择任务「${name}」；点「运行任务」开始一次运行。`);
    });
  }
  if (btnTaskCreate) btnTaskCreate.addEventListener('click', openCreateTaskDialog);
  if (btnCreateSubmit) btnCreateSubmit.addEventListener('click', submitCreateTask);
  if (btnCreateCancel) btnCreateCancel.addEventListener('click', closeCreateTaskDialog);
  if (createName) {
    createName.addEventListener('keydown', (ev) => {
      if (ev.key === 'Enter') {
        ev.preventDefault();
        submitCreateTask();
      }
    });
  }
  if (createDesc) {
    createDesc.addEventListener('keydown', (ev) => {
      if (ev.key === 'Enter' && (ev.ctrlKey || ev.metaKey)) {
        ev.preventDefault();
        submitCreateTask();
      }
    });
  }
  if (btnTaskRun) btnTaskRun.addEventListener('click', doRunTask);
  if (btnTaskStop) btnTaskStop.addEventListener('click', stopRunTask);
  if (runDry) runDry.addEventListener('change', syncRunButtonLabel);
  syncRunButtonLabel();
  if (btnGoalApply) btnGoalApply.addEventListener('click', applyTaskGoal);
  if (btnWbAssets) btnWbAssets.addEventListener('click', () => activatePanel('panel-assets'));
  if (btnWbFlow) btnWbFlow.addEventListener('click', () => activatePage('builder'));
  if (btnRegionClick) btnRegionClick.addEventListener('click', generateRegionClick);
  if (btnRegionDrag) btnRegionDrag.addEventListener('click', generateRegionDrag);
  if (btnRegionSave) btnRegionSave.addEventListener('click', () => {
    if (!roi) return;
    const name = (window.prompt('独立操作区域素材名称（全局唯一）', '操作区域') || '').trim();
    if (!name) return;
    const rec = { x: roi.x, y: roi.y, w: roi.w, h: roi.h };
    replaceResults([
      makeResultRow('region', name, name, rec, { ...rec }, null, candidateNameValid(name)),
    ], '独立操作区域');
    resultEl.textContent = `已把独立操作区域「${name}」加入识别结果；勾选后点「保存素材」。`;
  });
  // --- M2/N37: 调度面板 + 重置记忆 ---
  if (btnSchedRefresh) btnSchedRefresh.addEventListener('click', refreshScheduler);
  if (btnSchedReset) btnSchedReset.addEventListener('click', openResetMemoryDialog);
  if (btnResetAll) btnResetAll.addEventListener('click', () => resetTaskChecks('all'));
  if (btnResetInvert) btnResetInvert.addEventListener('click', () => resetTaskChecks('invert'));
  if (btnResetNone) btnResetNone.addEventListener('click', () => resetTaskChecks('none'));
  if (btnResetSubmit) btnResetSubmit.addEventListener('click', submitResetMemory);
  if (btnResetCancel) btnResetCancel.addEventListener('click', () => {
    if (resetMemoryDialog) resetMemoryDialog.close();
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
  // §22.22 全局素材库
  if (btnLibrarySave) btnLibrarySave.addEventListener('click', saveLibraryAsset);
  if (btnLibraryRefresh) btnLibraryRefresh.addEventListener('click', () => loadLibrary());
  if (btnLibraryMatch) btnLibraryMatch.addEventListener('click', matchLibrary);
  await loadLibrary();
  // §22.27 会话图片列表
  if (btnFrameUpload) btnFrameUpload.addEventListener('click', uploadFrames);
  if (btnFrameCapture) btnFrameCapture.addEventListener('click', openCaptureFrameDialog);
  if (btnFrameRelations) btnFrameRelations.addEventListener('click', openFrameAssetRelations);
  const relationsClose = document.getElementById('relations-close');
  if (relationsClose) relationsClose.addEventListener('click', () => {
    if (relationsDialog) relationsDialog.close();
  });
  const relationsRefresh = document.getElementById('relations-refresh');
  if (relationsRefresh) relationsRefresh.addEventListener('click', loadFrameAssetRelations);
  const relationsAnnotate = document.getElementById('relations-annotate');
  if (relationsAnnotate) relationsAnnotate.addEventListener('click', openAnnotationDialog);
  const annotationTypeEl = document.getElementById('annotation-type');
  if (annotationTypeEl) annotationTypeEl.addEventListener('change', fillAnnotationOptions);
  const annotationClose = document.getElementById('annotation-close');
  if (annotationClose) annotationClose.addEventListener('click', () => {
    if (annotationDialog) annotationDialog.close();
  });
  const annotationSubmit = document.getElementById('annotation-submit');
  if (annotationSubmit) annotationSubmit.addEventListener('click', submitRelationAnnotation);
  const btnRelationRecord = document.getElementById('btn-relation-record');
  if (btnRelationRecord) btnRelationRecord.addEventListener('click', openRelationRecordDialog);
  const relationRecordClose = document.getElementById('relation-record-close');
  if (relationRecordClose) relationRecordClose.addEventListener('click', () => {
    if (relationRecordDialog) relationRecordDialog.close();
  });
  if (relationRecordSubmit) relationRecordSubmit.addEventListener('click', submitRelationRecord);
  if (captureCancel) captureCancel.addEventListener('click', () => {
    if (capturePreviewImg) capturePreviewImg.removeAttribute('src');
    if (captureDialog) captureDialog.close();
  });
  if (captureDialog) captureDialog.addEventListener('close', () => {
    if (capturePreviewImg) capturePreviewImg.removeAttribute('src');
  });
  if (captureDialog) captureDialog.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter' || e.defaultPrevented) return;
    const tag = e.target && e.target.tagName ? e.target.tagName : '';
    // Input Enter is consumed above to confirm the field; button Enter is the
    // native click on 保存截图. Anywhere else on the dialog = save directly.
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || tag === 'BUTTON') return;
    e.preventDefault();
    submitCaptureFrame();
  });
  if (captureSubmit) captureSubmit.addEventListener('click', submitCaptureFrame);
  if (btnFramesRefresh) btnFramesRefresh.addEventListener('click', () => refreshFrames());
  await refreshAssetPositionFrames();
  const btnFrameCategoryApply = document.getElementById('btn-frame-category-apply');
  if (btnFrameCategoryApply) btnFrameCategoryApply.addEventListener('click', applyFrameCategoryFromSelect);
  const btnFrameCategoryRename = document.getElementById('btn-frame-category-rename');
  if (btnFrameCategoryRename) btnFrameCategoryRename.addEventListener('click', renameFrameCategory);
  if (btnFramesClear) btnFramesClear.addEventListener('click', () => clearFrames());
  if (btnCanvasPanels) {
    btnCanvasPanels.addEventListener('click', () => {
      if (!canvasWindows) return;
      const collapsed = canvasWindows.classList.toggle('collapsed');
      btnCanvasPanels.textContent = collapsed ? '显示结果/日志' : '收起结果/日志';
      fitCanvasToStage();
    });
  }
  if (canvasStage && window.ResizeObserver) {
    new ResizeObserver(() => fitCanvasToStage()).observe(canvasStage);
  }
  window.addEventListener('resize', fitCanvasToStage);
  fitCanvasToStage();
  await refreshFrames();
  // §22.22 deep link: /?lib=<id> selects that library asset and runs the match.
  try {
    const libParam = new URLSearchParams(location.search).get('lib');
    if (libParam && librarySelect) {
      librarySelect.value = libParam;
      await matchLibrary();
    }
  } catch (e) {
    log('素材库深链失败：' + e);
  }
  // N2: 生成点击指令 / 追加到 flow
  if (btnClickStep) btnClickStep.addEventListener('click', doGenerateClickStep);
  if (btnAppendStep) btnAppendStep.addEventListener('click', doAppendStepToFlow);
  if (btnBatchSave) btnBatchSave.addEventListener('click', saveSelectedCandidates);
  if (btnAssetsMigrate) {
    btnAssetsMigrate.addEventListener('click', async () => {
      if (!window.confirm('把现有所有任务素材和旧素材库迁移到统一素材库？\n\n重名会自动加后缀，并按任务重写 flow 引用；迁移报告会生成。')) return;
      if (assetsMigrateStatus) assetsMigrateStatus.textContent = '迁移中…';
      try {
        const r = await postJson('/assets/migrate', {});
        if (!r.ok) {
          if (assetsMigrateStatus) assetsMigrateStatus.textContent = '迁移失败：' + (r.errors || []).join('; ');
          return;
        }
        const rep = r.report || {};
        if (assetsMigrateStatus) {
          assetsMigrateStatus.textContent =
            `迁移完成：${rep.assets || 0} 素材 / ${rep.tasks || 0} 任务 / ${rep.flows_rewritten || 0} flow，重命名 ${(rep.renamed || []).length} 个`;
        }
        await refreshTaskAssets();
        await refreshTaskList(taskName.value);
      } catch (e) {
        if (assetsMigrateStatus) assetsMigrateStatus.textContent = '迁移异常：' + e;
      }
    });
  }
  if (btnAssetConflicts) {
    btnAssetConflicts.addEventListener('click', async () => {
      if (!assetConflicts) return;
      assetConflicts.textContent = '扫描中…';
      try {
        const r = await fetch('/assets/conflicts');
        const s = await r.json();
        if (!s.ok) throw new Error((s.errors || []).join('; ') || 'ok=false');
        assetConflicts.textContent = s.conflicts.length
          ? '重名（需处理）：\n' + s.conflicts.map((c) => `${c.id} ← ${c.sources.join(', ')}`).join('\n')
          : '没有跨任务/素材库的重名。';
      } catch (e) {
        assetConflicts.textContent = '扫描失败：' + (e && e.message ? e.message : e);
      }
    });
  }
  updateClickStepButtons();
  // N4: 点击分布 controls
  if (btnRunsRefresh) btnRunsRefresh.addEventListener('click', () => refreshRuns());
  if (btnClicksAggregate) btnClicksAggregate.addEventListener('click', () => loadAggregate());
  if (btnRunDelete) btnRunDelete.addEventListener('click', () => deleteSelectedRun());
  if (btnRunsTrashAll) btnRunsTrashAll.addEventListener('click', () => trashAllRuns());
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
    await loadFrame(true);
    await refreshState();
    await refreshFrames();
    startLivePreview();
    if (!ocrReady) {
      log('OCR 未加载：serve 需 `--features ocr` 构建并带 --model/--det-model（单行/多行识别不可用）');
    }
    log('已连接，实时画面已开启');
  } catch (e) {
    setStatus('没有可用帧，点「截取当前帧入库」');
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
  await refreshTaskAssets(deepTask || undefined);
  await loadTaskFlow((taskName.value || '').trim());
  // M1：任务列表下拉框（深链任务会被选中）。
  await refreshTaskList(taskName.value.trim());

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

  // --- v1.1 P3: app-level pages ---
  if (appNav) {
    for (const b of appNav.querySelectorAll('.app-nav-btn')) {
      b.addEventListener('click', () => activatePage(b.dataset.page));
    }
  }
  if (libRefresh) libRefresh.addEventListener('click', refreshLibrary);
  if (instRefresh) instRefresh.addEventListener('click', refreshInstance);
  if (instDevicesRefresh) instDevicesRefresh.addEventListener('click', refreshInstanceDevices);
  if (instDevicesSelect) instDevicesSelect.addEventListener('change', () => {
    if (instSerial && instDevicesSelect.value) instSerial.value = instDevicesSelect.value;
  });
  if (instSave) instSave.addEventListener('click', saveInstance);
  if (pageSchedRefresh) pageSchedRefresh.addEventListener('click', refreshSchedulePage);
  if (pageSchedReset) pageSchedReset.addEventListener('click', openResetMemoryDialog);
  if (settingsRefresh) settingsRefresh.addEventListener('click', refreshSettings);
  if (libNew) {
    libNew.addEventListener('click', () => {
      activatePage('workbench');
      openCreateTaskDialog();
    });
  }
  try {
    const wantPage = new URLSearchParams(location.search).get('page');
    if (wantPage && ['workbench', 'builder', 'tasks', 'instances', 'schedule', 'settings'].includes(wantPage)) {
      activatePage(wantPage);
    }
  } catch (e) {
    /* ignore */
  }
  refreshNavDevice();

  // M1：页面刷新时如果后台还有运行中的任务，继续轮询并把结果接回日志窗口。
  try {
    const r = await fetch('/task/run/status');
    const st = await r.json();
    if (st.ok && st.running) {
      log(`检测到后台任务仍在运行：${st.task}（${st.mode || ''}），继续等待结果…`);
      renderRunStatus(st);
      pollRunStatus();
    } else if (st.ok && st.task) {
      setRunStatus(`上次运行：任务「${st.task}」→ ${st.outcome || '—'}（exit ${st.exit == null ? '—' : st.exit}）`);
    }
  } catch (e) {
    /* 状态路由不可用时忽略（编辑器其它功能不受影响） */
  }
})();
