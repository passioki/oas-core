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
const btnTestMode = document.getElementById('btn-test-mode');
const btnOcr = document.getElementById('btn-ocr');
const btnClear = document.getElementById('btn-clear');
const btnLines = document.getElementById('btn-lines');
const linesList = document.getElementById('lines-list');
// U-T4/S4：真实点测的候选下拉（#序号 score）；演练时不显示。
const candidateSelect = document.getElementById('candidate-select');
const btnMatch = document.getElementById('btn-match');
const taskName = document.getElementById('task-name');
const btnSaveAsset = document.getElementById('btn-save-asset');
// B: canvas recognition-ROI editor (material library 识别区). Disabled/hidden
// until a row enters edit mode; templates save through the overwrite path.
const btnSaveAssetRoi = document.getElementById('btn-save-asset-roi');
const btnSaveAssetActionRoi = document.getElementById('btn-save-asset-action-roi');
const assetActionEditLabel = document.getElementById('asset-action-edit-label');
// P0-1: visible label next to the save button naming the asset being edited;
// the button itself lives in #canvas-footer so every surface can see it.
const assetRoiEditLabel = document.getElementById('asset-roi-edit-label');
const btnClickStep = document.getElementById('btn-click-step');
const btnAppendStep = document.getElementById('btn-append-step');
const btnRegionClick = document.getElementById('btn-region-click');
const btnRegionDrag = document.getElementById('btn-region-drag');
const btnRegionSave = document.getElementById('btn-region-save');
const btnToolRecognition = document.getElementById('btn-tool-recognition');
const btnToolAction = document.getElementById('btn-tool-action');
const btnActionInvert = document.getElementById('btn-action-invert');
// U-T4/S2：使用端OCR文字配置项（ocr_var）「识别文字」对话框。
const assetTextDialog = document.getElementById('asset-text-dialog');
const assetTextTitle = document.getElementById('asset-text-title');
const assetTextHint = document.getElementById('asset-text-hint');
const assetTextIdRow = document.getElementById('asset-text-id-row');
const assetTextId = document.getElementById('asset-text-id');
const assetTextRuntime = document.getElementById('asset-text-runtime');
const assetTextParamKey = document.getElementById('asset-text-paramkey');
const assetTextLabel = document.getElementById('asset-text-label');
const assetTextFullScreen = document.getElementById('asset-text-fullscreen');
const assetTextRoi = document.getElementById('asset-text-roi');
const assetTextRoiX = document.getElementById('asset-text-roi-x');
const assetTextRoiY = document.getElementById('asset-text-roi-y');
const assetTextRoiW = document.getElementById('asset-text-roi-w');
const assetTextRoiH = document.getElementById('asset-text-roi-h');
const assetTextPositions = document.getElementById('asset-text-positions');
const assetTextError = document.getElementById('asset-text-error');
const assetTextSave = document.getElementById('asset-text-save');
const assetTextCancel = document.getElementById('asset-text-cancel');
// seq131：素材设置→加入集合 对话框。
const collectionAddDialog = document.getElementById('collection-add-dialog');
const collectionAddTitle = document.getElementById('collection-add-title');
const collectionAddHint = document.getElementById('collection-add-hint');
const collectionAddTarget = document.getElementById('collection-add-target');
const collectionAddIdRow = document.getElementById('collection-add-id-row');
const collectionAddId = document.getElementById('collection-add-id');
const collectionAddInfo = document.getElementById('collection-add-info');
const collectionAddError = document.getElementById('collection-add-error');
const collectionAddSave = document.getElementById('collection-add-save');
const collectionAddCancel = document.getElementById('collection-add-cancel');
// U-T4/S5：素材「操作区」递归对话框（op_region 链）。
const opRegionDialog = document.getElementById('op-region-dialog');
const opRegionTitle = document.getElementById('op-region-title');
const opRegionBreadcrumb = document.getElementById('op-region-breadcrumb');
const opRegionLevels = document.getElementById('op-region-levels');
const opRegionError = document.getElementById('op-region-error');
const opRegionSave = document.getElementById('op-region-save');
const opRegionCancel = document.getElementById('op-region-cancel');
// §22.27 会话图片列表（多帧）
const frameFile = document.getElementById('frame-file');
const btnFrameUpload = document.getElementById('btn-frame-upload');
const btnFrameCapture = document.getElementById('btn-frame-capture');
const btnFramesRefresh = document.getElementById('btn-frames-refresh');
const btnFramesClear = document.getElementById('btn-frames-clear');
const framesList = document.getElementById('frames-list');
const frameDetail = document.getElementById('frame-detail');
const frameSearch = document.getElementById('frame-search');
const frameSearchClear = document.getElementById('frame-search-clear');
const frameSearchStatus = document.getElementById('frame-search-status');
const frameAssetsStatus = document.getElementById('frame-assets-status');
const frameAssetsList = document.getElementById('frame-assets-list');
const frameDetails = document.getElementById('frame-details');
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
let captureNamePicker = null;
let captureCategoryPicker = null;
let captureBusy = false;
// §22.28 任务素材列表（搜索 + 分类筛选）
const assetSearch = document.getElementById('asset-search');
const assetCategoryFilter = document.getElementById('asset-category-filter');
const assetsList = document.getElementById('assets-list');
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
// §22.19 long-run stats
const btnStatsRefresh = document.getElementById('btn-stats-refresh');
const statsSummary = document.getElementById('stats-summary');
// §22.22 全局素材库（v1.2.3：只保留统一 assets/ 全局库；旧 library/ UI 已移除）
// `#btn-library-refresh` used to be bound to a DOM id that did not exist; the
// button now exists inside the 素材库（全局） panel and refreshes that panel.
const btnLibraryRefresh = document.getElementById('btn-library-refresh');
const globalAssetsStatus = document.getElementById('global-assets-status');
const globalAssetsList = document.getElementById('global-assets-list');
// Same two controls the task library has, scoped to the global cache:
// #global-asset-search (id search) and #global-asset-category-filter (category).
const globalAssetSearch = document.getElementById('global-asset-search');
const globalAssetCategoryFilter = document.getElementById('global-asset-category-filter');
const taskAssetsHint = document.getElementById('task-assets-hint');
const taskShowAllAssetsEl = document.getElementById('task-show-all-assets');
// Backend global mode labels /task/assets with this task field; the frontend
// must not rely on it alone for data routing, but it is the only scope marker
// the current server returns (GET /task/assets has no separate `scope` field).
const GLOBAL_SCOPE_TASK = '（全局素材库）';
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
// PR5: 发布任务包（Studio-only）。只产出 .oaspkg；安装/卸载/回滚在 Runner。
const taskPublishDialog = document.getElementById('task-publish-dialog');
const taskPublishOut = document.getElementById('task-publish-out');
const taskPublishList = document.getElementById('task-publish-list');
const taskPublishStatus = document.getElementById('task-publish-status');
const taskPublishError = document.getElementById('task-publish-error');
const taskPublishSubmit = document.getElementById('task-publish-submit');
let taskPublishTask = '';
let taskPublishBusy = false;
// PR1/PR6: Studio and Runner are separate entry points. Studio never performs
// scheduling / runtime operations and owns no scheduling page any more; deep
// links and legacy shortcuts hand the user over to the Runner instead of
// hitting Runner-only routes (which return 404 on the Studio surface). The
// Runner port below is the local default (config/live.json runner.port can move
// the Runner, and this hand-off is best-effort on the Studio side).
const RUNNER_BASE_URL = 'http://127.0.0.1:8789/';
function runnerBaseUrl() {
  return RUNNER_BASE_URL;
}
function openRunnerPage(hash) {
  const target = runnerBaseUrl() + (hash || '');
  try {
    window.open(target, '_blank', 'noopener');
  } catch (e) {
    log('打开 Runner 失败：' + e, 'WARN');
  }
  log('调度/运行运维已迁移到 Runner：' + target, 'INFO');
}
// M1 运行状态轮询：已消费的日志行数 / 是否已有轮询在跑（避免重复轮询）
let runLogSeen = 0;
let runPolling = false;

let img = null;
let livePreview = true;
// 素材列表的「点击测试 / 拖动测试」默认只演练，绝不误点设备。切到真实模式时
// 需要用户确认；每次真正发送前还有一次 window.confirm 二次确认。
let assetTestReal = false;
let liveTimer = null;
let liveBusy = false;
let liveSocket = null;
let liveObjectUrl = null;
let liveReconnectTimer = null;
// Live-preview passthrough: a pending mouse gesture on the live canvas.
let liveInputDrag = null;
let roi = null; // {x,y,w,h} 识别框（前端识别/裁剪区域）
let roiBack = null; // v1.2 操作区域矩形（保存为 action_shape）；null = 默认识别区域
// #10 统一框选工具（2026-11-06 批次B3b）：唯一画布工具状态。
// 'none' = 未进入工具（保留旧的选择/识别交互）；'recognition' = 识别区域工具；
// 'action' = 操作区域工具。两种工具内手势一致：拖动=矩形，单击=多边形顶点，
// 双击=收尾闭合；反选只对操作区域有意义（识别掩膜无补集语义）。
const CANVAS_TOOLS = ['none', 'recognition', 'action'];
const CANVAS_DRAG_THRESHOLD = 4; // 帧坐标像素：位移超过它才算“拖动=矩形”
let canvasTool = 'none';
let canvasGesture = null; // { start: {x,y} } 按下时记录，move 判定拖动/点击
let actionPoly = null; // 操作区域多边形顶点
let actionPolyClosed = false;
// seq137：操作区 shape 的归属（仅当前选中素材/候选/工具时绘制；防残留）。
let actionShapeOwnerId = '';
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
// C: the API scope (`task`) of the asset whose action region is being edited.
// Frame-detail rows can be global or task-only, so the save must reuse the scope
// the row was opened with instead of a global "current library" guess.
let pendingActionAssetScope = null;
// B: the asset whose recognition ROI is being edited on the canvas.
// { it, scopeTask } while active; null otherwise.
let editingRecognitionAsset = null;
// P0-1: task library shows the flow-referenced subset of the global store.
// `taskShowAllAssets` is the explicit "show the whole global library" escape
// hatch so unreferenced assets can never silently disappear.
let taskShowAllAssets = false;
let assetsGlobalMode = false; // last /task/assets response was the global store
let globalAssetsCache = []; // full global library rows (items + collections)
const batchStatus = document.getElementById('batch-status');
// N4 click-distribution state (§21): the run list, the loaded points + metrics,
// and a cached offscreen bitmap so draw() stays cheap.
let runs = [];
let clickData = null; // {file, clicks, distinct, points}
let clickOverlay = null;
// seq130（#2）：点击分布面板激活时的运行记录刷新节流。
let lastClicksPanelRefreshAt = 0;
// §22.28 任务素材列表（分组/搜索/行内测试）
let taskAssets = [];
let taskAssetsMeta = {};
// §22.22: serve 能力位（OCR 模型是否就绪），避免"无模型/无 feature 构建"静默 501
let ocrReady = true;
let detReady = true;
// §22.10 多边形框选：顶点为画面坐标；闭合后 roi = 多边形外接矩形。
// 运行时：点击区 = 匹配外接矩形；匹配/OCR 使用多边形掩膜（N10b）。
let polygon = null;
let polyClosed = false;
let polyDrag = null; // 正在拖动的顶点下标（闭合后编辑）
// #6（验收反馈）：素材已存的识别多边形仅供显示（独立颜色，蓝色虚线），与“正在
// 画的多边形”（polygon，橙色）和操作区（action_shape）互不混淆。
let storedRecognitionPolygon = null; // [{x,y}] | null

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

// seq130（#2）：点击记录归一化。后端 points 同时含 actual(x,y) 与 target(target_x,
// target_y)；旧记录可能只有其中一类，miss=true 表示未命中（actual 不可用）。
function normalizeClickPoint(row) {
  const p = row && typeof row === 'object' && !Array.isArray(row) ? row : {};
  const num = (v) => {
    const n = Number(v);
    return Number.isFinite(n) ? n : null;
  };
  const miss = p.miss === true;
  const x = miss ? null : num(p.x);
  const y = miss ? null : num(p.y);
  const targetX = num(p.target_x);
  const targetY = num(p.target_y);
  const sel = p.sel === undefined || p.sel === null ? '' : String(p.sel);
  const ts = num(p.ts_ms);
  return {
    ts_ms: ts === null ? 0 : ts,
    x: x,
    y: y,
    target_x: targetX,
    target_y: targetY,
    has_actual: x !== null && y !== null,
    has_target: targetX !== null && targetY !== null,
    miss: miss,
    sel: sel,
    seed: num(p.seed),
  };
}

// 只保留可绘制的记录（至少 actual 或 target 之一），并按 miss 分类。
function normalizeClickPoints(points) {
  const out = [];
  for (const row of Array.isArray(points) ? points : []) {
    const p = normalizeClickPoint(row);
    if (!p.has_actual && !p.has_target) continue;
    out.push(p);
  }
  return out;
}

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

// --- seq130（#1）：画布上「点击已有框 → 拖动整体平移」纯函数/状态 --------------
let boxMove = null;      // { type, start, origRoi, origPolygon, origActionPoly, origRoiBack, origInvert }
let boxMoveUndo = null;  // 最近一次已完成的移动（Esc 撤销用；保存/新操作后清空）
let boxMoveHover = '';

function pointInFrameRect(pt, rect) {
  if (!pt || !rect) return false;
  const x = Number(rect.x);
  const y = Number(rect.y);
  const w = Number(rect.w);
  const h = Number(rect.h);
  if (![x, y, w, h].every((v) => Number.isFinite(v)) || w <= 0 || h <= 0) return false;
  return pt.x >= x && pt.x <= x + w && pt.y >= y && pt.y <= y + h;
}

function pointInFramePolygon(pt, points) {
  if (!pt || !Array.isArray(points) || points.length < 3) return false;
  let inside = false;
  for (let i = 0, j = points.length - 1; i < points.length; j = i, i += 1) {
    const xi = Number(points[i].x);
    const yi = Number(points[i].y);
    const xj = Number(points[j].x);
    const yj = Number(points[j].y);
    if (![xi, yi, xj, yj].every((v) => Number.isFinite(v))) continue;
    const intersect = ((yi > pt.y) !== (yj > pt.y)) &&
      (pt.x < ((xj - xi) * (pt.y - yi)) / ((yj - yi) || Number.EPSILON) + xi);
    if (intersect) inside = !inside;
  }
  return inside;
}

function translateFrameRect(rect, dx, dy) {
  return { x: Number(rect.x) + dx, y: Number(rect.y) + dy, w: Number(rect.w), h: Number(rect.h) };
}

function translateFramePoints(points, dx, dy) {
  return (Array.isArray(points) ? points : []).map(function (p) {
    return { x: Number(p.x) + dx, y: Number(p.y) + dy };
  });
}

// 平移后把矩形/polygon 的 bbox 夹回画面内（返回夹紧后的几何与是否发生夹紧）。
function clampRectToFrame(rect, frameW, frameH) {
  const w = Math.max(1, Number(rect.w));
  const h = Math.max(1, Number(rect.h));
  let x = Number(rect.x);
  let y = Number(rect.y);
  const maxX = Math.max(0, frameW - w);
  const maxY = Math.max(0, frameH - h);
  x = Math.max(0, Math.min(maxX, x));
  y = Math.max(0, Math.min(maxY, y));
  return { rect: { x: x, y: y, w: w, h: h }, clamped: x !== Number(rect.x) || y !== Number(rect.y) };
}

function clampPointsToFrame(points, frameW, frameH) {
  const pts = Array.isArray(points) ? points : [];
  if (!pts.length) return { points: [], clamped: false };
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const p of pts) {
    minX = Math.min(minX, p.x);
    minY = Math.min(minY, p.y);
    maxX = Math.max(maxX, p.x);
    maxY = Math.max(maxY, p.y);
  }
  let dx = 0;
  let dy = 0;
  if (minX < 0) dx = -minX;
  else if (maxX > frameW) dx = frameW - maxX;
  if (minY < 0) dy = -minY;
  else if (maxY > frameH) dy = frameH - maxY;
  return {
    points: translateFramePoints(pts, dx, dy),
    clamped: dx !== 0 || dy !== 0,
  };
}

function hitTestActionShape(pt, shape) {
  if (!shape) return false;
  if (shape.kind === 'polygon') {
    const inside = pointInFramePolygon(pt, shape.points || []);
    return shape.invert ? !inside : inside;
  }
  if (shape.kind === 'rect' && Array.isArray(shape.roi) && shape.roi.length === 4) {
    const inside = pointInFrameRect(pt, { x: shape.roi[0], y: shape.roi[1], w: shape.roi[2], h: shape.roi[3] });
    return shape.invert ? !inside : inside;
  }
  return false;
}

function beginBoxMove(type, pt) {
  boxMove = {
    type: type,
    start: { x: pt.x, y: pt.y },
    moved: false,
    origRoi: roi ? { x: roi.x, y: roi.y, w: roi.w, h: roi.h } : null,
    origPolygon: polygon ? polygon.map(function (p) { return { x: p.x, y: p.y }; }) : null,
    origActionPoly: actionPoly ? actionPoly.map(function (p) { return { x: p.x, y: p.y }; }) : null,
    origRoiBack: roiBack ? { x: roiBack.x, y: roiBack.y, w: roiBack.w, h: roiBack.h } : null,
    origInvert: actionInvert,
  };
  boxMoveUndo = null;
  setStatus('已选中框：按住拖动=整体移动；Esc=取消', '#e8c46a');
}

function applyBoxMove(pt) {
  if (!boxMove) return false;
  const dx = pt.x - boxMove.start.x;
  const dy = pt.y - boxMove.start.y;
  if (Math.abs(dx) < 1 && Math.abs(dy) < 1) return false;
  boxMove.moved = true;
  const frameW = canvas.width || 1280;
  const frameH = canvas.height || 720;
  if (boxMove.type === 'action') {
    if (boxMove.origActionPoly) {
      const moved = clampPointsToFrame(translateFramePoints(boxMove.origActionPoly, dx, dy), frameW, frameH);
      actionPoly = moved.points;
      roiBack = null;
      if (moved.clamped) setStatus('操作区域已到画面边界（已夹紧）', '#e8c46a');
    } else if (boxMove.origRoiBack) {
      const moved = clampRectToFrame(translateFrameRect(boxMove.origRoiBack, dx, dy), frameW, frameH);
      roiBack = moved.rect;
      actionPoly = null;
      if (moved.clamped) setStatus('操作区域已到画面边界（已夹紧）', '#e8c46a');
    }
  } else if (boxMove.type === 'recognitionPolygon') {
    const moved = clampPointsToFrame(translateFramePoints(boxMove.origPolygon, dx, dy), frameW, frameH);
    polygon = moved.points;
    polyClosed = true;
    roi = polygonBBox(polygon);
    storedRecognitionPolygon = null;
    if (moved.clamped) setStatus('识别多边形已到画面边界（已夹紧）', '#e8c46a');
  } else if (boxMove.type === 'recognitionRoi' && boxMove.origRoi) {
    const moved = clampRectToFrame(translateFrameRect(boxMove.origRoi, dx, dy), frameW, frameH);
    roi = moved.rect;
    if (moved.clamped) setStatus('识别区已到画面边界（已夹紧）', '#e8c46a');
  }
  updateRoiLabel();
  draw();
  return true;
}

function finishBoxMove() {
  if (!boxMove) return false;
  const moved = boxMove.moved;
  if (moved) {
    boxMoveUndo = {
      type: boxMove.type,
      roi: boxMove.origRoi,
      polygon: boxMove.origPolygon,
      actionPoly: boxMove.origActionPoly,
      roiBack: boxMove.origRoiBack,
      invert: boxMove.origInvert,
    };
    setStatus('框已移动：点对应「保存」写入素材；Esc 撤销本次移动', '#e8c46a');
    log(`画布框移动完成：${boxMove.type}（未保存，Esc 可撤销）`);
  }
  boxMove = null;
  draw();
  return moved;
}

function cancelBoxMove() {
  if (boxMove) {
    // 拖动中取消：恢复按下时的几何。
    const orig = boxMove;
    boxMove = null;
    if (orig.type === 'action') {
      actionPoly = orig.origActionPoly;
      roiBack = orig.origRoiBack;
      actionInvert = orig.origInvert;
    } else if (orig.type === 'recognitionPolygon') {
      polygon = orig.origPolygon;
      polyClosed = !!orig.origPolygon;
      roi = orig.origPolygon && orig.origPolygon.length >= 3 ? polygonBBox(orig.origPolygon) : orig.origRoi;
    } else if (orig.type === 'recognitionRoi') {
      roi = orig.origRoi;
    }
    updateRoiLabel();
    draw();
    setStatus('已取消框移动');
    return true;
  }
  if (boxMoveUndo) {
    const u = boxMoveUndo;
    boxMoveUndo = null;
    if (u.type === 'action') {
      actionPoly = u.actionPoly;
      roiBack = u.roiBack;
      actionInvert = u.invert;
    } else if (u.type === 'recognitionPolygon') {
      polygon = u.polygon;
      polyClosed = !!u.polygon;
      roi = u.polygon && u.polygon.length >= 3 ? polygonBBox(u.polygon) : u.roi;
    } else if (u.type === 'recognitionRoi') {
      roi = u.roi;
    }
    updateRoiLabel();
    draw();
    setStatus('已撤销框移动');
    log('已撤销上一次画布框移动');
    return true;
  }
  return false;
}

document.addEventListener('keydown', function (ev) {
  if (ev.key !== 'Escape') return;
  const page = document.getElementById('page-workbench');
  if (!page || !page.classList.contains('active')) return;
  if (cancelBoxMove()) {
    ev.preventDefault();
  }
});

function draw() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (img) ctx.drawImage(img, 0, 0);
  // seq134（UI B）：有识别多边形时多边形为主显示，矩形 ROI 仅作 bbox 辅助
  // （细虚线、不填充），候选/识别结果/选中素材共用这一层。
  const recognitionPolygonPrimary =
    (Array.isArray(storedRecognitionPolygon) && storedRecognitionPolygon.length >= 3) ||
    (Array.isArray(polygon) && polygon.length >= 3 && polyClosed);
  if (roi && roi.w > 0 && roi.h > 0) {
    ctx.save();
    if (recognitionPolygonPrimary) {
      ctx.setLineDash([3, 3]);
      ctx.strokeStyle = 'rgba(49,208,170,0.45)';
      ctx.lineWidth = 1;
      ctx.strokeRect(roi.x + 0.5, roi.y + 0.5, roi.w, roi.h);
      ctx.setLineDash([]);
      ctx.fillStyle = 'rgba(49,208,170,0.65)';
      ctx.font = '12px Consolas, monospace';
      ctx.fillText(`ROI bbox（辅助） ${roi.w}x${roi.h}`, roi.x + 4, Math.max(13, roi.y - 5));
    } else {
      ctx.strokeStyle = '#31d0aa';
      ctx.lineWidth = 2;
      ctx.strokeRect(roi.x + 0.5, roi.y + 0.5, roi.w, roi.h);
      ctx.fillStyle = 'rgba(49,208,170,0.12)';
      ctx.fillRect(roi.x, roi.y, roi.w, roi.h);
      ctx.fillStyle = '#31d0aa';
      ctx.font = '14px Consolas, monospace';
      ctx.fillText(`识别 ${roi.w}x${roi.h} @ ${roi.x},${roi.y}`, roi.x + 4, Math.max(14, roi.y - 6));
    }
    ctx.restore();
  }
  // #6（验收反馈）：素材已存的识别多边形只读显示（蓝色虚线，独立于橙色正在画的
  // 多边形与操作区域），矩形 ROI 仍照常显示。
  drawStoredRecognitionPolygonOverlay();
  // §22.25 / v1.2.4 操作区域（action_shape）：矩形和多边形共用同一个渲染器，
  // 统一橙色虚线；反选时“有效区域”（屏幕−区域）与“原选中区域”用不同底色。
  drawActionShapeOverlay();
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
    const boxPoly = Array.isArray(b.polygon) && b.polygon.length >= 3 ? b.polygon : null;
    if (boxPoly) {
      // seq134（UI B）：候选框有 polygon 时以多边形为主（填充+亮边），矩形仅辅助。
      ctx.setLineDash([3, 3]);
      ctx.strokeStyle = 'rgba(0,0,0,0.6)';
      ctx.lineWidth = 1;
      ctx.strokeRect(x + 0.5, y + 0.5, w, h);
      ctx.beginPath();
      ctx.moveTo(boxPoly[0].x, boxPoly[0].y);
      for (let i = 1; i < boxPoly.length; i += 1) ctx.lineTo(boxPoly[i].x, boxPoly[i].y);
      ctx.closePath();
      ctx.setLineDash([4, 3]);
      ctx.strokeStyle = 'rgba(0,0,0,0.85)';
      ctx.lineWidth = 5;
      ctx.stroke();
      ctx.strokeStyle = '#ffd54a';
      ctx.lineWidth = 2;
      ctx.stroke();
      ctx.fillStyle = 'rgba(255,213,74,0.14)';
      ctx.fill();
      ctx.setLineDash([]);
    } else {
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
    }
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
  // Draw order (§21): frame -> ROI/boxes -> click heatmap + scatter. Click
  // metrics/legend deliberately stay in the sidebar (#click-stats /
  // #clicks-legend): no on-canvas plate covers the game's top-left corner.
  drawClickOverlay();
}

// #6（验收反馈）：已存识别多边形只读渲染层。seq134（UI B）：有 polygon 时它是
// 主显示（更实的填充+轮廓），矩形 ROI 仅 bbox 辅助；颜色与可编辑几何区分：
//   蓝色 = 已存识别多边形（本层，只读）
//   橙色 = 正在画的多边形 / 操作区域 action_shape
function drawStoredRecognitionPolygonOverlay() {
  const pts = storedRecognitionPolygon;
  if (!Array.isArray(pts) || pts.length < 3) return;
  ctx.save();
  ctx.beginPath();
  ctx.moveTo(pts[0].x, pts[0].y);
  for (let i = 1; i < pts.length; i += 1) ctx.lineTo(pts[i].x, pts[i].y);
  ctx.closePath();
  ctx.fillStyle = 'rgba(77,159,255,0.20)';
  ctx.fill();
  ctx.setLineDash([5, 3]);
  ctx.strokeStyle = '#4d9fff';
  ctx.lineWidth = 2.5;
  ctx.stroke();
  ctx.setLineDash([]);
  const bbox = polygonBBox(pts);
  ctx.fillStyle = '#8fb8e8';
  ctx.font = '13px Consolas, monospace';
  ctx.fillText('识别区域（多边形） ' + pts.length + ' 顶点', bbox.x + 4, Math.max(14, bbox.y - 6));
  ctx.restore();
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
  const toolNote = canvasTool === 'recognition'
    ? ' · 工具：识别区域（拖动=矩形 / 单击=多边形顶点 / 双击收尾）'
    : canvasTool === 'action'
      ? ' · 工具：操作区域（拖动=矩形 / 单击=多边形顶点 / 双击收尾）'
      : '';
  if (!roi) {
    if (canvasTool === 'action' && (aShape || polygon || actionPoly)) {
      // 全屏 ocr_var / 未绑定识别区时的操作区域编辑：没有识别框也要能画/看操作区。
      roiEl.textContent = '识别框：未选择' + backInfo.replace(' · 操作区域未设置（=真实识别结果）', '') + toolNote;
      btnOcr.disabled = true;
      btnLines.disabled = true;
      if (btnMatch) btnMatch.disabled = true;
      if (btnRegionClick) btnRegionClick.disabled = true;
      if (btnRegionDrag) btnRegionDrag.disabled = true;
      if (btnRegionSave) btnRegionSave.disabled = true;
      return;
    }
    roiEl.textContent = (polygon
      ? `多边形：${polygon.length} 个顶点（未闭合）`
      : '未选择') + toolNote;
    btnOcr.disabled = true;
    btnLines.disabled = true;
    if (btnMatch) btnMatch.disabled = true;
    if (btnRegionClick) btnRegionClick.disabled = true;
    if (btnRegionDrag) btnRegionDrag.disabled = true;
    if (btnRegionSave) btnRegionSave.disabled = true;
    return;
  }
  const storedPolyInfo = Array.isArray(storedRecognitionPolygon) && storedRecognitionPolygon.length >= 3
    ? ` · 已存识别多边形 ${storedRecognitionPolygon.length} 顶点（只读显示）`
    : '';
  const polyInfo =
    polygon && polyClosed ? ` · 多边形 ${polygon.length} 顶点（识别掩膜）` : '';
  roiEl.textContent =
    `识别框 x=${roi.x} y=${roi.y} w=${roi.w} h=${roi.h}${polyInfo}${storedPolyInfo}${backInfo}${toolNote}`;
  btnOcr.disabled = roi.w < 2 || roi.h < 2;
  btnLines.disabled = roi.w < 2 || roi.h < 2;
  if (btnRegionClick) btnRegionClick.disabled = roi.w < 2 || roi.h < 2;
  if (btnRegionDrag) btnRegionDrag.disabled = roi.w < 2 || roi.h < 2;
  if (btnRegionSave) btnRegionSave.disabled = roi.w < 2 || roi.h < 2;
  if (btnMatch) btnMatch.disabled = roi.w < 2 || roi.h < 2;
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
  // v1.2.2: the workbench flow panel was removed; flow editing lives in
  // 任务构建. Copy the generated step to the clipboard instead of mutating a
  // hidden textarea.
  const json = JSON.stringify(step, null, 2);
  resultEl.textContent = `已复制${label || '步骤'}到剪贴板（到「任务构建」粘贴/添加）：\n${json}`;
  log(`已复制${label || '步骤'}到剪贴板（任务构建编辑）`);
  if (navigator.clipboard) navigator.clipboard.writeText(json).catch(() => {});
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
  // D 批（去参数）：battles 循环次数改为字面量（goal 由作者定，不再生成
  // param('max_battles')；旧 flow 若已有 loop n=param('max_battles') 仍可运行）。
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
      { op: 'loop', args: { n: goal.count }, body: actions },
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
    `flow 已生成 loop n=${goal.count}（字面量循环次数），并保留出口 ${flow.steps[1].args.exit}。`;
  if (typeof loadTaskFlow === 'function') await loadTaskFlow(task);
  log(`完成条件：${task} 打 ${goal.count} 场（loop n=${goal.count} 字面量）`);
}

// --- v1.1 P2: 任务工作台导航 -----------------------------------------------
// --- v1.1 P3: 应用级页面（工作台/任务库/实例/设置） ---------------------------
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
      // PR5: publish only produces a .oaspkg (Studio-side); installation is
      // Runner-only, so this button never calls /runner/*.
      const publishBtn = document.createElement('button');
      publishBtn.type = 'button';
      publishBtn.textContent = '发布…';
      publishBtn.title = '发布为 .oaspkg（含任务、子任务依赖与引用素材快照；终端用户在 Runner 安装）';
      publishBtn.addEventListener('click', () => openTaskPublishDialog(t.name));
      // PR1: scheduling configuration and run history belong to the Runner
      // surface. Studio only links over; it must not call /scheduler/config or
      // /runs/scheduler (Runner-only routes answer 404 on Studio).
      const runsBtn = document.createElement('button');
      runsBtn.type = 'button';
      runsBtn.textContent = '运行记录';
      runsBtn.title = '在 Runner 打开该任务的运行记录';
      runsBtn.addEventListener('click', () => openRunnerPage('#runs'));
      const del = document.createElement('button');
      del.type = 'button';
      del.textContent = '删除';
      del.addEventListener('click', () => deleteTaskFromLibrary(t.name));
      card.append(meta, open, publishBtn, runsBtn, del);
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


// D 批（去参数 UI）：Studio 用户界面不再暴露参数声明/编辑入口（任务库「配置项…」、
// #builder-params 面板、#task-config-dialog 已删除）。仅保留内部数据桥：builder 的
// 派生参数诊断 / ocr_var 保存后的 metadata 刷新使用；不再注册任何用户入口。
async function fetchTaskConfigParams(name) {
  const task = String(name === undefined || name === null ? "" : name).trim();
  if (!task) return [];
  const r = await fetch("/task/config?task=" + encodeURIComponent(task));
  const s = await r.json();
  if (!s.ok) throw new Error((s.errors || []).join("; ") || "配置项读取失败");
  return Array.isArray(s.params) ? s.params : [];
}

window.OAS_TASK_CONFIG = {
  refresh: fetchTaskConfigParams,
};
// --- PR5: 发布任务包（Studio-only；.oaspkg 由 Runner 安装） ------------------
// 下面 3 个纯函数被 ui_pure_test.js 直接抽取执行，不能依赖 DOM / 全局状态。

// POST /task/publish body. `out` is omitted when blank (server default
// dist/packages applies); include_deps defaults to true, force only travels
// when explicitly checked. reproducible is accepted for CLI parity.
function publishRequestBody(task, opts) {
  const name = String(task === undefined || task === null ? '' : task).trim();
  const o = opts && typeof opts === 'object' ? opts : {};
  const body = { task: name };
  const out = String(o.out === undefined || o.out === null ? '' : o.out).trim();
  if (out) body.out = out;
  if (o.force === true) body.force = true;
  body.include_deps = o.include_deps !== false;
  if (o.reproducible === true) body.reproducible = true;
  return body;
}

// One copyable line for a published package. The server may omit every field
// except path, so each part is optional and never renders "undefined".
function formatPackageSummary(pkg) {
  const p = pkg && typeof pkg === 'object' ? pkg : {};
  const text = (v) => (v === undefined || v === null || v === '' ? '' : String(v));
  const parts = [];
  parts.push(text(p.path) || '（服务端未返回 path）');
  const version = text(p.task_version !== undefined ? p.task_version : p.version);
  if (version) parts.push(/^v/i.test(version) ? version : 'v' + version);
  const packageId = text(p.package_id);
  if (packageId) {
    parts.push(packageId.length > 32 ? packageId.slice(0, 32) + '…' : packageId);
  }
  const sha = text(p.sha256);
  if (sha) parts.push('sha256 ' + (sha.length > 12 ? sha.slice(0, 12) + '…' : sha));
  return parts.join(' · ');
}

// 400/409 errors are rendered line by line; an empty body still says something
// actionable instead of a bare blank panel.
function publishErrorsText(errors) {
  let list;
  if (Array.isArray(errors)) {
    list = errors
      .filter((e) => e !== undefined && e !== null && String(e).trim() !== '')
      .map((e) => String(e).trim());
  } else if (errors === undefined || errors === null || String(errors).trim() === '') {
    list = [];
  } else {
    list = [String(errors).trim()];
  }
  if (!list.length) return '（服务端未返回错误详情）';
  return list.map((line) => '· ' + line).join('\n');
}

// P0-1a: POST /task/publish 成功响应的 exchange 结果（Studio 写发布通知）。
// 三种情况都不改变发布本身的结果：已写入（Runner 将看到）/ 未启用（Runner
// 不会自动收到）/ 写通知失败（仍然发布成功，只是 Runner 需手动安装）。
// 后端未实现 exchange 时字段缺失 -> 返回空串，调用方不追加任何行。
function publishExchangeText(exchange) {
  const ex = exchange && typeof exchange === 'object' && !Array.isArray(exchange) ? exchange : null;
  if (!ex) return '';
  const text = (v) => (v === undefined || v === null || v === '' ? '' : String(v).trim());
  const error = text(ex.error);
  if (ex.enabled !== true) {
    return '未启用 exchange，Runner 不会自动收到（可把 .oaspkg 拷到 Runner 后手动安装）';
  }
  if (error) {
    return '发布成功，但写通知失败：' + error + '（Runner 需手动安装）';
  }
  const eventId = text(ex.event_id);
  const indexPath = text(ex.index_path);
  const target = indexPath || eventId || '—';
  const copied = ex.copied === false ? ' · 包已存在，未重复复制' : '';
  return '已写入发布通知：' + target + '（Runner 将看到）' + copied;
}

function closeTaskPublishDialog() {
  if (!taskPublishDialog) return;
  if (typeof taskPublishDialog.close === 'function') {
    if (taskPublishDialog.open) taskPublishDialog.close();
  } else {
    taskPublishDialog.removeAttribute('open');
  }
}

async function loadPublishedPackages(task) {
  if (!taskPublishList) return;
  taskPublishList.textContent = '读取中…';
  try {
    const r = await fetch('/task/package?task=' + encodeURIComponent(task));
    const s = await r.json();
    if (taskPublishTask !== task) return; // 已关闭/切换任务，丢弃过期响应
    if (!s.ok) {
      taskPublishList.textContent =
        '已发布列表读取失败：' + ((s.errors || []).join('；') || '未知错误');
      return;
    }
    const packages = Array.isArray(s.packages) ? s.packages : [];
    taskPublishList.textContent = '';
    if (!packages.length) {
      taskPublishList.textContent = '尚未发布';
      return;
    }
    for (const pkg of packages) {
      const row = document.createElement('div');
      row.className = 'task-publish-row';
      row.textContent = formatPackageSummary(pkg);
      taskPublishList.appendChild(row);
    }
  } catch (e) {
    if (taskPublishTask !== task) return;
    taskPublishList.textContent = '已发布列表读取异常：' + e;
  }
}

function openTaskPublishDialog(task) {
  const name = String(task === undefined || task === null ? '' : task).trim();
  if (!taskPublishDialog || !name) return;
  taskPublishTask = name;
  const title = taskPublishDialog.querySelector('h3');
  if (title) title.textContent = '发布任务包 · ' + name;
  if (taskPublishOut) taskPublishOut.value = '';
  if (taskPublishList) taskPublishList.textContent = '读取中…';
  if (taskPublishStatus) {
    taskPublishStatus.textContent = '—';
    taskPublishStatus.className = 'muted small task-publish-result';
  }
  if (taskPublishError) taskPublishError.textContent = '';
  if (taskPublishSubmit) taskPublishSubmit.disabled = taskPublishBusy;
  const includeDeps = taskPublishDialog.querySelector('input[name="include_deps"]');
  if (includeDeps) includeDeps.checked = true;
  const force = taskPublishDialog.querySelector('input[name="force"]');
  if (force) force.checked = false;
  if (typeof taskPublishDialog.showModal === 'function') {
    if (!taskPublishDialog.open) taskPublishDialog.showModal();
  } else {
    taskPublishDialog.setAttribute('open', '');
  }
  loadPublishedPackages(name);
}

async function submitTaskPublish() {
  if (taskPublishBusy || !taskPublishTask) return;
  const name = taskPublishTask;
  const includeDeps = taskPublishDialog
    ? taskPublishDialog.querySelector('input[name="include_deps"]')
    : null;
  const force = taskPublishDialog
    ? taskPublishDialog.querySelector('input[name="force"]')
    : null;
  const body = publishRequestBody(name, {
    out: taskPublishOut ? taskPublishOut.value : '',
    include_deps: includeDeps ? includeDeps.checked : true,
    force: force ? force.checked : false,
  });
  taskPublishBusy = true;
  if (taskPublishSubmit) taskPublishSubmit.disabled = true;
  if (taskPublishError) taskPublishError.textContent = '';
  if (taskPublishStatus) {
    taskPublishStatus.textContent = '发布中…（校验 / 打包 / 计算 sha256）';
    taskPublishStatus.className = 'muted small task-publish-result';
  }
  try {
    const s = await postJson('/task/publish', body);
    if (!s.ok) {
      const errors = Array.isArray(s.errors) ? s.errors : [];
      let message = publishErrorsText(errors);
      if (errors.some((e) => /exists/i.test(String(e)))) {
        message = '包已存在，勾选 force 或提升版本。\n' + message;
      }
      if (taskPublishError) taskPublishError.textContent = message;
      if (taskPublishStatus) {
        taskPublishStatus.textContent = '发布失败';
        taskPublishStatus.className = 'muted small task-publish-result err';
      }
      return;
    }
    const lines = ['已发布：' + (s.path || '（服务端未返回路径）')];
    if (s.package_id) lines.push('package_id：' + s.package_id);
    lines.push('sha256：' + (s.sha256 || '—'));
    const warnings = Array.isArray(s.warnings) ? s.warnings : [];
    lines.push(warnings.length
      ? 'warnings：\n' + warnings.map((w) => '· ' + String(w)).join('\n')
      : 'warnings：无');
    // P0-1a: exchange（发布通知）结果追加一行；字段缺失/未启用/写失败都不影响发布成功。
    const exchangeLine = publishExchangeText(s.exchange);
    if (exchangeLine) lines.push(exchangeLine);
    if (taskPublishStatus) {
      taskPublishStatus.textContent = lines.join('\n');
      taskPublishStatus.className = 'muted small task-publish-result ok';
    }
    log(`已发布任务包：${s.path || name}`);
    await loadPublishedPackages(name);
  } catch (e) {
    if (taskPublishError) taskPublishError.textContent = '发布异常：' + e;
    if (taskPublishStatus) {
      taskPublishStatus.textContent = '发布失败';
      taskPublishStatus.className = 'muted small task-publish-result err';
    }
  } finally {
    taskPublishBusy = false;
    if (taskPublishSubmit) taskPublishSubmit.disabled = false;
  }
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

// v1.2.5 (O-2): build the `drag` step for a region drag. `shape` is the current
// operation shape in frame coordinates ({kind,roi|points,invert}) or null.
//
// - `from.roi` is always the shape's bbox when a shape exists (the user's
//   roiBack / polygon bbox), falling back to the recognized ROI only when the
//   shape has no usable geometry. This is what older runtimes and the bbox
//   path sample, so the user's marked area is never silently dropped.
// - `from.shape` is inlined only for a polygon or an inverted shape: a plain
//   non-inverted rect is already fully described by `roi` (runtime samples that
//   bbox with margin), so the historical bbox-only output is preserved there.
// Pure so the contract is testable without the DOM.
function regionDragStep(roi, shape, direction, distance) {
  const box = shape ? actionShapeBBox(shape) || roi : roi;
  const from = { kind: 'region', roi: box };
  if (shape && (shape.kind === 'polygon' || shape.invert)) from.shape = shape;
  return {
    op: 'drag',
    args: {
      from,
      direction,
      distance: Array.isArray(distance) && distance.length === 2 ? distance.slice() : [150, 500],
      duration_ms: [300, 650],
      margin: 0.15,
    },
  };
}

function generateRegionDrag() {
  const r = regionRoiArray();
  if (!r) return;
  const direction = promptDragDirection();
  if (!direction) return;
  appendStepToFlow(regionDragStep(r, currentActionShape(), direction), '区域拖动');
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
    // A fresh recognition result follows the real recognized bbox until the user
    // explicitly draws/drags an operation area; `actionShape` is only written by
    // that explicit edit and is what the save payload carries (no two-state).
    actionShape: null,
    sourceRoi: sourceRoi ? { ...sourceRoi } : null,
    selected: selected !== false && candidateNameValid(name || text || ''),
    fromLines: !!fromLines,
  };
}

function replaceResults(rows, sourceLabel) {
  saveCandidates = rows || [];
  activeCandidate = -1;
  pendingActionAsset = null;
  updateAssetActionSaveButton();
  // New recognition results must never inherit an old action polygon/invert
  // from a previous candidate or asset (P0-3 / P1-8). The rect too: a fresh
  // candidate follows its own recognized bbox.
  resetActionGeometry({ keepTarget: true, clearInvert: true, clearRect: true, redraw: false });
  // #6：新的识别结果替换旧素材上下文，已存识别多边形显示层一并清掉。
  storedRecognitionPolygon = null;
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
    adjust.title = '在画布上拖拽/画多边形设置该素材的操作区域';
    adjust.addEventListener('click', () => {
      // P1-8: entering a mode resets stale geometry from the previous target.
      resetActionGeometry({ keepTarget: true, clearInvert: true, redraw: false });
      activeCandidate = idx;
      setCanvasTool('action');
      pendingActionAsset = null;
      // Show the candidate's existing operation shape (if any) while editing.
      if (c.actionShape) {
        loadActionShapeForEdit(c.actionShape);
      } else {
        roiBack = { ...c.action };
      }
      updateRoiLabel();
      draw();
      log(`编辑「${c.name || c.text}」的操作区域：在画布上拖拽或画多边形`);
    });
    const reset = document.createElement('button');
    reset.type = 'button';
    reset.textContent = '重置';
    reset.title = '重置操作区域（恢复为识别区域）';
    reset.addEventListener('click', () => {
      c.action = { ...c.recognize };
      c.actionShape = null;
      if (activeCandidate === idx) {
        roiBack = null;
        // Unified reset: geometry, invert and any active action mode all go.
        resetActionGeometry({ keepTarget: true, clearInvert: true, redraw: false });
      }
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
    // U-T4/S2：把该识别结果存为「使用端OCR文字配置项」（ocr_var）——新建入口。
    const saveOcrVar = document.createElement('button');
    saveOcrVar.type = 'button';
    saveOcrVar.textContent = '存为OCR文字配置';
    saveOcrVar.title = '保存为「使用端OCR文字配置项」（ocr_var）：使用端可改识别文字，值存 values.json';
    saveOcrVar.addEventListener('click', () => {
      const positions = selectedAssetPositions();
      if (!positions.length) {
        resultEl.textContent = '先在工作台选择素材位置帧（保存素材必须至少一个位置帧），再存为识别文字配置项。';
        setStatus('缺少位置帧', '#ff8a80');
        return;
      }
      openAssetTextDialog(null, '', { candidate: c, positions: positions });
    });
    actions.append(adjust, reset, gen, append, saveOcrVar);
    row.append(headRow, info, actions);
    linesList.appendChild(row);
  });
  candidateStatusText(sourceLabel);
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

// B-01 / P1-1: the runtime translates a saved template action_shape by
// `matched_roi - asset.roi` before sampling. The UI dry-run must apply the same
// translation, otherwise a template that moved on screen reports/points at the
// old operation area. OCR/region shapes are already frame-absolute and are not
// translated here.
function translateActionShape(shape, dx, dy) {
  if (!shape) return null;
  const ox = Number(dx) || 0;
  const oy = Number(dy) || 0;
  if (shape.kind === 'rect') {
    if (!Array.isArray(shape.roi) || shape.roi.length !== 4) return null;
    return {
      kind: 'rect',
      roi: [shape.roi[0] + ox, shape.roi[1] + oy, shape.roi[2], shape.roi[3]],
      invert: !!shape.invert,
    };
  }
  if (shape.kind === 'polygon' && Array.isArray(shape.points)) {
    return {
      kind: 'polygon',
      points: shape.points.map((p) => [p[0] + ox, p[1] + oy]),
      invert: !!shape.invert,
    };
  }
  return null;
}

// The operation region is the asset's own `action_shape`; absent = the runtime
// uses the real recognized box. The old recognition/fixed two-state switch is
// gone (user 2026-11-06 事项 6): `action_shape` alone is the operation region.
// region assets keep their operation-area semantics: their `roi` *is* the
// operation region, so they never carry an extra shape.
function candidateActionPayload(c) {
  if (!c || c.kind === 'region') return {};
  return c.actionShape ? { action_shape: c.actionShape } : {};
}

// P1-8 / #10（2026-11-06 批次B3b）：统一框选工具。旧的四状态选择模式
// （recognitionPolygon/actionPolygon/actionRect）已被 canvasTool +
// 手势判定取代：
//   - `canvasTool` 只有 none/recognition/action 三种；
//   - 同一工具内：拖动=矩形，单击=多边形顶点，双击=收尾闭合（`canvasGestureShape` 纯函数）；
//   - `setCanvasTool` 是唯一入口，`syncCanvasToolButtons` 是唯一写按钮的地方；
//   - 几何清理由 resetSelection/resetActionGeometry/各保存路径负责。
function canvasGestureShape(start, end, threshold) {
  if (!start || !end) return 'polygon';
  // 4 是 CANVAS_DRAG_THRESHOLD 的默认值；显式传参便于纯测试覆盖。
  const t = Number.isFinite(Number(threshold)) ? Number(threshold) : 4;
  const dx = Math.abs(Number(end.x) - Number(start.x));
  const dy = Math.abs(Number(end.y) - Number(start.y));
  if (!Number.isFinite(dx) || !Number.isFinite(dy)) return 'polygon';
  return dx > t || dy > t ? 'rect' : 'polygon';
}

// 三个工具按钮的文案/激活/禁用（纯函数，ui_pure_test 直接断言）。
// 反选只对操作区域有意义：识别区域是识别掩膜，补集没有业务语义。
function canvasToolButtonState(tool, invert) {
  const t = tool === 'recognition' || tool === 'action' ? tool : 'none';
  const active = t === 'recognition';
  const activeAction = t === 'action';
  return [
    {
      key: 'recognition',
      label: active ? '识别区域（点击退出）' : '识别区域',
      active,
      disabled: false,
    },
    {
      key: 'action',
      label: activeAction ? '操作区域（点击退出）' : '操作区域',
      active: activeAction,
      disabled: false,
    },
    {
      key: 'invert',
      label: '反选：' + (invert ? '开' : '关'),
      active: !!invert,
      disabled: !activeAction,
    },
  ];
}

function syncCanvasToolButtons() {
  const states = canvasToolButtonState(canvasTool, actionInvert);
  const target = {
    recognition: btnToolRecognition,
    action: btnToolAction,
    invert: btnActionInvert,
  };
  for (const st of states) {
    const btn = target[st.key];
    if (!btn) continue;
    btn.textContent = st.label;
    btn.disabled = st.disabled;
    if (btn.classList) btn.classList.toggle('active', st.active);
  }
}

function setCanvasTool(tool) {
  const t = CANVAS_TOOLS.indexOf(tool) >= 0 ? tool : 'none';
  canvasTool = t;
  canvasGesture = null;
  polyDrag = null;
  dragStart = null;
  dragTarget = 'roi';
  syncCanvasToolButtons();
  return t;
}

function activeCanvasTool() {
  return canvasTool;
}

// —— 识别区草稿/保存计划（#6/#10，纯函数） ----------------------------------
// 从素材行载入可编辑草稿：roi 矩形 + 已存识别多边形（可继续编辑）。
function recognitionDraftFromAsset(asset) {
  const it = asset && typeof asset === 'object' ? asset : {};
  const roi = Array.isArray(it.roi) && it.roi.length === 4
    ? it.roi.map(Number)
    : null;
  const list = Array.isArray(it.polygon) ? it.polygon : [];
  const polygon = list.length >= 3
    ? list
        .filter((p) => Array.isArray(p) && p.length === 2)
        .map((p) => [Number(p[0]), Number(p[1])])
    : null;
  return {
    roi,
    polygon: polygon && polygon.length >= 3 ? polygon : null,
  };
}

// 顶点列表的 inclusive 外接矩形 [x,y,w,h]（与 polygonBBox 同口径）。
function recognitionPolygonBBox(points) {
  if (!Array.isArray(points) || points.length < 3) return null;
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const p of points) {
    if (!Array.isArray(p) || p.length !== 2) return null;
    const x = Number(p[0]);
    const y = Number(p[1]);
    if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x);
    maxY = Math.max(maxY, y);
  }
  return [minX, minY, maxX - minX + 1, maxY - minY + 1];
}

function rectEq4(a, b) {
  if (!Array.isArray(a) || !Array.isArray(b) || a.length !== 4 || b.length !== 4) return false;
  return a.every((v, i) => Number(v) === Number(b[i]));
}

function polygonPointsEq(a, b) {
  if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    if (Number(a[i][0]) !== Number(b[i][0]) || Number(a[i][1]) !== Number(b[i][1])) return false;
  }
  return true;
}

// 识别区域保存计划：返回 {ok, action:'none'|'update'|'overwrite', request, roi, reason}。
//   - 多边形：非模板 → update {polygon}（服务端由 bbox 反推 roi）；
//             模板 → bbox==现有 ROI 走 update {polygon}（不重新截图），
//                    否则 overwrite {roi,polygon}（模板必须重新截图，rev+1）；
//   - 矩形：模板 → overwrite {roi,polygon:null}（清空旧掩膜；保留覆盖保存语义）；
//           非模板 → update {roi,polygon:null}；region → update {roi,keep_action_shape:true}。
function recognitionSavePlan(asset, draft) {
  const it = asset && typeof asset === 'object' ? asset : {};
  const kind = String(it.kind === undefined || it.kind === null ? '' : it.kind).trim();
  const stored = recognitionDraftFromAsset(it);
  const d = draft && typeof draft === 'object' ? draft : {};
  const poly = Array.isArray(d.polygon) && d.polygon.length >= 3
    ? d.polygon.map((p) => [Number(p[0]), Number(p[1])])
    : null;
  const noneReason = '识别区域未变化';
  if (kind === 'template_set') {
    return { ok: false, action: 'none', request: null, roi: null, reason: '素材集合没有识别区域' };
  }
  if (poly) {
    const bbox = recognitionPolygonBBox(poly);
    if (!bbox) {
      return { ok: false, action: 'none', request: null, roi: null, reason: '多边形顶点无效（需要 >=3 个 [x,y]）' };
    }
    if (kind === 'region') {
      return { ok: false, action: 'none', request: null, roi: bbox, reason: 'region 素材没有识别掩膜：请拖拽画矩形' };
    }
    if (polygonPointsEq(poly, stored.polygon) && rectEq4(bbox, stored.roi)) {
      return { ok: true, action: 'none', request: null, roi: bbox, reason: noneReason };
    }
    if (kind === 'template') {
      if (stored.roi && rectEq4(bbox, stored.roi)) {
        return { ok: true, action: 'update', request: { polygon: poly }, roi: bbox, reason: '' };
      }
      return {
        ok: true,
        action: 'overwrite',
        request: { roi: bbox, polygon: poly },
        roi: bbox,
        reason: '模板识别区变化：用当前帧重新截图（rev+1），多边形随新识别区一起保存',
      };
    }
    return { ok: true, action: 'update', request: { polygon: poly }, roi: bbox, reason: '' };
  }
  const valid = assetRoiBoxValid(d.roi);
  if (!valid) {
    return { ok: false, action: 'none', request: null, roi: null, reason: '识别区无效：请拖拽画矩形，或单击画多边形' };
  }
  if (kind === 'region') {
    if (rectEq4(valid, stored.roi) && !stored.polygon) {
      return { ok: true, action: 'none', request: null, roi: valid, reason: noneReason };
    }
    return {
      ok: true,
      action: 'update',
      request: { roi: valid, keep_action_shape: true },
      roi: valid,
      reason: '',
    };
  }
  if (kind === 'template') {
    return {
      ok: true,
      action: 'overwrite',
      request: { roi: valid, polygon: null },
      roi: valid,
      reason: '模板识别区保存 = 用当前帧重新截图覆盖（rev+1），并清空旧识别多边形',
    };
  }
  if (rectEq4(valid, stored.roi) && !stored.polygon) {
    return { ok: true, action: 'none', request: null, roi: valid, reason: noneReason };
  }
  return {
    ok: true,
    action: 'update',
    request: { roi: valid, polygon: null },
    roi: valid,
    reason: '',
  };
}

function resetActionGeometry(opts) {
  const o = opts || {};
  actionPoly = null;
  actionPolyClosed = false;
  actionShapeOwnerId = '';
  if (o.clearInvert) {
    actionInvert = false;
    syncCanvasToolButtons();
  }
  if (!o.keepTarget) {
    activeCandidate = -1;
    pendingActionAsset = null;
    pendingActionAssetScope = null;
    updateAssetActionSaveButton();
  }
  if (o.clearDrag) {
    dragStart = null;
    dragTarget = 'roi';
    canvasGesture = null;
  }
  // New recognition results must not keep a rectangle drawn for a previous
  // asset/candidate: the fresh candidate's operation area is its own bbox.
  if (o.clearRect) roiBack = null;
  if (o.redraw !== false) {
    updateRoiLabel();
    draw();
  }
}

function resetSelection(opts) {
  // seq130（#1）：任何新的选择/清空都放弃未保存的框移动撤销点。
  boxMoveUndo = null;
  const o = opts || {};
  resetActionGeometry({
    keepTarget: o.keepTarget === true,
    clearInvert: o.clearInvert !== false,
    clearDrag: true,
    redraw: false,
  });
  polygon = null;
  polyClosed = false;
  polyDrag = null;
  // #6：选择状态被清空，已存识别多边形的只读显示也清掉。
  storedRecognitionPolygon = null;
  setCanvasTool('none');
  if (o.clearBoxes) {
    boxes = [];
    selectedBox = null;
    updateClickStepButtons();
  }
  if (o.clearRoi) {
    roi = null;
    roiBack = null;
  }
  updateRoiLabel();
  if (o.redraw !== false) draw();
}

// seq137（A）：统一“清空画布选择”入口——空白点击/检测框选择/切帧/切任务/Esc
// 必须同时清识别区 + 操作区（actionPoly/actionPolyClosed/roiBack/归属）。
function clearCanvasSelection(reason) {
  resetSelection({ clearInvert: true, clearRoi: true, clearBoxes: true, redraw: false });
  actionPoly = null;
  actionPolyClosed = false;
  actionShapeOwnerId = '';
  activeCandidate = -1;
  pendingActionAsset = null;
  pendingActionAssetScope = null;
  editingRecognitionAsset = null;
  updateAssetActionSaveButton();
  if (typeof updateAssetRoiSaveButton === 'function') updateAssetRoiSaveButton();
  updateRoiLabel();
  draw();
  if (reason) log('已清空画布识别区/操作区：' + reason);
}

function segmentIntersects(a, b, c, d) {
  const cross = (p, q, r) => (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
  const d1 = cross(c, d, a);
  const d2 = cross(c, d, b);
  const d3 = cross(a, b, c);
  const d4 = cross(a, b, d);
  return ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) &&
    ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0));
}

function polygonOverlapsRect(points, box) {
  if (!Array.isArray(points) || points.length < 3 || !box) return false;
  const [x, y, w, h] = box;
  const corners = [[x, y], [x + w - 1, y], [x + w - 1, y + h - 1], [x, y + h - 1]];
  // any polygon vertex inside rect
  if (points.some(([px, py]) => px >= x && px < x + w && py >= y && py < y + h)) return true;
  // any rect corner inside polygon
  for (const [cx, cy] of corners) {
    if (actionShapeContains({ kind: 'polygon', points }, cx, cy)) return true;
  }
  // any polygon edge crosses any rect edge
  for (let i = 0; i < points.length; i++) {
    const a = points[i];
    const b = points[(i + 1) % points.length];
    for (let j = 0; j < 4; j++) {
      if (segmentIntersects(a, b, corners[j], corners[(j + 1) % 4])) return true;
    }
  }
  return false;
}

// v1.2.4: geometry -> action_shape is pure so both kinds are testable and the
// seq136（UI B #2）：矩形只作为快捷键；实际保存/显示统一为 4 点多边形（rect=四角
// 直角多边形），已存 polygon 永不被降级。rectToPolygonPoints 供快捷拖动/展示使用。
function rectToPolygonPoints(rect) {
  if (!rect || !(Number(rect.w) > 0) || !(Number(rect.h) > 0)) return null;
  const x = Number(rect.x);
  const y = Number(rect.y);
  const w = Number(rect.w);
  const h = Number(rect.h);
  if (![x, y, w, h].every(Number.isFinite)) return null;
  return [{ x: x, y: y }, { x: x + w, y: y }, { x: x + w, y: y + h }, { x: x, y: y + h }];
}

// geometry → 保存用 action_shape：closed polygon 原样保留；rect 快捷升级为 4 点
// polygon（legacy rect 读取仍可显示，下一次保存即升级，顶点不丢）。
function actionShapeForSave(poly, closed, rect, invert) {
  const flag = !!invert;
  if (Array.isArray(poly) && closed === true && poly.length >= 3) {
    const points = poly.map((p) => [Number(p.x), Number(p.y)]);
    if (points.every((p) => Number.isFinite(p[0]) && Number.isFinite(p[1]))) {
      return { kind: 'polygon', points: points, invert: flag };
    }
  }
  const pts = rectToPolygonPoints(rect);
  if (pts) {
    return { kind: 'polygon', points: pts.map((p) => [p.x, p.y]), invert: flag };
  }
  return null;
}

// action_shape（polygon/rect legacy）→ 绝对坐标 [[x,y]...] 的 polygon 点（rect 视作
// 4 点直角多边形）；供画布/候选/预览统一 polygon 显示。
function actionShapePolygonPoints(shape) {
  if (!shape || typeof shape !== 'object') return null;
  const out = [];
  if (shape.kind === 'polygon' && Array.isArray(shape.points)) {
    for (const p of shape.points) {
      if (!Array.isArray(p) || p.length < 2) continue;
      const x = Number(p[0]);
      const y = Number(p[1]);
      if (Number.isFinite(x) && Number.isFinite(y)) out.push({ x: x, y: y });
    }
  } else if (shape.kind === 'rect' && Array.isArray(shape.roi) && shape.roi.length === 4) {
    const pts = rectToPolygonPoints({ x: shape.roi[0], y: shape.roi[1], w: shape.roi[2], h: shape.roi[3] });
    if (pts) return pts;
  }
  return out.length >= 3 ? out : null;
}

// seq137（B）：服务端 action_shape 优先。候选/点测/拖测返回的 action_shape 已是
// 帧绝对坐标（含 fixed/OCR/链终端解析结果），UI 直接画多边形；只有服务端没给
// action_shape 时才回退 action_box 矩形（不再前端二次平移）。
function serverActionShapeView(shape, fallbackBox, label) {
  const prefix = String(label === undefined || label === null ? '' : label);
  const poly = actionShapePolygonPoints(shape);
  if (poly) {
    const bbox = actionShapeBBox({ kind: 'polygon', points: poly.map((p) => [p.x, p.y]) });
    if (bbox) {
      return {
        text: (prefix ? prefix + ' ' : '') + '操作区（polygon）',
        score: 1,
        box: bbox,
        polygon: poly,
      };
    }
  }
  if (Array.isArray(fallbackBox) && matchBoxValid(fallbackBox)) {
    return { text: (prefix ? prefix + ' ' : '') + '操作区', score: 1, box: fallbackBox, polygon: null };
  }
  return null;
}

// invert flag can never be dropped on one of them. Polygon wins when closed;
// otherwise the last drawn rectangle is the shape. The flag applies to BOTH.
function actionShapeFromGeometry(poly, closed, rect, invert) {
  const flag = !!invert;
  if (Array.isArray(poly) && closed === true && poly.length >= 3) {
    return {
      kind: 'polygon',
      points: poly.map((p) => [Number(p.x), Number(p.y)]),
      invert: flag,
    };
  }
  if (rect && Number(rect.w) > 0 && Number(rect.h) > 0) {
    return {
      kind: 'rect',
      roi: [Number(rect.x), Number(rect.y), Number(rect.w), Number(rect.h)],
      invert: flag,
    };
  }
  return null;
}

function currentActionShape() {
  // seq136（UI B #2）：rect 快捷在保存/显示时升级为 4 点多边形；已存 polygon 原样。
  return actionShapeForSave(actionPoly, actionPolyClosed, roiBack, actionInvert);
}

// v1.2.4: normalize a saved action_shape for edit-loading. Pure so the tests can
// prove rect and polygon both restore `invert` (the old loader only painted the
// invert hint for polygons and left a stray bbox rect over polygon shapes).
function actionShapeEditState(shape) {
  const invert = !!(shape && shape.invert);
  if (shape && shape.kind === 'polygon' && Array.isArray(shape.points) && shape.points.length >= 3) {
    return {
      kind: 'polygon',
      points: shape.points.map((p) => [Number(p[0]), Number(p[1])]),
      roi: null,
      invert,
    };
  }
  if (shape && shape.kind === 'rect' && Array.isArray(shape.roi) && shape.roi.length === 4) {
    return {
      kind: 'rect',
      points: null,
      roi: [Number(shape.roi[0]), Number(shape.roi[1]), Number(shape.roi[2]), Number(shape.roi[3])],
      invert,
    };
  }
  return { kind: null, points: null, roi: null, invert };
}

function loadActionShapeForEdit(shape) {
  const st = actionShapeEditState(shape);
  actionPoly = null;
  actionPolyClosed = false;
  roiBack = null;
  actionInvert = st.invert;
  if (st.kind === 'polygon') {
    actionPoly = st.points.map(([x, y]) => ({ x, y }));
    actionPolyClosed = true;
  } else if (st.kind === 'rect') {
    roiBack = { x: st.roi[0], y: st.roi[1], w: st.roi[2], h: st.roi[3] };
  }
  // 纯函数测试里没有画布按钮：直接同步反选文案（浏览器里按钮一定存在）。
  if (btnActionInvert) {
    btnActionInvert.textContent = '反选：' + (actionInvert ? '开' : '关');
  }
}

// v1.2.4: one renderer for BOTH action_shape kinds.
// - solid -> dashed [8,5] (the rectangle's old style; the polygon used to be
//   solid, which made the two edit paths look like different features);
// - invert -> the effective region (whole screen minus shape) gets a cool teal
//   wash, while the original/excluded shape keeps the warm orange wash. The
//   difference is visible for rectangles and polygons alike.
const ACTION_SHAPE_DASH = [8, 5];
const ACTION_SHAPE_STROKE = '#ff9f40';
const ACTION_SHAPE_FILL = 'rgba(255,159,64,0.14)';
const ACTION_EXCLUDED_FILL = 'rgba(255,159,64,0.26)';
const ACTION_EFFECTIVE_FILL = 'rgba(49,208,170,0.10)';
const ACTION_EFFECTIVE_STROKE = 'rgba(49,208,170,0.55)';

function actionShapePath(shape) {
  ctx.beginPath();
  if (shape.kind === 'rect') {
    const [x, y, w, h] = shape.roi;
    ctx.rect(x + 0.5, y + 0.5, w, h);
    return;
  }
  const pts = shape.points || [];
  if (!pts.length) return;
  ctx.moveTo(pts[0][0], pts[0][1]);
  for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i][0], pts[i][1]);
  ctx.closePath();
}

function drawActionShapeOverlay() {
  // seq137（A）：归属门——仅当前操作区工具/候选/编辑目标/选中素材预览时绘制，
  // 防止其它路径留下的 actionPoly 在识别框旁残留。
  const owned = canvasTool === 'action' || !!pendingActionAsset || activeCandidate >= 0 ||
    !!actionShapeOwnerId;
  if (!owned) return;
  const shape = currentActionShape();
  const openPoly = actionPoly && !actionPolyClosed && actionPoly.length ? actionPoly : null;
  if (!shape && !openPoly) return;
  ctx.save();
  if (shape) {
    const invert = !!shape.invert;
    if (invert) {
      // “反选后的有效区域”底色：整屏冷色。
      ctx.fillStyle = ACTION_EFFECTIVE_FILL;
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.setLineDash(ACTION_SHAPE_DASH);
      ctx.strokeStyle = ACTION_EFFECTIVE_STROKE;
      ctx.lineWidth = 2;
      ctx.strokeRect(0.5, 0.5, canvas.width - 1, canvas.height - 1);
    }
    actionShapePath(shape);
    // “原选中区域”底色：暖橙色，与有效区域明显不同。
    ctx.fillStyle = invert ? ACTION_EXCLUDED_FILL : ACTION_SHAPE_FILL;
    ctx.fill();
    ctx.setLineDash(ACTION_SHAPE_DASH);
    ctx.strokeStyle = ACTION_SHAPE_STROKE;
    ctx.lineWidth = 2;
    ctx.stroke();
    const bbox = actionShapeBBox(shape);
    if (bbox) {
      const label = shape.kind === 'polygon'
        ? `操作区域 多边形 ${shape.points.length} 顶点${invert ? '（反选：有效=屏幕−区域）' : ''}`
        : `操作区域 ${shape.roi[2]}x${shape.roi[3]} @ ${shape.roi[0]},${shape.roi[1]}${invert ? '（反选：有效=屏幕−区域）' : ''}`;
      ctx.setLineDash([]);
      ctx.font = '14px Consolas, monospace';
      ctx.fillStyle = ACTION_SHAPE_STROKE;
      ctx.fillText(label, bbox[0] + 4, Math.max(14, bbox[1] - 6));
    }
  }
  if (openPoly) {
    // In-progress polygon: still dashed, no fill until the user closes it.
    ctx.setLineDash(ACTION_SHAPE_DASH);
    ctx.strokeStyle = ACTION_SHAPE_STROKE;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(openPoly[0].x, openPoly[0].y);
    for (let i = 1; i < openPoly.length; i++) ctx.lineTo(openPoly[i].x, openPoly[i].y);
    ctx.stroke();
  }
  ctx.restore();
}

function randomPointInAction(it, shapeOverride, fallbackBox) {
  const shape = shapeOverride !== undefined ? shapeOverride : (it && it.action_shape);
  const fw = canvas.width || 1280;
  const fh = canvas.height || 720;
  if (!shape) {
    const box = fallbackBox || (it && it.roi) || [0, 0, fw, fh];
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
  if (fallbackBox) return randomPointInBox(fallbackBox, 0.12);
  return { x: Math.floor(fw / 2), y: Math.floor(fh / 2) };
}

function assetActionBox(it, shapeOverride, fallbackBox) {
  const shape = shapeOverride !== undefined ? shapeOverride : it.action_shape;
  if (shape) {
    const b = actionShapeBBox(shape);
    if (b) return b;
  }
  return fallbackBox || it.roi || null;
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

// MATCH-BUG fix (was P1-7): a single asset test must run on the frame the user
// is actually looking at. Every saved asset has positions>=1, so the old
// "positions first" order silently ignored currentFrameId and tested a static
// screenshot — the template was still at its saved place there, hence the box
// never moved. Order is now: live preview > explicit frameId > current frame >
// saved position frames (fallback only when no frame is selected at all).
// Batch testing (POST /task/assets/batch_test) keeps covering every position
// frame. Live preview ignores frameId because the server gives live=1 priority.
//
// Pure on purpose: opts carries the editor globals so ui_pure_test can pin the
// frame-selection contract without a browser.
function testFramePlan(it, opts) {
  const o = opts || {};
  const live = 'livePreview' in o
    ? !!o.livePreview
    : (typeof livePreview !== 'undefined' && !!livePreview);
  if (live) {
    return [{ query: '&live=1', label: '实时帧', source: 'live', live: true, frameId: null }];
  }
  const current = 'currentFrameId' in o
    ? o.currentFrameId
    : (typeof currentFrameId !== 'undefined' ? currentFrameId : null);
  const explicit = o.frameId != null ? Number(o.frameId) : NaN;
  const cur = Number(current);
  const fid = Number.isInteger(explicit) && explicit > 0
    ? explicit
    : (Number.isInteger(cur) && cur > 0 ? cur : null);
  if (fid != null) {
    return [{
      query: `&frame_id=${fid}`,
      label: `当前帧 #${fid}`,
      source: `current:#${fid}`,
      live: false,
      frameId: fid,
    }];
  }
  const positions = Array.isArray(it && it.positions) ? it.positions : [];
  const ids = [];
  for (const p of positions) {
    const n = Number(p);
    if (Number.isInteger(n) && n > 0 && !ids.includes(n)) ids.push(n);
  }
  if (ids.length) {
    return ids.map((id) => ({
      query: `&frame_id=${id}`,
      label: `位置帧 #${id}`,
      source: `position:#${id}`,
      live: false,
      frameId: id,
    }));
  }
  return [{ query: '', label: '当前帧', source: 'current', live: false, frameId: null }];
}

// Thin wrapper: the real editor state comes from the page globals.
function testFrames(it, frameId) {
  return testFramePlan(it, { livePreview, currentFrameId, frameId });
}

// Kept for single-frame callers (collection etc.); multi-position callers use
// testFrames() directly.
function testFrameQuery(it, frameId) {
  return testFrames(it, frameId)[0].query;
}

function testFrameLabel(it, frameId) {
  const frames = testFrames(it, frameId);
  if (frames.length === 1) return frames[0].label;
  if (frames[0].live) return '实时帧';
  return `位置帧 ${frames.map((f) => `#${f.frameId}`).join('、')}`;
}

// /task/asset/match reports a miss as x=y=-1 while w/h stay the template size,
// so checking w/h alone would still draw a box at (-1,-1) and log ❌ next to it.
function matchBoxValid(box) {
  return Array.isArray(box) && box.length >= 4 &&
    Number(box[0]) >= 0 && Number(box[1]) >= 0 &&
    Number(box[2]) > 0 && Number(box[3]) > 0;
}

function translatedActionShapeForMatch(it, match) {
  if (!it || !it.action_shape) return null;
  // seq136（UI B #1）：op_region.mode=fixed 表示固定绝对坐标，不随识别结果平移。
  const op = it.op_region && typeof it.op_region === 'object' && !Array.isArray(it.op_region)
    ? it.op_region : null;
  if (op && String(op.mode === undefined || op.mode === null ? '' : op.mode).toLowerCase() === 'fixed') {
    return it.action_shape;
  }
  if (!Array.isArray(it.roi)) return it.action_shape;
  return translateActionShape(
    it.action_shape,
    Number(match.x) - it.roi[0],
    Number(match.y) - it.roi[1]
  );
}

// --- U-T4（事项 3/4/5）：候选识别 / 相对偏移 / op_region 链 纯函数 ------------
// 不依赖 DOM / 全局状态，ui_pure_test.js 直接抽取执行；保存/服务端校验在阶段 B。

// GET /task/asset/candidates 的 URL（frame.query 来自 testFramePlan，以 & 开头）。
function assetCandidatesUrl(task, id, frameQuery) {
  return '/task/asset/candidates?task=' + encodeURIComponent(String(task || '')) +
    '&id=' + encodeURIComponent(String(id || '')) + String(frameQuery || '');
}

// seq134（spec D）：唯一候选渲染器。统一字段
// index,x,y,w,h,score,pass,action_box,text?,member?,member_kind?,domain?。
// - 图像 kind（template/region/集合模板成员）：显示缩略图+分数；
// - OCR 文字行：显示 text+分数；
// - 集合成员行：member/member_kind/text；
// pass 只表示阈值（≥阈值/<阈值），不再有「期望/非期望文字」语义。
function candidateBoxOf(candidate) {
  const c = candidate && typeof candidate === 'object' && !Array.isArray(candidate) ? candidate : {};
  const num = (v) => (Number.isFinite(Number(v)) ? Number(v) : 0);
  return [Math.round(num(c.x)), Math.round(num(c.y)), Math.round(num(c.w)), Math.round(num(c.h))];
}

function candidateDomainText(domain) {
  const d = String(domain === undefined || domain === null ? '' : domain);
  if (d === 'roi') return 'ROI';
  if (d === 'screen') return '全屏';
  if (d === 'members') return '集合成员';
  if (d === 'screen_fallback') return 'ROI未命中→全屏回退';
  return d;
}

function candidateMemberKindLabel(kind) {
  const k = String(kind === undefined || kind === null ? '' : kind);
  if (k === 'template') return '模板';
  if (k === 'template_set') return '素材集合';
  if (k === 'ocr_line') return '单行文字';
  if (k === 'ocr_lines') return '多行文字';
  if (k === 'ocr_var') return '使用端OCR文字配置项';
  if (k === 'region') return '固定区域';
  return k;
}

function candidateFieldInfo(candidate) {
  const c = candidate && typeof candidate === 'object' && !Array.isArray(candidate) ? candidate : {};
  const index = Number.isInteger(Number(c.index)) ? Number(c.index) : 0;
  const score = Number(c.score);
  const text = c.text === undefined || c.text === null ? '' : String(c.text);
  const member = c.member === undefined || c.member === null ? '' : String(c.member);
  const memberKind = c.member_kind === undefined || c.member_kind === null ? '' : String(c.member_kind);
  const domain = c.domain === undefined || c.domain === null ? '' : String(c.domain);
  const hasPass = Object.prototype.hasOwnProperty.call(c, 'pass') &&
    c.pass !== undefined && c.pass !== null;
  const pass = c.pass === true;
  const kind = memberKind || (text.trim() ? 'text' : 'image');
  return {
    index: index,
    score: Number.isFinite(score) ? score : null,
    scoreText: Number.isFinite(score) ? score.toFixed(4) : '—',
    text: text,
    member: member,
    memberKind: memberKind,
    memberKindLabel: candidateMemberKindLabel(memberKind),
    domain: domain,
    domainText: candidateDomainText(domain),
    hasPass: hasPass,
    pass: pass,
    passText: hasPass ? (pass ? '≥阈值' : '<阈值') : '',
    kind: kind,
    hasText: text.trim() !== '',
  };
}

function candidateTextPreview(candidate, maxLen) {
  const info = candidateFieldInfo(candidate);
  const limit = Number.isFinite(Number(maxLen)) && Number(maxLen) > 0 ? Number(maxLen) : 24;
  if (!info.hasText) return '';
  const text = info.text.length > limit ? info.text.slice(0, limit) + '…' : info.text;
  return '「' + text + '」';
}

// 图像候选的缩略图描述（/task/asset/image）；文字行返回 null（显示 text 即可）。
function candidateThumbDescriptor(candidate, ctx) {
  const info = candidateFieldInfo(candidate);
  if (info.hasText) return null;
  const o = ctx && typeof ctx === 'object' ? ctx : {};
  const assetId = info.member || String(o.assetId || (candidate && (candidate.sel || candidate.id)) || '');
  if (!assetId) return null;
  return {
    id: assetId,
    kind: info.memberKind || info.kind,
    url: '/task/asset/image?task=' + encodeURIComponent(String(o.task || '')) +
      '&id=' + encodeURIComponent(assetId),
    box: candidateBoxOf(candidate),
  };
}

// 单行候选渲染（结果区/下拉/画布共用）：图像=缩略图+分数，文字=text+分数，集合=member/member_kind/text。
function candidateRenderLabel(candidate, ctx) {
  const info = candidateFieldInfo(candidate);
  const parts = ['#' + info.index, 'score=' + info.scoreText];
  if (info.member) {
    parts.push('成员 ' + info.member + (info.memberKind ? '（' + info.memberKindLabel + '）' : ''));
  }
  if (info.hasText) {
    parts.push('文字=' + JSON.stringify(info.text));
  } else if (!info.member) {
    const thumb = candidateThumbDescriptor(candidate, ctx);
    if (thumb) parts.push('缩略图 ' + thumb.id);
  }
  if (info.domain) parts.push(info.domainText);
  if (info.hasPass) parts.push(info.passText);
  return parts.join(' · ');
}

function candidateLabelText(candidate, ctx) {
  return candidateRenderLabel(candidate, ctx) + ' [' + candidateBoxOf(candidate).join(',') + ']';
}

// seq134（UI B）：候选框的多边形显示。模板全帧多候选时，把素材已存识别多边形
// 平移到该候选位置（候选 bbox vs 素材 roi 的偏移）；其它 kind 的候选是识别行/固定
// 区域，保持矩形。返回 [{x,y}] 或 null（无 polygon/无 ROI）。
function candidatePolygonForAsset(it, box) {
  const poly = it && Array.isArray(it.polygon) ? it.polygon : [];
  const roi = it && Array.isArray(it.roi) ? it.roi : null;
  if (poly.length < 3 || !roi || !Array.isArray(box) || box.length < 4) return null;
  if (String(it.kind) !== 'template') return null;
  const dx = Number(box[0]) - Number(roi[0]);
  const dy = Number(box[1]) - Number(roi[1]);
  const out = [];
  for (const p of poly) {
    if (!Array.isArray(p) || p.length < 2) continue;
    const x = Number(p[0]);
    const y = Number(p[1]);
    if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
    out.push({ x: x + dx, y: y + dy });
  }
  return out.length >= 3 ? out : null;
}

// 候选 → draw() 的 boxes 数组（画全部真实识别区域；无效框跳过）。
// 画布标签用紧凑版同一渲染器；thumb 供后续图像缩略图渲染使用。
function candidateBoxes(candidates, ctx) {
  const out = [];
  for (const candidate of Array.isArray(candidates) ? candidates : []) {
    if (!candidate || typeof candidate !== 'object') continue;
    const box = [Number(candidate.x), Number(candidate.y), Number(candidate.w), Number(candidate.h)];
    if (!matchBoxValid(box)) continue;
    const info = candidateFieldInfo(candidate);
    const thumb = candidateThumbDescriptor(candidate, ctx);
    const parts = ['#' + info.index, info.score === null ? '—' : info.score.toFixed(2)];
    if (info.member) parts.push(info.member);
    if (info.hasText) parts.push(candidateTextPreview(candidate, 16));
    else if (thumb) parts.push('缩略图');
    if (info.domain === 'screen_fallback') parts.push('全屏回退');
    out.push({
      text: parts.join(' '),
      score: info.score === null ? 0 : info.score,
      box,
      pass: info.pass,
      hasPass: info.hasPass,
      textValue: info.text,
      member: info.member,
      memberKind: info.memberKind,
      domain: info.domain,
      thumb: thumb,
    });
  }
  return out;
}

// runtime_text（使用端默认识别文字）：必填、trim 非空、≤200 字符、无控制字符。
function runtimeTextValid(text) {
  const raw = text === undefined || text === null ? '' : String(text);
  const value = raw.trim();
  if (!value) return { ok: false, value: '', error: '识别文字（runtime_text）必填' };
  if (value.length > 200) {
    return { ok: false, value, error: '识别文字最长 200 字符（当前 ' + value.length + '）' };
  }
  if (/[\u0000-\u001f\u007f]/.test(value)) {
    return { ok: false, value, error: '识别文字不能包含控制字符（换行/制表等）' };
  }
  return { ok: true, value, error: '' };
}

// op_region 单节点规范化（缺省 mode = self；pick 缺省 score）。
function opRegionNodeOf(opRegion) {
  const r = opRegion && typeof opRegion === 'object' && !Array.isArray(opRegion) ? opRegion : {};
  const modeRaw = r.mode === undefined || r.mode === null ? 'self' : String(r.mode);
  const mode = modeRaw.trim().toLowerCase() || 'self';
  const asset = String(r.asset === undefined || r.asset === null ? '' : r.asset).trim();
  const pickRaw = String(r.pick === undefined || r.pick === null ? '' : r.pick).trim().toLowerCase();
  const pick = pickRaw === 'nearest' ? 'nearest' : 'score';
  const offset = Array.isArray(r.offset) ? r.offset.map((v) => Number(v)) : [];
  return { mode, asset, pick, offset };
}

// op_region 链客户端预检（服务端 POST /task/asset/validate_op_region 是唯一权威；
// 这里只做即时提示）：self/offset 收尾、引用素材存在且类型合法、region 素材只作
// 末端固定区域、防环、深度上限 16。
// 返回 { ok, chain:[{mode,asset?,pick?,offset?}], depth, error }。
function opRegionChainSpec(opRegion, assets, rootAssetId) {
  const list = Array.isArray(assets) ? assets : [];
  const byId = {};
  for (const asset of list) {
    if (!asset || typeof asset !== 'object') continue;
    const id = String(asset.id === undefined || asset.id === null ? '' : asset.id).trim();
    if (id && !Object.prototype.hasOwnProperty.call(byId, id)) byId[id] = asset;
  }
  const rootId = String(rootAssetId === undefined || rootAssetId === null ? '' : rootAssetId).trim();
  const visited = {};
  if (rootId) visited[rootId] = true;
  const recognizable = ['template', 'template_set', 'ocr_line', 'ocr_lines', 'ocr_var'];
  const chain = [];
  let node = opRegion;
  for (let depth = 0; depth < 17; depth++) {
    if (depth >= 16) {
      return { ok: false, chain, depth: chain.length, error: '操作区引用链超过最大深度 16' };
    }
    if (node === undefined || node === null) node = { mode: 'self' };
    const n = opRegionNodeOf(node);
    if (n.mode === 'self') {
      chain.push({ mode: 'self', asset: '' });
      return { ok: true, chain, depth: chain.length, error: '' };
    }
    if (n.mode === 'offset') {
      if (n.offset.length !== 4 || n.offset.some((v) => !Number.isInteger(v))) {
        return { ok: false, chain, depth: chain.length, error: '相对偏移必须是 [dx,dy,w,h] 四个整数' };
      }
      if (n.offset[2] <= 0 || n.offset[3] <= 0) {
        return { ok: false, chain, depth: chain.length, error: '相对偏移的 w/h 必须大于 0' };
      }
      chain.push({ mode: 'offset', asset: '', offset: n.offset.slice() });
      return { ok: true, chain, depth: chain.length, error: '' };
    }
    if (n.mode !== 'asset') {
      return { ok: false, chain, depth: chain.length, error: '操作区模式不合法：「' + n.mode + '」' };
    }
    if (!n.asset) {
      return { ok: false, chain, depth: chain.length, error: '引用操作区缺少素材 id' };
    }
    const ref = byId[n.asset];
    if (!ref) {
      return { ok: false, chain, depth: chain.length, error: '操作区引用的素材「' + n.asset + '」不在当前素材库' };
    }
    if (visited[n.asset]) {
      return { ok: false, chain, depth: chain.length, error: '操作区引用链存在循环（重复出现素材「' + n.asset + '」）' };
    }
    visited[n.asset] = true;
    const refKind = String(ref.kind === undefined || ref.kind === null ? '' : ref.kind).trim();
    if (refKind === 'region') {
      if (ref.op_region !== undefined && ref.op_region !== null) {
        return {
          ok: false, chain, depth: chain.length,
          error: 'region 素材「' + n.asset + '」是末端固定区域，不能再配置操作区',
        };
      }
      chain.push({ mode: 'asset', asset: n.asset, pick: n.pick });
      chain.push({ mode: 'region', asset: n.asset });
      return { ok: true, chain, depth: chain.length, error: '' };
    }
    if (recognizable.indexOf(refKind) < 0) {
      return {
        ok: false, chain, depth: chain.length,
        error: '素材「' + n.asset + '」（' + (refKind || '未知类型') + '）不能作为操作区链节点',
      };
    }
    chain.push({ mode: 'asset', asset: n.asset, pick: n.pick });
    node = ref.op_region;
  }
  return { ok: false, chain, depth: chain.length, error: '操作区引用链超过最大深度 16' };
}

// --- U-T4/S5：操作区递归对话框草稿模型（纯函数，API 无关） -------------------
// draft = { rootAssetId, levels: [{ assetId, opRegion }] }
// levels[0] = 当前素材的 op_region 编辑值；选「引用素材」后递归展开被引用素材的
// op_region 编辑值（levels[1..]）。保存 = 把改动过的 levels 逐个 PATCH
// /task/asset/update（阶段 B），服务端 validate_op_region 仍是唯一权威。
function opRegionToJson(opRegion) {
  const n = opRegionNodeOf(opRegion);
  if (n.mode === 'offset') return { mode: 'offset', offset: n.offset.slice() };
  if (n.mode === 'asset') {
    return { mode: 'asset', asset: n.asset, pick: n.pick === 'nearest' ? 'nearest' : 'score' };
  }
  return { mode: 'self' };
}

function opRegionDraftStart(asset) {
  const it = asset && typeof asset === 'object' ? asset : {};
  const id = String(it.id === undefined || it.id === null ? '' : it.id).trim();
  const initial = it.op_region !== undefined && it.op_region !== null ? it.op_region : { mode: 'self' };
  return {
    rootAssetId: id,
    levels: [{ assetId: id, opRegion: opRegionToJson(initial) }],
  };
}

function opRegionDraftBreadcrumb(draft) {
  const d = draft && Array.isArray(draft.levels) ? draft : { levels: [] };
  return d.levels.map((level) => String(level && level.assetId ? level.assetId : ''));
}

function opRegionDraftBreadcrumbText(draft) {
  return opRegionDraftBreadcrumb(draft).filter(Boolean).join(' ▸ ');
}

// 设置某一级的 op_region；若改成引用素材，则递归追加被引用素材的当前 op_region
// （缺省 self）。返回新 draft（不改入参）。
function opRegionDraftSetLevel(draft, index, opRegion, assets) {
  const d = draft && Array.isArray(draft.levels) ? draft : { rootAssetId: '', levels: [] };
  const rawIndex = Number(index);
  const i = Math.max(0, Math.min(
    Number.isInteger(rawIndex) ? rawIndex : 0,
    Math.max(0, d.levels.length - 1)
  ));
  const levels = d.levels.slice(0, i + 1);
  const node = opRegionToJson(opRegion);
  levels[i] = { assetId: levels[i].assetId, opRegion: node };
  if (node.mode === 'asset' && node.asset) {
    const ref = (Array.isArray(assets) ? assets : []).find(
      (asset) => asset && String(asset.id) === node.asset
    );
    const refKind = ref ? String(ref.kind === undefined || ref.kind === null ? '' : ref.kind) : '';
    // Lead 裁决 11：region 素材只作末端固定区域节点（其 roi 即操作区），不再递归
    // 展开 op_region，也就没有可编辑的下一级。
    if (refKind !== 'region') {
      const refOp = ref && ref.op_region !== undefined && ref.op_region !== null
        ? ref.op_region
        : { mode: 'self' };
      levels.push({ assetId: node.asset, opRegion: opRegionToJson(refOp) });
    }
  }
  return { rootAssetId: d.rootAssetId, levels };
}

// 回退：删除第 index 级及其后所有级，并把第 index-1 级改回 self。
function opRegionDraftBack(draft, index) {
  const d = draft && Array.isArray(draft.levels) ? draft : null;
  const i = Number(index);
  if (!d || !Number.isInteger(i) || i <= 0 || i >= d.levels.length) return d;
  const levels = d.levels.slice(0, i);
  levels[i - 1] = { assetId: levels[i - 1].assetId, opRegion: { mode: 'self' } };
  return { rootAssetId: d.rootAssetId, levels };
}

// 用 draft 的编辑值覆盖素材库里的 op_region，走 opRegionChainSpec 做链校验。
function opRegionDraftChainSpec(draft, assets) {
  const d = draft && Array.isArray(draft.levels) ? draft : { rootAssetId: '', levels: [] };
  const overrides = {};
  for (const level of d.levels) {
    if (!level || !level.assetId) continue;
    overrides[level.assetId] = opRegionToJson(level.opRegion);
  }
  const merged = (Array.isArray(assets) ? assets : []).map((asset) => {
    if (!asset || typeof asset !== 'object') return asset;
    const id = String(asset.id === undefined || asset.id === null ? '' : asset.id).trim();
    if (!id || !Object.prototype.hasOwnProperty.call(overrides, id)) return asset;
    return Object.assign({}, asset, { op_region: overrides[id] });
  });
  const rootOp = d.levels.length ? d.levels[0].opRegion : { mode: 'self' };
  return opRegionChainSpec(rootOp, merged, d.rootAssetId);
}

// 保存载荷：只返回与素材库当前值不同的级，按 breadcrumb 顺序、去重。
function opRegionDraftUpdates(draft, assets) {
  const d = draft && Array.isArray(draft.levels) ? draft : { levels: [] };
  const list = Array.isArray(assets) ? assets : [];
  const byId = {};
  for (const asset of list) {
    if (!asset || typeof asset !== 'object') continue;
    const id = String(asset.id === undefined || asset.id === null ? '' : asset.id).trim();
    if (id) byId[id] = asset;
  }
  const out = [];
  const seen = {};
  for (const level of d.levels) {
    if (!level || !level.assetId || seen[level.assetId]) continue;
    seen[level.assetId] = true;
    const next = opRegionToJson(level.opRegion);
    const storedAsset = byId[level.assetId];
    const stored = storedAsset && storedAsset.op_region !== undefined && storedAsset.op_region !== null
      ? opRegionToJson(storedAsset.op_region)
      : { mode: 'self' };
    if (JSON.stringify(next) === JSON.stringify(stored)) continue;
    out.push({ assetId: level.assetId, op_region: next });
  }
  return out;
}

// 当前 draft 每一级的摘要（面包屑 + 模式），供对话框逐级渲染。
function opRegionDraftSummary(draft) {
  const d = draft && Array.isArray(draft.levels) ? draft : { levels: [] };
  return d.levels.map((level) => {
    const n = opRegionNodeOf(level && level.opRegion);
    let text = '当前素材操作区';
    if (n.mode === 'offset') text = '相对偏移 [' + n.offset.join(',') + ']';
    else if (n.mode === 'asset') {
      text = '引用素材「' + n.asset + '」' + (n.pick === 'nearest' ? '（离上一节点最近）' : '（评分最高）');
    }
    return { assetId: level && level.assetId ? String(level.assetId) : '', mode: n.mode, text };
  });
}

// pick 选项（仅引用级可设；Lead 裁决 2：评分最高 / 离上一节点真实识别区域最近）。
function opRegionPickOptions() {
  return [
    { value: 'score', label: '评分最高' },
    { value: 'nearest', label: '离上一节点最近' },
  ];
}

// 可引用素材列表（S5：搜索框列出全部素材并标注类型；region 素材可作末端固定区域，
// 用 terminal 标记；当前素材自身排除，环由 opRegionChainSpec 兜底拒绝）。
function opRegionRefOptions(assets, rootAssetId) {
  const root = String(rootAssetId === undefined || rootAssetId === null ? '' : rootAssetId).trim();
  const recognizable = ['template', 'template_set', 'ocr_line', 'ocr_lines', 'ocr_var'];
  const out = [];
  for (const asset of Array.isArray(assets) ? assets : []) {
    if (!asset || typeof asset !== 'object') continue;
    const id = String(asset.id === undefined || asset.id === null ? '' : asset.id).trim();
    if (!id || (root && id === root)) continue;
    const kind = String(asset.kind === undefined || asset.kind === null ? '' : asset.kind).trim();
    if (recognizable.indexOf(kind) >= 0) out.push({ id, kind, terminal: false });
    else if (kind === 'region') out.push({ id, kind, terminal: true });
  }
  return out;
}

// --- U-T4/S2：识别文字（ocr_var）对话框纯模型 -------------------------------
// 新建 ocr_var：runtime_text 必填≤200、param_key 自动生成（可改，创建后只读）、
// label、roi 可空（全屏 OCR）。编辑 ocr_var：改 runtime_text/label/roi；改 roi 为
// 空必须用 replace（update 的 roi=None 表示不变，无法清空）。纯函数，ui_pure_test
// 直接抽取执行；对话框/DOM 在下方（阶段 B）。
function isParamKeyValid(key) {
  const text = String(key === undefined || key === null ? '' : key).trim();
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(text) && text.length <= 64;
}

// 素材 id → 稳定的自动 param_key（p_ + 8 位十六进制；同 id 永远同 key）。
function autoParamKey(assetId) {
  const raw = String(assetId === undefined || assetId === null ? '' : assetId);
  let hash = 2166136261;
  for (let i = 0; i < raw.length; i += 1) {
    hash ^= raw.charCodeAt(i);
    hash = (hash * 16777619) >>> 0;
  }
  return 'p_' + (hash >>> 0).toString(16).padStart(8, '0');
}

function assetTextDraftStart(asset, opts) {
  const it = asset && typeof asset === 'object' ? asset : {};
  const o = opts && typeof opts === 'object' ? opts : {};
  const id = String(it.id === undefined || it.id === null ? '' : it.id).trim();
  const roi = Array.isArray(it.roi) && it.roi.length === 4
    ? it.roi.map((v) => Number(v))
    : null;
  return {
    mode: 'edit',
    id: id,
    runtimeText: String(it.runtime_text === undefined || it.runtime_text === null ? '' : it.runtime_text),
    paramKey: String(it.param_key === undefined || it.param_key === null ? '' : it.param_key),
    label: String(it.label === undefined || it.label === null ? '' : it.label),
    roi: roi,
    fullScreen: !roi,
    positions: Array.isArray(it.positions) ? it.positions.slice() : [],
    task: String(o.task === undefined || o.task === null ? '' : o.task).trim(),
    // #9（验收反馈）：replace 保存会整项覆盖，必须原样带回操作区/操作形状/阈值，
    // 否则改识别文字会把 op_region 链、action_shape、threshold 静默清空。
    opRegion: it.op_region === undefined ? null : it.op_region,
    actionShape: it.action_shape === undefined ? null : it.action_shape,
    threshold: it.threshold === undefined ? null : it.threshold,
  };
}

// 从「识别结果」候选新建 ocr_var：识别框作为 roi，文字作为 runtime_text，
// param_key 由素材 id 自动派生；位置帧取当前勾选的位置帧。
function assetTextDraftFromCandidate(candidate, opts) {
  const c = candidate && typeof candidate === 'object' ? candidate : {};
  const o = opts && typeof opts === 'object' ? opts : {};
  const rec = c.recognize && typeof c.recognize === 'object' ? c.recognize : null;
  const roi = rec
    ? [Number(rec.x), Number(rec.y), Number(rec.w), Number(rec.h)]
    : null;
  const id = String(o.name !== undefined && o.name !== null ? o.name : (c.name || '')).trim();
  return {
    mode: 'create',
    id: id,
    runtimeText: String(o.text !== undefined && o.text !== null ? o.text : (c.text || '')),
    paramKey: id ? autoParamKey(id) : '',
    label: id,
    roi: roi,
    fullScreen: !roi,
    positions: Array.isArray(o.positions) ? o.positions.slice() : [],
    task: String(o.task === undefined || o.task === null ? '' : o.task).trim(),
  };
}

// 对话框字段 → 保存前校验 errors[]（空=通过）。
// opts: { existingIds?: string[], existingParamKeys?: string[], task?: string }
function assetTextValidate(draft, opts) {
  const d = draft && typeof draft === 'object' ? draft : {};
  const o = opts && typeof opts === 'object' ? opts : {};
  const errors = [];
  const mode = d.mode === 'create' ? 'create' : 'edit';
  if (mode === 'create') {
    const id = String(d.id === undefined || d.id === null ? '' : d.id).trim();
    if (!id) {
      errors.push('素材 id 必填');
    } else if (!candidateNameValid(id)) {
      errors.push('素材 id 非法（字母/中文/数字/下划线，不能以数字开头）');
    } else {
      const ids = Array.isArray(o.existingIds) ? o.existingIds.map(String) : null;
      if (ids && ids.indexOf(id) >= 0) errors.push('素材 id「' + id + '」已存在（需全局唯一）');
    }
    if (!Array.isArray(d.positions) || !d.positions.length) {
      errors.push('新建素材至少需要一个位置帧（先在工作台选择位置帧）');
    }
    const keyText = String(d.paramKey === undefined || d.paramKey === null ? '' : d.paramKey).trim();
    const key = keyText || (id ? autoParamKey(id) : '');
    if (!key) {
      errors.push('param_key 必填（留空会按素材 id 自动生成）');
    } else if (!isParamKeyValid(key)) {
      errors.push('param_key 必须匹配 [A-Za-z_][A-Za-z0-9_]*（≤64 字符）');
    } else {
      const keys = Array.isArray(o.existingParamKeys) ? o.existingParamKeys.map(String) : null;
      if (keys && keys.indexOf(key) >= 0) errors.push('param_key「' + key + '」已被本任务其它素材使用');
    }
  }
  const rt = runtimeTextValid(d.runtimeText);
  if (!rt.ok) errors.push(rt.error);
  const label = String(d.label === undefined || d.label === null ? '' : d.label).trim();
  if (label.length > 80) errors.push('显示名 label 最长 80 字符');
  if (!d.fullScreen) {
    const roi = Array.isArray(d.roi) ? d.roi : null;
    const ints = roi && roi.length === 4 && roi.every((v) => Number.isInteger(Number(v)));
    if (!ints) {
      errors.push('识别区域 roi 必须是 4 个整数（或勾选「全屏 OCR」）');
    } else if (Number(roi[2]) <= 0 || Number(roi[3]) <= 0) {
      errors.push('识别区域 roi 的 w/h 必须大于 0');
    }
  }
  if (mode === 'edit' && d.fullScreen &&
      (!Array.isArray(d.positions) || !d.positions.length)) {
    errors.push('清空识别区域（全屏 OCR）需要素材已有位置帧（会走替换保存）');
  }
  return errors;
}

// draft → {url, payload, errors}（不改入参；errors 空才可保存）。
function assetTextSaveRequest(draft, opts) {
  const d = draft && typeof draft === 'object' ? draft : {};
  const o = opts && typeof opts === 'object' ? opts : {};
  const errors = assetTextValidate(d, o);
  const task = String(o.task !== undefined && o.task !== null ? o.task : (d.task || '')).trim();
  if (errors.length) return { ok: false, url: '', payload: null, errors: errors };
  const roi = d.fullScreen
    ? null
    : (Array.isArray(d.roi) ? d.roi.map((v) => Math.round(Number(v))) : null);
  const label = String(d.label === undefined || d.label === null ? '' : d.label).trim();
  const payload = { task: task };
  if (d.mode === 'create') {
    payload.id = String(d.id === undefined || d.id === null ? '' : d.id).trim();
    payload.kind = 'ocr_var';
    payload.runtime_text = String(d.runtimeText).trim();
    const keyText = String(d.paramKey === undefined || d.paramKey === null ? '' : d.paramKey).trim();
    payload.param_key = keyText || autoParamKey(payload.id);
    payload.positions = Array.isArray(d.positions) ? d.positions.slice() : [];
    if (label) payload.label = label;
    if (roi) payload.roi = roi;
    return { ok: true, url: '/task/save_asset', payload: payload, errors: [] };
  }
  // edit：清空 roi 只能走 replace（/task/asset/update 的 roi 缺省=不变）。
  if (d.fullScreen) {
    // #9：replace 整项覆盖 → 原样带回 op_region/action_shape/threshold（有才带，
    // 不凭空造字段；旧素材缺省时保持缺省，由服务端继承兜底）。
    const replacePayload = {
      task: task,
      id: String(d.id === undefined || d.id === null ? '' : d.id).trim(),
      kind: 'ocr_var',
      runtime_text: String(d.runtimeText).trim(),
      param_key: String(d.paramKey === undefined || d.paramKey === null ? '' : d.paramKey).trim(),
      positions: Array.isArray(d.positions) ? d.positions.slice() : [],
      replace: true,
      ...(label ? { label: label } : {}),
    };
    if (d.opRegion !== undefined && d.opRegion !== null) replacePayload.op_region = d.opRegion;
    if (d.actionShape !== undefined && d.actionShape !== null) replacePayload.action_shape = d.actionShape;
    if (d.threshold !== undefined && d.threshold !== null && Number.isFinite(Number(d.threshold))) {
      replacePayload.threshold = Number(d.threshold);
    }
    return {
      ok: true,
      url: '/task/save_asset',
      payload: replacePayload,
      errors: [],
    };
  }
  if (label) payload.label = label;
  payload.id = String(d.id === undefined || d.id === null ? '' : d.id).trim();
  payload.runtime_text = String(d.runtimeText).trim();
  payload.roi = roi;
  return { ok: true, url: '/task/asset/update', payload: payload, errors: [] };
}

// 操作区对话框保存计划：链校验 + 只含改动级的 update 列表（纯函数）。
function opRegionSavePlan(draft, assets, opts) {
  const o = opts && typeof opts === 'object' ? opts : {};
  const spec = opRegionDraftChainSpec(draft, assets);
  if (!spec.ok) return { ok: false, errors: [spec.error], updates: [], chain: spec.chain };
  const updates = opRegionDraftUpdates(draft, assets).map((u) => ({
    assetId: u.assetId,
    op_region: u.op_region,
    scope: o.scope === undefined || o.scope === null ? '' : String(o.scope),
  }));
  return { ok: true, errors: [], updates: updates, chain: spec.chain };
}

// 操作区对话框的来源（B3a 模型：两选一；引用素材只作识别门槛）。
// seq136（UI B #1）：fixed = 固定绝对坐标操作区（不随识别移动；仅根/链终端可选）。
function opRegionLevelChoices() {
  return [
    { value: 'self', label: '当前素材操作区（self）' },
    { value: 'asset', label: '引用素材作识别门槛（链）' },
    { value: 'fixed', label: '固定不变（绝对坐标，不随识别移动）' },
  ];
}

// seq127 用户纠正后的落点语义（模型 B）：
//   self  → 落点=当前素材自己的操作区
//   asset → 落点=链末端（最后一个被引用素材）自己的操作区；被引用素材只读
function opRegionLandingText(mode) {
  if (mode === 'fixed') return '固定操作区（绝对坐标，不随识别移动）';
  return mode === 'asset'
    ? '链末端（最后一个被引用素材）自己的操作区'
    : '当前素材操作区';
}

// 可作识别门槛的素材类型（region 只作操作区末端，不能作门槛）；
// 普通 OCR 未设置 ocr_text 时置灰并给出原因；排除自身。
function opRegionTargetKinds() {
  return ['template', 'template_set', 'ocr_line', 'ocr_lines', 'ocr_var'];
}

function opRegionRefChoices(assets, selfId) {
  const self = String(selfId === undefined || selfId === null ? '' : selfId).trim();
  return (Array.isArray(assets) ? assets : [])
    .filter((asset) => asset && String(asset.id) !== self &&
      opRegionTargetKinds().indexOf(String(asset.kind === undefined || asset.kind === null
        ? '' : asset.kind)) >= 0)
    .map((asset) => {
      const kind = String(asset.kind);
      const isPlainOcr = kind === 'ocr_line' || kind === 'ocr_lines';
      const hasText = String(asset.ocr_text === undefined || asset.ocr_text === null
        ? '' : asset.ocr_text).trim() !== '';
      const disabled = isPlainOcr && !hasText;
      return {
        id: String(asset.id),
        kind: kind,
        label: String(asset.label === undefined || asset.label === null ? asset.id : asset.label),
        disabled: disabled,
        reason: disabled ? '该素材未设置期望文字，请先在素材行设置' : '',
      };
    });
}

// 被引用素材现有 op_region 链 → 只读展示文本（不在本对话框修改）。
function opRegionReadonlyChainText(asset, assets) {
  const byId = {};
  for (const item of Array.isArray(assets) ? assets : []) {
    if (item && item.id !== undefined && item.id !== null) byId[String(item.id)] = item;
  }
  const parts = [];
  let node = asset ? asset.op_region : null;
  let depth = 0;
  const seen = {};
  while (depth < 16) {
    const n = opRegionNodeOf(node);
    if (n.mode === 'self') { parts.push('自身操作区（self）'); break; }
    if (n.mode === 'offset') { parts.push('旧相对偏移（兼容显示）'); break; }
    const refId = String(n.asset === undefined || n.asset === null ? '' : n.asset);
    parts.push('引用「' + refId + '」（' + (n.pick === 'nearest' ? '最近' : '评分最高') + '）');
    const ref = byId[refId];
    if (!ref) { parts.push('（引用素材不在库）'); break; }
    if (String(ref.kind) === 'region') {
      parts.push('末端 region「' + refId + '」（固定区域）');
      break;
    }
    if (seen[refId]) { parts.push('（循环）'); break; }
    seen[refId] = true;
    node = ref.op_region;
    depth += 1;
  }
  return parts.join(' → ') || '未配置（=self）';
}

// v1.2.4: asset-test real/dry-run mode.
// Default is dry-run. Real gestures are created by a pure request builder (so
// the payload contract is testable) and only sent when:
//   1) the user switched the header button to 真实点击 and confirmed that switch,
//   2) live preview is on (the server only accepts points from the live frame),
//   3) the user confirms the concrete gesture batch with window.confirm().
function assetTestModeLabel(real) {
  return real ? '真实点击' : '演练';
}

function assetTestGestureRequest(real, gesture, from, to, ms) {
  if (!real) return null;
  if (gesture === 'tap') {
    const x = Math.round(Number(from && from.x));
    const y = Math.round(Number(from && from.y));
    if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
    return {
      url: '/input/tap',
      payload: { x, y },
      gesture: 'tap',
      text: `真实已点击 (${x}, ${y})`,
    };
  }
  if (gesture === 'swipe') {
    const fx = Math.round(Number(from && from.x));
    const fy = Math.round(Number(from && from.y));
    const tx = Math.round(Number(to && to.x));
    const ty = Math.round(Number(to && to.y));
    if (![fx, fy, tx, ty].every((n) => Number.isFinite(n))) return null;
    const duration = Math.max(50, Math.min(2000, Math.round(Number(ms) || 300)));
    return {
      url: '/input/swipe',
      payload: { from: [fx, fy], to: [tx, ty], ms: duration },
      gesture: 'swipe',
      text: `真实已滑动 (${fx},${fy}) → (${tx},${ty}) ${duration}ms`,
    };
  }
  return null;
}

async function sendAssetTestGesture(real, gesture, from, to, ms) {
  const req = assetTestGestureRequest(real, gesture, from, to, ms);
  if (!req) return { ok: true, dry: true, text: '演练：未发送设备请求' };
  // seq133（#3）：设备手势请求 45s 超时（AbortController），不再无限等待。
  const r = await postJsonTimed(req.url, req.payload, ASSET_TEST_TIMEOUT_MS);
  if (!r || !r.ok) {
    throw new Error(((r && r.errors) || []).join('; ') || `${gesture} 请求失败`);
  }
  const device = r.device || '未知设备';
  const actual = r.gesture || gesture;
  return {
    ok: true,
    dry: false,
    device,
    gesture: actual,
    text: `${req.text} · device=${device} gesture=${actual}`,
  };
}

function realAssetTestGuard() {
  if (!assetTestReal) return { real: false, ok: true };
  if (!livePreview) {
    const msg =
      '真实操作已拒绝：实时预览已暂停。\n' +
      '请先点顶部「实时：开」恢复实时画面，再执行素材测试。';
    resultEl.textContent = msg;
    log('素材测试真实模式被拒绝：实时预览未开启', 'WARN');
    return { real: true, ok: false, message: msg };
  }
  return { real: true, ok: true };
}

// seq133（#3）：点测/拖测/长按请求超时。后端 adb 超时 ≤~20s；UI 45s 兜底后
// AbortController 中止，避免按钮/状态永久卡在 loading。超时文案统一。
const ASSET_TEST_TIMEOUT_MS = 45000;
let assetTestBusy = false;

function assetTestTimeoutError() {
  return new Error('设备无响应/ADB 超时，已中止；请检查模拟器/ADB 后重试');
}

// 带超时的 fetch+JSON（AbortController）；超时统一转成可读错误。
async function fetchJsonWithTimeout(url, opts, timeoutMs) {
  const ms = Number.isFinite(Number(timeoutMs)) ? Number(timeoutMs) : ASSET_TEST_TIMEOUT_MS;
  const controller = typeof AbortController === 'function' ? new AbortController() : null;
  const timer = controller && ms > 0 ? setTimeout(() => controller.abort(), ms) : null;
  try {
    const init = Object.assign({}, opts && typeof opts === 'object' ? opts : {});
    if (controller) init.signal = controller.signal;
    const r = await fetch(url, init);
    let body = null;
    try {
      body = await r.json();
    } catch (e) {
      throw new Error('服务端响应不是 JSON（HTTP ' + r.status + '）');
    }
    if (!r.ok) {
      const errors = body && Array.isArray(body.errors) ? body.errors.join('; ') : '';
      throw new Error(errors || (body && body.error) || ('HTTP ' + r.status));
    }
    return body;
  } catch (e) {
    if (e && (e.name === 'AbortError' || e.code === 'ABORT_ERR')) throw assetTestTimeoutError();
    throw e;
  } finally {
    if (timer) clearTimeout(timer);
  }
}

async function postJsonTimed(url, payload, timeoutMs) {
  return fetchJsonWithTimeout(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  }, timeoutMs);
}

// in-flight 防重 + 恢复行内测试按钮（只恢复本轮禁用的，不误伤原本 disabled 的）。
function setAssetTestBusy(busy) {
  assetTestBusy = !!busy;
  const buttons = document.querySelectorAll('.asset-row-action-test, .asset-menu-item');
  for (const btn of buttons) {
    if (busy) {
      if (!btn.disabled) {
        btn.dataset.testBusy = '1';
        btn.disabled = true;
      }
    } else if (btn.dataset && btn.dataset.testBusy) {
      btn.disabled = false;
      delete btn.dataset.testBusy;
    }
  }
  if (btnTestMode) btnTestMode.disabled = !!busy;
}

function beginAssetTest(label) {
  if (assetTestBusy) {
    setStatus('素材测试正在进行，请稍候…', '#ffd54a');
    return false;
  }
  setAssetTestBusy(true);
  setStatus(`${label || '素材测试'}进行中…`, '#ffd54a');
  return true;
}

function endAssetTest() {
  setAssetTestBusy(false);
  if (btnTestMode) btnTestMode.disabled = false;
}

// P2-6: `note` replaces the generic "上面的定位" caveat when the concrete
// gesture is decided by a second server request (collection match re-runs and
// samples inside the exact action shape).
function confirmAssetTestGestures(kind, operations, note) {
  const ops = operations || [];
  if (!ops.length) return false;
  const head = kind === 'drag' ? '真实拖动（滑动）' : '真实点击';
  const shown = ops.slice(0, 8).map((op) => `· ${op.label}`).join('\n');
  const extra = ops.length > 8 ? `\n… 共 ${ops.length} 个手势` : '';
  const caveat = note || '实时画面可能仍在变化，将按上面的定位发送。';
  return window.confirm(
    `即将对真实设备执行${head}，这不是演练。\n\n${shown}${extra}\n\n` +
    `${caveat}确认发送？`
  );
}

function updateAssetTestModeButton() {
  if (!btnTestMode) return;
  btnTestMode.textContent = `素材测试：${assetTestModeLabel(assetTestReal)}`;
  btnTestMode.classList.toggle('test-real', assetTestReal);
  btnTestMode.classList.toggle('test-dry', !assetTestReal);
  btnTestMode.setAttribute('aria-pressed', assetTestReal ? 'true' : 'false');
  btnTestMode.title = assetTestReal
    ? '真实模式：素材行「点击测试/拖动测试」会操作设备；必须开启实时预览，且每次发送前会二次确认'
    : '演练模式（默认）：素材行「点击测试/拖动测试」只计算定位，不操作设备';
}

// --- U-T4/S4：素材测试动作（test_action）纯函数 -------------------------------
// POST /task/asset/test_action（Studio-only）请求体：dry=演练，real=true 必须
// live=1（服务端拒绝不带实时帧的真实点击）。drag 本轮服务端 400，不接。
function testActionRequestBody(gesture, opts) {
  const o = opts && typeof opts === 'object' ? opts : {};
  const kind = gesture === 'long_press' ? 'long_press' : 'click';
  const body = {
    id: String(o.id === undefined || o.id === null ? '' : o.id).trim(),
    gesture: kind,
    real: o.real === true,
  };
  const task = String(o.task === undefined || o.task === null ? '' : o.task).trim();
  if (task) body.task = task;
  if (o.candidateIndex !== undefined && o.candidateIndex !== null &&
      Number.isInteger(Number(o.candidateIndex))) {
    body.candidate_index = Number(o.candidateIndex);
  }
  const frame = o.frame && typeof o.frame === 'object' ? o.frame : {};
  if (frame.live === true) body.live = true;
  else if (Number.isInteger(Number(frame.frameId))) body.frame_id = Number(frame.frameId);
  if (o.holdMs !== undefined && o.holdMs !== null && Number.isFinite(Number(o.holdMs))) {
    body.hold_ms = Math.round(Number(o.holdMs));
  }
  return body;
}

// seq134：真实点测/拖测候选下拉（与结果区/画布共用同一渲染器）。
function candidateOptionLabel(candidate, ctx) {
  return candidateRenderLabel(candidate, ctx);
}

// test_action dry 响应 → 结果区行（含候选序号/真实识别区域/操作区/目标点/链 steps）。
function testActionResultLines(result, candidateIndex, candidate) {
  const s = result && typeof result === 'object' && !Array.isArray(result) ? result : {};
  const index = Number.isInteger(Number(candidateIndex)) ? Number(candidateIndex) : 0;
  const info = candidate ? candidateFieldInfo(candidate) : null;
  const candidateSuffix = info
    ? ' · ' + (info.member ? '成员=' + JSON.stringify(info.member) : '') +
      (info.hasText ? ' 文字=' + JSON.stringify(info.text) : '') +
      (info.hasPass ? ' · ' + info.passText : '') +
      (info.domain === 'screen_fallback' ? ' · 全屏回退' : '')
    : '';
  if (!s.ok) {
    const errors = Array.isArray(s.errors) ? s.errors : ['test_action 失败'];
    return ['#' + index + ' ❌ ' + errors.join('; ') + candidateSuffix];
  }
  const boxText = (box) => (Array.isArray(box) && box.length >= 4
    ? '[' + box.join(',') + ']' : '—');
  const point = s.point && Number.isFinite(Number(s.point.x)) && Number.isFinite(Number(s.point.y))
    ? '(' + Number(s.point.x) + ',' + Number(s.point.y) + ')' : '—';
  const lines = [
    '#' + index + ' ✅ real_rect=' + boxText(s.real_rect) +
    ' action_box=' + boxText(s.action_box) +
    ' point=' + point +
    ' chain_depth=' + Number(s.chain_depth || 0) +
    (s.frame_source ? ' · 帧来源=' + s.frame_source : '') +
    candidateSuffix,
  ];
  for (const step of Array.isArray(s.steps) ? s.steps : []) {
    lines.push('  · ' + String(step.asset || '?') + ' [' + String(step.mode || '?') + ']' +
      ' real=' + boxText(step.real) +
      (step.score !== undefined && step.score !== null
        ? ' score=' + Number(step.score).toFixed(3) : '') +
      (step.text !== undefined && step.text !== null
        ? ' text=' + JSON.stringify(String(step.text)) : ''));
  }
  if (s.real) {
    lines.push('  · 真实模式：' + (s.clicked ? '已点击' : '未点击') +
      ' · tap_ok=' + String(s.tap_ok === true));
  }
  return lines;
}

// test_action dry 响应 → 画布 boxes（真实识别区域 + 操作区）。
function testActionBoxes(result, labelPrefix) {
  const s = result && typeof result === 'object' ? result : {};
  if (!s.ok) return [];
  const out = [];
  const prefix = String(labelPrefix === undefined || labelPrefix === null ? '' : labelPrefix);
  if (Array.isArray(s.real_rect) && matchBoxValid(s.real_rect)) {
    out.push({ text: (prefix ? prefix + ' ' : '') + '真实识别', score: Number(s.score || 1), box: s.real_rect });
  }
  // seq137（B）：点测/长按画服务端 action_shape 多边形；无 shape 才回退 action_box 矩形。
  const actionView = serverActionShapeView(s.action_shape, s.action_box, prefix);
  if (actionView) out.push(actionView);
  return out;
}

// validate_op_region 400({errors:[{path,msg}]}) → 可读多行错误。
function opRegionValidateErrorText(result) {
  const s = result && typeof result === 'object' && !Array.isArray(result) ? result : {};
  if (s.ok) return '';
  const errors = Array.isArray(s.errors) ? s.errors : [];
  const lines = errors.map((error) => {
    if (error && typeof error === 'object') {
      const path = error.path !== undefined && error.path !== null ? String(error.path) : '';
      const msg = error.msg !== undefined && error.msg !== null
        ? String(error.msg)
        : String(error.message !== undefined && error.message !== null ? error.message : '');
      return (path ? path + '：' : '') + (msg || '操作区不合法');
    }
    return String(error);
  });
  return lines.length ? lines.join('\n') : String(s.error || '操作区校验失败');
}

async function fetchActionCandidates(it, task, frame) {
  try {
    // seq133（#3）：候选获取同样带超时（设备/OCR 卡住时也能恢复状态）。
    const s = await fetchJsonWithTimeout(
      assetCandidatesUrl(task, it.id, frame && frame.query ? frame.query : ''),
      {},
      ASSET_TEST_TIMEOUT_MS
    );
    if (!s || s.error) return { ok: false, items: [], error: (s && s.error) || '候选识别失败' };
    return { ok: true, items: Array.isArray(s.candidates) ? s.candidates : [], error: '' };
  } catch (e) {
    return { ok: false, items: [], error: String(e && e.message ? e.message : e) };
  }
}

// 真实点测/拖测：用侧栏下拉选择候选（#序号 score · 文字 · 期望/非期望），返回候选序号。
// seq134：默认选中 index 0（最高分候选）；pass 只表示阈值，非 pass 候选同样可选测。
// 取消/没有候选返回 null。
function pickActionCandidate(candidates, it, purpose) {
  const what = purpose || '真实点击';
  const items = Array.isArray(candidates) ? candidates : [];
  if (!items.length) return Promise.resolve(null);
  return new Promise((resolve) => {
    if (!candidateSelect) {
      resolve(Number(items[0].index));
      return;
    }
    candidateSelect.textContent = '';
    for (const candidate of items) {
      const option = document.createElement('option');
      option.value = String(candidateFieldInfo(candidate).index);
      option.textContent = candidateOptionLabel(candidate);
      candidateSelect.appendChild(option);
    }
    // 默认 index 0（后端已按 score→y→x 排序）：直接高亮；改选其它行走 change。
    const defaultIndex = String(candidateFieldInfo(items[0]).index);
    candidateSelect.value = defaultIndex;
    candidateSelect.hidden = false;
    setStatus(`请选择候选区域：素材「${it.id}」（${what}；默认 #${defaultIndex} 最高分候选，可改选，Enter 确认）`, '#ffd54a');
    let done = false;
    const finish = (value) => {
      if (done) return;
      done = true;
      candidateSelect.onchange = null;
      candidateSelect.onkeydown = null;
      candidateSelect.onblur = null;
      candidateSelect.hidden = true;
      resolve(value === '' || value === null ? Number(items[0].index) : Number(value));
    };
    candidateSelect.onchange = () => finish(candidateSelect.value);
    candidateSelect.onkeydown = (ev) => {
      if (ev.key === 'Enter') {
        ev.preventDefault();
        finish(candidateSelect.value);
      }
    };
    candidateSelect.onblur = () => finish(candidateSelect.value);
  });
}

// U-T4/S4：素材行点测/长按 = POST /task/asset/test_action。
// dry：逐候选演练并画 real_rect/action_box/point + chain steps；
// real：先选候选（#candidate-select）→ dry 取目标点 → 二次确认 → real:true,live:1。
// drag 本轮服务端 400（「drag 测试暂未支持」）：拖测保持旧实现（testAssetDrag）。
async function runTestActionForAsset(it, gesture, frameId, scopeTask) {
  cancelRecognitionRoiEdit(gesture === 'long_press' ? '长按测试' : '点击测试');
  const mode = realAssetTestGuard();
  if (!mode.ok) return;
  const task = assetRowScopeTask(scopeTask, false);
  const real = assetTestReal;
  let frames = testFrames(it, frameId);
  if (real) {
    const liveFrame = frames.find((frame) => frame.live) ||
      { query: '&live=1', label: '实时帧', source: 'live', live: true, frameId: null };
    frames = [liveFrame];
  }
  const gestureLabel = gesture === 'long_press' ? '长按' : '点击';
  // seq133（#3）：in-flight 防重；任何成功/失败/超时都在 finally 恢复按钮/状态。
  if (!beginAssetTest(`${gestureLabel}测试`)) return;
  const lines = [];
  const collectedBoxes = [];
  // seq137（B）：操作区直接消费服务端 action_shape（帧绝对坐标，含 fixed/链终端），
  // 不再用素材自身 action_shape 做前端二次平移；无 shape 才回退 action_box 矩形。
  function pushCandidateActionBox(label, result) {
    const view = serverActionShapeView(result && result.action_shape, result && result.action_box, label);
    if (view) collectedBoxes.push(view);
  }
  resultEl.textContent = `素材「${it.id}」${gestureLabel}测试（${real ? '真实模式' : '演练'}）…`;
  log(`test_action 开始 asset=${it.id} gesture=${gesture} real=${real}`, 'INFO', {
    frames: frames.map((f) => f.label),
  });
  try {
    for (const frame of frames) {
      const candidates = await fetchActionCandidates(it, task, frame);
      if (!candidates.ok) {
        lines.push(`${frame.label}: ❌ ${candidates.error}`);
        continue;
      }
      if (!candidates.items.length) {
        lines.push(`${frame.label}: （识别区域内没有识别行；pass=false 的行也会显示）`);
        continue;
      }
      if (real) {
        const index = await pickActionCandidate(candidates.items, it);
        if (index === null) {
          lines.push(`${frame.label}: 已取消（未选择候选）`);
          continue;
        }
        const picked = candidates.items.find((c) => Number(candidateFieldInfo(c).index) === Number(index)) ||
          candidates.items[0];
        let dry = null;
        try {
          dry = await postJsonTimed('/task/asset/test_action', testActionRequestBody(gesture, {
            task, id: it.id, candidateIndex: index, frame, real: false, holdMs: gesture === 'long_press' ? 3000 : undefined,
          }), ASSET_TEST_TIMEOUT_MS);
        } catch (e) {
          lines.push(...testActionResultLines({ ok: false, errors: [String(e && e.message ? e.message : e)] }, index, picked));
          continue;
        }
        if (!dry || !dry.ok) {
          lines.push(...testActionResultLines(dry, index, picked));
          continue;
        }
        const point = dry.point || {};
        const gestureOp = {
          label: `${frame.label} 候选 #${index} (${point.x},${point.y})` +
            (candidateFieldInfo(picked).hasPass ? ' · ' + candidateFieldInfo(picked).passText : '') +
            (candidateFieldInfo(picked).domain === 'screen_fallback' ? ' · 全屏回退' : ''),
          x: Number(point.x),
          y: Number(point.y),
        };
        if (!confirmAssetTestGestures('click', [gestureOp])) {
          lines.push(`${frame.label} #${index}: 已取消真实${gestureLabel}，未发送设备请求`);
          continue;
        }
        let sent = null;
        try {
          sent = await postJsonTimed('/task/asset/test_action', testActionRequestBody(gesture, {
            task, id: it.id, candidateIndex: index, frame, real: true, holdMs: gesture === 'long_press' ? 3000 : undefined,
          }), ASSET_TEST_TIMEOUT_MS);
        } catch (e) {
          lines.push(...testActionResultLines({ ok: false, errors: [String(e && e.message ? e.message : e)] }, index, picked));
          continue;
        }
        lines.push(...testActionResultLines(sent, index, picked).map((line) => line.replace(/^#/, '#')));
        lines[lines.length - 1] += ' · 帧=' + frame.label;
        collectedBoxes.push(...testActionBoxes(sent, `${frame.label} #${index}`));
        pushCandidateActionBox(`${frame.label} #${index}`, sent);
      } else {
        for (const candidate of candidates.items) {
          let dry = null;
          try {
            dry = await postJsonTimed('/task/asset/test_action', testActionRequestBody(gesture, {
              task, id: it.id, candidateIndex: candidate.index, frame, real: false, holdMs: gesture === 'long_press' ? 3000 : undefined,
            }), ASSET_TEST_TIMEOUT_MS);
          } catch (e) {
            lines.push(...testActionResultLines({ ok: false, errors: [String(e && e.message ? e.message : e)] }, candidate.index, candidate));
            continue;
          }
          const candidateLines = testActionResultLines(dry, candidate.index, candidate);
          lines.push(`${frame.label} ${candidateLines[0]}`);
          for (let i = 1; i < candidateLines.length; i += 1) lines.push(candidateLines[i]);
          for (const box of testActionBoxes(dry, `${frame.label} #${candidate.index}`)) {
            collectedBoxes.push(box);
          }
          pushCandidateActionBox(`${frame.label} #${candidate.index}`, dry);
        }
      }
    }
    boxes = collectedBoxes;
    draw();
    const header = `素材「${it.id}」${gestureLabel}测试（${real ? '真实模式' : '演练，不实际点击'}）`;
    const tail = real
      ? '（真实模式：已按候选执行；设备/手势结果见上。候选=统一候选列表，pass=是否达到阈值。）'
      : '说明：候选=统一候选列表（图像=缩略图/分数，文字=text/分数，集合=member/member_kind/text）；' +
        'pass=是否达到阈值；默认 #0 最高分候选。演练只验证定位，不操作设备。';
    resultEl.textContent = `${header}\n` + lines.join('\n') + '\n' + tail;
    log(`test_action 完成 asset=${it.id} gesture=${gesture}`, 'INFO', { real, lines });
  } catch (e) {
    const msg = e && e.message ? e.message : String(e);
    resultEl.textContent = `${gestureLabel}测试异常：` + msg;
    log(`test_action 异常 asset=${it.id}`, 'ERROR', { error: msg });
  } finally {
    endAssetTest();
  }
}

async function testAssetClick(it, frameId, scopeTask) {
  return runTestActionForAsset(it, 'click', frameId, scopeTask);
}


async function testAssetDrag(it, frameId, scopeTask) {
  cancelRecognitionRoiEdit('拖动测试');
  const mode = realAssetTestGuard();
  if (!mode.ok) return;
  const direction = (window.prompt('拖动方向（up/down/left/right）', 'up') || '').trim();
  if (!['up', 'down', 'left', 'right'].includes(direction)) {
    resultEl.textContent = `${assetTestReal ? '真实滑动模式' : '演练'}拖动方向必须是 up/down/left/right`;
    return;
  }
  const task = assetRowScopeTask(scopeTask, false);
  const frames = testFrames(it, frameId);
  const lines = [];
  const realOps = [];
  const candidateBoxesForDraw = [];
  let stepAction = null;
  let stepShape = null;
  const fw = canvas.width || 1280;
  const fh = canvas.height || 720;
  // seq133（#3）：in-flight 防重；finally 恢复按钮/状态（含候选获取失败/超时）。
  if (!beginAssetTest('拖动测试')) return;
  try {
    for (const frame of frames) {
      // B3a（task-27 c）：拖测先取全部候选；多候选时用 #candidate-select 选一个，
      // 再用该候选的真实识别区域/操作区走旧拖测路径（不接后端 test_action drag，仍 400）。
      const fetched = await fetchActionCandidates(it, task, frame);
      if (!fetched.ok) {
        lines.push(`${frame.label}: ❌ ${fetched.error}`);
        continue;
      }
      if (!fetched.items.length) {
        lines.push(`${frame.label}: ❌ 识别区域内没有识别行（未生成该帧拖动点）`);
        continue;
      }
      let candidateIndex = Number(candidateFieldInfo(fetched.items[0]).index);
      let candidate = fetched.items[0];
      if (fetched.items.length > 1) {
        const picked = await pickActionCandidate(fetched.items, it, '拖测');
        if (picked === null) {
          lines.push(`${frame.label}: 已取消（未选择候选）`);
          continue;
        }
        candidateIndex = picked;
        candidate = fetched.items.find((c) => Number(candidateFieldInfo(c).index) === Number(picked)) || fetched.items[0];
      }
      const recognizedBox = [Number(candidate.x), Number(candidate.y), Number(candidate.w), Number(candidate.h)];
      if (!matchBoxValid(recognizedBox)) {
        lines.push(`${frame.label}: ❌ 候选坐标无效 #${candidateIndex} ` +
          `(${candidate.x},${candidate.y},${candidate.w},${candidate.h})（未生成该帧拖动点）`);
        continue;
      }
      const candidateInfoForDrag = candidateFieldInfo(candidate);
      const dragCandidateBoxes = candidateBoxes(fetched.items);
      for (const box of dragCandidateBoxes) {
        const poly = candidatePolygonForAsset(it, box.box);
        if (poly) box.polygon = poly;
        candidateBoxesForDraw.push(box);
      }
      candidateBoxesForDraw.push({
        text: `拖测 ${it.id} ${frame.label} 候选 #${candidateIndex}` +
          candidateTextPreview(candidate, 16) +
          (candidateInfoForDrag.member ? ' · ' + candidateInfoForDrag.member : '') +
          (candidateInfoForDrag.domain === 'screen_fallback' ? ' · 全屏回退' : ''),
        score: Number(candidate.score || 1),
        box: recognizedBox,
        polygon: candidatePolygonForAsset(it, recognizedBox),
        pass: candidateInfoForDrag.pass,
        hasPass: candidateInfoForDrag.hasPass,
        member: candidateInfoForDrag.member,
        memberKind: candidateInfoForDrag.memberKind,
        domain: candidateInfoForDrag.domain,
      });
      // seq137（B）：拖测绘制操作区直接消费服务端候选 action_shape；采样几何也优先
      // 用服务端 shape（无 shape 时才回退原素材 shape 的匹配平移）。
      const effShape = candidate.action_shape || (it.kind === 'template'
        ? translatedActionShapeForMatch(it, { x: recognizedBox[0], y: recognizedBox[1] })
        : (it.action_shape || null));
      const serverActionView = serverActionShapeView(
        candidate.action_shape, candidate.action_box, `操作区 #${candidateIndex}`
      );
      if (serverActionView) candidateBoxesForDraw.push(serverActionView);
      const action = assetActionBox(it, effShape, recognizedBox || it.roi) || it.roi;
      if (!action) {
        lines.push(`${frame.label}: ❌ 没有识别/操作区域`);
        continue;
      }
      const from = randomPointInAction(it, effShape, recognizedBox || it.roi);
      const dist = 150 + Math.floor(Math.random() * 200);
      const lateralMargin = Math.max(4, Math.floor((direction === 'up' || direction === 'down' ? action[2] : action[3]) * 0.3));
      const jitter = randomInt(-lateralMargin, lateralMargin);
      const to = {
        x: from.x + (direction === 'left' ? -dist : direction === 'right' ? dist : jitter),
        y: from.y + (direction === 'up' ? -dist : direction === 'down' ? dist : jitter),
      };
      to.x = Math.max(0, Math.min(fw - 1, to.x));
      to.y = Math.max(0, Math.min(fh - 1, to.y));
      if (direction === 'up' || direction === 'down') to.x = Math.max(0, Math.min(fw - 1, from.x + jitter));
      if (direction === 'left' || direction === 'right') to.y = Math.max(0, Math.min(fh - 1, from.y + jitter));
      if (!stepAction) stepAction = action.slice();
      // O-2: snapshot the frame shape too (template branch already translated
      // it via translatedActionShapeForMatch); the step stays frame-absolute.
      if (!stepShape && effShape) stepShape = effShape;
      lines.push(
        `${frame.label}: ${direction} (${from.x},${from.y}) → (${to.x},${to.y}) · ` +
        `操作区[${action.join(',')}]${it.kind === 'template' ? '（已按匹配平移）' : ''}` +
        ` · 候选 #${candidateIndex}（共 ${fetched.items.length} 个）` +
        (candidateInfoForDrag.member ? ' · 成员=' + JSON.stringify(candidateInfoForDrag.member) : '') +
        (candidateInfoForDrag.hasText ? ' · 文字=' + JSON.stringify(candidateInfoForDrag.text) : '') +
        (candidateInfoForDrag.hasPass ? ' · ' + candidateInfoForDrag.passText : '') +
        (candidateInfoForDrag.domain === 'screen_fallback' ? ' · 全屏回退' : '')
      );
      if (assetTestReal) {
        const swipeMs = 300 + Math.floor(Math.random() * 351);
        realOps.push({
          label: `${frame.label} 候选 #${candidateIndex} 滑动 (${from.x},${from.y}) → (${to.x},${to.y}) ${swipeMs}ms`,
          from,
          to,
          ms: swipeMs,
        });
      }
    }
    if (candidateBoxesForDraw.length) {
      boxes = candidateBoxesForDraw;
      draw();
    }
    if (!stepAction) {
      resultEl.textContent =
        `${assetTestReal ? '真实滑动模式' : '演练'}拖动测试失败：「${it.id}」没有可用操作区域\n` +
        lines.join('\n');
      return;
    }
    // O-2: same builder as 区域拖动; stepShape is the frame snapshot above.
    const step = regionDragStep(stepAction, stepShape, direction, [150, 350]);
    let modeNote = '';
    if (assetTestReal) {
      if (!realOps.length) {
        modeNote = '真实模式：没有可执行的滑动（未发送设备请求）。';
      } else if (!confirmAssetTestGestures('drag', realOps)) {
        modeNote = '真实模式：已取消，未发送设备请求。';
      } else {
        let sentOk = 0;
        let sentFail = 0;
        const sent = [];
        for (const op of realOps) {
          try {
            const res = await sendAssetTestGesture(true, 'swipe', op.from, op.to, op.ms);
            sentOk += 1;
            sent.push(`${op.label}: ${res.text}`);
          } catch (e) {
            sentFail += 1;
            sent.push(`${op.label}: ❌ 真实滑动失败：${e && e.message ? e.message : e}`);
          }
        }
        lines.push(...sent);
        modeNote =
          `真实模式：已发送 ${sentOk}/${realOps.length} 个滑动` +
          (sentFail ? `，失败 ${sentFail}` : '') +
          '（服务端 device/gesture 见上）。';
      }
    }
    const header = assetTestReal
      ? `拖动测试（真实滑动模式）「${it.id}」`
      : `拖动测试（演练，不实际滑动）「${it.id}」`;
    resultEl.textContent =
      `${header}\n` +
      `方向 ${direction} · 测试帧 ${testFrameLabel(it, frameId)}（${frames.length} 帧）\n` +
      lines.join('\n') +
      (assetTestReal ? '\n' + modeNote : '\n\n' + JSON.stringify(step, null, 2));
    log(
      assetTestReal
        ? `素材拖动测试（真实滑动模式）：${it.id} ${direction}`
        : `素材拖动测试（演练）：${it.id} ${direction}`,
      assetTestReal ? 'WARN' : 'INFO',
      {
        frames: frames.map((f) => f.label),
        details: lines,
        gestures: realOps.length,
      }
    );
  } catch (e) {
    const msg = e && e.message ? e.message : String(e);
    resultEl.textContent =
      `${assetTestReal ? '真实滑动模式' : '演练'}拖动测试异常：` + msg;
    log(`素材拖动测试异常：${it.id}`, 'ERROR', { error: msg });
  } finally {
    endAssetTest();
  }
}

async function generateAssetStep(it) {
  cancelRecognitionRoiEdit('生成素材步骤');
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

// seq134（spec D）：素材行识别/点测/拖测/长按统一走 testAssetCandidates →
// GET /task/asset/candidates。旧素材行 OCR 路径（testAssetOcr/probeAssetOcrFrame）
// 及其全屏 /lines 回退已删除；主画布 OCR 按钮的 fallbackFullscreenLines 保留。

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
    if (!p.has_actual) continue;
    const x = Number(p.x);
    const y = Number(p.y);
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
    if (!p.has_actual) continue;
    const x = Number(p.x);
    const y = Number(p.y);
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
  // seq130（#2）：目标点（target_x/target_y）单独渲染为橙色圆环+十字，与
  // 实际点击（白点/热力）区分；miss 记录加红叉提示未命中。
  for (const p of clickData.points) {
    if (!p.has_target) continue;
    const q = window.OAS_HEATMAP.mapPoint({ x: p.target_x, y: p.target_y }, w, h);
    octx.save();
    octx.strokeStyle = p.miss ? 'rgba(255,80,80,0.95)' : 'rgba(255,159,64,0.95)';
    octx.lineWidth = 1.6;
    octx.beginPath();
    octx.arc(q.x, q.y, 6, 0, Math.PI * 2);
    octx.stroke();
    octx.beginPath();
    octx.moveTo(q.x - 8, q.y);
    octx.lineTo(q.x + 8, q.y);
    octx.moveTo(q.x, q.y - 8);
    octx.lineTo(q.x, q.y + 8);
    octx.stroke();
    if (p.miss) {
      octx.beginPath();
      octx.moveTo(q.x - 4, q.y - 4);
      octx.lineTo(q.x + 4, q.y + 4);
      octx.moveTo(q.x + 4, q.y - 4);
      octx.lineTo(q.x - 4, q.y + 4);
      octx.stroke();
    }
    octx.restore();
  }
  overlay.dataset = overlay.dataset || {};
  clickOverlay = overlay;
}

// v1.2.4: the click-distribution canvas shows ONLY heat + scatter points.
// The old top-left `clicks=N distinct=K` plate and mini legend were removed so
// the game's top-left corner stays readable; the same numbers remain in the
// sidebar (#click-stats) and its legend (#clicks-legend).
function drawClickOverlay() {
  // The distribution overlay is only for the click-distribution panel. Drawing
  // it during normal素材采集 interferes with ROI/frame selection and hides the
  // game screen behind heat colors.
  const clickPanel = document.getElementById('panel-click');
  if (!clickPanel || !clickPanel.classList.contains('active')) return;
  if (!clickData) return;
  if (clickData.points && clickData.points.length) {
    const cacheKey = String(clickData.file || '') + '|' + clickData.points.length +
      '|' + canvas.width + 'x' + canvas.height;
    if (!clickOverlay || clickOverlay.__oasKey !== cacheKey) {
      buildClickOverlay();
      if (clickOverlay) clickOverlay.__oasKey = cacheKey;
    }
    if (clickOverlay) ctx.drawImage(clickOverlay, 0, 0);
  }
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
    const rawPoints = Array.isArray(s.points) ? s.points : [];
    const points = normalizeClickPoints(rawPoints);
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
    const rawPoints = Array.isArray(s.points) ? s.points : [];
    const points = normalizeClickPoints(rawPoints);
    const clicks = Number.isFinite(Number(s.clicks)) ? Number(s.clicks) : points.length;
    const distinct = Number.isFinite(Number(s.distinct))
      ? Number(s.distinct)
      : window.OAS_HEATMAP.countDistinct(points);
    clickData = { file: s.file || file, clicks, distinct, points };
    clickOverlay = null;
    draw();
    updateClickStatsText();
    const targetCount = points.filter((q) => q.has_target).length;
    const missCount = points.filter((q) => q.miss).length;
    setClicksStatus(
      `已叠加 ${clickData.file} · clicks=${clicks} distinct=${distinct}（${points.length} 个点 · 目标 ${targetCount} · miss ${missCount}）`
    );
    log(`点击分布已叠加：${clickData.file} clicks=${clicks} distinct=${distinct} 目标=${targetCount} miss=${missCount}`);
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

// Asset CRUD/test endpoints accept any non-empty `task` in global mode (the
// server ignores it and uses the unified assets/ store). This keeps global
// library management usable without pretending a real task exists.
function assetScopeTask(requireTask) {
  if (assetsGlobalMode) return GLOBAL_SCOPE_TASK;
  const t = (taskName.value || '').trim();
  if (!t && requireTask) {
    resultEl.textContent = '请先填任务名（或在素材页使用全局素材库）';
    if (taskName) taskName.focus();
    return null;
  }
  return t;
}

// C: row-scoped variant of assetScopeTask. `scopeTask` is an optional override
// used by the frame-detail list, whose rows come from BOTH libraries; without it
// (both libraries) the old behaviour is preserved byte for byte.
function assetRowScopeTask(scopeTask, requireTask) {
  const explicit = scopeTask == null ? '' : String(scopeTask).trim();
  if (explicit) return explicit;
  return assetScopeTask(requireTask);
}

// In global mode the API `task` is the scope label, which is NOT a real task
// name; refresh the task panel with the currently selected task instead.
// A row scope may also be GLOBAL_SCOPE_TASK (frame detail global row), which is
// never a task name: resolve it to the selected task as well.
function refreshTaskAssetsForScope(scopeTask) {
  if (assetsGlobalMode || scopeTask === GLOBAL_SCOPE_TASK) return refreshTaskAssets(undefined);
  return refreshTaskAssets(scopeTask);
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

// U-T4/S3（事项 3）：素材行「识别」= 返回**全部**真实识别区域 + 评分。
// 统一模板/集合/OCR/ocr_var/region：GET /task/asset/candidates（P0a 已上线），
// 每帧的候选全部画到画布（boxes），结果区按 `#i score=… [x,y,w,h]` 逐行列出。
// 旧单结果接口（/task/asset/match、/ocr、/lines）保留给点测/拖测等测试路径。
async function testAssetCandidates(it, frameId, scopeTask) {
  cancelRecognitionRoiEdit('素材候选识别');
  const task = assetRowScopeTask(scopeTask, false);
  const frames = testFrames(it, frameId);
  const kind = it.kind === 'template_set' ? 'collection' : it.kind;
  const fullScreenOcr = !Array.isArray(it.roi) &&
    (kind === 'ocr_var' || kind === 'ocr_line' || kind === 'ocr_lines');
  const lines = [];
  const collected = [];
  resultEl.textContent = `素材「${it.id}」候选识别中…（${frames.length} 帧）`;
  log(`候选识别开始 asset=${it.id}`, 'INFO', {
    frames: frames.map((f) => f.label),
    kind,
    roi: Array.isArray(it.roi) ? it.roi : null,
    full_screen_ocr: fullScreenOcr,
    task,
  });
  try {
    for (const frame of frames) {
      let s = null;
      try {
        // seq134：统一候选入口带超时，并识别服务端 {ok:false,errors} 错误体。
        s = await fetchJsonWithTimeout(
          assetCandidatesUrl(task, it.id, frame.query), {}, ASSET_TEST_TIMEOUT_MS
        );
      } catch (e) {
        const msg = e && e.message ? e.message : String(e);
        lines.push(`${frame.label}: ❌ 候选识别失败：${msg}`);
        log(`候选识别失败 asset=${it.id} frame=${frame.label}`, 'ERROR', { error: msg });
        continue;
      }
      if (!s || s.ok === false) {
        const errors = s && Array.isArray(s.errors) ? s.errors : [];
        const msg = errors.length ? errors.join('；') : ((s && s.error) || '候选识别失败');
        lines.push(`${frame.label}: ❌ ${msg}`);
        log(`候选识别错误 asset=${it.id}`, 'ERROR', { errors, error: msg });
        continue;
      }
      const candidates = Array.isArray(s.candidates) ? s.candidates : [];
      const domainText = candidateDomainText(s.domain) || '素材识别区';
      const thresholdText = s.threshold === null || s.threshold === undefined
        ? '—'
        : Number(s.threshold).toFixed(2);
      const screenFallback = s.domain === 'screen_fallback' ||
        candidates.some((c) => c && c.domain === 'screen_fallback');
      const valueHint = screenFallback
        ? '（ROI 未命中→全屏回退候选，可直接点测/拖测）'
        : (s.domain === 'screen' ? '（全屏）' : '');
      lines.push(
        `${frame.label}: ${Number(s.count != null ? s.count : candidates.length)} 个候选` +
        ` · 范围=${domainText}${valueHint} · 阈值=${thresholdText}` +
        ` · 帧来源=${s.frame_source || frame.source}`
      );
      if (!candidates.length) {
        lines.push('  （该范围内没有候选/识别行' +
          (s.expected_text ? '；期望文字=' + JSON.stringify(String(s.expected_text)) : '') + '）');
      }
      for (const candidate of candidates) {
        lines.push('  ' + candidateLabelText(candidate, { task, assetId: it.id }));
      }
      const candidateDrawBoxes = candidateBoxes(candidates, { task, assetId: it.id });
      for (const box of candidateDrawBoxes) {
        const poly = candidatePolygonForAsset(it, box.box);
        if (poly) box.polygon = poly;
      }
      // seq137（B）：候选操作区直接消费服务端 action_shape（帧绝对坐标，含 fixed/
      // OCR/链终端解析），不再用素材自身 shape 做前端二次平移。
      collected.push(...candidateDrawBoxes);
      for (const candidate of candidates) {
        const view = serverActionShapeView(candidate.action_shape, candidate.action_box, '候选');
        if (view) collected.push(view);
      }
    }
    boxes = collected;
    draw();
    const head = fullScreenOcr ? '识别区域为空 → 全屏候选（较慢）\n' : '';
    resultEl.textContent =
      `${head}素材「${it.id}」候选识别（${frames.length} 帧）\n` + lines.join('\n') +
      '\n\n说明：统一候选列表（图像=缩略图/分数，文字=text/分数，集合=member/member_kind/text；' +
      'pass=是否达到阈值）。OCR 有期望文字且 ROI 未命中时服务端返回 domain=screen_fallback 全屏候选，可直接点测/拖测。';
    log(`候选识别完成 asset=${it.id}`, 'INFO', {
      count: collected.length,
      frames: frames.length,
    });
  } catch (e) {
    const msg = e && e.message ? e.message : String(e);
    resultEl.textContent = '候选识别异常：' + msg;
    log(`候选识别异常 asset=${it.id}`, 'ERROR', { error: msg });
  }
}

function runAssetTest(it, frameId, scopeTask) {
  cancelRecognitionRoiEdit('素材测试');
  // region 素材本身即操作区域：选中它（保留旧行为），再显示候选列表（单候选）。
  if (it.kind === 'region') pickAssetRoi(it);
  return testAssetCandidates(it, frameId, scopeTask);
}

// U-T4/S2：识别文字（runtime_text）对话框。新建 ocr_var（从识别结果）与编辑
// 已有 ocr_var 共用；保存 POST /task/save_asset 或 /task/asset/update，成功后刷新
// 素材库 + 配置项面板（GET /task/config 现含派生行）。
function editAssetText(it, scopeTask) {
  if (!it) return;
  if (it.virtual === true) {
    // 用户简化令后后端不再合成虚拟素材；该分支仅作防御（不扩展迁移 UI）。
    resultEl.textContent = `素材「${it.id}」是只读虚拟素材，不能编辑识别文字。`;
    setStatus(`虚拟素材「${it.id}」只读`);
    return;
  }
  if (it.kind !== 'ocr_var') {
    resultEl.textContent = `素材「${it.id}」（${it.kind}）不是「使用端OCR文字配置项」，没有识别文字可编辑。`;
    setStatus('仅 ocr_var 素材可编辑识别文字');
    return;
  }
  openAssetTextDialog(it, scopeTask);
}

// ---------- U-T4/S2：识别文字对话框（纯 DOM 层） ----------
let assetTextDialogCtx = null;

// 两个素材库缓存合并（global 优先；同一 id 只出现一次）。
function assetDialogLibrary() {
  const out = [];
  const seen = {};
  for (const list of [globalAssetsCache || [], taskAssets || []]) {
    for (const asset of Array.isArray(list) ? list : []) {
      if (!asset || asset.id === undefined || asset.id === null) continue;
      const id = String(asset.id);
      if (seen[id]) continue;
      seen[id] = true;
      out.push(asset);
    }
  }
  return out;
}

function assetDialogScopeInfo(scopeTask) {
  const scope = assetRowScopeTask(scopeTask, false);
  const task = scope === GLOBAL_SCOPE_TASK
    ? String((taskName && taskName.value) || '').trim()
    : scope;
  return { scope: scope, task: task };
}

function openAssetTextDialog(asset, scopeTask, createCtx) {
  if (!assetTextDialog) return;
  const scopeInfo = assetDialogScopeInfo(scopeTask);
  let draft;
  if (createCtx && createCtx.candidate) {
    draft = assetTextDraftFromCandidate(createCtx.candidate, {
      name: createCtx.name,
      text: createCtx.text,
      positions: createCtx.positions,
      task: scopeInfo.task,
    });
  } else {
    if (!asset) return;
    draft = assetTextDraftStart(asset, { task: scopeInfo.task });
  }
  assetTextDialogCtx = {
    draft: draft,
    asset: asset || null,
    scope: scopeInfo.scope,
    task: scopeInfo.task,
  };
  renderAssetTextDialog();
  if (typeof assetTextDialog.showModal === 'function') {
    if (!assetTextDialog.open) assetTextDialog.showModal();
  } else {
    assetTextDialog.setAttribute('open', '');
  }
}

function renderAssetTextDialog() {
  const ctx = assetTextDialogCtx;
  if (!ctx) return;
  const d = ctx.draft;
  const create = d.mode === 'create';
  if (assetTextTitle) {
    assetTextTitle.textContent = create
      ? '新建「使用端OCR文字配置项」'
      : '识别文字 · ' + d.id;
  }
  if (assetTextHint) {
    assetTextHint.textContent = create
      ? '新建 ocr_var：runtime_text 必填（≤200 字）；param_key 按素材 id 自动生成，创建后只读；roi 留空 = 全屏 OCR。'
      : 'runtime_text 是默认识别文字；使用端在 Runner 覆盖。param_key 创建后只读。改 roi 为全屏会走替换保存（需要位置帧）。';
  }
  if (assetTextId) {
    assetTextId.value = d.id;
    assetTextId.readOnly = !create;
    assetTextId.title = create ? '素材 id（全局唯一）' : '素材 id 创建后不可修改';
  }
  if (assetTextRuntime) assetTextRuntime.value = d.runtimeText;
  if (assetTextParamKey) {
    assetTextParamKey.value = d.paramKey;
    assetTextParamKey.readOnly = !create;
    assetTextParamKey.title = create
      ? '按素材 id 自动生成（p_ + 8 位十六进制）；可改，创建后只读'
      : '创建后只读（改键会使 Runner 已保存的值失效）';
  }
  if (assetTextLabel) assetTextLabel.value = d.label;
  if (assetTextFullScreen) assetTextFullScreen.checked = !!d.fullScreen;
  if (assetTextRoi) assetTextRoi.hidden = !!d.fullScreen;
  const roi = Array.isArray(d.roi) ? d.roi : [];
  if (assetTextRoiX) assetTextRoiX.value = roi.length === 4 ? String(roi[0]) : '';
  if (assetTextRoiY) assetTextRoiY.value = roi.length === 4 ? String(roi[1]) : '';
  if (assetTextRoiW) assetTextRoiW.value = roi.length === 4 ? String(roi[2]) : '';
  if (assetTextRoiH) assetTextRoiH.value = roi.length === 4 ? String(roi[3]) : '';
  if (assetTextPositions) {
    assetTextPositions.textContent = '位置帧：' + (d.positions.length
      ? d.positions.map((id) => '#' + id).join('、')
      : '（未选择；新建必须至少一个位置帧）');
  }
  if (assetTextError) assetTextError.textContent = '';
  if (assetTextSave) assetTextSave.disabled = false;
}

function readAssetTextDraftFromDialog() {
  const ctx = assetTextDialogCtx;
  if (!ctx) return null;
  const intOrNaN = (el) => {
    const text = String(el && el.value !== undefined && el.value !== null ? el.value : '').trim();
    if (!text) return null;
    const n = Number(text);
    return Number.isInteger(n) ? n : NaN;
  };
  const fullScreen = !!(assetTextFullScreen && assetTextFullScreen.checked);
  const x = intOrNaN(assetTextRoiX);
  const y = intOrNaN(assetTextRoiY);
  const w = intOrNaN(assetTextRoiW);
  const h = intOrNaN(assetTextRoiH);
  let roi = null;
  if (!fullScreen) {
    if ([x, y, w, h].every((v) => v !== null && !Number.isNaN(v))) roi = [x, y, w, h];
  }
  return Object.assign({}, ctx.draft, {
    id: String(assetTextId && assetTextId.value !== undefined ? assetTextId.value : '').trim(),
    runtimeText: String(assetTextRuntime && assetTextRuntime.value !== undefined
      ? assetTextRuntime.value : ''),
    paramKey: String(assetTextParamKey && assetTextParamKey.value !== undefined
      ? assetTextParamKey.value : '').trim(),
    label: String(assetTextLabel && assetTextLabel.value !== undefined
      ? assetTextLabel.value : '').trim(),
    fullScreen: fullScreen,
    roi: roi,
  });
}

function closeAssetTextDialog() {
  const dlg = assetTextDialog;
  assetTextDialogCtx = null;
  if (!dlg) return;
  try {
    if (typeof dlg.close === 'function' && dlg.open) dlg.close();
  } catch (e) { /* cosmetic */ }
  dlg.removeAttribute('open');
}

async function refreshAssetsAfterOcrVarChange(task, scope) {
  try { await refreshTaskAssetsForScope(scope); } catch (e) { /* library refresh is best-effort */ }
  try { await refreshGlobalAssets(); } catch (e) { /* same */ }
  // D 批：用户侧参数 UI 已删除；仅通过内部桥刷新派生 metadata（builder 诊断用）。
  if (window.OAS_TASK_CONFIG && typeof window.OAS_TASK_CONFIG.refresh === 'function' && task) {
    try { await window.OAS_TASK_CONFIG.refresh(task); } catch (e) { /* same */ }
  }
}

async function saveAssetTextDialog() {
  const ctx = assetTextDialogCtx;
  if (!ctx || !assetTextSave) return;
  const draft = readAssetTextDraftFromDialog();
  if (!draft) return;
  const library = assetDialogLibrary();
  const request = assetTextSaveRequest(draft, {
    task: ctx.task,
    existingIds: library.map((item) => String(item.id)),
    existingParamKeys: library
      .filter((item) => item && item.kind === 'ocr_var' && item.param_key !== undefined && item.param_key !== null)
      .map((item) => String(item.param_key)),
  });
  if (!request.ok) {
    if (assetTextError) assetTextError.textContent = request.errors.join('\n');
    return;
  }
  assetTextSave.disabled = true;
  if (assetTextError) assetTextError.textContent = '';
  try {
    const saved = await postJson(request.url, request.payload);
    if (!saved || !saved.ok) {
      const errors = (saved && saved.errors) || ['保存失败'];
      if (assetTextError) assetTextError.textContent = errors.join('\n');
      return;
    }
    const item = saved.item || request.payload;
    const itemId = String(item.id === undefined || item.id === null ? '' : item.id);
    const runtimeText = item.runtime_text !== undefined && item.runtime_text !== null
      ? String(item.runtime_text) : String(request.payload.runtime_text);
    const paramKey = item.param_key !== undefined && item.param_key !== null
      ? String(item.param_key) : String(request.payload.param_key);
    const roiText = Array.isArray(item.roi) && item.roi.length === 4
      ? '[' + item.roi.join(',') + ']' : '空（全屏 OCR）';
    closeAssetTextDialog();
    setStatus(`识别文字已保存：${itemId}`, '#7bd88f');
    resultEl.textContent =
      `素材「${itemId}」识别文字已保存\n` +
      `runtime_text：${runtimeText}\nparam_key：${paramKey}\n识别区：${roiText}`;
    log(`识别文字保存 asset=${itemId} kind=ocr_var`);
    await refreshAssetsAfterOcrVarChange(ctx.task, ctx.scope);
  } catch (e) {
    if (assetTextError) assetTextError.textContent = '保存异常：' + e;
  } finally {
    if (assetTextSave) assetTextSave.disabled = false;
  }
}

// ---------- U-T4/S5：操作区递归对话框（op_region 链，实装） ----------
let opRegionDialogCtx = null;

function opRegionDialogAssets() {
  return assetDialogLibrary();
}

function opRegionDialogScopeForAsset(assetId, fallbackScope) {
  const id = String(assetId === undefined || assetId === null ? '' : assetId);
  const inGlobal = (globalAssetsCache || []).some((a) => a && String(a.id) === id);
  const inTask = (taskAssets || []).some((a) => a && String(a.id) === id);
  if (inGlobal && !inTask) return GLOBAL_SCOPE_TASK;
  if (inTask && !inGlobal) return fallbackScope;
  return fallbackScope || assetScopeTask(false);
}

function showOpRegionDialog() {
  if (!opRegionDialog) return;
  if (typeof opRegionDialog.showModal === 'function') {
    if (!opRegionDialog.open) opRegionDialog.showModal();
  } else {
    opRegionDialog.setAttribute('open', '');
  }
}

function hideOpRegionDialog() {
  if (!opRegionDialog) return;
  try {
    if (typeof opRegionDialog.close === 'function' && opRegionDialog.open) opRegionDialog.close();
  } catch (e) { /* cosmetic */ }
  opRegionDialog.removeAttribute('open');
}

function closeOpRegionDialog() {
  opRegionDialogCtx = null;
  hideOpRegionDialog();
}

function reopenOpRegionDialog() {
  if (!opRegionDialogCtx) return;
  renderOpRegionDialog();
  showOpRegionDialog();
}

function openOpRegionDialog(asset, scopeTask) {
  if (!asset || !opRegionDialog) return;
  if (String(asset.kind) === 'region') {
    setStatus('region 素材本身即操作区域，无需再设置操作区');
    return;
  }
  const scopeInfo = assetDialogScopeInfo(scopeTask);
  // seq127 模型 B 语义：两态；旧 offset 数据按 self 打开并提示将被替换。
  const legacyOp = opRegionNodeOf(asset.op_region);
  const initialMode = legacyOp.mode === 'asset'
    ? 'asset'
    : (legacyOp.mode === 'fixed' ? 'fixed' : 'self');
  opRegionDialogCtx = {
    asset: asset,
    scope: scopeInfo.scope,
    task: scopeInfo.task,
    mode: initialMode,
    refAsset: legacyOp.mode === 'asset' ? String(legacyOp.asset || '') : '',
    pick: legacyOp.mode === 'asset' && legacyOp.pick === 'nearest' ? 'nearest' : 'score',
    error: legacyOp.mode === 'offset'
      ? '旧「相对偏移」操作区已不再支持：保存后将改为当前素材操作区（self）。'
      : '',
  };
  renderOpRegionDialog();
  showOpRegionDialog();
}

function opRegionKindLabel(kind) {
  if (window.OASAssetUI && typeof window.OASAssetUI.kindLabel === 'function') {
    return window.OASAssetUI.kindLabel(kind);
  }
  return String(kind === undefined || kind === null ? '' : kind);
}

function opRegionLibAsset(lib, id) {
  const key = String(id === undefined || id === null ? '' : id);
  return (Array.isArray(lib) ? lib : []).find((a) => a && String(a.id) === key) || null;
}

// B3a 模型渲染：两选一；引用素材只读展示其现有链，不编辑被引用素材。
function buildOpRegionModelA(ctx, lib) {
  const box = document.createElement('div');
  box.className = 'op-region-level';
  const head = document.createElement('div');
  head.className = 'op-region-level-head';
  const title = document.createElement('span');
  title.className = 'op-region-level-title';
  title.textContent = '当前素材：' + String(ctx.asset.id) + '（动作落点=' +
    opRegionLandingText(ctx.mode) + '）';
  head.appendChild(title);
  box.appendChild(head);

  const modeRow = document.createElement('div');
  modeRow.className = 'op-region-mode-row';
  for (const choice of opRegionLevelChoices()) {
    const label = document.createElement('label');
    const radio = document.createElement('input');
    radio.type = 'radio';
    radio.name = 'op-region-mode-root';
    radio.value = choice.value;
    radio.checked = ctx.mode === choice.value;
    radio.addEventListener('change', () => {
      ctx.mode = choice.value;
      ctx.error = '';
      renderOpRegionDialog();
    });
    label.appendChild(radio);
    label.appendChild(document.createTextNode(' ' + choice.label));
    modeRow.appendChild(label);
  }
  box.appendChild(modeRow);

  const body = document.createElement('div');
  body.className = 'op-region-level-body';
  if (ctx.mode !== 'asset') {
    const isFixed = ctx.mode === 'fixed';
    body.textContent = isFixed
      ? '固定操作区：使用当前素材 action_shape 的绝对坐标，不随识别结果平移（仅根/链终端支持 fixed）。'
      : '使用当前素材自身的操作区域（action_shape；否则用真实识别区域）。';
    // #10（批次B3b）：当前素材的操作区域直接用统一框选工具在画布上画（矩形/多边形/反选）。
    const editBtn = document.createElement('button');
    editBtn.type = 'button';
    editBtn.textContent = '在画布上编辑当前素材操作区…';
    editBtn.title = '统一框选：拖动=矩形快捷（保存为 4 点多边形），单击=多边形顶点，双击收尾；反选仅操作区域可用';
    editBtn.addEventListener('click', () => {
      const asset = ctx.asset;
      const scope = ctx.scope;
      closeOpRegionDialog();
      editAssetActionRoi(asset, scope);
    });
    body.appendChild(editBtn);
    const actionHint = document.createElement('div');
    actionHint.className = 'muted small';
    actionHint.textContent = isFixed
      ? '画完后点画布底部「保存操作区域」写入 action_shape；保存的 op_region 为 {mode:"fixed"}（识别移动时操作区保持绝对位置）。'
      : '画完后点画布底部「保存操作区域」写入该素材的 action_shape（含反选开关）。';
    body.appendChild(actionHint);
  } else {
    const choices = opRegionRefChoices(lib, ctx.asset.id);
    const refAsset = opRegionLibAsset(lib, ctx.refAsset);
    const refChoice = choices.find((c) => c.id === ctx.refAsset) || null;
    if (!ctx.refAsset) {
      if (window.OASAssetUI && typeof window.OASAssetUI.createSearchPicker === 'function') {
        const picker = window.OASAssetUI.createSearchPicker({
          items: choices,
          value: '',
          placeholder: '搜索可识别素材作识别门槛（不含 region/自身）…',
          className: 'op-region-ref-picker',
          label: (item) => item.id + ' · ' + opRegionKindLabel(item.kind) +
            (item.disabled ? '（置灰：未设置期望文字）' : ''),
          onChoose: (item) => {
            if (item && item.disabled) {
              ctx.error = item.reason || '该素材未设置期望文字，请先在素材行设置';
              renderOpRegionDialog();
              return;
            }
            ctx.refAsset = item.id;
            ctx.error = '';
            renderOpRegionDialog();
          },
        });
        body.appendChild(picker);
      }
      const hint = document.createElement('div');
      hint.className = 'muted small';
      hint.textContent = '引用素材只作识别门槛（逐级识别，不修改被引用素材）；' +
        '落点=链末端（最后一个被引用素材）自己的操作区。';
      body.appendChild(hint);
    } else if (!refAsset || !refChoice) {
      body.textContent = '引用素材「' + ctx.refAsset + '」不在当前素材库（或不可作为门槛）。';
    } else {
      const line = document.createElement('div');
      line.textContent = '识别门槛：引用素材「' + refAsset.id + '」 · ' +
        opRegionKindLabel(refAsset.kind);
      body.appendChild(line);
      if (refChoice.disabled) {
        const warn = document.createElement('div');
        warn.className = 'op-region-warn';
        warn.textContent = '⚠ ' + refChoice.reason;
        body.appendChild(warn);
      }
      const pickLabel = document.createElement('label');
      pickLabel.className = 'op-region-ref-pick';
      pickLabel.textContent = '多候选取点：';
      const pickSelect = document.createElement('select');
      for (const option of opRegionPickOptions()) {
        const opt = document.createElement('option');
        opt.value = option.value;
        opt.textContent = option.label;
        pickSelect.appendChild(opt);
      }
      pickSelect.value = ctx.pick === 'nearest' ? 'nearest' : 'score';
      pickSelect.addEventListener('change', () => {
        ctx.pick = pickSelect.value === 'nearest' ? 'nearest' : 'score';
        ctx.error = '';
        renderOpRegionDialog();
      });
      pickLabel.appendChild(pickSelect);
      body.appendChild(pickLabel);
      const landing = document.createElement('div');
      landing.className = 'op-region-terminal';
      landing.textContent = '动作落点：' + opRegionLandingText('asset') +
        '（被引用素材只读；保存只写当前素材的操作区）。';
      body.appendChild(landing);
      const readonly = document.createElement('div');
      readonly.className = 'op-region-terminal';
      readonly.textContent = '被引用素材现有操作区（只读，不在本对话框修改）：' +
        opRegionReadonlyChainText(refAsset, lib);
      body.appendChild(readonly);
      const warn = document.createElement('div');
      warn.className = 'op-region-warn';
      warn.textContent = '引用素材只作识别门槛：落点=链末端（最后一个被引用素材）自己的操作区；' +
        '保存只写当前素材的操作区，不改被引用素材。';
      body.appendChild(warn);
      const reselect = document.createElement('button');
      reselect.type = 'button';
      reselect.textContent = '重新选择素材';
      reselect.addEventListener('click', () => {
        ctx.refAsset = '';
        ctx.error = '';
        renderOpRegionDialog();
      });
      body.appendChild(reselect);
    }
  }
  box.appendChild(body);
  return box;
}

function renderOpRegionDialog() {
  const ctx = opRegionDialogCtx;
  if (!ctx || !opRegionLevels) return;
  const lib = opRegionDialogAssets();
  if (opRegionTitle) opRegionTitle.textContent = '操作区 · ' + String(ctx.asset.id);
  if (opRegionBreadcrumb) {
    opRegionBreadcrumb.textContent = ctx.mode === 'fixed'
      ? '固定操作区：绝对坐标，不随识别结果平移（仅根/链终端支持 fixed）。'
      : (ctx.mode === 'asset'
        ? '引用素材只作识别门槛；动作落点=链末端（最后一个被引用素材）自己的操作区（被引用素材只读）。'
        : '动作落点=当前素材操作区（self）。');
  }
  if (opRegionError) opRegionError.textContent = ctx.error || '';
  opRegionLevels.textContent = '';
  opRegionLevels.appendChild(buildOpRegionModelA(ctx, lib));
  if (opRegionSave) opRegionSave.disabled = false;
}

async function refreshAssetsAfterOpRegionSave(task, scope) {
  try { await refreshTaskAssetsForScope(scope); } catch (e) { /* best-effort */ }
  try { await refreshGlobalAssets(); } catch (e) { /* best-effort */ }
  void task;
}

async function saveOpRegionDialog() {
  const ctx = opRegionDialogCtx;
  if (!ctx || !opRegionSave) return;
  const lib = opRegionDialogAssets();
  // seq127 模型 B：保存只写当前素材；引用素材只作识别门槛（不 PATCH 被引用素材）。
  // seq136（UI B #1）：fixed 固定绝对坐标操作区（仅根/链终端；对话框只编辑根）。
  const op = ctx.mode === 'fixed'
    ? { mode: 'fixed' }
    : (ctx.mode === 'asset'
      ? { mode: 'asset', asset: String(ctx.refAsset || ''), pick: ctx.pick === 'nearest' ? 'nearest' : 'score' }
      : { mode: 'self' });
  if (op.mode === 'asset') {
    if (!op.asset) {
      ctx.error = '请选择引用素材（识别门槛）';
      renderOpRegionDialog();
      return;
    }
    const choice = opRegionRefChoices(lib, ctx.asset.id).find((c) => c.id === op.asset);
    if (!choice) {
      ctx.error = '引用素材「' + op.asset + '」不在当前素材库（或不可作为识别门槛）';
      renderOpRegionDialog();
      return;
    }
    if (choice.disabled) {
      ctx.error = choice.reason || '该素材未设置期望文字，请先在素材行设置';
      renderOpRegionDialog();
      return;
    }
  }
  ctx.error = '';
  opRegionSave.disabled = true;
  try {
    // 保存前仍调服务端权威校验（环/深度/悬空/末端），失败不写。
    const validated = await postJson('/task/asset/validate_op_region', {
      task: ctx.task,
      id: ctx.asset.id,
      op_region: op,
    });
    if (!validated || !validated.ok) {
      ctx.error = '操作区校验失败（服务端权威，未写入）：\n' + opRegionValidateErrorText(validated);
      renderOpRegionDialog();
      return;
    }
    const saved = await postJson('/task/asset/update', {
      task: ctx.scope,
      id: ctx.asset.id,
      op_region: op,
    });
    if (!saved || !saved.ok) {
      ctx.error = '保存操作区失败：' + ((saved && saved.errors) || ['未知错误']).join('；');
      renderOpRegionDialog();
      await refreshAssetsAfterOpRegionSave(ctx.task, ctx.scope);
      return;
    }
    closeOpRegionDialog();
    setStatus('操作区已保存（只写当前素材）', '#7bd88f');
    resultEl.textContent =
      '操作区已保存（落点=' + opRegionLandingText(op.mode) + '）：\n' +
      '· ' + ctx.asset.id + '：' + JSON.stringify(op);
    log('操作区保存（模型 B） asset=' + ctx.asset.id + ' op_region=' + JSON.stringify(op));
    await refreshAssetsAfterOpRegionSave(ctx.task, ctx.scope);
  } catch (e) {
    ctx.error = '保存异常：' + e;
    renderOpRegionDialog();
  } finally {
    if (opRegionSave) opRegionSave.disabled = false;
  }
}

function collectionAssetRows(collections) {
  return (collections || []).map((c) => ({
    id: c.id,
    kind: 'template_set',
    members: c.members || [],
    threshold: c.threshold,
    action_shape: c.action_shape,
    category: c.category || '素材集合',
    _collection: true,
  }));
}

function renderTaskAssetsHint() {
  if (!taskAssetsHint) return;
  const m = taskAssetsMeta || {};
  if (!m.task) {
    taskAssetsHint.textContent = '未选择任务；任务素材库为空（全部素材见「素材」页的全局素材库）。';
    taskAssetsHint.style.color = '';
    return;
  }
  if (m.scope === 'global') {
    if (m.unreferenced > 0) {
      taskAssetsHint.textContent =
        `未引用提示：全局库共 ${m.globalTotal} 个素材，其中 ${m.unreferenced} 个未被当前任务 flow 引用` +
        (m.showAll ? '（当前显示全部全局素材）' : '（勾选「显示全部全局素材」可查看/管理）');
      taskAssetsHint.style.color = '#ffb74d';
    } else {
      taskAssetsHint.textContent = `任务素材库 = 全局库中当前 flow 引用的子集（${m.count} 个；无未引用素材）`;
      taskAssetsHint.style.color = '';
    }
    return;
  }
  taskAssetsHint.textContent = `任务素材库：${m.count} 个素材（任务模式）`;
  taskAssetsHint.style.color = '';
}

async function refreshTaskAssets(task) {
  const explicit = task != null && String(task).trim();
  const selected = explicit || ((taskSelect && taskSelect.value) || '').trim();
  if (!selected) {
    taskAssets = [];
    taskAssetsMeta = { task: '', design: null, count: 0, scope: assetsGlobalMode ? 'global' : 'task' };
    if (assetsList) {
      assetsList.textContent = '（未选择任务；任务素材库为空）';
    }
    renderTaskAssetsHint();
    renderAssetPositionInheritPicker();
    return;
  }
  task = selected;
  try {
    const r = await fetch('/task/assets?task=' + encodeURIComponent(task));
    const s = await r.json();
    if (!s.ok) {
      if (assetsList) assetsList.textContent = '读取失败：' + (s.errors || []).join('; ');
      if (taskAssetsHint) {
        taskAssetsHint.textContent = '任务素材库读取失败：' + (s.errors || []).join('; ');
        taskAssetsHint.style.color = '#ff8a80';
      }
      return;
    }
    // Unified global mode: the task panel shows the subset referenced by this
    // task's flow, not every global asset. Keep the full list so the explicit
    // "show all" view and the unreferenced counter are not silently lost.
    assetsGlobalMode = s.task === GLOBAL_SCOPE_TASK;
    const allItems = (s.items || []).concat(collectionAssetRows(s.collections));
    if (assetsGlobalMode) {
      let referenced = new Set();
      try {
        const fr = await fetch('/task/flow?task=' + encodeURIComponent(task));
        const fs = await fr.json();
        referenced = fs.ok && fs.flow ? collectFlowAssetIds(fs.flow) : new Set();
      } catch (e) {
        referenced = new Set();
      }
      const referencedItems = allItems.filter((a) => referenced.has(a.id));
      const unreferenced = allItems.length - referencedItems.length;
      taskAssets = taskShowAllAssets ? allItems : referencedItems;
      taskAssetsMeta = {
        task,
        design: s.design_resolution,
        count: referencedItems.length,
        visible: taskAssets.length,
        scope: 'global',
        globalTotal: allItems.length,
        unreferenced,
        showAll: taskShowAllAssets,
      };
    } else {
      taskAssets = allItems;
      taskAssetsMeta = { task, design: s.design_resolution, count: taskAssets.length, scope: 'task' };
    }
    refreshAssetCategoryFilter();
    renderAssetList();
    renderTaskAssetsHint();
    renderAssetPositionInheritPicker();
    // Frame rows show how many current-task assets reference each frame; keep
    // that derived relation in sync after asset CRUD/load.
    if (typeof refreshFrames === 'function') {
      try { await refreshFrames(); } catch (e) { /* frame panel may be hidden */ }
    }
  } catch (e) {
    if (assetsList) assetsList.textContent = '读取异常：' + e;
    if (taskAssetsHint) {
      taskAssetsHint.textContent = '任务素材库读取异常：' + e;
      taskAssetsHint.style.color = '#ff8a80';
    }
  }
}

// --- seq130（#4）：工作台任务页素材自动刷新 -----------------------------------
// 可见/聚焦/每 8s 自动刷新任务素材；隐藏标签页暂停；保持滚动位置与选中的素材，
// 失败静默（日志）；in-flight 保护避免请求风暴。未保存的编辑状态（画布 ROI/
// action_shape/表单）不由 refreshTaskAssets 改写，这里只重绘列表。
const ASSETS_REFRESH_INTERVAL_MS = 8000;
let assetsRefreshTimer = null;
let assetsRefreshInFlight = false;
let lastAssetsRefreshAt = 0;
let lastAssetsFocusRefreshAt = 0;

function assetsRefreshDue(now, lastAt, intervalMs, inFlight) {
  if (inFlight) return false;
  const n = Number(now) || 0;
  const l = Number(lastAt) || 0;
  const i = Number(intervalMs) > 0 ? Number(intervalMs) : 0;
  return l <= 0 || n - l >= i;
}

function activeWorkbenchPage() {
  const page = document.querySelector('.app-page.active');
  return page ? String(page.id) : '';
}

function captureAssetsRefreshState() {
  const scroll = {};
  const nodes = { assetsList: assetsList, globalAssetsList: globalAssetsList, framesList: framesList, frameAssetsList: frameAssetsList };
  for (const key of Object.keys(nodes)) {
    const el = nodes[key];
    if (el && Number.isFinite(el.scrollTop)) scroll[key] = el.scrollTop;
  }
  let selectedAsset = '';
  if (window.OASAssetUI && typeof window.OASAssetUI.getSelectedAsset === 'function') {
    selectedAsset = String(window.OASAssetUI.getSelectedAsset() || '');
  }
  return { scroll: scroll, selectedAsset: selectedAsset };
}

function restoreAssetsRefreshState(state) {
  const s = state && typeof state === 'object' ? state : {};
  const scroll = s.scroll && typeof s.scroll === 'object' ? s.scroll : {};
  const nodes = { assetsList: assetsList, globalAssetsList: globalAssetsList, framesList: framesList, frameAssetsList: frameAssetsList };
  for (const key of Object.keys(nodes)) {
    const el = nodes[key];
    const top = Number(scroll[key]);
    if (el && Number.isFinite(top)) el.scrollTop = top;
  }
  if (s.selectedAsset && window.OASAssetUI && typeof window.OASAssetUI.selectAsset === 'function') {
    try { window.OASAssetUI.selectAsset(s.selectedAsset); } catch (e) { /* cosmetic */ }
  }
}

async function autoRefreshTaskAssets(reason) {
  if (document.hidden) return false;
  const page = activeWorkbenchPage();
  if (page !== 'page-workbench' && page !== 'page-builder') return false;
  if (!taskName || !String(taskName.value || '').trim()) return false;
  if (!assetsRefreshDue(Date.now(), lastAssetsRefreshAt, ASSETS_REFRESH_INTERVAL_MS, assetsRefreshInFlight)) {
    return false;
  }
  assetsRefreshInFlight = true;
  const state = captureAssetsRefreshState();
  try {
    await refreshTaskAssets();
    lastAssetsRefreshAt = Date.now();
    return true;
  } catch (e) {
    log('素材自动刷新失败（' + reason + '）：' + (e && e.message ? e.message : e));
    return false;
  } finally {
    restoreAssetsRefreshState(state);
    assetsRefreshInFlight = false;
  }
}

function stopAssetsAutoRefresh() {
  if (assetsRefreshTimer) {
    clearInterval(assetsRefreshTimer);
    assetsRefreshTimer = null;
  }
}

function startAssetsAutoRefresh() {
  stopAssetsAutoRefresh();
  if (typeof setInterval !== 'function') return;
  assetsRefreshTimer = setInterval(function () {
    autoRefreshTaskAssets('interval');
  }, ASSETS_REFRESH_INTERVAL_MS);
}

document.addEventListener('visibilitychange', function () {
  if (document.hidden) {
    stopAssetsAutoRefresh();
    log('素材自动刷新：标签页隐藏，暂停');
  } else {
    startAssetsAutoRefresh();
    autoRefreshTaskAssets('visibility');
  }
});

window.addEventListener('focus', function () {
  const now = Date.now();
  if (now - lastAssetsFocusRefreshAt < 3000) return;
  lastAssetsFocusRefreshAt = now;
  autoRefreshTaskAssets('focus');
});

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

// Shared filter used by BOTH asset libraries (task panel + global panel). One
// implementation so the two search/category toolbars cannot drift apart.
function assetMatchesFilter(it, query, category) {
  const q = String(query || '').trim().toLowerCase();
  const cat = String(category || '');
  const id = String((it && it.id) || '').toLowerCase();
  return (!q || id.includes(q)) && (!cat || assetCategory(it) === cat);
}

function filterAssets(items, query, category) {
  const list = Array.isArray(items) ? items : [];
  return list.filter((it) => assetMatchesFilter(it, query, category));
}

function assetCategoryNames(items) {
  return [...new Set((items || []).map(assetCategory))].sort((a, b) => {
    if (a === '未分类') return 1;
    if (b === '未分类') return -1;
    return a.localeCompare(b, 'zh-CN');
  });
}

function fillAssetCategoryFilter(select, items) {
  if (!select) return;
  const keep = select.value;
  const cats = assetCategoryNames(items);
  select.textContent = '';
  const all = document.createElement('option');
  all.value = '';
  all.textContent = '全部分类';
  select.appendChild(all);
  for (const cat of cats) {
    const o = document.createElement('option');
    o.value = cat;
    o.textContent = cat;
    select.appendChild(o);
  }
  // Keep the current selection across refreshes when it still exists.
  select.value = cats.includes(keep) ? keep : '';
}

// One row builder for every asset library (task, global, and the frame detail
// list). The row DOM itself still comes from the shared OASAssetUI.createRow;
// this app-level entry only adds a minimal fallback so all three call sites
// cannot drift apart. No HTML strings, no per-surface badge: requirement 5 makes
// the frame-detail rows visually identical to the global library.
function assetRowElement(it, opts) {
  const o = opts || {};
  let row = null;
  if (window.OASAssetUI && typeof window.OASAssetUI.createRow === 'function') {
    row = window.OASAssetUI.createRow(it, o);
  } else {
    row = document.createElement('div');
    row.className = 'asset-row';
    row.dataset.assetId = it.id;
    const main = document.createElement('button');
    main.type = 'button';
    main.className = 'asset-main';
    main.textContent = `${it.id} · ${it.kind || '?'}`;
    row.appendChild(main);
  }
  return row;
}

// Shared grouping/empty-state wrapper. renderGlobalAssets and renderAssetList
// both go through this; the frame detail list passes per-row row metadata, so it
// calls assetRowElement directly (same row DOM, different context).
function renderAssetRows(container, items, opts) {
  if (!container) return;
  const o = opts || {};
  if (o.clear !== false) container.textContent = '';
  const list = Array.isArray(items) ? items : [];
  if (!list.length) {
    const empty = document.createElement('div');
    empty.className = 'muted small';
    empty.textContent = o.emptyText || '（没有素材）';
    container.appendChild(empty);
    return;
  }
  if (o.group) {
    const groupBy = o.groupBy || 'kind';
    const groups = new Map();
    for (const it of list) {
      const key = groupBy === 'category' ? assetCategory(it) : (it.kind || '');
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(it);
    }
    for (const [key, groupItems] of groups) {
      const h = document.createElement('h4');
      h.className = 'asset-group';
      h.textContent = groupBy === 'category'
        ? `${key}（${groupItems.length}）`
        : `${ASSET_KIND_LABELS[key] || key}（${groupItems.length}）`;
      container.appendChild(h);
      for (const it of groupItems) container.appendChild(assetRowElement(it, o));
    }
    return;
  }
  for (const it of list) container.appendChild(assetRowElement(it, o));
}

function refreshAssetCategoryFilter() {
  fillAssetCategoryFilter(assetCategoryFilter, taskAssets);
}

function renderAssetList() {
  if (!assetsList) return;
  const q = ((assetSearch && assetSearch.value) || '').trim();
  const cat = assetCategoryFilter ? assetCategoryFilter.value : '';
  const items = filterAssets(taskAssets, q, cat);
  assetsList.textContent = '';
  if (taskAssetsMeta.task) {
    const res = taskAssetsMeta.design ? taskAssetsMeta.design.join('x') : '未设置';
    const scopeText = taskAssetsMeta.scope === 'global'
      ? ` · flow 引用 ${taskAssetsMeta.count}/${taskAssetsMeta.globalTotal}` +
        (taskAssetsMeta.showAll ? ' · 已显示全部' : '')
      : '';
    const head = document.createElement('div');
    head.className = 'muted small';
    head.textContent = `任务素材库 ${taskAssetsMeta.task} · design ${res} · ${taskAssets.length} 个素材` +
      scopeText +
      (cat ? ` · 分类：${cat}` : '');
    assetsList.appendChild(head);
  }
  if (!items.length) {
    const empty = document.createElement('div');
    empty.className = 'muted small';
    if (q || cat) {
      empty.textContent = '（没有匹配的素材）';
    } else if (taskAssetsMeta.scope === 'global' && taskAssetsMeta.unreferenced > 0) {
      empty.textContent = taskShowAllAssets
        ? '（没有可显示的全局素材）'
        : `（当前任务 flow 未引用素材；全局库有 ${taskAssetsMeta.unreferenced} 个未引用素材，勾选「显示全部全局素材」查看）`;
    } else {
      empty.textContent = '（任务还没有素材）';
    }
    assetsList.appendChild(empty);
    return;
  }
  renderAssetRows(assetsList, items, {
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
    onEditRoi: (it) => editAssetRecognitionRoi(it),
    onRename: (it) => renameAsset(it),
    onThreshold: (it) => editAssetThreshold(it),
    onPosition: (it) => editAssetPositions(it),
    onDelete: (it) => deleteAsset(it),
    onOpRegion: (it) => openOpRegionDialog(it, ''),
    onCollectionAdd: (it) => openCollectionAddDialog(it),
    onLongPress: (it) => runTestActionForAsset(it, 'long_press'),
    onClickTest: (it) => testAssetClick(it),
    onDragTest: (it) => testAssetDrag(it),
    onStep: (it) => generateAssetStep(it),
    onEditText: (it) => editAssetText(it),
    onCollectionTest: (it) => testAssetCandidates(it),
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
  // P1-1: a reorder is a write to the shared asset store too; refresh the global
  // cache so the global library keeps the new order and the frame detail list
  // (global rows come first) is repainted from the same order.
  await refreshGlobalAssets();
}

async function deleteCollection(it, scopeTask) {
  cancelRecognitionRoiEdit('删除素材集合');
  const task = assetRowScopeTask(scopeTask, true);
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
  // Refresh the task panel with the *selected task*, not the global scope label
  // used for the API call (global mode ignores `task` but flow lookup would fail).
  await refreshTaskAssetsForScope(task);
  await refreshGlobalAssets();
}

async function batchTestAssets() {
  // P1-3: the button lives next to 刷新全局库 in the 素材 panel. The test target
  // is therefore the visible list of the CURRENT surface, not the old
  // "current task's flow-referenced subset":
  //   - unified/global store -> the filtered global library (GLOBAL_SCOPE_TASK);
  //   - task bundle           -> the visible task rows (show-all already applied).
  const globalMode = assetsGlobalMode;
  let task = '';
  if (globalMode) {
    task = GLOBAL_SCOPE_TASK;
  } else {
    task = assetScopeTask(true);
    if (!task) return;
  }
  const query = globalMode
    ? ((globalAssetSearch && globalAssetSearch.value) || '')
    : ((assetSearch && assetSearch.value) || '');
  const category = globalMode
    ? ((globalAssetCategoryFilter && globalAssetCategoryFilter.value) || '')
    : ((assetCategoryFilter && assetCategoryFilter.value) || '');
  const selection = batchTestAssetSelection({
    globalMode,
    globalItems: globalAssetsCache,
    taskItems: taskAssets,
    query,
    category,
    task: (taskName.value || '').trim(),
  });
  if (!selection.ids.length) {
    resultEl.textContent = batchTestEmptyText(globalMode);
    return;
  }
  const scopeText = globalMode ? '全局素材库' : '任务素材库';
  const body = { task, assets: selection.ids, live: !!livePreview };
  resultEl.textContent = (livePreview ? '批量实时帧测试中…' : '批量位置帧测试中…') +
    `（对象：${scopeText}可见 ${selection.ids.length} 个）`;
  try {
    const r = await postJson('/task/assets/batch_test', body);
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
      `批量帧测试（对象：${scopeText}可见 ${selection.ids.length} 个）：${r.passed}/${r.total} 通过\n` +
      lines.join('\n');
    log(`批量帧测试：${r.passed}/${r.total}（${livePreview ? '实时帧' : '位置帧'}；` +
      `${scopeText} ${selection.ids.length} 个）`);
  } catch (e) {
    resultEl.textContent = '批量帧测试异常：' + e;
  }
}

async function deleteAsset(it, scopeTask) {
  cancelRecognitionRoiEdit('删除素材');
  const task = assetRowScopeTask(scopeTask, true);
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
  setStatus(`已删除素材「${it.id}」；两个素材库已刷新`);
  await refreshTaskAssetsForScope(task);
  await refreshGlobalAssets();
}

async function editAssetPositions(it, scopeTask) {
  cancelRecognitionRoiEdit('修改位置帧');
  const task = assetRowScopeTask(scopeTask, true);
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
  const inheritItems = (assetsGlobalMode ? globalAssetsCache : (taskAssets || []))
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
    await refreshTaskAssetsForScope(task);
    await refreshGlobalAssets();
    await refreshAssetPositionFrames();
  });
  actions.append(cancel, save);
  box.append(title, hint, pickerHost, inheritHost, list, msg, actions);
  overlay.appendChild(box);
  document.body.appendChild(overlay);
  renderChosen();
  picker.input.focus();
}

// The 设置 menu edits one field at a time. These parsers stay pure so the
// validation is testable in Node (ui_pure_test.js); null means invalid and the
// caller decides whether to warn. The old prompt-based ROI text parser is gone:
// 识别区 is drawn on the canvas and validated by assetRoiBoxValid below, which
// owns the single set of bounds (x,y >= 0, w,h > 0, <= 100000).
function parseAssetThresholdText(text) {
  const raw = String(text == null ? '' : text).trim();
  if (!raw) return null;
  const n = Number(raw);
  if (!Number.isFinite(n) || n < 0 || n > 1) return null;
  return n;
}

// Shared bounds for every ROI the UI writes (drawn canvas box). One validator so
// the save path cannot bypass the w/h > 0 and 100000 sanity limits.
function assetRoiBoxValid(values) {
  if (!Array.isArray(values) || values.length !== 4) return null;
  const nums = values.map(Number);
  if (!nums.every((n) => Number.isInteger(n))) return null;
  const [x, y, w, h] = nums;
  if (x < 0 || y < 0 || w <= 0 || h <= 0) return null;
  // A ROI beyond this is certainly a typo (the server accepts i32, but no frame
  // or design resolution comes close); reject early instead of failing at OCR.
  if (nums.some((n) => n > 100000)) return null;
  return nums;
}

// P2-9: a canvas box that extends past the frame would crop/OCR out of bounds.
// The server accepts any positive w/h, so the UI rejects it early. Unknown frame
// dimensions (0/NaN) skip the check instead of inventing a limit.
function assetRoiWithinFrame(box, frameWidth, frameHeight) {
  const valid = assetRoiBoxValid(box);
  if (!valid) {
    return { ok: false, text: `识别区无效：[${(box || []).join(',')}] 需要 x,y >= 0 且 w,h > 0` };
  }
  const fw = Number(frameWidth);
  const fh = Number(frameHeight);
  if (!Number.isFinite(fw) || !Number.isFinite(fh) || fw <= 0 || fh <= 0) {
    return { ok: true, text: '' };
  }
  const [x, y, w, h] = valid;
  if (x + w > fw || y + h > fh) {
    return {
      ok: false,
      text: `识别区 [${valid.join(',')}] 超出当前帧 ${fw}x${fh}；请重新画框`,
    };
  }
  return { ok: true, text: '' };
}

// P1-1: a recognition rectangle as a 4-point polygon. The server derives
// roi = polygon bbox on save, so [x,y,w,h] round-trips exactly (inclusive
// convention: max = origin + size - 1).
function assetRoiRectanglePolygon(roi) {
  const [x, y, w, h] = roi;
  return [[x, y], [x + w - 1, y], [x + w - 1, y + h - 1], [x, y + h - 1]];
}

// P1-1 (leftover 2): a stored recognition polygon is a SHAPE, not a rectangle.
// `/task/save_asset` derives roi from the polygon bbox (oas-task save_asset), so
// when the canvas ROI is MOVED the polygon must be moved with it (same vertex
// count, same shape) instead of being replaced by the new ROI rectangle. A pure
// translation keeps the bbox size, so the moved bbox is exactly the new ROI.
// If the ROI was resized (or the stored polygon/roi is inconsistent) the
// translated bbox cannot be the new ROI, and sending it would silently move the
// ROI back to the old size; only then fall back to the rectangle, with an
// explicit reason the save path shows to the user.
function assetRoiPolygonPlan(polygon, roi, oldRoi) {
  if (!Array.isArray(roi) || roi.length !== 4) {
    return { polygon: null, fallback: false, reason: '' };
  }
  const rect = assetRoiRectanglePolygon(roi);
  if (!Array.isArray(polygon) || polygon.length < 3) {
    return { polygon: null, fallback: false, reason: '' };
  }
  const bad = '识别区多边形数据无效，已改用矩形识别区';
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const p of polygon) {
    if (!Array.isArray(p) || p.length !== 2) return { polygon: rect, fallback: true, reason: bad };
    const px = Number(p[0]);
    const py = Number(p[1]);
    if (!Number.isInteger(px) || !Number.isInteger(py)) {
      return { polygon: rect, fallback: true, reason: bad };
    }
    minX = Math.min(minX, px);
    minY = Math.min(minY, py);
    maxX = Math.max(maxX, px);
    maxY = Math.max(maxY, py);
  }
  const old = Array.isArray(oldRoi) && oldRoi.length === 4 &&
    oldRoi.every((n) => Number.isInteger(Number(n)))
    ? oldRoi.map(Number)
    : null;
  if (!old) {
    // No stored ROI to translate from: keep the historical reuse rule (only a
    // polygon that already bounds the new ROI may be sent unchanged).
    const sameBox = minX === roi[0] && minY === roi[1] &&
      maxX - minX + 1 === roi[2] && maxY - minY + 1 === roi[3];
    return sameBox
      ? { polygon: polygon.map((p) => [Number(p[0]), Number(p[1])]), fallback: false, reason: '' }
      : { polygon: rect, fallback: true, reason: '原素材没有保存的识别区，无法平移多边形，已改用矩形识别区' };
  }
  const dx = Number(roi[0]) - old[0];
  const dy = Number(roi[1]) - old[1];
  const moved = polygon.map((p) => [Number(p[0]) + dx, Number(p[1]) + dy]);
  let mMinX = Infinity;
  let mMinY = Infinity;
  let mMaxX = -Infinity;
  let mMaxY = -Infinity;
  for (const p of moved) {
    mMinX = Math.min(mMinX, p[0]);
    mMinY = Math.min(mMinY, p[1]);
    mMaxX = Math.max(mMaxX, p[0]);
    mMaxY = Math.max(mMaxY, p[1]);
  }
  // The server derives roi from this bbox: it must be exactly the new ROI and
  // stay inside the frame (negative points are rejected by oas-task), otherwise
  // the translation cannot represent the edit faithfully.
  const fits = mMinX === roi[0] && mMinY === roi[1] &&
    mMaxX - mMinX + 1 === roi[2] && mMaxY - mMinY + 1 === roi[3] &&
    mMinX >= 0 && mMinY >= 0;
  if (!fits) {
    return {
      polygon: rect,
      fallback: true,
      reason: '识别区尺寸变了，多边形无法只平移匹配新识别区（越界或宽高不一致），已改用矩形识别区',
    };
  }
  return { polygon: moved, fallback: false, reason: '' };
}

// Backwards-compatible thin wrapper: the polygon to send (or null when the
// asset has no polygon). Kept because the payload builder and the pure tests
// only need the geometry; the save path uses the full plan for the user notice.
function assetRoiPolygonForSave(polygon, roi, oldRoi) {
  return assetRoiPolygonPlan(polygon, roi, oldRoi).polygon;
}

// P1-1/P1-2: `/task/save_asset` replaces the WHOLE AssetDecl, so the payload
// must be built from the LATEST stored asset (not the snapshot captured when the
// ROI editor opened) and must carry every persisted field; only roi changes.
// Template saves only need the template-relevant fields plus the optional OCR
// provenance fields, which are forwarded when present so a replace never drops
// stored data silently.
function assetRoiReplacePayload(task, asset, roi, explicitPolygon) {
  const it = asset || {};
  const payload = {
    task,
    replace: true,
    id: it.id,
    kind: it.kind || 'template',
    roi: roi.slice(),
    positions: Array.isArray(it.positions) ? it.positions.slice() : [],
  };
  if (it.threshold != null && Number.isFinite(Number(it.threshold))) {
    payload.threshold = Number(it.threshold);
  }
  // The asset's own operation region (operation shape) travels with the replace;
  // absent = the runtime uses the real recognized box. No two-state switch.
  if (it.action_shape) payload.action_shape = it.action_shape;
  // #9：op_region 链也是持久字段，replace 必须原样带回（类型节点/链根会用它）。
  if (it.op_region !== undefined && it.op_region !== null) payload.op_region = it.op_region;
  // #10（批次B3b）：调用方画出的识别多边形优先（显式 null = 清空掩膜，数组 =
  // 当前帧坐标）；未传时才回退到旧的“按 ROI 平移已存多边形”逻辑。
  if (explicitPolygon !== undefined) {
    payload.polygon = explicitPolygon === null
      ? null
      : explicitPolygon.map((p) => [Number(p[0]), Number(p[1])]);
  } else {
    const polygon = assetRoiPolygonForSave(it.polygon, roi, it.roi);
    if (polygon) payload.polygon = polygon;
  }
  if (it.ocr_mode != null) payload.ocr_mode = it.ocr_mode;
  if (Array.isArray(it.ocr_source_roi) && it.ocr_source_roi.length === 4) {
    payload.ocr_source_roi = it.ocr_source_roi.slice();
  }
  if (it.ocr_text != null) payload.ocr_text = it.ocr_text;
  return payload;
}

// P0-2/P1-2: resolve the latest copy of the asset being edited from the cache
// that owns the row's API scope. `scope === GLOBAL_SCOPE_TASK` means the unified
// global store; any other non-empty value is a task name (task bundle cache).
// The other cache is consulted only while the scoped cache is empty (not loaded
// yet) — never as a second guess, because an id collision across stores must
// cancel the save instead of writing the wrong asset.
// Never trust the editing snapshot: threshold/positions/roi may have changed
// while the canvas editor stayed open (the settings menu deliberately does not
// collapse).
function findAssetById(scope, id, lists) {
  const src = lists || {};
  const globalItems = Array.isArray(src.global) ? src.global : [];
  const taskItems = Array.isArray(src.task) ? src.task : [];
  const key = id == null ? '' : String(id);
  if (!key) return null;
  const find = (list) =>
    (list || []).find((a) => a && a.id != null && String(a.id) === key) || null;
  const globalScope = scope === GLOBAL_SCOPE_TASK;
  const primary = globalScope ? globalItems : taskItems;
  const secondary = globalScope ? taskItems : globalItems;
  return find(primary) || (primary.length ? null : find(secondary));
}

// P1-3: the batch test must cover exactly what the user sees next to the button.
// Global mode -> the filtered global library (the unified store); task mode ->
// the visible task list (refreshTaskAssets already applied the show-all switch).
// Collections have no frames, so they never enter the batch.
function batchTestAssetSelection(opts) {
  const o = opts || {};
  const globalMode = o.globalMode === true;
  const source = globalMode ? (o.globalItems || []) : (o.taskItems || []);
  const items = filterAssets(source, o.query || '', o.category || '');
  const ids = items
    .filter((a) => a && a.id != null && a.kind !== 'template_set')
    .map((a) => String(a.id));
  return {
    globalMode,
    scope: globalMode ? GLOBAL_SCOPE_TASK : String(o.task || ''),
    ids,
    visible: items.length,
  };
}

function batchTestEmptyText(globalMode) {
  return globalMode
    ? '全局素材库没有可批量测试的素材（素材集合不参与）；请检查搜索/分类筛选，或先刷新全局库'
    : '任务素材库没有可批量测试的素材；请检查搜索/分类筛选，或勾选「显示全部全局素材」后重试';
}

async function renameAsset(it, scopeTask) {
  cancelRecognitionRoiEdit('重命名素材');
  const task = assetRowScopeTask(scopeTask, true);
  if (!task) return;
  const input = window.prompt(`重命名素材「${it.id}」：输入新 id（留空 = 取消）`, it.id);
  const newId = (input || '').trim();
  if (!newId || newId === it.id) return;
  const r = await postJson('/task/asset/rename', { task, from: it.id, to: newId });
  if (!r.ok) {
    showErrors('重命名失败：', r.errors);
    return;
  }
  const text = `素材重命名：${it.id} → ${newId}`;
  resultEl.textContent = text;
  log(text);
  setStatus(`素材「${newId}」已重命名；两个素材库已刷新`);
  await refreshTaskAssetsForScope(task);
  await refreshGlobalAssets();
}

// 阈值 means two different things and the dialog must say which:
//   - template: NCC match pass line (default 0.85);
//   - ocr_line/ocr_lines: minimum OCR confidence (unset = no limit).
// region / template_set have no threshold at all.
function assetThresholdKind(it) {
  const kind = (it && it.kind) || '';
  if (kind === 'template') return 'template';
  if (kind === 'ocr_line' || kind === 'ocr_lines' || kind === 'ocr_var') return 'ocr';
  return '';
}

async function editAssetThreshold(it, scopeTask) {
  cancelRecognitionRoiEdit('修改阈值');
  const thresholdKind = assetThresholdKind(it);
  if (!thresholdKind) {
    resultEl.textContent = `素材「${it.id}」不支持阈值（仅 template / ocr_line / ocr_lines / ocr_var）`;
    return;
  }
  const task = assetRowScopeTask(scopeTask, true);
  if (!task) return;
  const isTemplate = thresholdKind === 'template';
  const hasValue = it.threshold != null && Number.isFinite(Number(it.threshold));
  // P2-4: templates have no "unset = unlimited" mode — oas-task saves them with
  // DEFAULT_THRESHOLD (0.85) when no threshold is stored, and the runtime reads
  // 0.85 for legacy assets. Always show/prefill that effective value.
  // OCR: the runtime normalizes an explicit 0 to "no filter" (OCR-THRESHOLD
  // §P1-4), so 0 is the way to clear the field; blank still means "cancel".
  const effective = hasValue ? Number(it.threshold) : (isTemplate ? 0.85 : null);
  const currentText = isTemplate
    ? (hasValue ? String(effective) : '未保存，按默认 0.85')
    : (hasValue && Number(effective) > 0 ? String(effective) : '未设或 0（不限制）');
  const input = window.prompt(
    isTemplate
      ? `模板阈值 0~1（当前 ${currentText}；留空 = 取消）`
      : `OCR 最低置信度 0~1（当前 ${currentText}；留空 = 取消；0 = 不限制）`,
    effective != null ? String(effective) : ''
  );
  if (input == null || String(input).trim() === '') return;
  const threshold = parseAssetThresholdText(input);
  if (threshold == null) {
    resultEl.textContent = `素材「${it.id}」阈值无效：必须是 0~1 之间的数字`;
    window.alert('阈值必须是 0~1 之间的数字');
    return;
  }
  const r = await postJson('/task/asset/update', { task, id: it.id, threshold });
  if (!r.ok) {
    showErrors('修改阈值失败：', r.errors);
    return;
  }
  const text = `素材「${it.id}」${isTemplate ? '模板阈值' : 'OCR 最低置信度'}：${threshold}`;
  resultEl.textContent = text;
  log(text);
  setStatus(`素材「${it.id}」阈值已保存；两个素材库已刷新`);
  await refreshTaskAssetsForScope(task);
  await refreshGlobalAssets();
}

// B: 识别区 is edited on the CANVAS, exactly like 操作区. The old prompt editor
// is gone: a ROI is a rectangle drawn on the real frame, not four typed numbers.
// Templates use an identical gesture but a different save path (re-crop
// overwrite through /task/save_asset + replace:true, rev+1), because a
// template's roi is baked into its cropped image.
//
// P0-1: the button and the "editing <id>" label live in #canvas-footer (outside
// every rail panel), so the task library and frame detail surfaces can reach
// them without switching to the 素材 panel.
function updateAssetRoiSaveButton() {
  const editing = editingRecognitionAsset;
  if (btnSaveAssetRoi) {
    btnSaveAssetRoi.hidden = !editing;
    btnSaveAssetRoi.disabled = !editing;
    if (editing && editing.it) {
      btnSaveAssetRoi.title = `保存识别区到素材「${editing.it.id}」` +
        (editing.it.kind === 'template'
          ? '（模板：矩形=用当前帧重新截图覆盖 rev+1；只改识别多边形=不重截图）'
          : '（OCR/区域：保存矩形 roi 或识别多边形）');
    }
  }
  if (assetRoiEditLabel) {
    assetRoiEditLabel.hidden = !editing;
    assetRoiEditLabel.textContent = editing && editing.it
      ? `识别区编辑中：${editing.it.id}`
      : '';
  }
}

// seq130-fix: 操作区域编辑改为「显式保存」——与识别区域一致：编辑时在 #canvas-footer
// 显示保存按钮/编辑标签，画完/移动后不自动落盘，点「保存操作区域（写入素材）」才提交。
function updateAssetActionSaveButton() {
  const editing = !!pendingActionAsset;
  if (btnSaveAssetActionRoi) {
    btnSaveAssetActionRoi.hidden = !editing;
    btnSaveAssetActionRoi.disabled = !editing;
    if (editing) {
      btnSaveAssetActionRoi.title = `保存操作区域到素材「${pendingActionAsset}」（矩形/多边形/反选）`;
    }
  }
  if (assetActionEditLabel) {
    assetActionEditLabel.hidden = !editing;
    assetActionEditLabel.textContent = editing ? `操作区编辑中：${pendingActionAsset}` : '';
  }
}
// --- seq131: 素材设置→加入集合 ------------------------------------------------
// 用户 2026-11-07 08:10 反馈：把素材添加为集合的 UI 没了；按钮放到素材设置里。
// 集合 = 多模板目标（flow 引用集合 id），后端 POST /task/asset/collection
// {task,id,members[,threshold,action_shape]}。同一模板只能属于一个集合；
// 已在其它集合时先移出旧集合（剩 0 个则删除旧集合），再写入目标集合。
let collectionAddCtx = null;

// seq132：集合成员白名单（用户 08:26「扩展到所有素材」）。集合嵌套仍禁止。
// region 成员=固定区域（总是命中，作为兜底）。
const COLLECTION_MEMBER_KINDS = ['template', 'ocr_line', 'ocr_lines', 'ocr_var', 'region'];

function collectionMemberKindAllowed(kind) {
  return COLLECTION_MEMBER_KINDS.indexOf(String(kind === undefined || kind === null ? '' : kind)) >= 0;
}

function collectionMemberKindNote(kind) {
  const k = String(kind === undefined || kind === null ? '' : kind);
  if (k === 'template') return '模板图成员（按模板匹配识别）';
  if (k === 'ocr_line' || k === 'ocr_lines') return 'OCR 文字成员（按 OCR 类型识别）';
  if (k === 'ocr_var') return '使用端OCR文字配置项成员（按识别文字识别）';
  if (k === 'region') return '固定区域成员（总是命中，作为兜底）';
  return '';
}

function collectionMembersWith(members, templateId) {
  const out = [];
  for (const m of Array.isArray(members) ? members : []) {
    if (typeof m === 'string' && m && m !== templateId && !out.includes(m)) out.push(m);
  }
  out.push(templateId);
  return out;
}

function collectionContaining(collections, templateId) {
  return (Array.isArray(collections) ? collections : [])
    .find((c) => Array.isArray(c.members) && c.members.includes(templateId)) || null;
}

function collectionAddTargetLabel(c, current) {
  const n = Array.isArray(c.members) ? c.members.length : 0;
  return `${c.id}（成员 ${n}）` + (current && current.id === c.id ? ' · 当前集合' : '');
}

function collectionAddMembersFor(collections, targetId, templateId) {
  const base = (Array.isArray(collections) ? collections : []).find((c) => c.id === targetId) || null;
  return collectionMembersWith(base ? base.members : [], templateId);
}

async function openCollectionAddDialog(it, scopeTask) {
  if (!it) return;
  // seq132：所有可识别/固定素材都可加入集合；仅 template_set（集合嵌套）禁止。
  if (!collectionMemberKindAllowed(it.kind)) {
    resultEl.textContent = `素材「${it.id}」（${it.kind || '?'}）不能加入素材集合：` +
      '素材集合不能加入另一个集合（不支持集合嵌套）。';
    log(`加入集合失败：${it.id} kind=${it.kind || '?'}（集合嵌套不支持）`, 'WARN');
    return;
  }
  const task = assetRowScopeTask(scopeTask, true);
  if (task == null) return;
  // 全局模式下 /task/assets 用当前选中任务取“全局模式”视图（同 refreshTaskAssets）。
  const fetchTask = assetsGlobalMode
    ? ((taskSelect && taskSelect.value) || (taskName && taskName.value) || '')
    : (task === GLOBAL_SCOPE_TASK ? ((taskSelect && taskSelect.value) || '') : task);
  let collections = [];
  try {
    const r = await fetch('/task/assets?task=' + encodeURIComponent(fetchTask || ''));
    const s = await r.json();
    if (!s.ok) throw new Error((s.errors || ['读取素材库失败']).join('；'));
    collections = Array.isArray(s.collections) ? s.collections : [];
  } catch (e) {
    resultEl.textContent = '读取素材集合失败：' + e;
    log('读取素材集合失败：' + e, 'ERROR');
    return;
  }
  collectionAddCtx = {
    it: it,
    task: task,
    scopeTask: scopeTask,
    collections: collections,
    current: collectionContaining(collections, it.id),
    error: '',
  };
  renderCollectionAddDialog();
  if (collectionAddDialog && !collectionAddDialog.open) collectionAddDialog.showModal();
}

function renderCollectionAddDialog() {
  const ctx = collectionAddCtx;
  if (!ctx) return;
  if (collectionAddTitle) collectionAddTitle.textContent = `把素材「${ctx.it.id}」加入素材集合`;
  if (collectionAddTarget) {
    collectionAddTarget.textContent = '';
    for (const c of ctx.collections) {
      const opt = document.createElement('option');
      opt.value = c.id;
      opt.textContent = collectionAddTargetLabel(c, ctx.current);
      collectionAddTarget.appendChild(opt);
    }
    const newOpt = document.createElement('option');
    newOpt.value = '__new__';
    newOpt.textContent = '＋ 新建集合…';
    collectionAddTarget.appendChild(newOpt);
    const preferred = ctx.current ? ctx.current.id : (ctx.collections.length ? ctx.collections[0].id : '__new__');
    collectionAddTarget.value = preferred;
  }
  syncCollectionAddFields();
  if (collectionAddError) collectionAddError.textContent = ctx.error || '';
}

function syncCollectionAddFields() {
  const ctx = collectionAddCtx;
  if (!ctx) return;
  const isNew = !collectionAddTarget || collectionAddTarget.value === '__new__';
  if (collectionAddIdRow) collectionAddIdRow.hidden = !isNew;
  if (isNew && collectionAddId && !collectionAddId.value) {
    collectionAddId.value = '集合_' + ctx.it.id;
  }
  if (!collectionAddInfo) return;
  const parts = [];
  if (ctx.current) parts.push(`当前已在集合「${ctx.current.id}」`);
  const kindNote = collectionMemberKindNote(ctx.it.kind);
  if (kindNote) parts.push('成员类型：' + kindNote);
  if (isNew) {
    parts.push('将新建集合并把该素材作为成员（集合 id 在 flow 中引用）');
  } else {
    const base = ctx.collections.find((c) => c.id === collectionAddTarget.value);
    if (base) {
      const others = (base.members || []).filter((m) => m !== ctx.it.id);
      parts.push(`目标集合「${base.id}」现有成员：${others.join('、') || '无'}；保存后加入该素材`);
    }
  }
  collectionAddInfo.textContent = parts.join('；') || '—';
}

function closeCollectionAddDialog() {
  collectionAddCtx = null;
  if (collectionAddDialog && collectionAddDialog.open) collectionAddDialog.close();
}

async function saveCollectionAddDialog() {
  const ctx = collectionAddCtx;
  if (!ctx) return;
  const isNew = !collectionAddTarget || collectionAddTarget.value === '__new__';
  const targetId = isNew ? String(collectionAddId && collectionAddId.value || '').trim() : collectionAddTarget.value;
  if (!targetId) {
    ctx.error = '请填写新集合 id';
    renderCollectionAddDialog();
    return;
  }
  if (isNew && ctx.collections.some((c) => c.id === targetId)) {
    ctx.error = `集合「${targetId}」已存在：请直接选择已有集合，或换一个 id`;
    renderCollectionAddDialog();
    return;
  }
  try {
    // 已经在别的集合：先移出旧集合（旧集合剩 0 个成员时删除；被 flow 引用会 400）。
    if (ctx.current && ctx.current.id !== targetId) {
      const remain = (ctx.current.members || []).filter((m) => m !== ctx.it.id);
      if (remain.length) {
        const r0 = await postJson('/task/asset/collection', {
          task: ctx.task, id: ctx.current.id, members: remain,
          threshold: ctx.current.threshold, action_shape: ctx.current.action_shape,
        });
        if (!r0.ok) throw new Error('移出旧集合失败：' + (r0.errors || []).join('；'));
      } else {
        const r0 = await postJson('/task/asset/collection/delete', { task: ctx.task, id: ctx.current.id });
        if (!r0.ok) throw new Error('删除旧集合失败：' + (r0.errors || []).join('；'));
      }
    }
    const base = ctx.collections.find((c) => c.id === targetId) || null;
    const body = { task: ctx.task, id: targetId, members: collectionMembersWith(base ? base.members : [], ctx.it.id) };
    if (base && base.threshold != null) body.threshold = base.threshold;
    if (base && base.action_shape) body.action_shape = base.action_shape;
    const r = await postJson('/task/asset/collection', body);
    if (!r.ok) throw new Error((r.errors || ['保存素材集合失败']).join('；'));
    const rewritten = Number(r.tasks_rewritten || 0);
    log(`素材「${ctx.it.id}」已加入集合「${targetId}」` + (rewritten ? `；自动改写引用任务 ${rewritten} 个` : ''));
    setStatus(`已加入素材集合「${targetId}」`);
    const scopeTask = ctx.scopeTask;
    closeCollectionAddDialog();
    await refreshTaskAssetsForScope(scopeTask);
    await refreshGlobalAssets();
  } catch (e) {
    ctx.error = String(e);
    if (collectionAddError) collectionAddError.textContent = ctx.error;
    log('加入素材集合失败：' + ctx.error, 'ERROR');
  }
}
// P0-2: every path that leaves the ROI editor must go through here. Keeping the
// editing target armed while the canvas shows another asset/frame is what let
// "edit A, click B, save" overwrite A with B's box. Cancelling drops the canvas
// geometry too, so a stale rectangle can never be mistaken for a new target.
function cancelRecognitionRoiEdit(reason) {
  const editing = editingRecognitionAsset;
  if (!editing) return false;
  const id = editing.it ? editing.it.id : '';
  editingRecognitionAsset = null;
  resetSelection({ clearInvert: true, clearRoi: true, clearBoxes: false, redraw: false });
  dragStart = null;
  dragTarget = 'roi';
  lastLinesRoi = null;
  updateAssetRoiSaveButton();
  updateRoiLabel();
  draw();
  const text = `已取消素材「${id}」的识别区编辑` + (reason ? `（${reason}）` : '');
  setStatus(text);
  log(text);
  return true;
}

function editAssetRecognitionRoi(it, scopeTask) {
  if (!it) return;
  // P0-2: opening another edit always drops the previous target first, even if
  // this new target turns out to be invalid.
  cancelRecognitionRoiEdit('切换识别区编辑');
  if (it.kind === 'template_set') {
    resultEl.textContent = `素材集合「${it.id}」没有识别区域`;
    return;
  }
  const emptyRoiAllowed = !Array.isArray(it.roi) && String(it.kind) === 'ocr_var';
  if (!Array.isArray(it.roi) && !emptyRoiAllowed) {
    resultEl.textContent = `素材「${it.id}」没有识别区域，无法在画布上编辑`;
    return;
  }
  const task = assetRowScopeTask(scopeTask, true);
  if (!task) return;
  // P2-2: while 实时 is on the canvas mousedown is forwarded to the device, so a
  // ROI drag would never reach the editor. Pause it on entry.
  const wasLive = livePreview;
  if (wasLive) pauseLivePreview();
  // Leave every action/polygon mode first: from here a canvas drag must draw the
  // recognition box (the default drag target), not an operation region.
  resetSelection({ clearInvert: true, clearBoxes: true, redraw: false });
  activeCandidate = -1;
  pendingActionAsset = null;
  pendingActionAssetScope = null;
  updateAssetActionSaveButton();
  // Load the asset's CURRENT roi (and operation region) onto the canvas.
  pickAssetRoi(it);
  // #10（批次B3b）：进入识别区域工具时，已存识别多边形载入“可编辑”状态
  // （橙色可拖顶点/双击删点），而不是只读蓝层；直接画矩形/新多边形即覆盖。
  const draft = recognitionDraftFromAsset(it);
  if (draft.polygon) {
    polygon = draft.polygon.map(([x, y]) => ({ x, y }));
    polyClosed = true;
    storedRecognitionPolygon = null;
    roi = draft.roi
      ? { x: draft.roi[0], y: draft.roi[1], w: draft.roi[2], h: draft.roi[3] }
      : polygonBBox(polygon);
  }
  dragStart = null;
  dragTarget = 'roi';
  editingRecognitionAsset = { it, scopeTask: task };
  setCanvasTool('recognition');
  updateAssetRoiSaveButton();
  updateRoiLabel();
  draw();
  const text = `编辑素材「${it.id}」识别区（统一框选工具）：拖动=矩形；单击=多边形顶点，双击收尾` +
    (draft.polygon ? `；已载入现有识别多边形 ${draft.polygon.length} 顶点（可双击删顶点/拖动顶点）` : '') +
    (emptyRoiAllowed && !draft.polygon ? '；当前为全屏 OCR（无识别区），画一个矩形/多边形即可限定识别区域' : '') +
    '\n画好后点画布底部「保存识别区（覆盖素材）」' +
    (it.kind === 'template' ? '\n模板：ROI 变化=用当前帧重新截图覆盖（rev+1）；只改多边形=不重截图' : '') +
    (wasLive ? '\n实时预览已暂停，避免拖拽被当作设备点击；保存后可手动恢复' : '');
  resultEl.textContent = text;
  log(text);
}

async function saveAssetRecognitionRoi() {
  const editing = editingRecognitionAsset;
  if (!editing || !editing.it) return;
  const editingId = editing.it.id;
  // #10：画布草稿 = 已收尾的多边形（优先）或矩形 roi。
  const draftPoly = polygon && polyClosed && polygon.length >= 3
    ? polygon.map((p) => [Math.round(p.x), Math.round(p.y)])
    : null;
  let draftRoi = roi && roi.w > 0 && roi.h > 0
    ? [Math.round(roi.x), Math.round(roi.y), Math.round(roi.w), Math.round(roi.h)]
    : null;
  if (!draftRoi && draftPoly) draftRoi = recognitionPolygonBBox(draftPoly);
  // P2-9: the rectangle must fit the frame that is on screen（多边形由服务端按帧校验）。
  if (!draftPoly && draftRoi) {
    const frameCheck = assetRoiWithinFrame(draftRoi, canvas.width, canvas.height);
    if (!frameCheck.ok) {
      resultEl.textContent = frameCheck.text;
      window.alert(frameCheck.text);
      return;
    }
  }
  const task = assetRowScopeTask(editing.scopeTask, true);
  if (!task) return;
  // P1-2: the settings menu stays open while the ROI editor is active, so
  // threshold/positions/rename may have changed since entry. Re-read the latest
  // stored asset instead of replaying the entry snapshot.
  const latest = findAssetById(editing.scopeTask, editingId, {
    global: globalAssetsCache,
    task: taskAssets,
  });
  if (!latest) {
    const text = `素材「${editingId}」已不存在（可能被删除或重命名），已取消识别区编辑`;
    resultEl.textContent = text;
    window.alert(text);
    cancelRecognitionRoiEdit('素材已不存在');
    return;
  }
  const it = latest;
  const plan = recognitionSavePlan(it, { roi: draftRoi, polygon: draftPoly });
  if (!plan.ok) {
    resultEl.textContent = '无法保存识别区：' + plan.reason;
    window.alert('无法保存识别区：' + plan.reason);
    return;
  }
  if (plan.action === 'none') {
    resultEl.textContent = `素材「${it.id}」${plan.reason}，无需保存`;
    setStatus(`识别区未变化：${it.id}`);
    log(`识别区未变化：${it.id}`);
    return;
  }
  if (plan.action === 'overwrite') {
    // 模板覆盖保存 = 用当前帧重新截图（rev+1）；多边形/矩形按本次绘制显式提交。
    const positions = Array.isArray(it.positions) ? it.positions : [];
    if (!positions.length) {
      const text = `模板「${it.id}」没有位置帧，无法覆盖保存；请先在「位置帧」里加至少一个帧`;
      resultEl.textContent = text;
      window.alert(text);
      return;
    }
    const polyText = plan.request.polygon
      ? `并保存识别多边形 ${plan.request.polygon.length} 顶点`
      : '识别多边形将被清空';
    if (!window.confirm(
      `用当前帧重新截图并覆盖模板「${it.id}」（rev+1，不可撤销；位置帧 ${positions.length} 个；${polyText}）？`
    )) return;
    const payload = assetRoiReplacePayload(task, it, plan.request.roi, plan.request.polygon);
    const r = await postJson('/task/save_asset', payload);
    if (!r.ok) {
      showErrors('覆盖模板失败：', r.errors);
      return;
    }
    const text = `模板「${it.id}」已用当前帧重新截图覆盖：识别区 [${plan.roi.join(',')}]（rev+1）` +
      (plan.request.polygon ? `，识别多边形 ${plan.request.polygon.length} 顶点` : '，识别多边形已清空');
    resultEl.textContent = text;
    log(text);
    setStatus(`模板「${it.id}」已覆盖；两个素材库已刷新`);
  } else {
    const payload = Object.assign({ task, id: it.id }, plan.request);
    const r = await postJson('/task/asset/update', payload);
    if (!r.ok) {
      showErrors('更新识别区域失败：', r.errors);
      return;
    }
    let text;
    if (it.kind === 'region') {
      text = `素材「${it.id}」识别区域：[${plan.roi.join(',')}]（操作区 action_shape 保持不变）`;
    } else if (plan.request.polygon) {
      text = `素材「${it.id}」识别区已保存：多边形 ${plan.request.polygon.length} 顶点，roi=[${plan.roi.join(',')}]` +
        (it.kind === 'template' ? '（未重新截图）' : '');
    } else {
      text = `素材「${it.id}」识别区域：[${plan.roi.join(',')}]（识别多边形已清空）`;
    }
    resultEl.textContent = text;
    log(text);
    setStatus(`素材「${it.id}」识别区域已保存；两个素材库已刷新`);
  boxMoveUndo = null;
  }
  editingRecognitionAsset = null;
  updateAssetRoiSaveButton();
  setCanvasTool('none');
  // #6：保存后旧的“已存识别多边形”显示层已过期，清掉并重绘（刷新后可重新选中查看）。
  storedRecognitionPolygon = null;
  updateRoiLabel();
  draw();
  await refreshTaskAssetsForScope(task);
  await refreshGlobalAssets();
}

function editAssetActionRoi(it, scopeTask) {
  // 全屏 ocr_var 没有识别区（roi 为空）也允许画操作区（action_shape 绝对坐标）。
  if (!it.roi && String(it.kind) !== 'ocr_var') {
    resultEl.textContent = `素材「${it.id}」没有识别区域`;
    return;
  }
  // A recognition-ROI edit must not survive into an action-region edit.
  cancelRecognitionRoiEdit('切换到操作区域编辑');
  // P1-8: entering action editing clears the other selection mode and any stale
  // action geometry; loadActionShapeForEdit then restores this asset's shape.
  resetActionGeometry({ keepTarget: true, clearInvert: true, redraw: false });
  pendingActionAsset = it.id;
  pendingActionAssetScope = assetRowScopeTask(scopeTask, false);
  activeCandidate = -1;
  roi = it.roi
    ? { x: it.roi[0], y: it.roi[1], w: it.roi[2], h: it.roi[3] }
    : null;
  const actionBox = actionShapeBBox(it.action_shape);
  // v1.2.4: restore BOTH kinds (rect + polygon) and their invert flag. The
  // loader clears the stale rect first; a polygon must not keep a bbox rect.
  loadActionShapeForEdit(it.action_shape);
  actionShapeOwnerId = String(it.id);
  if (!actionBox && roi) roiBack = { ...roi };
  // #10：统一框选工具的操作区域模式（作用对象=该素材）。
  setCanvasTool('action');
  updateRoiLabel();
  updateAssetActionSaveButton();
  draw();
  log(
    `编辑素材「${it.id}」操作区域（统一框选）：拖动=矩形，单击=多边形顶点，双击收尾；` +
      '反选=整个屏幕−区域；保存后刷新两个素材库'
  );
}

async function updateAssetActionRoi(id) {
  // C: the row scope stored when editing started (frame detail rows can be
  // global or task-only); no scope => the old assetScopeTask behaviour.
  const task = assetRowScopeTask(pendingActionAssetScope, true);
  const shape = currentActionShape();
  if (!task || !shape) {
    pendingActionAsset = null;
    pendingActionAssetScope = null;
    updateAssetActionSaveButton();
    return;
  }
  const r = await postJson('/task/asset/update', {
    task,
    id,
    action_shape: shape,
  });
  if (!r.ok) {
    showErrors('更新操作区域失败：', r.errors);
    return;
  }
  // P0-3: saved — drop the local polygon/rect so the next candidate or asset
  // edit cannot accidentally reuse this shape.
  pendingActionAsset = null;
  pendingActionAssetScope = null;
  roiBack = null;
  resetActionGeometry({ keepTarget: false, clearInvert: true, redraw: false });
  setCanvasTool('none');
  updateRoiLabel();
  updateAssetActionSaveButton();
  draw();
  log(`素材「${id}」操作区域已更新`);
  setStatus(`素材「${id}」操作区域已保存；两个素材库已刷新`);
  boxMoveUndo = null;
  await refreshTaskAssetsForScope(task);
  await refreshGlobalAssets();
}

function pickAssetRoi(it) {
  // seq130-fix: 选中其它素材行=退出操作区域编辑（避免保存按钮把旧素材操作区写错对象）。
  if (pendingActionAsset) {
    pendingActionAsset = null;
    pendingActionAssetScope = null;
    updateAssetActionSaveButton();
  }
  // seq130-fix: 选中其它素材行=退出操作区域编辑（避免保存按钮把旧素材操作区写错对象）。
  if (pendingActionAsset) {
    pendingActionAsset = null;
    pendingActionAssetScope = null;
    updateAssetActionSaveButton();
  }
  // 全屏 ocr_var（roi 为空）也允许选中：画布只显示操作区/已存多边形，识别区留空。
  if (!Array.isArray(it.roi)) {
    if (String(it.kind) !== 'ocr_var') {
      resultEl.textContent = `素材「${it.id}」没有 roi`;
      return;
    }
    resultEl.textContent = `素材「${it.id}」是全屏 OCR（无识别区）：可在「识别区域」工具里画矩形/多边形限定识别区域`;
  }
  // P0-2: clicking another row (main body / 识别 / region test) replaces the
  // canvas ROI, so the previous asset's ROI editor must not stay armed.
  cancelRecognitionRoiEdit('选中其它素材');
  const shapeBox = actionShapeBBox(it.action_shape);
  roi = Array.isArray(it.roi)
    ? { x: it.roi[0], y: it.roi[1], w: it.roi[2], h: it.roi[3] }
    : null;
  loadActionShapeForEdit(it.action_shape);
  actionShapeOwnerId = String(it.id);
  // #6（验收反馈）：恢复已存识别多边形为只读显示（不再只剩矩形 ROI）。
  storedRecognitionPolygon = Array.isArray(it.polygon) && it.polygon.length >= 3
    ? it.polygon.map((p) => ({ x: Number(p[0]), y: Number(p[1]) }))
    : null;
  polygon = null;
  polyClosed = false;
  updateRoiLabel();
  draw();
  log(`已用素材「${it.id}」设置识别区域${shapeBox ? '（含操作区域）' : ''}` +
    (storedRecognitionPolygon ? `（含已存识别多边形 ${storedRecognitionPolygon.length} 顶点）` : ''));
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
  const items = (assetsGlobalMode ? globalAssetsCache : (taskAssets || []))
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

function currentRecognitionPolygon() {
  return polygon && polyClosed && polygon.length >= 3
    ? polygon.map((p) => [p.x, p.y])
    : null;
}

// The recognition polygon is the OCR/match mask drawn in polygon mode. Once a
// candidate is saved, the mask must travel in the same payload as the ROI so the
// runtime can reuse it (backend field: `polygon`). `candidate.polygon` wins when
// a caller ever snapshots a per-candidate mask; otherwise the current canvas
// polygon is used. Never invent a polygon: absent state => field omitted.
function recognitionPolygonPayload(poly) {
  if (!Array.isArray(poly) || poly.length < 3) return null;
  return poly.map((p) => [p[0], p[1]]);
}

// Pure payload builders shared by the save path and ui_pure_test.js.
// OCR rows keep the multi-line recognition metadata (`ocr_mode`/`ocr_text`) and
// use the multi-line source ROI as the recognition roi (v1.2.3 semantics).
function ocrAssetPayload(c, positions, recognitionPolygon) {
  const rec = (c.fromLines && c.sourceRoi) ? c.sourceRoi : c.recognize;
  const row = {
    id: String(c.name || '').trim(),
    kind: 'ocr_line',
    roi: [rec.x, rec.y, rec.w, rec.h],
    positions: Array.isArray(positions) ? positions.slice() : [],
    ...candidateActionPayload(c),
  };
  if (c.fromLines && c.sourceRoi) {
    row.ocr_mode = 'lines';
    row.ocr_text = c.text;
  }
  const poly = recognitionPolygonPayload(c.polygon || recognitionPolygon);
  if (poly) row.polygon = poly;
  return row;
}

function singleAssetPayload(c, positions, recognitionPolygon) {
  const payload = {
    id: String(c.name || '').trim(),
    kind: c.kind,
    roi: [c.recognize.x, c.recognize.y, c.recognize.w, c.recognize.h],
    positions: Array.isArray(positions) ? positions.slice() : [],
    ...candidateActionPayload(c),
  };
  if (c.kind === 'template') payload.threshold = 0.85;
  const poly = recognitionPolygonPayload(c.polygon || recognitionPolygon);
  if (poly) payload.polygon = poly;
  return payload;
}

async function saveAssetUnified() {
  // Saving a new asset is another asset mutation: the ROI editor must not stay
  // armed against the previous asset while the list gets rebuilt.
  cancelRecognitionRoiEdit('保存新素材');
  // In unified global mode the store is task-independent; allow saving a new
  // global asset without inventing a task name. The API ignores `task` there.
  const typedTask = (taskName.value || '').trim();
  const task = assetsGlobalMode ? (typedTask || GLOBAL_SCOPE_TASK) : currentTask();
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
  const recognitionPolygon = currentRecognitionPolygon();
  const errors = [];
  let saved = 0;
  try {
    if (ocrRows.length) {
      // Keep the batch endpoint for OCR rows so ocr_mode/ocr_text survive.
      // Both endpoints now receive the recognition polygon when one exists.
      const assets = ocrRows.map((c) => ocrAssetPayload(c, positions, recognitionPolygon));
      const r = await postJson('/task/save_assets', { task, assets });
      if (!r.ok) {
        errors.push(...(r.errors || ['批量保存失败']));
      } else {
        saved += (r.saved || []).length;
      }
    }
    for (const c of otherRows) {
      const payload = { task, ...singleAssetPayload(c, positions, recognitionPolygon) };
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
    if (saved) {
      await refreshTaskAssetsForScope(task);
      await refreshGlobalAssets();
    }
    return;
  }
  resultEl.textContent = `已保存 ${saved} 个素材`;
  log(`统一保存素材：${saved} 个`);
  setStatus(`已保存 ${saved} 个素材；任务素材库与全局素材库已刷新`);
  replaceResults([], '');
  await refreshTaskAssetsForScope(task);
  // P0-1: a new asset must be visible in BOTH libraries immediately, even when
  // the current task's flow does not reference it yet.
  await refreshGlobalAssets();
  pendingAssetPositions = [];
  await refreshAssetPositionFrames();
  renderPendingPositions();
}



// The visual flow editor was removed with the old workbench DOM. This loader
// is still reachable (task switch / boot / ?task= deep link) and validates that
// the bundle has a flow; the task asset panel filters by flow on its own.
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
    const steps = Array.isArray(s.flow.steps) ? s.flow.steps.length : 0;
    log(`已载入任务「${task}」的 flow（${steps} 步）`);
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

// P0-1: the only global-library refresh entry. Called on boot, by the real
// `#btn-library-refresh` button, and after every save/delete/position/ROI edit.
async function refreshGlobalAssets() {
  const ok = await loadGlobalAssets();
  // P1-1: the frame detail list and every frame row's 素材 N are derived from
  // globalAssetsCache, so a global-library save/delete/position edit must repaint
  // them too (taskAssets alone is empty in global mode). Cache-only => no extra
  // /frames request and no recursion (renderFrameAssetViews never reloads).
  renderFrameAssetViews();
  setStatus(ok ? '全局素材库已刷新' : '全局素材库刷新失败（见素材库状态）');
}

// Status line contract for both filtered and unfiltered views:
//   全局素材库：共 N / 显示 M ...
function globalAssetStatusText(total, shown, opts) {
  const o = opts || {};
  let text = `全局素材库：共 ${total} / 显示 ${shown}`;
  if (o.collections) text += `（含 ${o.collections} 个集合）`;
  if (o.query) text += ` · 搜索：${o.query}`;
  if (o.category) text += ` · 分类：${o.category}`;
  return text;
}

function refreshGlobalAssetCategoryFilter() {
  fillAssetCategoryFilter(globalAssetCategoryFilter, globalAssetsCache);
}

// Render the CACHED global rows through the same filterAssets() the task panel
// uses. Re-rendering after refresh/save keeps the current search/category
// because neither input is reset here (the <select> only drops a category that
// no longer exists).
function renderGlobalAssets() {
  if (!globalAssetsList) return false;
  const q = (globalAssetSearch && globalAssetSearch.value) || '';
  const cat = globalAssetCategoryFilter ? globalAssetCategoryFilter.value : '';
  const all = Array.isArray(globalAssetsCache) ? globalAssetsCache : [];
  const items = filterAssets(all, q, cat);
  const collections = all.filter((a) => a.kind === 'template_set').length;
  if (globalAssetsStatus) {
    globalAssetsStatus.textContent = globalAssetStatusText(all.length, items.length, {
      collections,
      query: q.trim(),
      category: cat,
    });
  }
  globalAssetsList.textContent = '';
  if (!items.length) {
    globalAssetsList.textContent = (q.trim() || cat)
      ? '（没有匹配的全局素材）'
      : '（暂无全局素材）';
    return true;
  }
  renderAssetRows(globalAssetsList, items, {
    task: '',
    pick: false,
    manage: true,
    actions: true,
    group: true,
    groupBy: 'category',
    clear: true,
    draggable: false,
    emptyText: (q.trim() || cat) ? '（没有匹配的全局素材）' : '（暂无全局素材）',
    onPick: (it) => pickAssetRoi(it),
    onTest: runAssetTest,
    onEditRoi: (it) => editAssetRecognitionRoi(it),
    onRename: (it) => renameAsset(it),
    onThreshold: (it) => editAssetThreshold(it),
    onPosition: (it) => editAssetPositions(it),
    onDelete: (it) => deleteAsset(it),
    onOpRegion: (it) => openOpRegionDialog(it, ''),
    onCollectionAdd: (it) => openCollectionAddDialog(it),
    onLongPress: (it) => runTestActionForAsset(it, 'long_press'),
    onClickTest: (it) => testAssetClick(it),
    onDragTest: (it) => testAssetDrag(it),
    onStep: (it) => generateAssetStep(it),
    onEditText: (it) => editAssetText(it),
    onCollectionTest: (it) => testAssetCandidates(it),
    onCollectionDelete: (it) => deleteCollection(it),
  });
  return true;
}

async function loadGlobalAssets() {
  if (!globalAssetsList) return false;
  try {
    const r = await fetch('/task/assets?task=');
    const s = await r.json();
    if (!s.ok) {
      globalAssetsCache = [];
      refreshGlobalAssetCategoryFilter();
      if (globalAssetsStatus) {
        globalAssetsStatus.textContent = '统一全局素材库未初始化：' + (s.errors || []).join('; ');
      }
      globalAssetsList.textContent = '（暂无全局素材）';
      return false;
    }
    assetsGlobalMode = s.task === GLOBAL_SCOPE_TASK;
    globalAssetsCache = (s.items || []).concat(collectionAssetRows(s.collections));
    refreshGlobalAssetCategoryFilter();
    return renderGlobalAssets();
  } catch (e) {
    if (globalAssetsStatus) globalAssetsStatus.textContent = '全局素材库读取异常：' + e;
    return false;
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

// --- 帧搜索纯函数：只做前端过滤，绝不改变 /frames 的请求参数 ---
function frameMatchesQuery(frame, query) {
  const f = frame || {};
  const raw = String(query == null ? '' : query).trim().toLowerCase();
  if (!raw) return true;
  let idQuery = raw;
  if (idQuery.charAt(0) === '#') idQuery = idQuery.slice(1).trim();
  if (!idQuery) return true;
  if (/^\d+$/.test(idQuery) && String(f.id == null ? '' : f.id) === idQuery) return true;
  const name = String(f.name || '').toLowerCase();
  const category = String(f.category || '').toLowerCase();
  const idText = String(f.id == null ? '' : f.id).toLowerCase();
  return name.includes(raw) || category.includes(raw) || idText.includes(raw);
}

function filterFramesByQuery(frames, query) {
  const list = Array.isArray(frames) ? frames : [];
  return list.filter((f) => frameMatchesQuery(f, query));
}

function frameSearchStatusText(total, shown) {
  const all = Math.max(0, Number(total) || 0);
  const visible = Math.max(0, Number(shown) || 0);
  if (!all) return '共 0 帧';
  if (!visible) return `共 ${all} 帧 · 没有匹配的帧`;
  return `共 ${all} 帧 · 匹配 ${visible} 帧`;
}

// --- 帧详情素材纯函数：全局库 + 任务素材按 asset id 去重并合并来源 ---
function assetHasFramePosition(it, frameId) {
  const fid = Number(frameId);
  if (!Number.isFinite(fid)) return false;
  const positions = it && Array.isArray(it.positions) ? it.positions : [];
  return positions.some((p) => Number(p) === fid);
}

function assetsForFrame(globalItems, taskItems, frameId) {
  const out = [];
  const byId = new Map();
  function add(list, source) {
    for (const it of (Array.isArray(list) ? list : [])) {
      if (!it || it.id == null) continue;
      if (!assetHasFramePosition(it, frameId)) continue;
      const key = String(it.id);
      let row = byId.get(key);
      if (!row) {
        row = Object.assign({}, it, { sources: [] });
        byId.set(key, row);
        out.push(row);
      }
      if (!row.sources.includes(source)) row.sources.push(source);
    }
  }
  add(globalItems, 'global');
  add(taskItems, 'task');
  return out;
}

// C: the API scope of ONE frame-detail row. Frame detail merges both libraries,
// so the row's own sources decide where a write/test must go:
//   global (or global+task) -> the unified global store (GLOBAL_SCOPE_TASK);
//   task-only               -> the task currently selected in the panel.
// The source badge itself is gone (requirement 5): the frame list behaves like
// the global library, and only the hidden scope differs per row.
function frameAssetScope(row, currentTaskName) {
  const sources = row && Array.isArray(row.sources) ? row.sources : [];
  if (sources.includes('global')) return GLOBAL_SCOPE_TASK;
  return String(currentTaskName == null ? '' : currentTaskName).trim();
}

function frameAssetsSummary(items, frameId) {
  const list = Array.isArray(items) ? items : [];
  function count(source) {
    return list.filter((it) => {
      const sources = it && Array.isArray(it.sources) ? it.sources : [];
      return sources.includes(source);
    }).length;
  }
  const globalCount = count('global');
  const taskCount = count('task');
  const total = list.length;
  return {
    frameId: frameId == null ? null : frameId,
    total,
    global: globalCount,
    task: taskCount,
    text: total
      ? `该帧素材 ${total} 条（全局 ${globalCount} · 任务 ${taskCount}）`
      : '该帧没有素材记录',
  };
}

function frameAssetIdsByFrame() {
  const map = new Map();
  function add(list) {
    for (const a of (Array.isArray(list) ? list : [])) {
      const positions = a && Array.isArray(a.positions) ? a.positions : [];
      for (const fid of positions) {
        const key = Number(fid);
        if (!Number.isFinite(key)) continue;
        if (!map.has(key)) map.set(key, []);
        const ids = map.get(key);
        if (!ids.includes(a.id)) ids.push(a.id);
      }
    }
  }
  add(globalAssetsCache);
  add(taskAssets);
  return map;
}

function renderFrameDetail(f, assetIds) {
  if (!frameDetail) return;
  if (!f) {
    frameDetail.textContent = '（选择一帧查看详情）';
    return;
  }
  const ids = Array.isArray(assetIds)
    ? assetIds
    : assetsForFrame(globalAssetsCache, taskAssets, f.id).map((a) => a.id);
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
    `该帧素材：${ids && ids.length ? ids.length : 0} 条\n` +
    `关联素材：${ids && ids.length ? ids.join('、') : '（无）'}`;
}

// The frame detail list is the third asset-library surface. Requirement 5 makes
// it behave EXACTLY like the global library: same row DOM, same 设置 menu, no
// source badge, every action enabled. Only the API scope differs per row
// (`frameAssetScope`): global rows write to the unified store, task-only rows to
// the selected task.
function renderFrameAssets(f, rows) {
  const list = Array.isArray(rows) ? rows : [];
  if (frameAssetsStatus) {
    frameAssetsStatus.textContent = f ? frameAssetsSummary(list, f.id).text : '—';
  }
  if (!frameAssetsList) return;
  frameAssetsList.textContent = '';
  if (!f) {
    frameAssetsList.textContent = '（选择一帧查看该帧素材）';
    return;
  }
  if (!list.length) {
    frameAssetsList.textContent = '该帧没有素材记录';
    return;
  }
  for (const row of list) {
    const scopeTask = frameAssetScope(row, taskName ? taskName.value : '');
    frameAssetsList.appendChild(assetRowElement(row, {
      // Thumbnails read pixels from the owning store: the unified store has no
      // task name, a task bundle does.
      task: scopeTask === GLOBAL_SCOPE_TASK ? '' : scopeTask,
      pick: false,
      manage: true,
      actions: true,
      group: false,
      draggable: false,
      onPick: (it) => pickAssetRoi(it),
      onTest: (it) => runAssetTest(it, f.id, scopeTask),
      onClickTest: (it) => testAssetClick(it, f.id, scopeTask),
      onDragTest: (it) => testAssetDrag(it, f.id, scopeTask),
      onStep: (it) => generateAssetStep(it),
      onEditRoi: (it) => editAssetRecognitionRoi(it, scopeTask),
      onRename: (it) => renameAsset(it, scopeTask),
      onThreshold: (it) => editAssetThreshold(it, scopeTask),
      onPosition: (it) => editAssetPositions(it, scopeTask),
      onDelete: (it) => deleteAsset(it, scopeTask),
      onOpRegion: (it) => openOpRegionDialog(it, scopeTask),
      onCollectionAdd: (it) => openCollectionAddDialog(it, scopeTask),
      onLongPress: (it) => runTestActionForAsset(it, 'long_press', null, scopeTask),
      onEditText: (it) => editAssetText(it, scopeTask),
      onCollectionTest: (it) => testAssetCandidates(it, f.id, scopeTask),
      onCollectionDelete: (it) => deleteCollection(it, scopeTask),
    }));
  }
}

// Re-render the frame list from the cache so typing in #frame-search never
// issues a /frames request. Category grouping is applied AFTER search.
function renderFramesList() {
  if (!framesList) return;
  const all = Array.isArray(framesCache) ? framesCache : [];
  const q = (frameSearch && frameSearch.value) || '';
  const filtered = filterFramesByQuery(all, q);
  if (frameSearchStatus) {
    frameSearchStatus.textContent = frameSearchStatusText(all.length, filtered.length);
    // Only reserve the extra line while a query is active: the static count is
    // already in framesStatus, and every saved pixel keeps the frame list visible.
    frameSearchStatus.style.display = String(q).trim() ? '' : 'none';
  }
  framesList.textContent = '';
  if (!all.length) {
    framesList.textContent = '（暂无图片，点「截取当前帧入库」或上传）';
    return;
  }
  if (!filtered.length) {
    framesList.textContent = '没有匹配的帧';
    return;
  }
  const groups = new Map();
  for (const f of filtered) {
    const cat = (f.category || '').trim() || '未分类';
    if (!groups.has(cat)) groups.set(cat, []);
    groups.get(cat).push(f);
  }
  const orderedCats = Array.from(groups.keys()).sort((a, b) => {
    if (a === '未分类') return 1;
    if (b === '未分类') return -1;
    return 0;
  });
  // Only render the selected category, not every group expanded.
  const selectedCat = categoryFilterValue || '';
  const visibleCats = selectedCat ? [selectedCat] : orderedCats;
  const assetsByFrame = frameAssetIdsByFrame();
  let groupsShown = 0;
  for (const cat of visibleCats) {
    if (!groups.has(cat)) continue;
    groupsShown += 1;
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
      const linked = assetsByFrame.get(f.id) || [];
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
  if (!groupsShown) framesList.textContent = '没有匹配的帧';
}

// P1-1: asset saves/deletes/position edits change the frame<->asset relation but
// never the frames themselves. Every asset CRUD path refreshes the task panel
// first, and refreshTaskAssets() calls refreshFrames() *before* the global
// library is reloaded — so the frame detail list and each frame row's 素材 N
// would keep reading the previous globalAssetsCache until the next manual
// refresh. Repaint both surfaces from the refreshed caches.
// Deliberately cache-only: calling refreshFrames() here would issue a second
// GET /frames (plus one more through refreshAssetPositionFrames) right after the
// request refreshTaskAssets() just made, while nothing about the frames changed.
// Nothing on this path calls refreshGlobalAssets(), so it cannot recurse either.
function renderFrameAssetViews() {
  const frames = Array.isArray(framesCache) ? framesCache : [];
  // Before the frame list has ever loaded there is nothing to repaint; doing it
  // anyway would overwrite the panel with a false "no frames" empty state.
  if (!frames.length && currentFrameId == null) return false;
  const f = currentFrameId == null
    ? null
    : frames.find((x) => Number(x.id) === Number(currentFrameId)) || null;
  renderFramesList();
  if (!f) {
    renderFrameDetail(null);
    renderFrameAssets(null, []);
    return false;
  }
  const rows = assetsForFrame(globalAssetsCache, taskAssets, f.id);
  renderFrameDetail(f, rows.map((a) => a.id));
  renderFrameAssets(f, rows);
  return true;
}

// seq138 V2-C：工作台当前帧唯一状态——地图 tab 通过该事件实时刷新预览/命中。
function notifyWorkbenchFrameChanged() {
  try {
    document.dispatchEvent(new CustomEvent('oas:frame-changed', {
      detail: { frameId: currentFrameId },
    }));
  } catch (e) { /* CustomEvent unsupported: map tab falls back to activation read */ }
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
    if (!frames.length) {
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
      renderFrameAssets(null, []);
      renderFramesList();
      return;
    }
    const groups = new Map();
    const unseenByCat = new Map();
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
    const currentAssets = currentFrame
      ? assetsForFrame(globalAssetsCache, taskAssets, currentFrame.id)
      : [];
    renderFrameDetail(currentFrame, currentAssets.map((a) => a.id));
    renderFrameAssets(currentFrame, currentAssets);
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
    renderFramesList();
    framesStatus.textContent = `共 ${frames.length} 帧；拖动排序/换分组，点行切换当前帧`;
    await refreshAssetPositionFrames();
    notifyWorkbenchFrameChanged();
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
  // P0-2: a recognition ROI is frame-relative; switching frames invalidates it.
  cancelRecognitionRoiEdit('切换帧');
  // seq137：切帧统一清识别区+操作区（旧几何不能画到新帧上）。
  clearCanvasSelection('切换帧');
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
    // MATCH-BUG: result boxes belong to the frame they were measured on. Clear
    // them before loading the new image (loadFrame draws), otherwise the last
    // test's rectangles paint over the new frame during the /state round-trip
    // and look like "the box never changes".
    boxes = [];
    selectedBox = null;
    await loadFrame();
    notifyWorkbenchFrameChanged();
    await refreshState();
    draw();
    log(`已切换当前帧 #${id}（${s.width}x${s.height}）`);
    await refreshFrames();
    // Clicking a frame is the explicit request to inspect it: reveal the
    // (collapsed by default) frame-detail / per-frame asset section.
    if (frameDetails) frameDetails.open = true;
  } catch (e) {
    framesStatus.textContent = '切换异常：' + e;
  }
}

async function deleteFrame(id) {
  // The edited ROI belongs to whatever frame is on screen; deleting frames may
  // change it (or the deleted one may be current), so leave the editor.
  cancelRecognitionRoiEdit('删除帧');
  // P2-2: capture this before refreshFrames() replaces currentFrameId, so the
  // fallback repaint below can drop test boxes measured on the removed image.
  const wasCurrent = currentFrameId != null && Number(currentFrameId) === Number(id);
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
    // Same "the box never changes" illusion as selectFrame: the fallback frame
    // is painted by loadFrame()+draw() below and must not inherit boxes that
    // were measured on the deleted frame.
    if (wasCurrent) {
      boxes = [];
      selectedBox = null;
    }
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
  // Clearing the frame list invalidates the frame the ROI editor is drawing on.
  cancelRecognitionRoiEdit('清空帧列表');
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

// Canvas sizing writes inline CSS on #screen, which is a child of the observed
// #canvas-stage. Doing that synchronously from the ResizeObserver callback
// re-queues the observer in the same frame and Chromium reports
// "ResizeObserver loop completed with undelivered notifications". Coalesce all
// fit requests into one rAF pass and make the style writes idempotent so a
// no-op fit cannot feed the observer again.
let canvasFitScheduled = false;
let canvasFitW = 0;
let canvasFitH = 0;

function scheduleCanvasFit() {
  if (canvasFitScheduled) return;
  canvasFitScheduled = true;
  const raf = (typeof window !== 'undefined' && typeof window.requestAnimationFrame === 'function')
    ? window.requestAnimationFrame.bind(window)
    : (cb) => setTimeout(cb, 16);
  raf(() => {
    canvasFitScheduled = false;
    fitCanvasToStage();
  });
}

function fitCanvasToStage() {
  if (!canvasStage || !canvas) return;
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
  w = Math.max(1, Math.floor(w));
  h = Math.max(1, Math.floor(h));
  // Same target as the last applied fit => no style write, so no observer
  // notification and no layout invalidation.
  if (w === canvasFitW && h === canvasFitH) return;
  const wpx = `${w}px`;
  const hpx = `${h}px`;
  if (canvas.style.width !== wpx) canvas.style.width = wpx;
  if (canvas.style.height !== hpx) canvas.style.height = hpx;
  canvasFitW = w;
  canvasFitH = h;
}

// Live frames arrive continuously. The canvas only needs a re-fit when the
// intrinsic frame size actually changes; stage resizes stay the sole job of the
// ResizeObserver, so the per-frame path never writes CSS dimensions.
function setCanvasFrameSize(w, h) {
  if (!canvas || !(w > 0) || !(h > 0)) return false;
  if (canvas.width === w && canvas.height === h) return false;
  canvas.width = w;
  canvas.height = h;
  return true;
}

async function loadFrame(live = false) {
  return new Promise((resolve, reject) => {
    const im = new Image();
    im.onload = () => {
      img = im;
      if (setCanvasFrameSize(im.width, im.height)) scheduleCanvasFit();
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
    if (setCanvasFrameSize(im.width, im.height)) scheduleCanvasFit();
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
  // P2-1/P0-2: live frames replace whatever is on the canvas, so a canvas ROI
  // edit cannot survive the switch.
  cancelRecognitionRoiEdit('开启实时预览');
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
  // P2-1/P0-2: the canvas geometry no longer matches a stable frame.
  cancelRecognitionRoiEdit('暂停实时预览');
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
  // #10 统一框选工具：工具模式下由本段统一接管手势。
  //   拖动(位移>阈值)=矩形；单击=多边形顶点；双击=收尾（dblclick 处理）。
  if (canvasTool !== 'none') {
    if (ev.detail >= 2) {
      ev.preventDefault();
      return; // 双击的第二次按下交给 dblclick 收尾，避免多加一个重复顶点
    }
    if (canvasTool === 'recognition' && polygon && polygon.length) {
      const vi = nearestVertex(p0);
      if (vi >= 0 && polyClosed) {
        polyDrag = vi; // 已收尾的识别多边形：拖动顶点微调
        log(`拖动识别多边形顶点 ${vi + 1}/${polygon.length}`);
        updateRoiLabel();
        draw();
        return;
      }
    }
    // seq130（#1）：点击已有框（操作区 / 识别多边形 / 识别矩形）优先选中 → 拖动整体平移。
    const boxActionShape = currentActionShape();
    if (boxActionShape && hitTestActionShape(p0, boxActionShape)) {
      beginBoxMove('action', p0);
      return;
    }
    if (canvasTool === 'recognition' && polygon && polyClosed && pointInFramePolygon(p0, polygon)) {
      beginBoxMove('recognitionPolygon', p0);
      return;
    }
    if (canvasTool === 'recognition' && !polygon && roi && pointInFrameRect(p0, roi)) {
      beginBoxMove('recognitionRoi', p0);
      return;
    }
    boxMoveUndo = null;
    canvasGesture = { start: p0 };
    dragStart = p0;
    dragTarget = canvasTool === 'recognition' ? 'roi' : 'back';
    draw();
    return;
  }
  // 非工具模式：保留旧的选择区块 / 拖拽识别框行为。
  if (boxes.length) {
    for (let i = boxes.length - 1; i >= 0; i--) {
      const b = boxes[i];
      const [bx, by, bw, bh] = b.box;
      if (p0.x >= bx && p0.x <= bx + bw && p0.y >= by && p0.y <= by + bh) {
        // seq137：检测框选择也要清掉旧操作区几何（actionPoly/roiBack），只保留识别框。
        resetSelection({ clearInvert: true, clearRoi: false, clearBoxes: false, redraw: false });
        roi = { x: bx, y: by, w: bw, h: bh };
        roiBack = null;
        polygon = null;
        polyClosed = false;
        storedRecognitionPolygon = null;
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
  // seq137：空白点击统一清识别区 + 操作区（共享入口；不再残留 actionPoly/roiBack）。
  clearCanvasSelection('点击画布空白');
  dragStart = p0;
  dragTarget = 'roi';
  draw();
});

canvas.addEventListener('mousemove', (ev) => {
  if (liveInputAllowed()) {
    if (liveInputDrag) ev.preventDefault();
    return;
  }
  const p = toFrame(ev);
  // seq130（#1）：正在整体移动已有框。
  if (boxMove) {
    ev.preventDefault();
    applyBoxMove(p);
    return;
  }
  // 识别多边形已收尾：拖动顶点微调。
  if (canvasTool === 'recognition' && polyDrag !== null && polygon) {
    polygon[polyDrag] = p;
    if (polygon.length >= 3) roi = polygonBBox(polygon);
    updateRoiLabel();
    draw();
    return;
  }
  if (canvasTool !== 'none' && !dragStart && polyDrag === null) {
    // seq130（#1）：悬停在已有框上 = 可拖动光标 + 状态提示。
    let hover = '';
    const hoverShape = currentActionShape();
    if (hoverShape && hitTestActionShape(p, hoverShape)) hover = 'move';
    else if (canvasTool === 'recognition' && polygon && polyClosed && pointInFramePolygon(p, polygon)) hover = 'move';
    else if (canvasTool === 'recognition' && !polygon && roi && pointInFrameRect(p, roi)) hover = 'move';
    if (hover !== boxMoveHover) {
      boxMoveHover = hover;
      canvas.style.cursor = hover || '';
      if (hover) setStatus('可拖动：按住鼠标移动已有框；点击空白=新建，单击=加顶点，双击收尾', '#e8c46a');
    }
  }
  if (!dragStart) return;
  const start = canvasGesture ? canvasGesture.start : dragStart;
  // #10 工具模式：只有位移超过阈值才预览矩形；否则保持“点击=加点”的候选状态。
  if (canvasTool !== 'none') {
    if (canvasGestureShape(start, p) !== 'rect') return;
    if (canvasTool === 'recognition') {
      if (polygon && polygon.length) {
        polygon = null;
        polyClosed = false;
      }
      storedRecognitionPolygon = null;
      roi = normalizeRect(start, p);
    } else {
      actionPoly = null;
      actionPolyClosed = false;
      roiBack = normalizeRect(start, p);
    }
    updateRoiLabel();
    draw();
    return;
  }
  if (dragTarget === 'back') {
    roiBack = normalizeRect(dragStart, p);
    updateRoiLabel();
    draw();
    return;
  }
  roi = normalizeRect(dragStart, p);
  updateRoiLabel();
  draw();
});

// 操作区域手势/多边形收尾后的落盘：候选行更新 or 素材 action_shape 保存。
// `shapeOverride` 供“双击收尾多边形”显式传入当前 polygon 形状（不能用矩形路径的
// roiBack 判断，否则多边形会被误判成“未设置操作区”）。
function finishActionGesture(shapeOverride) {
  const shape = shapeOverride !== undefined ? shapeOverride : currentActionShape();
  const bbox = shape ? actionShapeBBox(shape) : null;
  const finished = bbox
    ? { x: bbox[0], y: bbox[1], w: bbox[2], h: bbox[3] }
    : (roiBack ? { ...roiBack } : null);
  if (activeCandidate >= 0 && saveCandidates[activeCandidate]) {
    const row = saveCandidates[activeCandidate];
    row.action = finished || { ...row.recognize };
    // Only this explicit gesture stores an operation shape; the bbox and the
    // polygon/invert carried by currentActionShape are persisted together.
    row.actionShape = shape;
    activeCandidate = -1;
    setCanvasTool('none');
    renderCandidateList(lastLinesRoi ? '本次识别' : '识别结果');
    log(
      `操作区域已更新：${row.actionShape
        ? `${row.actionShape.kind}${row.actionShape.invert ? '（反选）' : ''}`
        : '使用识别区域'}`
    );
  } else if (pendingActionAsset) {
    // seq130-fix: 不自动落盘——与识别区域一致，等用户点「保存操作区域」。
    log(`操作区域已画好（素材「${pendingActionAsset}」）：点底部「保存操作区域」写入素材`);
    setStatus('操作区域未保存：点底部「保存操作区域」写入素材', '#e8c46a');
    updateAssetActionSaveButton();
  } else {
    resultEl.textContent =
      '操作区域已画好，但当前没有绑定候选/素材。\n' +
      '请先在识别结果点「调操作区域」，或在素材行点「操作区…→ 在画布上编辑」，再画操作区域。';
    log('操作区域已画好，但未绑定候选/素材', 'WARN');
  }
  updateRoiLabel();
  draw();
}

canvas.addEventListener('mouseup', (ev) => {
  if (liveInputAllowed() && liveInputDrag) {
    finishLiveInput(ev);
    return;
  }
  const p = toFrame(ev);
  // seq130（#1）：整体移动收尾（无论是否真的移动过都结束本次手势）。
  if (boxMove) {
    finishBoxMove();
    return;
  }
  // 识别多边形顶点拖动收尾。
  if (canvasTool === 'recognition' && polyDrag !== null) {
    polyDrag = null;
    if (polygon && polygon.length >= 3) roi = polygonBBox(polygon);
    updateRoiLabel();
    draw();
    if (roi) log(`顶点已调整：${polygon.length} 顶点，bbox ${roi.w}x${roi.h} @ ${roi.x},${roi.y}`);
    return;
  }
  // #10 工具模式：位移判定矩形/多边形。双击的第二次 mouseup 直接忽略。
  if (canvasTool !== 'none' && canvasGesture) {
    const gesture = canvasGesture;
    canvasGesture = null;
    dragStart = null;
    if (ev.detail >= 2) return; // dblclick 收尾
    const shape = canvasGestureShape(gesture.start, p);
    if (shape === 'rect') {
      if (canvasTool === 'recognition') {
        roi = normalizeRect(gesture.start, p);
        storedRecognitionPolygon = null;
        updateRoiLabel();
        draw();
        log(`识别区矩形 ${roi.w}x${roi.h} @ ${roi.x},${roi.y}（点「保存识别区」写入素材）`);
      } else {
        // seq136（UI B #2）：拖动=矩形快捷，但实际生成 4 点多边形（不保存 rect）。
        const quickRect = normalizeRect(gesture.start, p);
        const quickPoly = rectToPolygonPoints(quickRect);
        if (quickPoly) {
          actionPoly = quickPoly;
          actionPolyClosed = true;
          roiBack = null;
          log(`操作区域多边形（矩形快捷→4 点）${quickRect.w}x${quickRect.h} @ ${quickRect.x},${quickRect.y}`);
          finishActionGesture();
        } else {
          roiBack = quickRect;
          log(`操作区域矩形 ${roiBack.w}x${roiBack.h} @ ${roiBack.x},${roiBack.y}`);
          finishActionGesture();
        }
      }
      return;
    }
    // 单击 = 加多边形顶点（工具内）。
    if (canvasTool === 'recognition') {
      if (!polygon || polyClosed) {
        polygon = [];
        polyClosed = false;
        storedRecognitionPolygon = null;
        roi = null;
      }
      polygon.push({ x: p.x, y: p.y });
      if (polygon.length >= 3) roi = polygonBBox(polygon);
      updateRoiLabel();
      draw();
      log(`识别多边形：${polygon.length} 顶点（单击继续加点，双击收尾）`);
    } else {
      if (!actionPoly || actionPolyClosed) {
        actionPoly = [];
        actionPolyClosed = false;
      }
      actionPoly.push({ x: p.x, y: p.y });
      updateRoiLabel();
      draw();
      log(`操作区域多边形：${actionPoly.length} 顶点（单击继续加点，双击收尾）`);
    }
    return;
  }
  if (!dragStart) return;
  dragStart = null;
  if (dragTarget === 'back') {
    dragTarget = 'roi';
    finishActionGesture();
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

// §22.10 / #10：双击收尾 —— 识别工具：闭合多边形（闭合后可双击顶点删除）；
// 操作区域工具：闭合后立即落盘（候选行 or 素材 action_shape）。
canvas.addEventListener('dblclick', (ev) => {
  if (liveInputAllowed()) return;
  if (canvasTool === 'action') {
    if (!actionPoly || actionPoly.length < 3) return;
    ev.preventDefault();
    actionPolyClosed = true;
    const shape = currentActionShape();
    setCanvasTool('none');
    if (activeCandidate >= 0 && saveCandidates[activeCandidate]) {
      finishActionGesture(shape);
    } else if (pendingActionAsset) {
      // seq130-fix: 多边形闭合后不自动保存，等用户点「保存操作区域」。
      log(`操作区域多边形已闭合（素材「${pendingActionAsset}」）：点底部「保存操作区域」写入素材`);
      setStatus('操作区域未保存：点底部「保存操作区域」写入素材', '#e8c46a');
      updateAssetActionSaveButton();
    } else {
      resultEl.textContent =
        '操作区域多边形已闭合，但当前没有绑定候选/素材。\n' +
        '请先在识别结果点「调操作区域」，或在素材行点「操作区…→ 在画布上编辑」，再画多边形。';
      log('操作区域多边形已闭合，但未绑定候选/素材', 'WARN');
    }
    updateRoiLabel();
    draw();
    return;
  }
  if (canvasTool !== 'recognition' || !polygon || polygon.length < 3) return;
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
    `识别多边形已收尾：${polygon.length} 顶点，bbox ${roi.w}x${roi.h} @ ${roi.x},${roi.y}` +
      '（保存识别区时写 roi=bbox + polygon；双击可删顶点）'
  );
});

// #10：统一的三个工具按钮。识别区域/操作区域是互斥工具；反选只在操作区域工具内可用。
function toggleCanvasTool(tool) {
  const turningOff = canvasTool === tool;
  canvasGesture = null;
  polyDrag = null;
  dragStart = null;
  dragTarget = 'roi';
  canvasTool = 'none';
  if (!turningOff) {
    if (tool === 'action') {
      // 操作区域编辑接管画布：识别区编辑态先退出（其几何保留，可重新进入）。
      cancelRecognitionRoiEdit('切换到操作区域工具');
      resetActionGeometry({ keepTarget: true, clearInvert: false, redraw: false });
      log(
        `操作区域工具：拖动=矩形，单击=多边形顶点，双击收尾；反选=整个屏幕−区域` +
          (activeCandidate >= 0 ? '（候选已绑定）' : pendingActionAsset ? `（素材「${pendingActionAsset}」已绑定）` : '（未绑定候选/素材：先选中素材或候选）')
      );
    } else {
      if (pendingActionAsset) {
        pendingActionAsset = null;
        pendingActionAssetScope = null;
        updateAssetActionSaveButton();
      }
      // 切到识别区域工具：操作区域几何不再可编辑，清掉以免与识别框混淆。
      resetActionGeometry({ keepTarget: false, clearInvert: true, clearRect: true, clearDrag: true, redraw: false });
      log('识别区域工具：拖动=矩形，单击=多边形顶点，双击收尾；点画布底部「保存识别区」写入素材');
    }
    setCanvasTool(tool);
  } else {
    syncCanvasToolButtons();
    log('已退出画布工具');
  }
  updateRoiLabel();
  draw();
}

btnToolRecognition.addEventListener('click', () => toggleCanvasTool('recognition'));
btnToolAction.addEventListener('click', () => toggleCanvasTool('action'));

btnActionInvert.addEventListener('click', () => {
  if (canvasTool !== 'action') return; // 识别区域没有反选语义（按钮本应 disabled）
  actionInvert = !actionInvert;
  syncCanvasToolButtons();
  log(`反选：${actionInvert ? '开（有效区=整个屏幕−区域）' : '关'}`);
  updateRoiLabel();
  draw();
});

btnClear.addEventListener('click', () => {
  // P0-3 / P1-8: the clear button must clear BOTH state machines, invert and
  // cached recognition/test state. Old polygons were previously left behind and
  // silently applied to the next candidate.
  // B/P0-2: a cleared canvas has no box to save; leave the recognition-ROI
  // editor through the shared cancel path (hides/disables the save button).
  cancelRecognitionRoiEdit('清除识别/操作区域');
  resetSelection({ clearInvert: true, clearRoi: true, clearBoxes: true, redraw: false });
  activeCandidate = -1;
  pendingActionAsset = null;
  pendingActionAssetScope = null;
  updateAssetActionSaveButton();
  lastLinesRoi = null;
  replaceResults([], '');
  updateRoiLabel();
  updateClickStepButtons();
  draw();
  resultEl.textContent = '—';
  setStatus('已清除识别/操作区域与选择状态');
});

// U-T4/S2：识别文字对话框按钮 + 新建时 param_key 随素材 id 自动生成。
if (assetTextSave) {
  assetTextSave.addEventListener('click', () => saveAssetTextDialog());
}
if (assetTextCancel) {
  assetTextCancel.addEventListener('click', () => closeAssetTextDialog());
}
// seq131：素材设置→加入集合 对话框。
if (collectionAddSave) collectionAddSave.addEventListener('click', () => saveCollectionAddDialog());
if (collectionAddCancel) collectionAddCancel.addEventListener('click', () => closeCollectionAddDialog());
if (collectionAddTarget) collectionAddTarget.addEventListener('change', syncCollectionAddFields);
if (assetTextFullScreen) {
  assetTextFullScreen.addEventListener('change', () => {
    if (!assetTextDialogCtx) return;
    if (assetTextRoi) assetTextRoi.hidden = assetTextFullScreen.checked;
  });
}
if (assetTextId) {
  assetTextId.addEventListener('input', () => {
    const ctx = assetTextDialogCtx;
    if (!ctx || ctx.draft.mode !== 'create' || !assetTextParamKey) return;
    const id = String(assetTextId.value !== undefined ? assetTextId.value : '').trim();
    const oldAuto = autoParamKey(ctx.draft.id);
    if (!assetTextParamKey.value || assetTextParamKey.value === oldAuto) {
      assetTextParamKey.value = id ? autoParamKey(id) : '';
    }
  });
}

// U-T4/S5：操作区递归对话框按钮。
if (opRegionSave) {
  opRegionSave.addEventListener('click', () => saveOpRegionDialog());
}
if (opRegionCancel) {
  opRegionCancel.addEventListener('click', () => closeOpRegionDialog());
}

// P2-4: keep the full-screen fallback on the SAME frame the failed single-line
// probe ran on. Without frameId it silently fell back to the editor's current
// frame, which can differ from the frame-detail row that was clicked (stale row,
// another tab). `it` may be null (main OCR button), so `it`'s positions are only
// a last resort.
async function fallbackFullscreenLines(reason, it, frameId) {
  if (!detReady) {
    log('OCR 全屏回退不可用：无 det model', 'ERROR', { reason });
    resultEl.textContent = `单行识别失败（${reason}）；多行识别不可用（无 det model）`;
    return false;
  }
  const fw = canvas.width || (img && img.naturalWidth) || (roi && roi.x + roi.w) || 1280;
  const fh = canvas.height || (img && img.naturalHeight) || (roi && roi.y + roi.h) || 720;
  const full = { x: 0, y: 0, w: fw, h: fh };
  resultEl.textContent = `单行识别失败（${reason}），改用全屏多行识别…`;
  log(`OCR 全屏回退开始`, 'WARN', { reason, full_roi: full, frames: testFrameLabel(it, frameId) });
  const r = await fetch(`/lines?x=0&y=0&w=${fw}&h=${fh}${testFrameQuery(it, frameId)}`);
  const s = await r.json();
  if (s.error || !Array.isArray(s.lines)) {
    log('OCR 全屏回退失败', 'ERROR', { reason, error: s.error || '无结果' });
    resultEl.textContent = `全屏多行回退失败：${s.error || '无结果'}（帧来源：${s.frame_source || testFrameLabel(it, frameId)}）`;
    return false;
  }
  const lines = s.lines.map((b) => ({
    ...b,
    box: [b.box[0], b.box[1], b.box[2], b.box[3]],
  }));
  log('OCR 全屏回退成功', 'INFO', {
    reason, full_roi: full, count: lines.length,
    elapsed_ms: s.elapsed_ms,
    frame_source: s.frame_source || testFrameLabel(it, frameId),
    lines: lines.map((l) => ({ text: l.text, score: l.score })),
  });
  renderOcrLines(lines, full, '全屏多行回退');
  resultEl.textContent =
    `单行识别失败（${reason}）→ 全屏多行识别 ${lines.length} 行（帧来源：${s.frame_source || testFrameLabel(it, frameId)}）；` +
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
    // P1-6: the main OCR button must test the live frame while live preview is
    // on (or the explicit current frame when it is off), not stale State.frame.
    const q = `x=${roi.x}&y=${roi.y}&w=${roi.w}&h=${roi.h}${testFrameQuery()}`;
    const r = await fetch('/ocr?' + q);
    const s = await r.json();
    const frameSource = s.frame_source || testFrameLabel();
    if (s.error || !String(s.text || '').trim()) {
      if (await fallbackFullscreenLines(s.error || 'ROI 内没有文字', null)) return;
      resultEl.textContent = `错误：${s.error || 'ROI 内没有文字'}\n帧来源：${frameSource}`;
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
      resultEl.textContent =
        `文字 = ${JSON.stringify(s.text)}\n得分 = ${s.score.toFixed(4)}\n` +
        `耗时 = ${s.elapsed_ms.toFixed(1)} ms\n帧来源 = ${frameSource}`;
      log(`OCR → ${s.text} (${s.score.toFixed(3)})`, 'INFO', { frame: frameSource });
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
    '（已生成「图片匹配」候选；到「识别结果」勾选后点「保存素材」）';
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

if (btnTestMode) {
  btnTestMode.addEventListener('click', () => {
    if (!assetTestReal) {
      const ok = window.confirm(
        '切换到素材测试「真实点击」模式？\n\n' +
        '切换后，素材行的「点击测试/拖动测试」会操作真实设备；\n' +
        '必须开启实时预览，每次真正发送前还会再次确认。'
      );
      if (!ok) return;
      assetTestReal = true;
      log('素材测试模式：真实点击（每次发送前仍会二次确认）', 'WARN');
    } else {
      assetTestReal = false;
      log('素材测试模式：演练（不会操作设备）');
    }
    updateAssetTestModeButton();
  });
}
updateAssetTestModeButton();

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
    // P1-6: same frame-source contract as the single-line OCR button.
    const q = `x=${roi.x}&y=${roi.y}&w=${roi.w}&h=${roi.h}${testFrameQuery()}`;
    const r = await fetch('/lines?' + q);
    const s = await r.json();
    const frameSource = s.frame_source || testFrameLabel();
    if (s.error) {
      resultEl.textContent = '错误：' + s.error + `\n帧来源：${frameSource}`;
      log('多行识别错误：' + s.error, 'ERROR', { frame: frameSource });
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
      resultEl.textContent =
        `${boxes.length} 行 / ${s.elapsed_ms.toFixed(0)} ms\n帧来源 = ${frameSource}\n` + txt;
      renderOcrLines(boxes, lastLinesRoi, '本次多行识别');
      log(`多行识别 → ${boxes.length} 行（可直接点击任一行）`, 'INFO', { frame: frameSource });
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
    // seq138 V2-C：地图是工作台最后一个 tab；激活时展开大工作区并共享当前帧。
    document.body.classList.toggle('map-tab-active', panelId === 'panel-map');
    if (window.OAS_MAP_UI) {
      if (panelId === 'panel-map' && typeof window.OAS_MAP_UI.onActivate === 'function') {
        window.OAS_MAP_UI.onActivate();
      } else if (typeof window.OAS_MAP_UI.onDeactivate === 'function') {
        window.OAS_MAP_UI.onDeactivate();
      }
    }
    // seq130（#2）：进入点击分布面板时刷新运行记录（2s 节流）并重绘叠加。
    if (panelId === 'panel-click') {
      const now = Date.now();
      if (now - lastClicksPanelRefreshAt > 2000) {
        lastClicksPanelRefreshAt = now;
        refreshRuns().catch(function (e) {
          log('点击分布：进入面板刷新运行记录失败：' + (e && e.message ? e.message : e));
        });
      }
      if (clickData && typeof draw === 'function') draw();
      return;
    }
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
  // §22.28 任务素材列表（搜索/分类）+ 全局素材库（同一套过滤函数）
  if (btnAssetsRefresh) {
    btnAssetsRefresh.addEventListener('click', async () => {
      await refreshTaskAssets((taskName.value || '').trim() || undefined);
      await refreshGlobalAssets();
    });
  }
  if (btnAssetsBatchTest) btnAssetsBatchTest.addEventListener('click', batchTestAssets);
  if (assetSearch) assetSearch.addEventListener('input', renderAssetList);
  if (assetCategoryFilter) assetCategoryFilter.addEventListener('change', renderAssetList);
  if (globalAssetSearch) globalAssetSearch.addEventListener('input', renderGlobalAssets);
  if (globalAssetCategoryFilter) globalAssetCategoryFilter.addEventListener('change', renderGlobalAssets);
  if (railButtons.length) activatePanel(railButtons[0].dataset.panel);
  // 深链 ?panel=flow / ?panel=task 供验收/直达使用
  try {
    const wantPanel = new URLSearchParams(location.search).get('panel');
    const ok = ['panel-assets', 'panel-images', 'panel-click', 'panel-task'];
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
  // 深链 ?reset=1：Runner-only 操作，Studio 不再提供
  try {
    if (new URLSearchParams(location.search).get('reset') === '1') {
      log('重置记忆属于运行运维，请在 Runner（8789）中操作。', 'WARN');
    }
  } catch (e) {
    /* ignore */
  }

  btnSaveAsset.addEventListener('click', saveAssetUnified);
  // B: save the canvas-drawn recognition box into the asset being edited
  // (template => re-crop overwrite rev+1; OCR/region => roi update).
  if (btnSaveAssetRoi) {
    btnSaveAssetRoi.addEventListener('click', () => {
      saveAssetRecognitionRoi().catch((e) => {
        resultEl.textContent = '保存识别区失败：' + e;
        log('保存识别区失败：' + e, 'ERROR');
      });
    });
  }
  // seq130-fix: 操作区域显式保存（与识别区域同款按钮）。
  if (btnSaveAssetActionRoi) {
    btnSaveAssetActionRoi.addEventListener('click', () => {
      if (!pendingActionAsset) return;
      const id = pendingActionAsset;
      if (!currentActionShape()) {
        resultEl.textContent = '还没有画操作区域：请先在画布上拖动/点击画框';
        return;
      }
      updateAssetActionRoi(id).catch((e) => {
        resultEl.textContent = '保存操作区域失败：' + e;
        log('保存操作区域失败：' + e, 'ERROR');
      });
    });
  }
  // P2-1 / #10: Esc 放弃本次绘制并退出画布工具（asset_ui.js 已负责关设置菜单）。
  // 打开着模态对话框时不抢 Esc。
  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    if (document.querySelector && document.querySelector('dialog[open]')) return;
    if (editingRecognitionAsset) {
      cancelRecognitionRoiEdit('Esc');
      return;
    }
    if (pendingActionAsset) {
      pendingActionAsset = null;
      pendingActionAssetScope = null;
      updateAssetActionSaveButton();
      resetActionGeometry({ keepTarget: false, clearInvert: true, clearRect: true, redraw: false });
      setCanvasTool('none');
      updateRoiLabel();
      draw();
      setStatus('已取消操作区域编辑（未写入素材）');
      log('Esc：已取消操作区域编辑（未写入素材）');
      return;
    }
    if (canvasTool !== 'none') {
      canvasGesture = null;
      polyDrag = null;
      dragStart = null;
      dragTarget = 'roi';
      setCanvasTool('none');
      updateRoiLabel();
      draw();
      log('Esc：已退出画布工具（本次绘制放弃）');
      return;
    }
    // seq137：即使只是选中素材预览，Esc 也统一清识别区+操作区。
    if (roi || polygon || storedRecognitionPolygon || actionPoly || roiBack || activeCandidate >= 0) {
      clearCanvasSelection('Esc');
      setStatus('已清空画布识别区/操作区');
    }
  });
  // 初始按钮状态：反选在未进入操作区域工具前禁用。
  syncCanvasToolButtons();
  taskName.addEventListener('change', async () => {
    // P0-2: the asset caches are about to be replaced with another task's rows.
    clearCanvasSelection('切换任务');
    cancelRecognitionRoiEdit('切换任务');
    syncTaskSelect(taskName.value);
    await refreshTaskAssets((taskName.value || '').trim() || undefined);
    await loadTaskFlow((taskName.value || '').trim());
  });
  // --- N38/M1: 任务列表 / 新建任务 / 运行任务 ---
  if (btnTaskRefresh) btnTaskRefresh.addEventListener('click', () => refreshTaskList());
  if (taskSelect) {
    taskSelect.addEventListener('change', async () => {
      const name = (taskSelect.value || '').trim();
      if (!name) return;
      clearCanvasSelection('切换任务');
      cancelRecognitionRoiEdit('切换任务');
      taskName.value = name;
      await refreshTaskAssets(name);
      await loadTaskFlow(name);
      setRunStatus(`已选择任务「${name}」；点「运行任务」开始一次运行。`);
    });
  }
  if (btnTaskCreate) btnTaskCreate.addEventListener('click', openCreateTaskDialog);
  if (btnCreateSubmit) btnCreateSubmit.addEventListener('click', submitCreateTask);
  if (btnCreateCancel) btnCreateCancel.addEventListener('click', closeCreateTaskDialog);
  // PR5: 发布任务包对话框（发布 / 关闭 / Esc；发布中禁用发布按钮）。
  if (taskPublishSubmit) taskPublishSubmit.addEventListener('click', submitTaskPublish);
  if (taskPublishDialog) {
    taskPublishDialog.addEventListener('click', (ev) => {
      const target = ev.target;
      if (target && target.getAttribute && target.getAttribute('data-publish-close') === '1') {
        closeTaskPublishDialog();
      }
    });
    taskPublishDialog.addEventListener('close', () => {
      taskPublishTask = '';
      // Never clear taskPublishBusy here: an in-flight publish still owns the
      // busy flag until its finally block runs.
    });
  }
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
  // §22.19 长跑统计
  if (btnStatsRefresh) btnStatsRefresh.addEventListener('click', () => loadStats());
  await loadStats();
  // §22.22 全局素材库（旧 library/ UI 已移除；此按钮现在真实存在）
  if (btnLibraryRefresh) {
    btnLibraryRefresh.addEventListener('click', () => { refreshGlobalAssets(); });
  }
  if (taskShowAllAssetsEl) {
    taskShowAllAssets = !!taskShowAllAssetsEl.checked;
    taskShowAllAssetsEl.addEventListener('change', async () => {
      taskShowAllAssets = !!taskShowAllAssetsEl.checked;
      await refreshTaskAssets((taskName.value || '').trim() || undefined);
    });
  }
  await loadGlobalAssets();
  // §22.27 会话图片列表
  if (btnFrameUpload) btnFrameUpload.addEventListener('click', uploadFrames);
  if (btnFrameCapture) btnFrameCapture.addEventListener('click', openCaptureFrameDialog);
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
  // 帧搜索只过滤本地 framesCache；不改变 /frames 请求，也不重置分类。
  if (frameSearch) {
    frameSearch.addEventListener('input', () => renderFramesList());
    frameSearch.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        frameSearch.value = '';
        renderFramesList();
      }
    });
  }
  if (frameSearchClear) {
    frameSearchClear.addEventListener('click', () => {
      if (!frameSearch) return;
      frameSearch.value = '';
      renderFramesList();
      frameSearch.focus();
    });
  }
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
      scheduleCanvasFit();
    });
  }
  if (canvasStage && window.ResizeObserver) {
    // Never fit synchronously inside the observer callback: that is what makes
    // Chromium emit "ResizeObserver loop completed with undelivered
    // notifications". The rAF scheduler coalesces this with other fits.
    new ResizeObserver(() => scheduleCanvasFit()).observe(canvasStage);
  }
  window.addEventListener('resize', () => scheduleCanvasFit());
  // Boot is not inside a ResizeObserver callback, so an immediate fit avoids a
  // first-paint jump; later fits are rAF-coalesced and idempotent.
  fitCanvasToStage();
  await refreshFrames();
  // N2: 生成点击指令 / 追加到 flow
  if (btnClickStep) btnClickStep.addEventListener('click', doGenerateClickStep);
  if (btnAppendStep) btnAppendStep.addEventListener('click', doAppendStepToFlow);
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

  // §22.18: load the task's existing flow into the visual editor; `?task=<name>`
  // is a deep link used by the headless acceptance run.
  let deepTask = '';
  try {
    deepTask = new URLSearchParams(location.search).get('task') || '';
  } catch (e) {
    deepTask = '';
  }
  if (deepTask) taskName.value = deepTask;
  // Boot ordering: populate #task-select BEFORE painting the task-scoped panels.
  // refreshTaskList() auto-selects tasks[0] and mirrors it into #task-name; when
  // the assets loaded first they saw only the empty placeholder option, so the
  // panel stayed on "未选择任务" until the user switched tasks manually.
  // 先填任务下拉框，再按它的选中值加载素材/流程。
  await refreshTaskList(taskName.value.trim());
  let bootTask = ((taskSelect && taskSelect.value) || '').trim();
  // A ?task= deep link stays usable when /task/list could not resolve a real
  // option (route failure); with no deep link bootTask stays empty (无任务状态).
  if (!bootTask && deepTask) bootTask = deepTask;
  try {
    await refreshTaskAssets(bootTask || undefined);
    await loadTaskFlow(bootTask);
    // seq130（#4）：任务页可见/聚焦/定时自动刷新素材；隐藏时暂停。
    lastAssetsRefreshAt = Date.now();
    startAssetsAutoRefresh();
  } catch (e) {
    log('任务素材读取失败：' + e);
  }

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
  if (settingsRefresh) settingsRefresh.addEventListener('click', refreshSettings);
  if (libNew) {
    libNew.addEventListener('click', () => {
      activatePage('workbench');
      openCreateTaskDialog();
    });
  }
  try {
    const wantPage = new URLSearchParams(location.search).get('page');
    if (wantPage === 'schedule') {
      // PR6: the Studio scheduling page is gone; keep old deep links working by
      // opening the Runner scheduling page instead of an empty pane.
      log('提示：调度页已迁移到 Runner（8789）；已打开 Runner 的「队列与调度」页。', 'WARN');
      openRunnerPage('#queue');
    } else if (wantPage && ['workbench', 'builder', 'tasks', 'instances', 'settings'].includes(wantPage)) {
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
