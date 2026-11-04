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
const btnMatch = document.getElementById('btn-match');
const taskName = document.getElementById('task-name');
const btnSaveAsset = document.getElementById('btn-save-asset');
// B: canvas recognition-ROI editor (material library 识别区). Disabled/hidden
// until a row enters edit mode; templates save through the overwrite path.
const btnSaveAssetRoi = document.getElementById('btn-save-asset-roi');
// P0-1: visible label next to the save button naming the asset being edited;
// the button itself lives in #canvas-footer so every surface can see it.
const assetRoiEditLabel = document.getElementById('asset-roi-edit-label');
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
const pageSchedSummary = document.getElementById('page-sched-summary');
const pageSchedEntries = document.getElementById('page-sched-entries');
const pageSchedRefresh = document.getElementById('page-sched-refresh');
const pageSchedReset = document.getElementById('page-sched-reset');
// P3: editable scheduler page (status card / entries / queues / advanced JSON).
const pageSchedMode = document.getElementById('page-sched-mode');
const pageSchedPasses = document.getElementById('page-sched-passes');
const pageSchedNextWake = document.getElementById('page-sched-next-wake');
const pageSchedRunning = document.getElementById('page-sched-running');
const pageSchedQuota = document.getElementById('page-sched-quota');
const pageSchedConfigStatus = document.getElementById('page-sched-config-status');
const pageSchedSave = document.getElementById('page-sched-save');
const pageSchedReload = document.getElementById('page-sched-reload');
const pageSchedAddEntry = document.getElementById('page-sched-add-entry');
const pageSchedQueues = document.getElementById('page-sched-queues');
const pageSchedAddQueue = document.getElementById('page-sched-add-queue');
const pageSchedJson = document.getElementById('page-sched-json');
const pageSchedJsonSave = document.getElementById('page-sched-json-save');
const pageSchedJsonReload = document.getElementById('page-sched-json-reload');
const pageSchedJsonStatus = document.getElementById('page-sched-json-status');
// P4: scheduler run history (list / filters / detail dialog).
const pageSchedRunsSummary = document.getElementById('page-sched-runs-summary');
const pageSchedRunsRefresh = document.getElementById('page-sched-runs-refresh');
const pageSchedRunsTask = document.getElementById('page-sched-runs-task');
const pageSchedRunsOutcome = document.getElementById('page-sched-runs-outcome');
const pageSchedRunsLimit = document.getElementById('page-sched-runs-limit');
const pageSchedRunsClear = document.getElementById('page-sched-runs-clear');
const pageSchedRunsStatus = document.getElementById('page-sched-runs-status');
const pageSchedRunsList = document.getElementById('page-sched-runs-list');
const schedRunDialog = document.getElementById('page-sched-runs-dialog');
const schedRunDialogTitle = document.getElementById('page-sched-runs-detail-title');
const schedRunDialogBody = document.getElementById('page-sched-runs-detail-body');
const schedRunDialogError = document.getElementById('page-sched-runs-detail-error');
const schedRunDialogRefresh = document.getElementById('page-sched-runs-detail-refresh');
const schedRunDialogClose = document.getElementById('page-sched-runs-detail-close');
const btnSchedOpenPage = document.getElementById('btn-sched-open-page');
const schedEntryDialog = document.getElementById('sched-entry-dialog');
const schedEntryTitle = document.getElementById('sched-entry-title');
const schedEntryModeHint = document.getElementById('sched-entry-mode-hint');
const schedEntryKind = document.getElementById('sched-entry-kind');
const schedEntryTarget = document.getElementById('sched-entry-target');
const schedEntryIdEl = document.getElementById('sched-entry-id');
const schedEntryEnabled = document.getElementById('sched-entry-enabled');
const schedEntryLifecycle = document.getElementById('sched-entry-lifecycle');
const schedEntryTempHint = document.getElementById('sched-entry-temp-hint');
const schedEntryWindows = document.getElementById('sched-entry-windows');
const schedEntryWindowsJson = document.getElementById('sched-entry-windows-json');
const schedEntryQuotaPer = document.getElementById('sched-entry-quota-per');
const schedEntryQuotaTimes = document.getElementById('sched-entry-quota-times');
const schedEntryRetry = document.getElementById('sched-entry-retry');
const schedEntryCooldown = document.getElementById('sched-entry-cooldown');
const schedEntryParams = document.getElementById('sched-entry-params');
const schedEntryError = document.getElementById('sched-entry-error');
const schedEntryCancel = document.getElementById('sched-entry-cancel');
const schedEntrySave = document.getElementById('sched-entry-save');
const schedQueueDialog = document.getElementById('sched-queue-dialog');
const schedQueueTitle = document.getElementById('sched-queue-title');
const schedQueueId = document.getElementById('sched-queue-id');
const schedQueueName = document.getElementById('sched-queue-name');
const schedQueueMemberAddBtn = document.getElementById('sched-queue-member-add');
const schedQueueMembers = document.getElementById('sched-queue-members');
const schedQueueTaskPool = document.getElementById('sched-queue-task-pool');
const schedQueueTotalRounds = document.getElementById('sched-queue-total-rounds');
const schedQueueOnFailure = document.getElementById('sched-queue-on-failure');
const schedQueueBetweenMs = document.getElementById('sched-queue-between-ms');
const schedQueueRoundsHint = document.getElementById('sched-queue-rounds-hint');
const schedQueueLockHint = document.getElementById('sched-queue-lock-hint');
const schedQueueError = document.getElementById('sched-queue-error');
const schedQueueCancel = document.getElementById('sched-queue-cancel');
const schedQueueSave = document.getElementById('sched-queue-save');
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
// P3: full scheduler.json document + the GET /scheduler/config response shape.
let scheduleConfig = null;
let scheduleConfigMeta = null;
let scheduleConfigError = '';
// P1-3: a failed /scheduler/status read must be visible on the status card; the
// page must never present passes=0 / 锁=空闲 / quota：无限制 for an unknown state.
let scheduleStatusError = '';
// P1-4: canonical JSON of the GET /scheduler/config document this page was loaded
// from. The toolbar save compares a fresh GET against it before POSTing, so a
// stale in-memory snapshot can never overwrite an external edit.
let scheduleConfigBase = '';
let scheduleTasksCache = [];
let scheduleBusy = false;
let scheduleJsonDirty = false;
let scheduleEntryDialogState = null;
let scheduleQueueDialogState = null;
// P4: scheduler run history state. `scheduleRunsFilter.queueId` is a local-only
// filter (the backend list endpoint filters by task/outcome only); it is set by
// a queue row's 「运行记录」 shortcut and cleared by the clear button.
let scheduleRunsData = null;
let scheduleRunsError = '';
let scheduleRunsLoading = false;
let scheduleRunsFilter = { limit: 200, task: '', outcome: '', queueId: '' };
let scheduleRunDetailRunId = '';
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
  // Draw order (§21): frame -> ROI/boxes -> click heatmap + scatter. Click
  // metrics/legend deliberately stay in the sidebar (#click-stats /
  // #clicks-legend): no on-canvas plate covers the game's top-left corner.
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
    scheduleTasksCache = (s.tasks || []).map((t) => t.name).filter(Boolean);
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
      // P3: edit this task's scheduler entry (create / update via /scheduler/config).
      const sched = document.createElement('button');
      sched.type = 'button';
      sched.textContent = '调度设置…';
      sched.title = '为该任务加入调度，或编辑它已有的调度条目';
      sched.addEventListener('click', () => openTaskScheduleDialog(t.name));
      // P4: jump to the schedule page's run history filtered to this task.
      const runsBtn = document.createElement('button');
      runsBtn.type = 'button';
      runsBtn.textContent = '运行记录';
      runsBtn.title = '打开调度页并筛选该任务的运行记录';
      runsBtn.addEventListener('click', () => showScheduleRunsForTask(t.name));
      const del = document.createElement('button');
      del.type = 'button';
      del.textContent = '删除';
      del.addEventListener('click', () => deleteTaskFromLibrary(t.name));
      card.append(meta, open, sched, runsBtn, del);
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
  setScheduleBusy(true);
  try {
    let status = null;
    // P1-3: remember a failed status read so the card can never claim "正常".
    scheduleStatusError = '';
    try {
      const r = await fetch('/scheduler/status');
      let body = null;
      try { body = await r.json(); } catch (e) { body = null; }
      if (!r.ok || !body || body.ok === false) {
        scheduleStatusError = (body && Array.isArray(body.errors) && body.errors.length)
          ? body.errors.join('；')
          : (r.ok ? '响应不是 JSON 对象' : `HTTP ${r.status}`);
        log(schedStatusErrorText(scheduleStatusError), 'ERROR');
      } else {
        status = body;
      }
    } catch (e) {
      scheduleStatusError = String(e);
      log(schedStatusErrorText(e), 'WARN');
    }
    const cfg = await loadScheduleConfig(true);
    if (status) schedulerStatus = status;
    // P4: run history is a soft dependency — a failed read is rendered in its
    // own section and must never abort the status card / entries / queues.
    populateScheduleRunsTaskOptions();
    await refreshScheduleRuns();
    syncScheduleRunsControls();
    renderScheduleStatusCard(status);
    if (cfg) {
      if (pageSchedJson && !scheduleJsonDirty) {
        pageSchedJson.value = JSON.stringify(cfg, null, 2);
        if (pageSchedJsonStatus) pageSchedJsonStatus.textContent = '已从磁盘载入';
      }
    } else if (pageSchedJson && !scheduleJsonDirty) {
      pageSchedJson.value = '';
    }
    renderScheduleEntries(status, cfg);
    renderScheduleQueues(status, cfg);
  } catch (e) {
    if (pageSchedSummary) pageSchedSummary.textContent = '读取异常：' + e;
    log('刷新调度页异常：' + e, 'ERROR');
  } finally {
    setScheduleBusy(false);
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
    // P0-2: a fresh recognition result follows the real recognized bbox until
    // the user explicitly draws/drags an operation area. `actionShape` is only
    // written when that explicit edit happens (see actionSource below).
    actionShape: null,
    actionSource: 'recognition',
    sourceRoi: sourceRoi ? { ...sourceRoi } : null,
    selected: selected !== false && candidateNameValid(name || text || ''),
    fromLines: !!fromLines,
  };
}

function replaceResults(rows, sourceLabel) {
  saveCandidates = rows || [];
  activeCandidate = -1;
  pendingActionAsset = null;
  // New recognition results must never inherit an old action polygon/invert
  // from a previous candidate or asset (P0-3 / P1-8). The rect too: a fresh
  // candidate follows its own recognized bbox.
  resetActionGeometry({ keepTarget: true, clearInvert: true, clearRect: true, redraw: false });
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
      `识别 [${r.x},${r.y},${r.w},${r.h}] · 操作 [${a.x},${a.y},${a.w},${a.h}] · ` +
      `${c.actionSource === 'fixed' ? '固定操作区' : '跟随识别框（recognition）'}`;

    const actions = document.createElement('div');
    actions.className = 'candidate-actions';
    const adjust = document.createElement('button');
    adjust.type = 'button';
    adjust.textContent = '调操作区域';
    adjust.title = '在画布上拖拽/画多边形设置该素材的固定操作区域（默认=跟随识别区域）';
    adjust.addEventListener('click', () => {
      // P1-8: entering a mode resets stale geometry from the previous target.
      resetActionGeometry({ keepTarget: true, clearInvert: true, redraw: false });
      activeCandidate = idx;
      setSelectionMode('actionRect');
      pendingActionAsset = null;
      // Show the candidate's existing fixed shape (if any) while editing.
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
    reset.title = '操作区域恢复为跟随识别框（recognition）';
    reset.addEventListener('click', () => {
      c.action = { ...c.recognize };
      c.actionShape = null;
      c.actionSource = 'recognition';
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
    actions.append(adjust, reset, gen, append);
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

// P0-2: only an explicit user edit (`actionSource === 'fixed'`) may persist
// action_shape. Everything else is `recognition` and must not carry a shape.
// region assets keep their operation-area semantics: their `roi` *is* the
// operation region, and the server normalizes them to fixed+rect(roi).
function candidateActionPayload(c) {
  if (!c || c.kind === 'region') return {};
  if (c.actionSource === 'fixed' && c.actionShape) {
    return { action_shape: c.actionShape, action_source: 'fixed' };
  }
  return { action_source: 'recognition' };
}

// P1-8: the editor has two geometries (recognition polygon mask vs. action
// region shape) but they must never be drawable at the same time. These helpers
// are the single convergence point:
//   - `selectionModeFromFlags` is pure and mirrors the mousedown dispatch order;
//   - `setSelectionMode` is the ONLY place that flips the three mode flags;
//   - `syncSelectionButtons` is the only place that writes the mode button text.
// Geometry is cleared separately by resetSelection/resetActionGeometry.
const SELECTION_MODES = ['none', 'recognitionPolygon', 'actionPolygon', 'actionRect'];

function selectionModeFromFlags(polyModeFlag, actionPolyModeFlag, backModeFlag) {
  if (actionPolyModeFlag) return 'actionPolygon';
  if (polyModeFlag) return 'recognitionPolygon';
  if (backModeFlag) return 'actionRect';
  return 'none';
}

function selectionActiveMode() {
  return selectionModeFromFlags(polyMode, actionPolyMode, backMode);
}

function syncSelectionButtons() {
  if (btnPoly) {
    btnPoly.textContent = polyMode
      ? '退出多边形模式（点顶点，双击闭合；闭合后可拖/双击删顶点）'
      : '多边形框选（点顶点，双击闭合）';
  }
  if (btnActionPoly) {
    btnActionPoly.textContent = actionPolyMode
      ? '退出操作区域多边形（点顶点，双击闭合）'
      : '操作区域多边形（点顶点，双击闭合）';
  }
  if (btnBackMode) {
    btnBackMode.textContent = backMode
      ? '退出操作区域编辑（拖拽画操作区域）'
      : '绘制操作区域（默认=识别区域）';
  }
}

// Enter/leave a selection mode. The guards keep the states mutually exclusive:
// no stale action polygon while the recognition polygon mode is active, and no
// stale rect mode while a polygon mode is active.
function setSelectionMode(mode) {
  const m = SELECTION_MODES.includes(mode) ? mode : 'none';
  polyMode = m === 'recognitionPolygon';
  actionPolyMode = m === 'actionPolygon';
  backMode = m === 'actionRect';
  syncSelectionButtons();
  return m;
}

function resetActionGeometry(opts) {
  const o = opts || {};
  actionPoly = null;
  actionPolyClosed = false;
  setSelectionMode(selectionModeFromFlags(polyMode, false, false));
  if (o.clearInvert) {
    actionInvert = false;
    if (btnActionInvert) btnActionInvert.textContent = '操作区域反选：关';
  }
  if (!o.keepTarget) {
    activeCandidate = -1;
    pendingActionAsset = null;
  }
  if (o.clearDrag) {
    dragStart = null;
    dragTarget = 'roi';
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
  setSelectionMode('none');
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
  return actionShapeFromGeometry(actionPoly, actionPolyClosed, roiBack, actionInvert);
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
  if (btnActionInvert) btnActionInvert.textContent = `操作区域反选：${actionInvert ? '开' : '关'}`;
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

function rectsOverlap(a, b) {
  return a[0] < b[0] + b[2] && a[0] + a[2] > b[0] && a[1] < b[1] + b[3] && a[1] + a[3] > b[1];
}

function actionValidity(it, recognizedBox, shapeOverride) {
  if (!recognizedBox) return { valid: null, text: '无法计算：没有真实识别框' };
  // P1-1: template callers pass the translated shape (`matched - roi`); the
  // runtime uses that same translation. Other callers may omit it.
  const shape = shapeOverride !== undefined ? shapeOverride : it.action_shape;
  const source = it.action_source || 'recognition';
  if (!shape) {
    if (source === 'recognition') return { valid: true, text: '有效（操作区=真实识别框）' };
    return { valid: false, text: '无效：action_source=fixed 但没有 action_shape' };
  }
  const isRect = shape.kind === 'rect' && Array.isArray(shape.roi);
  const hit = shape.invert
    ? false
    : isRect
      ? rectsOverlap(recognizedBox, shape.roi)
      : polygonOverlapsRect(shape.points || [], recognizedBox);
  if (!shape.invert) {
    if (source === 'fixed') {
      return {
        valid: true,
        text: hit ? '有效（固定操作区与识别框重叠）' : '独立操作区域（fixed，允许不重叠）',
        independent: !hit,
      };
    }
    return { valid: hit, text: hit ? '有效（识别框在操作区内）' : '无效：真实识别框与操作区不重叠' };
  }
  // Inverted shape: effective region is the whole screen minus shape. A
  // recognized box overlaps it if any corner is outside the excluded shape.
  const corners = [
    [recognizedBox[0], recognizedBox[1]],
    [recognizedBox[0] + recognizedBox[2] - 1, recognizedBox[1]],
    [recognizedBox[0], recognizedBox[1] + recognizedBox[3] - 1],
    [recognizedBox[0] + recognizedBox[2] - 1, recognizedBox[1] + recognizedBox[3] - 1],
  ];
  const outside = corners.some(([x, y]) => !actionShapeContains(shape, x, y));
  if (source === 'fixed') {
    return {
      valid: true,
      text: outside ? '有效（反选固定操作区与识别框相交）' : '独立操作区域（反选；识别框完全在排除区内）',
      independent: !outside,
    };
  }
  return { valid: outside, text: outside ? '有效（反选操作区与识别框相交）' : '无效：真实识别框完全落在反选排除区内' };
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
  if (!Array.isArray(it.roi)) return it.action_shape;
  return translateActionShape(
    it.action_shape,
    Number(match.x) - it.roi[0],
    Number(match.y) - it.roi[1]
  );
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
  const r = await postJson(req.url, req.payload);
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

async function testAssetClick(it, frameId, scopeTask) {
  cancelRecognitionRoiEdit('点击测试');
  const mode = realAssetTestGuard();
  if (!mode.ok) return;
  const task = assetRowScopeTask(scopeTask, false);
  const frames = testFrames(it, frameId);
  const details = [];
  const boxesForFrames = [];
  const realOps = [];
  try {
    for (const frame of frames) {
      if (it.kind === 'template') {
        if (!Array.isArray(it.roi)) {
          details.push(`${frame.label}: ❌ 素材没有 roi`);
          continue;
        }
        const r = await fetch(
          `/task/asset/match?task=${encodeURIComponent(task)}&id=${encodeURIComponent(it.id)}${frame.query}`
        );
        const s = await r.json();
        if (s.error) {
          details.push(`${frame.label}: ❌ ${s.error}`);
          continue;
        }
        const recognizedBox = [Number(s.x), Number(s.y), Number(s.w), Number(s.h)];
        if (!matchBoxValid(recognizedBox)) {
          details.push(`${frame.label}: ❌ 匹配坐标无效 (${s.x},${s.y},${s.w},${s.h})`);
          continue;
        }
        // P1-1: translate the saved action_shape exactly like the runtime does.
        const effShape = translatedActionShapeForMatch(it, s);
        const action = assetActionBox(it, effShape, recognizedBox) || recognizedBox;
        const point = randomPointInAction(it, effShape, recognizedBox);
        const validity = actionValidity(it, recognizedBox, effShape);
        const pass = s.pass !== false;
        details.push(
          `${frame.label}: ${pass ? '✅' : '⚠ 未过阈值'} score=${Number(s.score).toFixed(4)} ` +
          `匹配 @ (${s.x},${s.y}) · 操作区[${action.join(',')}] · 目标点 (${point.x},${point.y}) · ` +
          `区域有效性：${validity.text} · 帧来源：${s.frame_source || frame.source}`
        );
        boxesForFrames.push({
          text: `点测 ${it.id} ${frame.label} ${Number(s.score).toFixed(2)}`,
          score: Number(s.score),
          box: recognizedBox,
        });
        if (assetTestReal) {
          realOps.push({ label: `${frame.label} 点击 (${point.x},${point.y})`, x: point.x, y: point.y });
        }
      } else if (it.kind === 'ocr_line' || it.kind === 'ocr_lines') {
        const probe = await probeAssetOcrFrame(it, frame, task);
        if (!probe.ok) {
          details.push(`${frame.label}: ❌ ${probe.error}`);
          continue;
        }
        const recognizedBox = probe.recognizedBox || null;
        // Single-line /ocr has no bbox: probeAssetOcrFrame reports the stored
        // ROI as roiBox only, so a stale-looking saved rectangle is never
        // labelled (or drawn) as a real detection box.
        const roiBox = Array.isArray(probe.roiBox) ? probe.roiBox : null;
        const boxNote = recognizedBox
          ? `真实识别框 [${recognizedBox.join(',')}]`
          : `识别区域（存储 ROI，无独立检测框）${roiBox ? ` [${roiBox.join(',')}]` : '（无）'}`;
        const effShape = it.action_shape || null;
        const action = assetActionBox(it, effShape, recognizedBox || it.roi) || it.roi;
        const point = randomPointInAction(it, effShape, recognizedBox || it.roi);
        const validity = actionValidity(it, recognizedBox, effShape);
        const what = probe.lines
          ? `多行 ${probe.lines.length} 行，命中 ${JSON.stringify(probe.text || '')}`
          : `文字 ${JSON.stringify(probe.text || '')}`;
        details.push(
          `${frame.label}: ✅ ${what} score=${Number(probe.score || 0).toFixed(4)} · ` +
          `${boxNote} · ` +
          `操作区[${action.join(',')}] · 目标点 (${point.x},${point.y}) · 区域有效性：${validity.text} · ` +
          `帧来源：${probe.frameSource || frame.source}`
        );
        if (recognizedBox) {
          boxesForFrames.push({ text: `点测 ${it.id} ${frame.label}`, score: Number(probe.score || 0), box: recognizedBox });
        }
        if (assetTestReal) {
          realOps.push({ label: `${frame.label} 点击 (${point.x},${point.y})`, x: point.x, y: point.y });
        }
      } else {
        const action = assetActionBox(it) || it.roi;
        if (!action) {
          details.push(`${frame.label}: ❌ 没有识别/操作区域`);
          continue;
        }
        const point = randomPointInAction(it, undefined, it.roi);
        details.push(`${frame.label}: 操作区[${action.join(',')}] · 目标点 (${point.x},${point.y}) · 帧来源：${frame.source}`);
        if (assetTestReal) {
          realOps.push({ label: `${frame.label} 点击 (${point.x},${point.y})`, x: point.x, y: point.y });
        }
      }
    }
    boxes = boxesForFrames;
    draw();
    let modeNote = '';
    if (assetTestReal) {
      if (!realOps.length) {
        modeNote = '真实模式：没有可执行的点击（未发送设备请求）。';
      } else if (!confirmAssetTestGestures('click', realOps)) {
        modeNote = '真实模式：已取消，未发送设备请求。';
      } else {
        let sentOk = 0;
        let sentFail = 0;
        const sent = [];
        for (const op of realOps) {
          try {
            const res = await sendAssetTestGesture(true, 'tap', { x: op.x, y: op.y });
            sentOk += 1;
            sent.push(`${op.label}: ${res.text}`);
          } catch (e) {
            sentFail += 1;
            sent.push(`${op.label}: ❌ 真实点击失败：${e && e.message ? e.message : e}`);
          }
        }
        details.push(...sent);
        modeNote =
          `真实模式：已发送 ${sentOk}/${realOps.length} 个点击` +
          (sentFail ? `，失败 ${sentFail}` : '') +
          '（服务端 device/gesture 见上）。';
      }
    }
    const header = assetTestReal
      ? `点击测试（真实点击模式）「${it.id}」`
      : `点击测试（演练，不实际点击）「${it.id}」`;
    const tail = assetTestReal
      ? (modeNote || '真实模式。')
      : '注意：这里只验证定位/目标点；实际点击请在 flow 中运行。';
    resultEl.textContent =
      `${header}\n` +
      `测试帧：${testFrameLabel(it, frameId)}（${frames.length} 帧）\n` +
      details.join('\n') +
      '\n' + tail;
    log(
      assetTestReal ? `素材点击测试（真实点击模式）：${it.id}` : `素材点击测试（演练）：${it.id}`,
      assetTestReal ? 'WARN' : 'INFO',
      {
        frames: frames.map((f) => f.label),
        kind: it.kind,
        details,
        gestures: realOps.length,
      }
    );
  } catch (e) {
    resultEl.textContent =
      `${assetTestReal ? '真实点击模式' : '演练'}点击测试异常：` + e;
  }
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
  let stepAction = null;
  let stepShape = null;
  const fw = canvas.width || 1280;
  const fh = canvas.height || 720;
  try {
    for (const frame of frames) {
      let effShape = it.action_shape || null;
      let recognizedBox = null;
      if (it.kind === 'template') {
        if (!Array.isArray(it.roi)) {
          lines.push(`${frame.label}: ❌ 素材没有 roi`);
          continue;
        }
        const r = await fetch(
          `/task/asset/match?task=${encodeURIComponent(task)}&id=${encodeURIComponent(it.id)}${frame.query}`
        );
        const s = await r.json();
        if (s.error || s.pass === false) {
          lines.push(`${frame.label}: ❌ ${s.error || '匹配未过阈值'}（未生成该帧拖动点）`);
          continue;
        }
        recognizedBox = [Number(s.x), Number(s.y), Number(s.w), Number(s.h)];
        if (!matchBoxValid(recognizedBox)) {
          lines.push(`${frame.label}: ❌ 匹配坐标无效 (${s.x},${s.y},${s.w},${s.h})（未生成该帧拖动点）`);
          continue;
        }
        effShape = translatedActionShapeForMatch(it, s);
      }
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
        `操作区[${action.join(',')}]${it.kind === 'template' ? '（已按匹配平移）' : ''}`
      );
      if (assetTestReal) {
        const swipeMs = 300 + Math.floor(Math.random() * 351);
        realOps.push({
          label: `${frame.label} 滑动 (${from.x},${from.y}) → (${to.x},${to.y}) ${swipeMs}ms`,
          from,
          to,
          ms: swipeMs,
        });
      }
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
    resultEl.textContent =
      `${assetTestReal ? '真实滑动模式' : '演练'}拖动测试异常：` + e;
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

// v1.1: asset-list OCR action — ocr_lines uses /lines (not single-line /ocr);
// v1.2.3: exactly one recognition ROI; legacy source ROI is migrated server-side.
function assetRecognitionBox(it) {
  // v1.2.3: exactly one recognition ROI. Legacy source ROI is migrated by the
  // server at asset-load time, so the editor never picks a second region.
  return it && Array.isArray(it.roi) ? it.roi : null;
}

async function probeAssetOcrFrame(it, frame, task) {
  const rec = assetRecognitionBox(it);
  if (!rec) return { ok: false, error: '素材没有识别区域（roi）', frame };
  const derived = it.kind === 'ocr_line' && it.ocr_mode === 'lines' && !!it.ocr_text;
  const roi = { x: rec[0], y: rec[1], w: rec[2], h: rec[3] };
  const q = `x=${roi.x}&y=${roi.y}&w=${roi.w}&h=${roi.h}` +
    `&task=${encodeURIComponent(task || '')}&id=${encodeURIComponent(it.id)}` +
    (frame && frame.query ? frame.query : '');
  try {
    if (it.kind === 'ocr_lines' || derived) {
      if (!detReady) {
        return { ok: false, error: '多行识别不可用：serve 需 --features ocr 且带 --det-model', frame };
      }
      const r = await fetch('/lines?' + q);
      const s = await r.json();
      if (s.error) return { ok: false, error: s.error, s, frame };
      const lines = (s.lines || []).map((b) => ({
        ...b,
        box: [roi.x + b.box[0], roi.y + b.box[1], b.box[2], b.box[3]],
      }));
      if (!lines.length) return { ok: false, error: '识别区域内没有文字', lines, s, frame };
      const frameSource = s.frame_source || (frame && frame.source) || 'current';
      if (derived) {
        let matched = null;
        let matchInfo = null;
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
          return {
            ok: false,
            error: `识别区域未找到期望文字 ${JSON.stringify(it.ocr_text)}：${raw}`,
            lines,
            s,
            frame,
            frameSource,
          };
        }
        return {
          ok: true,
          lines,
          text: matched.text,
          score: Number(matched.score),
          recognizedBox: matched.box,
          matchInfo,
          s,
          frame,
          frameSource,
        };
      }
      return {
        ok: true,
        lines,
        text: lines[0].text,
        score: Number(lines[0].score),
        recognizedBox: lines[0].box,
        s,
        frame,
        frameSource,
      };
    }
    const r = await fetch('/ocr?' + q);
    const s = await r.json();
    if (s.error || !String(s.text || '').trim()) {
      return { ok: false, error: s.error || '识别区域内没有文字', s, frame };
    }
    return {
      ok: true,
      text: s.text,
      score: Number(s.score),
      // Single-line /ocr returns text+score only (no bbox). The stored ROI is
      // the recognition area, not a detected box — report it as roiBox so the
      // caller never paints/labels it as a real detection box.
      recognizedBox: null,
      roiBox: [rec[0], rec[1], rec[2], rec[3]],
      s,
      frame,
      frameSource: s.frame_source || (frame && frame.source) || 'current',
    };
  } catch (e) {
    return { ok: false, error: String(e), frame };
  }
}

async function testAssetOcr(it, frameId, scopeTask) {
  const rec = assetRecognitionBox(it);
  if (!rec) {
    resultEl.textContent = `素材「${it.id}」没有识别区域（roi）`;
    return;
  }
  const frames = testFrames(it, frameId);
  const task = assetRowScopeTask(scopeTask, false);
  const action = assetActionBox(it);
  const areaLine =
    `识别区域 [${rec.join(',')}]` +
    (action ? `；操作区域 [${action.join(',')}]（仅点击，不参与识别）` : '');
  const header = `${areaLine}\n测试帧：${testFrameLabel(it, frameId)}（${frames.length} 帧）`;
  resultEl.textContent = `素材「${it.id}」识别中…\n${header}`;
  log(`OCR 测试开始 asset=${it.id}`, 'INFO', {
    frames: frames.map((f) => f.label),
    kind: it.kind,
    mode: it.ocr_mode || 'single',
    roi: { x: rec[0], y: rec[1], w: rec[2], h: rec[3] },
    action,
    expected: it.ocr_text || null,
    task,
  });
  const summaries = [];
  let lastLines = null;
  try {
    for (const frame of frames) {
      const probe = await probeAssetOcrFrame(it, frame, task);
      if (!probe.ok) {
        summaries.push(`${frame.label}: ❌ ${probe.error}`);
        log(`OCR 测试失败 asset=${it.id}`, 'WARN', {
          frame: frame.label,
          error: probe.error,
          expected: it.ocr_text || null,
        });
        // Preserve the old single-frame full-screen fallback; multi-frame tests
        // stay a compact per-frame summary instead of replacing candidates.
        // P2-4: hand it the probe's own frame so the fallback cannot land on the
        // editor's current frame when a different frame was explicitly tested.
        if (frames.length === 1 && await fallbackFullscreenLines(probe.error, it, frame.frameId)) {
          resultEl.textContent = header + '\n' + resultEl.textContent;
          return;
        }
        continue;
      }
      const what = probe.lines
        ? `${probe.lines.length} 行` +
          (probe.matchInfo ? `，命中 ${JSON.stringify(probe.text)}（${probe.matchInfo.mode}）` : '')
        : `文字 ${JSON.stringify(probe.text)}`;
      summaries.push(
        `${frame.label}: ✅ ${what} score=${Number(probe.score || 0).toFixed(4)} · ` +
        `耗时 ${Number((probe.s && probe.s.elapsed_ms) || 0).toFixed(0)} ms · 帧来源：${probe.frameSource}`
      );
      log(`OCR 测试成功 asset=${it.id}`, 'INFO', {
        frame: frame.label,
        frame_source: probe.frameSource,
        text: probe.text,
        score: probe.score,
        count: probe.lines ? probe.lines.length : 1,
      });
      if (probe.lines) lastLines = probe.lines;
    }
    if (lastLines && frames.length === 1) {
      // Single-frame behaviour unchanged: show the line candidates so the user
      // can still turn a saved OCR asset's lines into new assets.
      renderOcrLines(lastLines, { x: rec[0], y: rec[1], w: rec[2], h: rec[3] }, `素材「${it.id}」`);
    }
    resultEl.textContent = `素材「${it.id}」识别结果\n${header}\n` + summaries.join('\n');
    if (!summaries.length) resultEl.textContent = `素材「${it.id}」识别结果\n${header}\n（没有结果）`;
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
    if (
      !clickOverlay ||
      clickOverlay.width !== canvas.width ||
      clickOverlay.height !== canvas.height
    ) {
      buildClickOverlay();
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

function runAssetTest(it, frameId, scopeTask) {
  cancelRecognitionRoiEdit('素材测试');
  if (it.kind === 'template') testAssetMatch(it, frameId, scopeTask);
  else if (it.kind === 'region') {
    pickAssetRoi(it);
    resultEl.textContent =
      `操作区域「${it.id}」[${(it.roi || []).join(',')}]\n` +
      `流程引用：click_target/find {kind:"region", sel:"${it.id}"}`;
  } else testAssetOcr(it, frameId, scopeTask);
}

function collectionAssetRows(collections) {
  return (collections || []).map((c) => ({
    id: c.id,
    kind: 'template_set',
    members: c.members || [],
    threshold: c.threshold,
    action_shape: c.action_shape,
    action_source: c.action_source,
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
  // P1-1: a reorder is a write to the shared asset store too; refresh the global
  // cache so the global library keeps the new order and the frame detail list
  // (global rows come first) is repainted from the same order.
  await refreshGlobalAssets();
}

// Collection test helpers. The URL builder is shared by the dry-run preview and
// the real request so a dry-run call stays byte-identical to the old URL. The
// preview's action-box centre is only an estimate: the real request re-matches
// and the server samples inside the exact operation shape, so the authoritative
// point is the returned `click_point`.
function assetCollectionMatchUrl(task, id, frameQuery, real) {
  return `/task/asset/collection/match?task=${encodeURIComponent(task)}&id=${encodeURIComponent(id)}` +
    `${frameQuery || ''}${real ? '&real=1' : ''}`;
}

function assetCollectionClickPoint(actionBox) {
  if (!Array.isArray(actionBox) || actionBox.length !== 4) return null;
  const [x, y, w, h] = actionBox.map(Number);
  if (![x, y, w, h].every(Number.isFinite) || w <= 0 || h <= 0) return null;
  return [x + Math.floor(w / 2), y + Math.floor(h / 2)];
}

function assetCollectionMatchLine(label, s, fallbackSource) {
  const prefix = label ? `${label}: ` : '';
  const source = s.frame_source || fallbackSource || '';
  return `${prefix}最佳成员 ${s.member} · score=${Number(s.score).toFixed(4)} · ` +
    `threshold=${Number(s.threshold).toFixed(3)} · pass=${s.pass} · ` +
    `位置 (${s.x},${s.y}) · 帧来源：${source}` +
    (s.action_box ? ` · 操作区域[${s.action_box.join(',')}]` : '');
}

async function testAssetCollection(it, frameId, scopeTask) {
  cancelRecognitionRoiEdit('素材集合测试');
  const mode = realAssetTestGuard();
  if (!mode.ok) return;
  const real = assetTestReal;
  const task = assetRowScopeTask(scopeTask, false);
  if (!Array.isArray(it.members) || !it.members.length) {
    resultEl.textContent = `集合「${it.id}」没有成员`;
    return;
  }
  const frames = testFrames(it, frameId);
  const summaries = [];
  try {
    for (const frame of frames) {
      // Dry preview first: it never taps, and in real mode it gives the user a
      // concrete point to confirm before the one request allowed to click.
      let s = await fetch(
        assetCollectionMatchUrl(task, it.id, frame.query, false)
      ).then((r) => r.json());
      if (!s.ok) {
        summaries.push(`${frame.label}: ❌ ` + (s.errors || []).join('; '));
        continue;
      }
      if (!real) {
        summaries.push(assetCollectionMatchLine(frame.label, s, frame.source));
        log(`集合测试「${it.id}」`, 'INFO', {
          frame: frame.label,
          frame_source: s.frame_source || frame.source,
          member: s.member,
          score: Number(s.score),
          threshold: Number(s.threshold),
          pass: s.pass,
          action_box: s.action_box || null,
        });
        continue;
      }
      if (s.pass !== true) {
        summaries.push(
          `${frame.label}: ⚠ 未过阈值（未点击）· ` +
          assetCollectionMatchLine('', s, frame.source)
        );
        log(`集合测试「${it.id}」未过阈值，未点击`, 'INFO', {
          frame: frame.label,
          member: s.member,
          score: Number(s.score),
          threshold: Number(s.threshold),
        });
        continue;
      }
      const point = assetCollectionClickPoint(s.action_box);
      if (!point) {
        summaries.push(`${frame.label}: ❌ 匹配通过但没有可点击的操作区域（未点击）`);
        continue;
      }
      const confirmed = confirmAssetTestGestures('click', [{
        label: `${frame.label} 集合「${it.id}」成员 ${s.member} 点击 (${point[0]}, ${point[1]})`,
      }], '服务端将重新匹配并点击最新操作区（shape 内采样）；实际点击点以返回 click_point 为准。');
      if (!confirmed) {
        summaries.push(`${frame.label}: 真实模式已取消（未点击）`);
        continue;
      }
      const response = await fetch(
        assetCollectionMatchUrl(task, it.id, frame.query, true)
      );
      s = await response.json();
      if (!s.ok) {
        // P1-2: the server answers 409 when the re-match is below threshold;
        // that is a refused real click, not a generic transport exception.
        if (response.status === 409) {
          summaries.push(`${frame.label}: ⚠ 真实点击未执行：重新匹配未过阈值（未点击）`);
        } else {
          summaries.push(
            `${frame.label}: ❌ 真实点击失败：` +
            ((s.errors || []).join('; ') || `HTTP ${response.status}`)
          );
        }
        continue;
      }
      const [x, y] = s.click_point || [];
      if (s.clicked === true && s.tap_ok === true) {
        summaries.push(
          `${frame.label}: 真实点击（已点击 ${x},${y}）· ` +
          assetCollectionMatchLine('', s, frame.source)
        );
      } else if (s.pass === false) {
        summaries.push(
          `${frame.label}: ⚠ 重新匹配未过阈值（未点击）· ` +
          assetCollectionMatchLine('', s, frame.source)
        );
      } else {
        summaries.push(
          `${frame.label}: ⚠ 服务端未确认点击（未点击）· ` +
          assetCollectionMatchLine('', s, frame.source)
        );
      }
      log(`集合测试「${it.id}」真实点击`, 'WARN', {
        frame: frame.label,
        member: s.member,
        clicked: s.clicked === true,
        click_point: s.click_point || null,
        frame_source: s.frame_source || frame.source,
      });
    }
    const header = real
      ? `素材集合「${it.id}」测试（真实点击模式；${frames.length} 帧；与 runtime 同源）：`
      : `素材集合「${it.id}」测试（${frames.length} 帧；与 runtime 同源）：`;
    const modeLine = real
      ? '模式：真实点击（匹配通过后先确认，再发真实点击；匹配失败/取消不点击）'
      : '模式：演练（未点击）——只计算定位/操作区域，绝不发真实点击';
    resultEl.textContent = header + '\n' + modeLine + '\n' + summaries.join('\n');
  } catch (e) {
    resultEl.textContent = `集合「${it.id}」测试异常：${e}`;
  }
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

// P1-1: keep the saved polygon (click/matching mask) ONLY while it still bounds
// the new ROI. If the canvas box moved, re-send the new rectangle as the
// polygon: otherwise `/task/save_asset` derives roi from the stale mask bbox
// (oas-task:1721) and silently discards the edit. No polygon => stay polygon-free.
function assetRoiPolygonForSave(polygon, roi) {
  if (!Array.isArray(polygon) || polygon.length < 3) return null;
  const [x, y, w, h] = roi;
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const p of polygon) {
    if (!Array.isArray(p) || p.length !== 2) return assetRoiRectanglePolygon(roi);
    const px = Number(p[0]);
    const py = Number(p[1]);
    if (!Number.isInteger(px) || !Number.isInteger(py)) return assetRoiRectanglePolygon(roi);
    minX = Math.min(minX, px);
    minY = Math.min(minY, py);
    maxX = Math.max(maxX, px);
    maxY = Math.max(maxY, py);
  }
  const sameBox = minX === x && minY === y && maxX - minX + 1 === w && maxY - minY + 1 === h;
  if (sameBox) return polygon.map((p) => [Number(p[0]), Number(p[1])]);
  return assetRoiRectanglePolygon(roi);
}

// P1-1/P1-2: `/task/save_asset` replaces the WHOLE AssetDecl, so the payload
// must be built from the LATEST stored asset (not the snapshot captured when the
// ROI editor opened) and must carry every persisted field; only roi changes.
// Template saves only need the template-relevant fields plus the optional OCR
// provenance fields, which are forwarded when present so a replace never drops
// stored data silently.
function assetRoiReplacePayload(task, asset, roi) {
  const it = asset || {};
  const payload = {
    task,
    replace: true,
    id: it.id,
    kind: it.kind || 'template',
    roi: roi.slice(),
    positions: Array.isArray(it.positions) ? it.positions.slice() : [],
    action_source: it.action_source === 'fixed' ? 'fixed' : 'recognition',
  };
  if (it.threshold != null && Number.isFinite(Number(it.threshold))) {
    payload.threshold = Number(it.threshold);
  }
  // Server contract: recognition must NOT carry a shape; fixed REQUIRES one.
  if (payload.action_source === 'fixed' && it.action_shape) payload.action_shape = it.action_shape;
  const polygon = assetRoiPolygonForSave(it.polygon, roi);
  if (polygon) payload.polygon = polygon;
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
  if (kind === 'ocr_line' || kind === 'ocr_lines') return 'ocr';
  return '';
}

async function editAssetThreshold(it, scopeTask) {
  cancelRecognitionRoiEdit('修改阈值');
  const thresholdKind = assetThresholdKind(it);
  if (!thresholdKind) {
    resultEl.textContent = `素材「${it.id}」不支持阈值（仅 template / ocr_line / ocr_lines）`;
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
          ? '（模板：用当前帧重新截图并覆盖，rev+1）'
          : '（OCR/区域：直接更新 roi）');
    }
  }
  if (assetRoiEditLabel) {
    assetRoiEditLabel.hidden = !editing;
    assetRoiEditLabel.textContent = editing && editing.it
      ? `识别区编辑中：${editing.it.id}`
      : '';
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
  if (!Array.isArray(it.roi) || it.roi.length !== 4) {
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
  // Load the asset's CURRENT roi (and operation region) onto the canvas.
  pickAssetRoi(it);
  dragStart = null;
  dragTarget = 'roi';
  editingRecognitionAsset = { it, scopeTask: task };
  updateAssetRoiSaveButton();
  updateRoiLabel();
  draw();
  const text = `编辑素材「${it.id}」识别区：在画布上拖拽画出新的识别框，然后点画布底部的「保存识别区（覆盖素材）」` +
    (it.kind === 'template' ? '\n模板保存 = 用当前帧重新截图覆盖（rev+1）' : '') +
    (wasLive ? '\n实时预览已暂停，避免拖拽被当作设备点击；保存后可手动恢复' : '');
  resultEl.textContent = text;
  log(text);
}

async function saveAssetRecognitionRoi() {
  const editing = editingRecognitionAsset;
  if (!editing || !editing.it) return;
  const editingId = editing.it.id;
  const box = roi && roi.w > 0 && roi.h > 0 ? [roi.x, roi.y, roi.w, roi.h] : null;
  const valid = assetRoiBoxValid(box);
  if (!valid) {
    resultEl.textContent =
      `识别区无效：请在画布上拖拽画出 w/h > 0 的框（当前 ${box ? box.join(',') : '未选择'}）`;
    window.alert('识别区无效：请在画布上拖拽画出有效的识别框');
    return;
  }
  // P2-9: the box must fit the frame that is on screen.
  const frameCheck = assetRoiWithinFrame(valid, canvas.width, canvas.height);
  if (!frameCheck.ok) {
    resultEl.textContent = frameCheck.text;
    window.alert(frameCheck.text);
    return;
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
  if (it.kind === 'template') {
    // P2-6: `/task/save_asset` requires positions; tell the user before the
    // request instead of surfacing the server's "添加素材必须选择至少一个位置帧".
    const positions = Array.isArray(it.positions) ? it.positions : [];
    if (!positions.length) {
      const text = `模板「${it.id}」没有位置帧，无法覆盖保存；请先在「位置帧」里加至少一个帧`;
      resultEl.textContent = text;
      window.alert(text);
      return;
    }
    // Overwrite = re-crop: keep everything else the template owns (id, threshold,
    // positions, fixed action shape, polygon) and swap only the geometry.
    if (!window.confirm(
      `用当前帧重新截图并覆盖模板「${it.id}」（rev+1，不可撤销；位置帧 ${positions.length} 个）？`
    )) return;
    // P1-1/P1-2: build the replace payload from the latest object so no field
    // (polygon/ocr_*/threshold/positions/action_shape) is dropped or rolled back.
    const payload = assetRoiReplacePayload(task, it, valid);
    const r = await postJson('/task/save_asset', payload);
    if (!r.ok) {
      showErrors('覆盖模板失败：', r.errors);
      return;
    }
    const text = `模板「${it.id}」已用当前帧重新截图覆盖：识别区 [${valid.join(',')}]（rev+1）`;
    resultEl.textContent = text;
    log(text);
    setStatus(`模板「${it.id}」已覆盖；两个素材库已刷新`);
  } else {
    const r = await postJson('/task/asset/update', { task, id: it.id, roi: valid });
    if (!r.ok) {
      showErrors('更新识别区域失败：', r.errors);
      return;
    }
    const text = `素材「${it.id}」识别区域：[${valid.join(',')}]`;
    resultEl.textContent = text;
    log(text);
    setStatus(`素材「${it.id}」识别区域已保存；两个素材库已刷新`);
  }
  editingRecognitionAsset = null;
  updateAssetRoiSaveButton();
  await refreshTaskAssetsForScope(task);
  await refreshGlobalAssets();
}

function editAssetActionRoi(it, scopeTask) {
  if (!it.roi) {
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
  setSelectionMode('actionRect');
  roi = { x: it.roi[0], y: it.roi[1], w: it.roi[2], h: it.roi[3] };
  const actionBox = actionShapeBBox(it.action_shape);
  // v1.2.4: restore BOTH kinds (rect + polygon) and their invert flag. The
  // loader clears the stale rect first; a polygon must not keep a bbox rect.
  loadActionShapeForEdit(it.action_shape);
  if (!actionBox) roiBack = { ...roi };
  updateRoiLabel();
  draw();
  log(`编辑素材「${it.id}」操作区域：在画布上拖拽画框`);
}

async function updateAssetActionRoi(id) {
  // C: the row scope stored when editing started (frame detail rows can be
  // global or task-only); no scope => the old assetScopeTask behaviour.
  const task = assetRowScopeTask(pendingActionAssetScope, true);
  const shape = currentActionShape();
  if (!task || !shape) {
    pendingActionAsset = null;
    pendingActionAssetScope = null;
    return;
  }
  const r = await postJson('/task/asset/update', {
    task,
    id,
    action_shape: shape,
    action_source: 'fixed',
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
  updateRoiLabel();
  draw();
  log(`素材「${id}」操作区域已更新（action_source=fixed）`);
  setStatus(`素材「${id}」操作区域已保存；两个素材库已刷新`);
  await refreshTaskAssetsForScope(task);
  await refreshGlobalAssets();
}

function pickAssetRoi(it) {
  if (!it.roi) {
    resultEl.textContent = `素材「${it.id}」没有 roi`;
    return;
  }
  // P0-2: clicking another row (main body / 识别 / region test) replaces the
  // canvas ROI, so the previous asset's ROI editor must not stay armed.
  cancelRecognitionRoiEdit('选中其它素材');
  const shapeBox = actionShapeBBox(it.action_shape);
  roi = { x: it.roi[0], y: it.roi[1], w: it.roi[2], h: it.roi[3] };
  loadActionShapeForEdit(it.action_shape);
  polygon = null;
  polyClosed = false;
  updateRoiLabel();
  draw();
  log(`已用素材「${it.id}」设置识别区域${shapeBox ? '（含操作区域）' : ''}`);
}

async function testAssetMatch(it, frameId, scopeTask) {
  const task = assetRowScopeTask(scopeTask, false);
  const frames = testFrames(it, frameId);
  const summaries = [];
  const framesBoxes = [];
  resultEl.textContent = `素材「${it.id}」全屏匹配中…（${frames.length} 帧）`;
  try {
    for (const frame of frames) {
      const r = await fetch(
        `/task/asset/match?task=${encodeURIComponent(task)}&id=${encodeURIComponent(it.id)}${frame.query}`
      );
      const s = await r.json();
      if (s.error) {
        summaries.push(`${frame.label}: ❌ ${s.error}`);
        continue;
      }
      const pass = s.pass !== false ? '通过' : '未过阈值';
      const recognizedBox = [Number(s.x), Number(s.y), Number(s.w), Number(s.h)];
      // A miss still carries w/h (template size) with x=y=-1: never draw it.
      if (!matchBoxValid(recognizedBox)) {
        summaries.push(`${frame.label}: ❌ 匹配坐标无效 (${s.x},${s.y},${s.w},${s.h})`);
        continue;
      }
      const effShape = translatedActionShapeForMatch(it, s);
      const validity = actionValidity(it, recognizedBox, effShape);
      summaries.push(
        `${frame.label}: ${s.pass !== false ? '✅' : '❌'} ${pass} · ` +
        `得分 ${Number(s.score).toFixed(4)}（阈值 ${Number(s.threshold).toFixed(2)}） · ` +
        `位置 (${s.x},${s.y}) ${s.w}x${s.h} · 区域有效性：${validity.text} · ` +
        `耗时 ${Number(s.elapsed_ms).toFixed(0)} ms · ` +
        `帧来源 ${s.frame_source || frame.source}`
      );
      framesBoxes.push({
        text: `测试 ${s.id} ${frame.label} ${Number(s.score).toFixed(2)}`,
        score: Number(s.score),
        box: [s.x, s.y, s.w, s.h],
      });
      log(`素材测试：${s.id}`, 'INFO', {
        frame: frame.label,
        frame_source: s.frame_source || frame.source,
        score: Number(s.score),
        threshold: Number(s.threshold),
        pass: s.pass,
        point: [s.x, s.y],
        size: [s.w, s.h],
        elapsed_ms: s.elapsed_ms,
      });
    }
    boxes = framesBoxes;
    draw();
    resultEl.textContent =
      `素材「${it.id}」测试（${frames.length} 帧）\n` + summaries.join('\n');
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
    onAction: (it) => editAssetActionRoi(it),
    onClickTest: (it) => testAssetClick(it),
    onDragTest: (it) => testAssetDrag(it),
    onStep: (it) => generateAssetStep(it),
    onCollectionTest: (it) => testAssetCollection(it),
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
      onAction: (it) => editAssetActionRoi(it, scopeTask),
      onCollectionTest: (it) => testAssetCollection(it, f.id, scopeTask),
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
  // P0-2: a recognition ROI is frame-relative; switching frames invalidates it.
  cancelRecognitionRoiEdit('切换帧');
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
  // One dispatch point: selectionActiveMode() derives from the same flags that
  // setSelectionMode() writes, so two modes can never both handle this event.
  const mode = selectionActiveMode();
  if (mode === 'actionPolygon') {
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
  if (mode === 'recognitionPolygon') {
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
  if (mode === 'actionRect') {
    // P0-3: a new rectangle is an explicit replacement. Clear the previously
    // closed action polygon so currentActionShape() cannot return the old
    // polygon and save the wrong geometry.
    actionPoly = null;
    actionPolyClosed = false;
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
  if (selectionActiveMode() === 'recognitionPolygon') {
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
  if (selectionActiveMode() === 'recognitionPolygon') {
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
      const row = saveCandidates[activeCandidate];
      row.action = finished || { ...row.recognize };
      // P0-2/P0-3: only this explicit drag creates a fixed shape. The bbox and
      // the polygon/invert carried by currentActionShape are persisted together.
      row.actionShape = finished ? currentActionShape() : null;
      row.actionSource = row.actionShape ? 'fixed' : 'recognition';
      activeCandidate = -1;
      setSelectionMode('none');
      renderCandidateList(lastLinesRoi ? '本次识别' : '识别结果');
      log(
        `操作区域已更新：${row.actionSource === 'fixed' && row.actionShape
          ? `${row.actionShape.kind}${row.actionShape.invert ? '（反选）' : ''}`
          : '跟随识别框（recognition）'}`
      );
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
  const mode = selectionActiveMode();
  if (mode === 'actionPolygon') {
    if (!actionPoly || actionPoly.length < 3) return;
    ev.preventDefault();
    actionPolyClosed = true;
    setSelectionMode('none');
    const shape = currentActionShape();
    const bbox = shape ? actionShapeBBox(shape) : null;
    if (activeCandidate >= 0 && saveCandidates[activeCandidate]) {
      const row = saveCandidates[activeCandidate];
      row.actionShape = shape;
      row.actionSource = shape ? 'fixed' : 'recognition';
      if (bbox) row.action = { x: bbox[0], y: bbox[1], w: bbox[2], h: bbox[3] };
      activeCandidate = -1;
      renderCandidateList(lastLinesRoi ? '本次识别' : '识别结果');
      log(
        `候选操作区域多边形已保存到候选：「${row.name}」` +
        `${actionInvert ? '（反选：屏幕−区域）' : ''}`
      );
    } else if (pendingActionAsset) {
      // Existing asset: closing the polygon must immediately persist it; the
      // old implementation only logged and lost the shape.
      log(`操作区域多边形已闭合，保存到素材「${pendingActionAsset}」…`);
      updateAssetActionRoi(pendingActionAsset).catch((e) => {
        log(`保存素材操作区域失败：${e}`, 'ERROR');
        resultEl.textContent = '保存操作区域失败：' + e;
      });
    } else {
      resultEl.textContent =
        '操作区域多边形已闭合，但当前没有绑定候选/素材。\n' +
        '请先在识别结果点「调操作区域」，或在素材行点「操作区」，再画多边形。';
      log('操作区域多边形已闭合，但未绑定候选/素材', 'WARN');
    }
    updateRoiLabel();
    draw();
    return;
  }
  if (mode !== 'recognitionPolygon' || !polygon || polygon.length < 3) return;
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
  const want = selectionActiveMode() !== 'recognitionPolygon';
  // P1-8: entering recognition polygon mode drops any action polygon/invert/
  // backMode so mouse events can never keep drawing the previous operation
  // area. Leaving the mode keeps the current ROI/shape untouched.
  if (want) {
    // P0-2: a recognition-ROI edit is over once the user starts a new polygon.
    cancelRecognitionRoiEdit('切换到多边形框选');
    resetSelection({ clearInvert: true, clearRoi: true, redraw: false });
    polygon = null;
    polyClosed = false;
    polyDrag = null;
    roi = null;
    roiBack = null;
    dragStart = null;
    setSelectionMode('recognitionPolygon');
    log('多边形模式：逐点点击，双击闭合；闭合后可拖动顶点、双击删除顶点');
  } else {
    setSelectionMode('none');
    polyDrag = null;
  }
  updateRoiLabel();
  draw();
});

btnActionPoly.addEventListener('click', () => {
  const want = selectionActiveMode() !== 'actionPolygon';
  if (want) {
    // P0-2: another canvas edit mode replaces the recognition-ROI editor.
    cancelRecognitionRoiEdit('切换到操作区域多边形');
    // Keep the target (candidate/pending asset) and the invert flag, but leave
    // rect mode / recognition polygon mode and any stale action polygon behind.
    resetActionGeometry({ keepTarget: true, clearInvert: false, redraw: false });
    setSelectionMode('actionPolygon');
    log(`操作区域多边形模式：逐点点击，双击闭合${actionInvert ? '（反选=整个屏幕−区域）' : ''}`);
  } else {
    setSelectionMode('none');
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
  const want = selectionActiveMode() !== 'actionRect';
  if (want) {
    // P0-2: drawing an operation region replaces the recognition-ROI editor.
    cancelRecognitionRoiEdit('切换到操作区域编辑');
    resetActionGeometry({ keepTarget: true, clearInvert: false, redraw: false });
    setSelectionMode('actionRect');
  } else {
    setSelectionMode('none');
    activeCandidate = -1;
    pendingActionAsset = null;
  }
  log(
    want
      ? '操作区域模式：在画面上拖拽画出「操作区域」（action_shape，可大于识别区域）'
      : '已退出操作区域模式'
  );
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
  lastLinesRoi = null;
  replaceResults([], '');
  updateRoiLabel();
  updateClickStepButtons();
  draw();
  resultEl.textContent = '—';
  setStatus('已清除识别/操作区域与选择状态');
});

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

// --- P3: 调度配置纯函数（可被 ui_pure_test.js 提取；不碰 DOM） ---------------
// Everything below is plain data in -> plain data out so the JSON transform /
// entry builder / queue payload / error-text rules are testable without a
// browser. UI functions live further down and call these.

function schedEntryKindLabel(kind) {
  return kind === 'queue' ? 'queue' : 'task';
}

// `task-<任务名>` entry ids must stay unique and control-character free. Task
// names may contain Chinese (keep them), but never path separators. Split/join
// instead of one quoted regex keeps this function extractable by ui_pure_test.js.
function schedSanitizeIdSegment(name) {
  let s = String(name == null ? '' : name).trim();
  s = s.replace(/[\u0000-\u001f\u007f]/g, '-');
  s = s.split('').map((c) => ('<>:"/\\|?*'.indexOf(c) >= 0 ? '-' : c)).join('');
  s = s.replace(/\.\.+/g, '.');
  s = s.replace(/^[.\s-]+|[.\s-]+$/g, '');
  if (!s) s = 'entry';
  if (s.length > 96) s = s.slice(0, 96).replace(/[.\s-]+$/g, '');
  return s || 'entry';
}

// Task-name/queue-id grammar shared by the UI validators: no separators, no
// control characters (the backend additionally checks Windows reserved names).
function schedHasUnsafeNameChar(value) {
  const s = String(value == null ? '' : value);
  for (const c of s) {
    const code = c.charCodeAt(0);
    if (code < 32 || code === 127) return true;
    if ('<>:"/\\|?*'.indexOf(c) >= 0) return true;
  }
  return false;
}

function schedUniqueId(base, used) {
  const taken = used instanceof Set ? used : new Set(used || []);
  let id = String(base || 'entry');
  let n = 2;
  while (taken.has(id)) {
    id = String(base || 'entry') + '-' + n;
    n += 1;
  }
  return id;
}

function schedEntryIdForTarget(kind, target, config) {
  const prefix = schedEntryKindLabel(kind) === 'queue' ? 'queue' : 'task';
  const used = new Set(((config && config.entries) || []).map((e) => String((e && e.entry_id) || '')));
  return schedUniqueId(prefix + '-' + schedSanitizeIdSegment(target), used);
}

function schedEmptyConfig() {
  return {
    schema_version: 4,
    config: {
      // P2-2: a newly created scheduler.json must actually keep scheduling, like
      // the shipped dist/stage default (`start-scheduler.bat` passes no --mode).
      // mode=stop would run a single pass and exit, which looks like "调度没生效".
      mode: 'loop',
      loop_interval_s: 60,
      loop_rounds: 0,
      utc_offset_min: 480,
      calendar: { day_reset: '00:00', week_reset: { weekday: 0, time: '00:00' } },
      guards: { active_hours: [], blackouts: [] },
      defaults: { retry: 1, cooldown: 300 },
    },
    queues: [],
    entries: [],
  };
}

// Normalize whatever GET /scheduler/config returned into the v4 document the UI
// edits: schema 4, entries[].kind defaulted, arrays/objects guaranteed. Unknown
// keys are preserved here; the backend also writes a submitted v4 document back
// verbatim, so they survive a save (only a v3 document is re-normalized/upgraded).
function schedNormalizeConfig(raw) {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return schedEmptyConfig();
  const base = schedEmptyConfig();
  const out = Object.assign({}, base, raw);
  out.schema_version = 4;
  out.config = Object.assign(
    {},
    base.config,
    raw.config && typeof raw.config === 'object' && !Array.isArray(raw.config) ? raw.config : {}
  );
  out.queues = Array.isArray(raw.queues)
    ? raw.queues.filter((q) => q && typeof q === 'object').map((q) => Object.assign({}, q))
    : [];
  out.entries = Array.isArray(raw.entries)
    ? raw.entries.filter((e) => e && typeof e === 'object').map((e) => {
      const entry = Object.assign({}, e);
      entry.kind = schedEntryKindLabel(entry.kind);
      if (!Array.isArray(entry.windows)) entry.windows = [];
      if (!entry.params || typeof entry.params !== 'object' || Array.isArray(entry.params)) entry.params = {};
      return entry;
    })
    : [];
  return out;
}

// New-entry defaults required by the P3 spec: kind=task, enabled, periodic,
// full-day windows (empty array = always in window), quota day/1, config defaults.
function schedDefaultTaskEntry(task, config, kind) {
  const cfg = config || schedEmptyConfig();
  const k = schedEntryKindLabel(kind || 'task');
  const defaults = (cfg.config && cfg.config.defaults) || {};
  const retry = Number.isInteger(defaults.retry) ? defaults.retry : 1;
  const cooldown = Number.isInteger(defaults.cooldown) ? defaults.cooldown : 300;
  return {
    entry_id: schedEntryIdForTarget(k, task, cfg),
    task: String(task == null ? '' : task),
    kind: k,
    enabled: true,
    lifecycle: 'periodic',
    windows: [],
    quota: { per: 'day', times: 1 },
    params: {},
    retry,
    cooldown,
  };
}

function schedWindowsToText(windows) {
  const list = Array.isArray(windows) ? windows : [];
  if (!list.length) return '全天';
  return list.map((w) => {
    const days = Array.isArray(w.days) ? w.days : [];
    const prefix = days.length ? '[' + days.join(',') + ']' : '';
    return prefix + (w.start || '00:00') + '-' + (w.end || '00:00');
  }).join(', ');
}

// Simplified windows text: `HH:MM-HH:MM`, comma separated, optional `[1,3]`
// weekday prefix per window. Empty (or 全天) = no window restriction.
function schedParseWindowsText(text) {
  const raw = String(text == null ? '' : text).trim();
  if (!raw || raw === '全天' || raw === '全天候') return { ok: true, windows: [], error: '' };
  // Commas inside `[1,3]` separate weekdays, not windows: mask them first.
  const guarded = raw.replace(/\[([^\]]*)\]/g, (m, inner) => '[' + inner.replace(/[,，]/g, ' ') + ']');
  const parts = guarded.split(/[,，;；\n]+/).map((s) => s.trim()).filter(Boolean);
  const windows = [];
  for (const part of parts) {
    let days = [];
    let rest = part;
    const m = /^\[([0-9\s]+)\](.*)$/.exec(part);
    if (m) {
      const set = new Set();
      for (const d of m[1].split(/\s+/).filter(Boolean)) {
        const n = Number(d);
        if (!Number.isInteger(n) || n < 0 || n > 6) {
          return { ok: false, windows: [], error: `周几必须是 0..6（0=周一）：${part}` };
        }
        set.add(n);
      }
      days = Array.from(set).sort((a, b) => a - b);
      rest = m[2].trim();
    }
    const hm = /^(\d{1,2}):(\d{2})-(\d{1,2}):(\d{2})$/.exec(rest);
    if (!hm) {
      return { ok: false, windows: [], error: `时间窗口格式应为 HH:MM-HH:MM（可加 [1,3] 前缀）：${part}` };
    }
    const h1 = Number(hm[1]);
    const h2 = Number(hm[3]);
    if (h1 > 23 || h2 > 23 || Number(hm[2]) > 59 || Number(hm[4]) > 59) {
      return { ok: false, windows: [], error: `时间超出范围：${part}` };
    }
    windows.push({
      days,
      start: String(h1).padStart(2, '0') + ':' + hm[2],
      end: String(h2).padStart(2, '0') + ':' + hm[4],
    });
  }
  return { ok: true, windows, error: '' };
}

function schedParseWindowsJson(text) {
  const raw = String(text == null ? '' : text).trim();
  if (!raw) return { ok: true, windows: [], error: '' };
  let v;
  try {
    v = JSON.parse(raw);
  } catch (e) {
    return { ok: false, windows: [], error: 'windows JSON 解析失败：' + (e && e.message ? e.message : e) };
  }
  if (!Array.isArray(v)) return { ok: false, windows: [], error: 'windows JSON 必须是数组' };
  const windows = [];
  for (let i = 0; i < v.length; i++) {
    const w = v[i];
    if (!w || typeof w !== 'object' || Array.isArray(w)) {
      return { ok: false, windows: [], error: `windows[${i}] 必须是对象` };
    }
    const days = w.days == null ? [] : w.days;
    if (!Array.isArray(days)) return { ok: false, windows: [], error: `windows[${i}].days 必须是数组` };
    const outDays = [];
    for (const d of days) {
      const n = Number(d);
      if (!Number.isInteger(n) || n < 0 || n > 6) {
        return { ok: false, windows: [], error: `windows[${i}].days 必须是 0..6` };
      }
      if (!outDays.includes(n)) outDays.push(n);
    }
    outDays.sort((a, b) => a - b);
    const start = String(w.start == null ? '' : w.start);
    const end = String(w.end == null ? '' : w.end);
    const hhmm = /^(\d{1,2}):(\d{2})$/;
    const sm = hhmm.exec(start);
    const em = hhmm.exec(end);
    if (!sm || !em || Number(sm[1]) > 23 || Number(em[1]) > 23 || Number(sm[2]) > 59 || Number(em[2]) > 59) {
      return { ok: false, windows: [], error: `windows[${i}] 的 start/end 需要 HH:MM` };
    }
    windows.push({
      days: outDays,
      start: String(Number(sm[1])).padStart(2, '0') + ':' + sm[2],
      end: String(Number(em[1])).padStart(2, '0') + ':' + em[2],
    });
  }
  return { ok: true, windows, error: '' };
}

function schedParseParamsText(text) {
  const raw = String(text == null ? '' : text).trim();
  if (!raw) return { ok: true, params: {}, error: '' };
  try {
    const v = JSON.parse(raw);
    if (!v || typeof v !== 'object' || Array.isArray(v)) {
      return { ok: false, params: {}, error: 'params 必须是 JSON 对象（{...}）' };
    }
    return { ok: true, params: v, error: '' };
  } catch (e) {
    return { ok: false, params: {}, error: 'params JSON 解析失败：' + (e && e.message ? e.message : e) };
  }
}

function schedEntryToForm(entry, defaults) {
  const e = entry || {};
  const d = defaults || {};
  const quota = e.quota && typeof e.quota === 'object' ? e.quota : null;
  const defaultRetry = Number.isInteger(d.retry) ? d.retry : 1;
  const defaultCooldown = Number.isInteger(d.cooldown) ? d.cooldown : 300;
  return {
    entry_id: String(e.entry_id || ''),
    kind: schedEntryKindLabel(e.kind),
    task: String(e.task || ''),
    enabled: e.enabled !== false,
    lifecycle: e.lifecycle === 'temp' ? 'temp' : 'periodic',
    windows_text: schedWindowsToText(e.windows),
    windows_json: '',
    quota_per: quota && (quota.per === 'day' || quota.per === 'week') ? quota.per : '',
    quota_times: quota && quota.times ? String(quota.times) : '1',
    retry: e.retry == null ? String(defaultRetry) : String(e.retry),
    cooldown: e.cooldown == null ? String(defaultCooldown) : String(e.cooldown),
    params_text: e.params && Object.keys(e.params).length ? JSON.stringify(e.params, null, 2) : '',
  };
}

// Build one entry from form values. `base` supplies editable entry_id/unknown
// keys; temp forces windows=[] and quota=null (the backend rejects both).
function schedBuildEntryFromForm(form, base, config) {
  const f = form || {};
  const b = base || {};
  const cfg = config || schedEmptyConfig();
  const kind = schedEntryKindLabel(f.kind || b.kind);
  const task = String(f.task == null ? (b.task || '') : f.task).trim();
  const lifecycle = f.lifecycle === 'temp' ? 'temp' : 'periodic';
  const temp = lifecycle === 'temp';
  const id = String(f.entry_id || b.entry_id || '').trim() || schedEntryIdForTarget(kind, task, cfg);
  let windows = [];
  let quota = null;
  if (!temp) {
    if (Array.isArray(f.windows)) windows = f.windows.map((w) => Object.assign({}, w));
    else if (Array.isArray(b.windows)) windows = b.windows.map((w) => Object.assign({}, w));
    if (f.quota_per === 'day' || f.quota_per === 'week') {
      const times = Math.floor(Number(f.quota_times));
      quota = { per: f.quota_per, times: Math.max(1, Math.min(1000, Number.isFinite(times) ? times : 1)) };
    } else if (f.quota_per === '') {
      quota = null;
    } else if (b.quota && (b.quota.per === 'day' || b.quota.per === 'week')) {
      quota = { per: b.quota.per, times: b.quota.times };
    }
  }
  let params = {};
  if (f.params && typeof f.params === 'object' && !Array.isArray(f.params)) params = f.params;
  else if (b.params && typeof b.params === 'object' && !Array.isArray(b.params)) params = b.params;
  const retryNum = Number(f.retry);
  const cooldownNum = Number(f.cooldown);
  const retry = Number.isInteger(retryNum) && String(f.retry == null ? '' : f.retry).trim() !== ''
    ? retryNum
    : (Number.isInteger(b.retry) ? b.retry : 1);
  const cooldown = Number.isInteger(cooldownNum) && String(f.cooldown == null ? '' : f.cooldown).trim() !== ''
    ? cooldownNum
    : (Number.isInteger(b.cooldown) ? b.cooldown : 300);
  return Object.assign({}, b, {
    entry_id: id,
    task,
    kind,
    enabled: f.enabled == null ? b.enabled !== false : !!f.enabled,
    lifecycle,
    windows,
    quota,
    params,
    retry,
    cooldown,
  });
}

function schedFindTaskEntries(config, task) {
  const t = String(task == null ? '' : task);
  return (((config && config.entries) || [])).filter(
    (e) => schedEntryKindLabel(e.kind) === 'task' && e.task === t
  );
}

function schedFindTaskQueueMember(config, task) {
  const t = String(task == null ? '' : task);
  for (const q of (config && config.queues) || []) {
    if (Array.isArray(q.members) && q.members.includes(t)) return String(q.queue_id || '');
  }
  return '';
}

// Rule 2: one queue has at most one trigger entry.
function schedQueueEntryConflict(config, queueId, exceptEntryId) {
  const qid = String(queueId == null ? '' : queueId);
  const except = String(exceptEntryId == null ? '' : exceptEntryId);
  const hit = ((config && config.entries) || []).find(
    (e) => schedEntryKindLabel(e.kind) === 'queue' && e.task === qid && e.entry_id !== except
  );
  return hit ? String(hit.entry_id || '') : '';
}

function schedValidateEntryForm(form, config, entryId, baseEntry) {
  const cfg = schedNormalizeConfig(config || schedEmptyConfig());
  const f = form || {};
  const errors = [];
  const kind = schedEntryKindLabel(f.kind);
  const target = String(f.task == null ? '' : f.task).trim();
  const lifecycle = f.lifecycle === 'temp' ? 'temp' : 'periodic';
  if (!target) errors.push(kind === 'queue' ? '请选择要触发的队列（queue_id）' : '请选择任务');
  if (kind === 'queue') {
    if (target && !(cfg.queues || []).some((q) => q.queue_id === target)) {
      errors.push(`队列「${target}」不存在；请先在队列面板新建`);
    }
    if (target) {
      const other = schedQueueEntryConflict(cfg, target, entryId || '');
      if (other) errors.push(`队列「${target}」已有触发条目「${other}」；一个队列最多一个 entry（防双跑）`);
    }
  } else if (target) {
    const queueId = schedFindTaskQueueMember(cfg, target);
    if (queueId) errors.push(`任务「${target}」已是队列「${queueId}」的成员，不能再单独加入调度（防双跑）`);
  }
  let windows = [];
  let quota = null;
  if (lifecycle !== 'temp') {
    const jsonText = String(f.windows_json == null ? '' : f.windows_json).trim();
    if (jsonText) {
      const parsedJson = schedParseWindowsJson(jsonText);
      if (!parsedJson.ok) errors.push(parsedJson.error);
      else windows = parsedJson.windows;
    } else {
      const parsedText = schedParseWindowsText(f.windows_text);
      if (!parsedText.ok) errors.push(parsedText.error);
      else windows = parsedText.windows;
    }
    if (f.quota_per === 'day' || f.quota_per === 'week') {
      const times = Number(f.quota_times);
      if (!Number.isInteger(times) || times < 1 || times > 1000) errors.push('quota 次数必须是 1..1000 的整数');
      else quota = { per: f.quota_per, times };
    } else if (f.quota_per != null && f.quota_per !== '') {
      errors.push('quota 只能是 无 / day / week');
    }
  }
  const retry = Number(f.retry);
  if (!Number.isInteger(retry) || retry < 0 || retry > 10) errors.push('retry 必须是 0..10 的整数');
  const cooldown = Number(f.cooldown);
  if (!Number.isInteger(cooldown) || cooldown < 0 || cooldown > 604800) {
    errors.push('cooldown_s 必须是 0..604800 的整数');
  }
  const params = schedParseParamsText(f.params_text);
  if (!params.ok) errors.push(params.error);
  const entry = schedBuildEntryFromForm(Object.assign({}, f, {
    entry_id: String(entryId || '').trim(),
    windows,
    quota_per: quota ? quota.per : '',
    quota_times: quota ? quota.times : '',
    params: params.params,
    retry: Number.isInteger(retry) ? retry : 0,
    cooldown: Number.isInteger(cooldown) ? cooldown : 0,
  }), entryId ? (baseEntry || {}) : Object.assign({}, baseEntry || {}, { entry_id: '' }), cfg);
  if (entry.entry_id.length > 128) errors.push('entry_id 超过 128 字符');
  return { ok: errors.length === 0, errors, entry };
}

function schedUpsertEntry(config, entry) {
  const cfg = schedNormalizeConfig(config);
  const idx = cfg.entries.findIndex((e) => e.entry_id === entry.entry_id);
  if (idx >= 0) cfg.entries[idx] = Object.assign({}, entry);
  else cfg.entries.push(Object.assign({}, entry));
  return cfg;
}

function schedRemoveEntryById(config, entryId) {
  const cfg = schedNormalizeConfig(config);
  cfg.entries = cfg.entries.filter((e) => e.entry_id !== entryId);
  return cfg;
}

function schedMoveEntry(config, index, delta) {
  const cfg = schedNormalizeConfig(config);
  const from = Number(index);
  const to = from + Number(delta);
  if (!Number.isInteger(from) || from < 0 || from >= cfg.entries.length) return cfg;
  if (!Number.isInteger(to) || to < 0 || to >= cfg.entries.length || to === from) return cfg;
  const moved = cfg.entries.splice(from, 1)[0];
  cfg.entries.splice(to, 0, moved);
  return cfg;
}

function schedQueueToForm(queue) {
  const q = queue || {};
  return {
    queue_id: String(q.queue_id || ''),
    name: String(q.name || ''),
    members: Array.isArray(q.members) ? q.members.slice() : [],
    total_rounds: q.total_rounds == null ? '0' : String(q.total_rounds),
    on_failure: q.on_failure === 'fail_fast' ? 'fail_fast' : 'continue',
    between_ms: q.between_ms == null ? '500' : String(q.between_ms),
  };
}

function schedQueueMemberAdd(members, task) {
  const out = (Array.isArray(members) ? members : []).slice();
  const t = String(task == null ? '' : task).trim();
  if (t && !out.includes(t)) out.push(t);
  return out;
}

function schedQueueMemberRemove(members, task) {
  return (Array.isArray(members) ? members : []).filter((m) => m !== task);
}

function schedQueueMemberMove(members, index, delta) {
  const out = (Array.isArray(members) ? members : []).slice();
  const from = Number(index);
  const to = from + Number(delta);
  if (!Number.isInteger(from) || from < 0 || from >= out.length) return out;
  if (!Number.isInteger(to) || to < 0 || to >= out.length || to === from) return out;
  const moved = out.splice(from, 1)[0];
  out.splice(to, 0, moved);
  return out;
}

function schedBuildQueueFromForm(form, base) {
  const f = form || {};
  const b = base || {};
  const roundsRaw = Number(String(f.total_rounds == null ? '' : f.total_rounds).trim() || 0);
  const betweenRaw = Number(String(f.between_ms == null ? '' : f.between_ms).trim() || 0);
  return {
    queue_id: String(f.queue_id == null ? (b.queue_id || '') : f.queue_id).trim(),
    name: String(f.name == null ? (b.name || '') : f.name).trim(),
    members: (Array.isArray(f.members) ? f.members : (b.members || [])).map((m) => String(m).trim()).filter(Boolean),
    total_rounds: Number.isFinite(roundsRaw) ? Math.max(0, Math.min(1000000, Math.floor(roundsRaw))) : 0,
    on_failure: f.on_failure === 'fail_fast' ? 'fail_fast' : 'continue',
    between_ms: Number.isFinite(betweenRaw) ? Math.max(0, Math.min(600000, Math.floor(betweenRaw))) : 0,
  };
}

function schedValidateQueueForm(form, config, selfId, baseQueue) {
  const cfg = schedNormalizeConfig(config || schedEmptyConfig());
  const f = form || {};
  const errors = [];
  const queueId = String(f.queue_id == null ? '' : f.queue_id).trim();
  if (!queueId) errors.push('queue_id 不能为空');
  if (queueId.length > 120) errors.push('queue_id 最长 120 字符');
  if (queueId === '.' || queueId === '..') errors.push('queue_id 不能是 . 或 ..');
  if (schedHasUnsafeNameChar(queueId)) errors.push('queue_id 不能包含路径/控制字符');
  if (queueId.endsWith('.')) errors.push('queue_id 不能以 . 结尾');
  if (selfId && queueId !== String(selfId)) errors.push('queue_id 不可修改；如需改 id 请删除该队列后重建');
  if (queueId && (cfg.queues || []).some((q) => q.queue_id === queueId && q.queue_id !== selfId)) {
    errors.push(`queue_id「${queueId}」已存在`);
  }
  const members = (Array.isArray(f.members) ? f.members : []).map((m) => String(m).trim()).filter(Boolean);
  if (!members.length) errors.push('至少选择一个成员任务');
  if (members.length > 200) errors.push('成员最多 200 个');
  const seen = new Set();
  for (const m of members) {
    if (seen.has(m)) errors.push(`成员「${m}」重复`);
    seen.add(m);
    if (m.length > 120) errors.push(`成员「${m}」超过 120 字符`);
    if (schedHasUnsafeNameChar(m)) errors.push(`成员「${m}」包含路径/控制字符`);
    const hit = (cfg.entries || []).find((e) => schedEntryKindLabel(e.kind) === 'task' && e.task === m);
    if (hit) {
      errors.push(`成员「${m}」已是任务条目「${hit.entry_id}」的目标；同一任务不能同时在任务条目和队列成员中（防双跑）`);
    }
  }
  const roundsRaw = Number(String(f.total_rounds == null ? '' : f.total_rounds).trim() || 0);
  if (!Number.isInteger(roundsRaw) || roundsRaw < 0 || roundsRaw > 1000000) {
    errors.push('total_rounds 必须是 0..1000000 的整数（0=无限）');
  }
  const betweenRaw = Number(String(f.between_ms == null ? '' : f.between_ms).trim() || 0);
  if (!Number.isInteger(betweenRaw) || betweenRaw < 0 || betweenRaw > 600000) {
    errors.push('between_ms 必须是 0..600000 的整数');
  }
  const queue = schedBuildQueueFromForm(f, baseQueue || {});
  return { ok: errors.length === 0, errors, queue };
}

function schedUpsertQueue(config, queue) {
  const cfg = schedNormalizeConfig(config);
  const idx = cfg.queues.findIndex((q) => q.queue_id === queue.queue_id);
  if (idx >= 0) cfg.queues[idx] = Object.assign({}, queue);
  else cfg.queues.push(Object.assign({}, queue));
  return cfg;
}

// Removing a queue also removes every queue entry pointing at it (otherwise the
// remaining entry would reference an unknown queue and fail validation).
function schedRemoveQueue(config, queueId) {
  const cfg = schedNormalizeConfig(config);
  const removed = cfg.entries
    .filter((e) => schedEntryKindLabel(e.kind) === 'queue' && e.task === queueId)
    .map((e) => String(e.entry_id || ''));
  cfg.entries = cfg.entries.filter(
    (e) => !(schedEntryKindLabel(e.kind) === 'queue' && e.task === queueId)
  );
  cfg.queues = cfg.queues.filter((q) => q.queue_id !== queueId);
  return { config: cfg, removed_entry_ids: removed };
}

function schedQueueProgressText(queue, progress) {
  const q = queue || {};
  const p = progress || {};
  const total = Number(q.total_rounds) || 0;
  const done = Number(p.rounds_completed) || 0;
  const parts = [total === 0 ? `轮次 ${done}/无限` : `轮次 ${done}/${total}`];
  const empty = Number(p.empty_rounds) || 0;
  if (empty) parts.push(`空轮 ${empty}`);
  if (p.finished || (total > 0 && done >= total)) parts.push('已完成');
  else if (p.current_round) {
    const idx = Number(p.current_round.index) || 0;
    const mem = Number(p.current_round.member_index) || 0;
    parts.push(`进行中：第 ${idx} 轮 · 成员 ${mem + 1}`);
  }
  return parts.join(' · ');
}

function schedQueueSummary(queue, progress) {
  const q = queue || {};
  const members = Array.isArray(q.members) ? q.members : [];
  return [
    `成员 ${members.length}：${members.join(' → ') || '—'}`,
    schedQueueProgressText(q, progress),
    `失败策略 ${q.on_failure === 'fail_fast' ? 'fail_fast（本轮中止）' : 'continue'}`,
    `成员间隔 ${q.between_ms == null ? '?' : q.between_ms}ms`,
  ].join(' · ');
}

function schedEntrySummary(entry, statusEntry) {
  const e = entry || {};
  const st = statusEntry || {};
  const parts = [];
  parts.push(e.enabled === false ? '已禁用' : '已启用');
  parts.push(e.lifecycle === 'temp' ? 'temp（每 pass 一次）' : 'periodic');
  if (e.lifecycle !== 'temp') {
    parts.push('窗口 ' + schedWindowsToText(e.windows));
    parts.push(e.quota && e.quota.per ? `quota ${e.quota.per === 'week' ? '周' : '日'} ${e.quota.times}` : '无配额');
  }
  parts.push(`retry ${e.retry == null ? '?' : e.retry}`);
  parts.push(`冷却 ${e.cooldown == null ? '?' : e.cooldown}s`);
  if (e.params && Object.keys(e.params).length) parts.push('params ' + JSON.stringify(e.params));
  if (st.last_result) parts.push(`上次 ${st.last_result}${st.last_outcome ? '(' + st.last_outcome + ')' : ''}`);
  if (st.blocked_reason) parts.push(`等待 ${st.blocked_reason}`);
  return parts.join(' · ');
}

function schedQuotaSummary(status) {
  const entries = (status && status.entries) || [];
  const limited = entries.filter((e) => e.quota && e.quota.per);
  if (!limited.length) return 'quota：无限制';
  const used = limited.reduce((n, e) => n + (Number(e.quota.used) || 0), 0);
  const times = limited.reduce((n, e) => n + (Number(e.quota.times) || 0), 0);
  return `quota：${limited.length} 条限制 · 本周期已用 ${used}/${times}`;
}

// P1-3: /scheduler/status failures need a stable, testable sentence. The status
// card renders every value as `—` for an unknown state, never the normal values.
function schedStatusErrorText(err) {
  const detail = String(err == null ? '' : err).trim();
  return '调度器状态读取失败：' + (detail || '未知错误');
}

// P1-4: canonical JSON (object keys sorted, array order kept) so comparing the
// loaded snapshot with a fresh GET is insensitive to key order but sensitive to
// any value change.
function schedCanonicalJson(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return '[' + value.map(schedCanonicalJson).join(',') + ']';
  return '{' + Object.keys(value).sort().map((k) =>
    JSON.stringify(k) + ':' + schedCanonicalJson(value[k])).join(',') + '}';
}

// Snapshot of the raw GET /scheduler/config document (`''` when the file does not
// exist yet). loadScheduleConfig stores it as `scheduleConfigBase`.
function schedConfigSnapshotText(config) {
  if (config == null) return '';
  return schedCanonicalJson(config);
}

function schedConfigSnapshotChanged(base, fresh) {
  return String(base == null ? '' : base) !== String(fresh == null ? '' : fresh);
}

function schedExternalConfigChangeText() {
  return '配置已被外部修改，请先【重新加载】';
}

function schedResponseErrorText(status, body) {
  const errors = body && Array.isArray(body.errors) ? body.errors.filter(Boolean).map(String) : [];
  const detail = errors.length ? errors.join('；') : '';
  if (status === 409) {
    return '调度器运行中：请先在外部停止守护进程（stop-scheduler.bat）再保存。' +
      (detail ? ' 服务端：' + detail : '');
  }
  if (status === 400) return '配置校验失败：' + (detail || '服务端返回 400');
  if (status === 500) return '写入失败（500）：' + (detail || '未知错误');
  if (status === 404) return '服务端未提供 /scheduler/config（需要 P3A 后端）：' + (detail || 'HTTP 404');
  if (!status) return '网络错误：' + (detail || '请求未送达');
  return `保存失败（HTTP ${status}）：` + (detail || '未知错误');
}

function schedSaveBlockedReason(running) {
  return running
    ? '调度器运行中：保存会返回 409，请先在外部运行 stop-scheduler.bat 再保存。'
    : '';
}

function schedParseConfigJson(text) {
  const raw = String(text == null ? '' : text).trim();
  if (!raw) return { ok: false, config: null, error: 'JSON 为空' };
  let v;
  try {
    v = JSON.parse(raw);
  } catch (e) {
    return { ok: false, config: null, error: 'JSON 解析失败：' + (e && e.message ? e.message : e) };
  }
  if (!v || typeof v !== 'object' || Array.isArray(v)) {
    return { ok: false, config: null, error: '配置必须是 JSON 对象' };
  }
  if (v.entries != null && !Array.isArray(v.entries)) {
    return { ok: false, config: null, error: 'entries 必须是数组' };
  }
  if (v.queues != null && !Array.isArray(v.queues)) {
    return { ok: false, config: null, error: 'queues 必须是数组' };
  }
  return { ok: true, config: schedNormalizeConfig(v), error: '' };
}

// --- P4: 调度运行记录（/runs/scheduler）纯函数 -------------------------------
// Contract (P4A): GET /runs/scheduler?limit=200&task=&outcome= returns
// {ok, dir, count, runs:[{run_id,file,task,kind,entry_id,queue_id,round,
//  member_index,attempt,started_ms,finished_ms,duration_ms,class,outcome,exit,
//  exit_code,clicks,error}]} newest→oldest; GET /runs/scheduler/run?run_id=
// returns one full run. Legacy GET /runs (clicks JSONL) is untouched.
// Keep every helper self-contained: ui_pure_test extracts functions one by one.
function schedRunValueText(value) {
  if (value === undefined || value === null || value === '') return '—';
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  try {
    const text = JSON.stringify(value);
    return text == null ? '—' : text;
  } catch (e) {
    return String(value);
  }
}

function schedRunLimitValue(value) {
  const allowed = [100, 200, 500, 1000];
  const n = Number(value);
  return allowed.indexOf(n) >= 0 ? n : 200;
}

function schedRunsBuildQuery(opts) {
  // P4A filters on the persisted `outcome` (Done/Failed/Exhausted/error). The
  // plan's class words (success/resource/blocked/retryable/fatal) live in the
  // separate `class` field, which the list endpoint cannot filter; those are
  // applied locally by schedRunsFilterLocal so both spellings work.
  const classOutcomes = ['success', 'resource', 'blocked', 'retryable', 'fatal'];
  const persistedOutcomes = ['Done', 'Failed', 'Exhausted', 'error'];
  const o = opts || {};
  const parts = ['limit=' + schedRunLimitValue(o.limit)];
  const task = String(o.task == null ? '' : o.task).trim();
  if (task) parts.push('task=' + encodeURIComponent(task));
  const rawOutcome = String(o.outcome == null ? '' : o.outcome).trim();
  if (rawOutcome) {
    const isClassWord = classOutcomes.some((v) => v.toLowerCase() === rawOutcome.toLowerCase());
    if (!isClassWord) {
      const canonical = persistedOutcomes.find((v) => v.toLowerCase() === rawOutcome.toLowerCase()) || '';
      if (canonical) parts.push('outcome=' + encodeURIComponent(canonical));
    }
  }
  return parts.join('&');
}

function schedRunsBuildUrl(opts) {
  return '/runs/scheduler?' + schedRunsBuildQuery(opts);
}

function schedRunDetailUrl(runId) {
  return '/runs/scheduler/run?run_id=' + encodeURIComponent(String(runId == null ? '' : runId).trim());
}

function schedRunTimeText(ms) {
  if (ms === undefined || ms === null || ms === '') return '—';
  const n = Number(ms);
  if (!isFinite(n) || n <= 0) return '—';
  try {
    return new Date(n).toLocaleString('zh-CN', { hour12: false });
  } catch (e) {
    return String(ms);
  }
}

function schedRunDurationMs(run) {
  const r = run || {};
  const direct = Number(r.duration_ms);
  if (r.duration_ms !== undefined && r.duration_ms !== null && r.duration_ms !== '' &&
      isFinite(direct) && direct >= 0) {
    return direct;
  }
  const start = Number(r.started_ms);
  const end = Number(r.finished_ms);
  if (r.started_ms !== undefined && r.started_ms !== null && r.started_ms !== '' &&
      r.finished_ms !== undefined && r.finished_ms !== null && r.finished_ms !== '' &&
      isFinite(start) && isFinite(end) && end >= start) {
    return end - start;
  }
  return null;
}

function schedRunDurationText(ms) {
  if (ms === undefined || ms === null || ms === '') return '—';
  const n = Number(ms);
  if (!isFinite(n) || n < 0) return '—';
  if (n < 1000) return Math.round(n) + 'ms';
  if (n < 60000) return (n / 1000).toFixed(1) + 's';
  const total = Math.round(n / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return h + 'h' + m + 'm';
  return m + 'm' + s + 's';
}

function schedRunOutcomeLabel(outcome) {
  const raw = String(outcome == null ? '' : outcome).trim();
  const key = raw.toLowerCase();
  const labels = {
    success: '成功',
    done: '完成',
    resource: '资源不足',
    blocked: '被阻断',
    retryable: '可重试',
    fatal: '致命',
    failed: '失败',
    exhausted: '耗尽',
    error: '错误',
  };
  return labels[key] || raw || '未知';
}

function schedRunOutcomeBadgeClass(outcome) {
  const key = String(outcome == null ? '' : outcome).trim().toLowerCase();
  if (key === 'success' || key === 'done') return 'ok';
  if (key === 'fatal' || key === 'failed' || key === 'error') return 'err';
  if (key === 'resource' || key === 'blocked' || key === 'retryable' || key === 'exhausted') return 'warn';
  return 'kind';
}

// Colour by the failure class first (success/resource/blocked/retryable/fatal),
// then by the persisted outcome (Done/Failed/Exhausted/error) for older files.
function schedRunStatusBadgeClass(run) {
  const r = run || {};
  const byClass = schedRunOutcomeBadgeClass(r.class);
  if (byClass !== 'kind') return byClass;
  return schedRunOutcomeBadgeClass(r.outcome);
}

function schedRunKindText(kind) {
  const key = String(kind == null ? '' : kind).trim().toLowerCase();
  if (key === 'queue_member') return 'queue_member';
  return key === 'queue' ? 'queue' : 'task';
}

function schedRunIsQueueRun(run) {
  const kind = schedRunKindText((run || {}).kind);
  return kind === 'queue' || kind === 'queue_member';
}

function schedQueueDisplayName(cfg, queueId) {
  const id = String(queueId == null ? '' : queueId).trim();
  if (!id) return '';
  for (const q of (cfg && cfg.queues) || []) {
    if (!q || String(q.queue_id == null ? '' : q.queue_id) !== id) continue;
    const name = String(q.name == null ? '' : q.name).trim();
    return name && name !== id ? name : id;
  }
  return id;
}

function schedRunTaskLabel(run, cfg) {
  const r = run || {};
  const task = String(r.task == null ? '' : r.task).trim() || '—';
  if (!schedRunIsQueueRun(r)) return task;
  const queueId = String(r.queue_id == null ? '' : r.queue_id).trim();
  const queueName = schedQueueDisplayName(cfg, queueId) || queueId;
  return queueName ? queueName + '/' + task : task;
}

function schedRunRoundMemberText(run) {
  const r = run || {};
  if (!schedRunIsQueueRun(r)) return '—';
  const bits = [];
  if (r.round !== undefined && r.round !== null && r.round !== '') bits.push('第' + r.round + '轮');
  const mi = Number(r.member_index);
  if (r.member_index !== undefined && r.member_index !== null && r.member_index !== '' &&
      isFinite(mi) && mi >= 0) {
    bits.push('成员' + (Math.floor(mi) + 1));
  }
  return bits.length ? bits.join(' · ') : '—';
}

function schedRunAttemptText(run) {
  const r = run || {};
  const n = Number(r.attempt);
  if (r.attempt === undefined || r.attempt === null || r.attempt === '' || !isFinite(n) || n <= 0) {
    return '—';
  }
  return String(Math.floor(n));
}

function schedRunExitText(run) {
  const r = run || {};
  if (r.exit_code !== undefined && r.exit_code !== null && r.exit_code !== '') {
    return schedRunValueText(r.exit_code);
  }
  if (r.exit !== undefined && r.exit !== null && r.exit !== '') return schedRunValueText(r.exit);
  return '—';
}

function schedRunClicksText(run) {
  const r = run || {};
  const c = r.clicks;
  if (c === undefined || c === null || c === '') return '—';
  if (typeof c === 'number' || typeof c === 'string') return String(c);
  return schedRunValueText(c);
}

function schedRunErrorText(error) {
  if (error === undefined || error === null) return '';
  if (typeof error === 'string') return error.trim();
  if (typeof error === 'number' || typeof error === 'boolean') return String(error);
  try {
    const text = JSON.stringify(error);
    return text == null ? '' : text;
  } catch (e) {
    return String(error);
  }
}

function schedRunErrorSummary(error, maxLen) {
  const text = schedRunErrorText(error).replace(/\s+/g, ' ').trim();
  if (!text) return '';
  const limit = Number(maxLen) > 0 ? Math.floor(Number(maxLen)) : 80;
  return text.length > limit ? text.slice(0, Math.max(0, limit - 1)) + '…' : text;
}

function schedRunResumeText(run) {
  const r = run || {};
  if (r.resume === true) return '是';
  if (r.resume === false) return '否';
  return '—';
}

function schedRunDetailRows(run) {
  const r = run || {};
  const rows = [];
  const known = {};
  const add = (key, value) => {
    known[key] = true;
    rows.push([key, value]);
  };
  add('run_id', schedRunValueText(r.run_id));
  add('file', schedRunValueText(r.file));
  add('schema_version', schedRunValueText(r.schema_version));
  add('source', schedRunValueText(r.source));
  add('task', schedRunValueText(r.task));
  add('kind', schedRunKindText(r.kind));
  add('entry_id', schedRunValueText(r.entry_id));
  add('queue_id', schedRunValueText(r.queue_id));
  add('round', schedRunValueText(r.round));
  add('member_index', schedRunValueText(r.member_index));
  add('attempt', schedRunAttemptText(r));
  add('class', schedRunValueText(r.class));
  const rawOutcome = schedRunValueText(r.outcome);
  add('outcome', rawOutcome === '—'
    ? schedRunOutcomeLabel(r.outcome)
    : schedRunOutcomeLabel(r.outcome) + '（' + rawOutcome + '）');
  add('exit', schedRunExitText(r));
  add('exit_code', schedRunValueText(r.exit_code));
  add('clicks', schedRunClicksText(r));
  const durationMs = schedRunDurationMs(r);
  add('duration_ms', durationMs == null
    ? '—'
    : schedRunValueText(durationMs) + '（' + schedRunDurationText(durationMs) + '）');
  add('started_ms', r.started_ms === undefined || r.started_ms === null || r.started_ms === ''
    ? '—'
    : schedRunValueText(r.started_ms) + '（' + schedRunTimeText(r.started_ms) + '）');
  add('finished_ms', r.finished_ms === undefined || r.finished_ms === null || r.finished_ms === ''
    ? '—'
    : schedRunValueText(r.finished_ms) + '（' + schedRunTimeText(r.finished_ms) + '）');
  add('resume', schedRunResumeText(r));
  add('clicks_file', schedRunValueText(r.clicks_file));
  add('error', schedRunErrorText(r.error) || '—');
  // "全字段"：unknown/future keys are appended (sorted) instead of being hidden.
  for (const key of Object.keys(r).sort()) {
    if (!known[key]) rows.push([key, schedRunValueText(r[key])]);
  }
  return rows;
}

function schedRunDetailTitle(run, cfg) {
  const r = run || {};
  return '运行详情 · ' + schedRunTaskLabel(r, cfg) + ' · ' +
    schedRunOutcomeLabel(r.outcome) + ' · ' + schedRunTimeText(r.started_ms);
}

function schedRunRowCells(run, cfg) {
  const r = run || {};
  return {
    time: schedRunTimeText(r.started_ms),
    task: schedRunTaskLabel(r, cfg),
    kind: schedRunKindText(r.kind),
    round_member: schedRunRoundMemberText(r),
    attempt: schedRunAttemptText(r),
    class: schedRunValueText(r.class),
    outcome: schedRunOutcomeLabel(r.outcome),
    outcome_class: schedRunStatusBadgeClass(r),
    exit: schedRunExitText(r),
    duration: schedRunDurationText(schedRunDurationMs(r)),
    clicks: schedRunClicksText(r),
    error: schedRunErrorSummary(r.error, 80),
    error_full: schedRunErrorText(r.error),
  };
}

function schedRunIsFailure(run) {
  const r = run || {};
  const cls = String(r.class == null ? '' : r.class).trim().toLowerCase();
  if (cls) return cls !== 'success';
  const outcome = String(r.outcome == null ? '' : r.outcome).trim().toLowerCase();
  return !!outcome && outcome !== 'done' && outcome !== 'success';
}

function schedRunsFailureCount(runs) {
  let n = 0;
  for (const r of (Array.isArray(runs) ? runs : [])) {
    if (schedRunIsFailure(r)) n += 1;
  }
  return n;
}

function schedRunsStatusCardText(data, error) {
  if (error) return '运行记录：读取失败（见下方运行记录区）';
  if (!data) return '运行记录：—';
  const runs = Array.isArray(data.runs) ? data.runs : [];
  if (!runs.length) return '运行记录：暂无';
  const last = runs[0] || {};
  return '运行记录：最近 ' + schedRunOutcomeLabel(last.outcome) + ' · ' +
    (String(last.task == null ? '' : last.task).trim() || '?') + ' · ' +
    schedRunTimeText(last.started_ms) + ' · 失败 ' +
    schedRunsFailureCount(runs) + '/' + runs.length;
}

function schedRunBodyErrors(body) {
  if (!body || typeof body !== 'object') return '';
  if (Array.isArray(body.errors) && body.errors.length) {
    return body.errors.filter((e) => e !== null && e !== undefined && e !== '').map((e) => String(e)).join('；');
  }
  if (body.error !== undefined && body.error !== null && body.error !== '') return String(body.error);
  if (body.message !== undefined && body.message !== null && body.message !== '') return String(body.message);
  return '';
}

function schedRunsErrorText(reason) {
  const text = String(reason == null ? '' : reason).trim();
  return '运行记录读取失败：' + (text || '未知错误');
}

function schedRunsResponseErrorText(status, body) {
  const detail = schedRunBodyErrors(body);
  const suffix = detail ? '：' + detail : '';
  if (!status) return '运行记录读取失败：网络错误' + suffix;
  if (status === 404) return '运行记录读取失败：服务端未提供 /runs/scheduler（需要 P4A 后端）' + suffix;
  return '运行记录读取失败：HTTP ' + status + suffix;
}

function schedRunDetailErrorText(status, body) {
  const detail = schedRunBodyErrors(body);
  const suffix = detail ? '：' + detail : '';
  if (!status) return '详情读取失败：网络错误' + suffix;
  if (status === 400) return '详情读取失败：run_id 无效（HTTP 400）' + suffix;
  if (status === 404) return '详情读取失败：运行记录不存在或已被删除（HTTP 404）' + suffix;
  return '详情读取失败：HTTP ' + status + suffix;
}

function schedRunsTaskChoices(status, cfg) {
  const seen = {};
  const out = [];
  const add = (value) => {
    const text = String(value == null ? '' : value).trim();
    if (!text || seen[text]) return;
    seen[text] = true;
    out.push(text);
  };
  for (const c of (status && status.task_choices) || []) add(c);
  for (const e of (status && status.entries) || []) {
    if (!e || String(e.kind || '') === 'queue') continue;
    add(e.task);
  }
  for (const e of (cfg && cfg.entries) || []) {
    if (!e || String(e.kind || '') === 'queue') continue;
    add(e.task);
  }
  for (const q of (cfg && cfg.queues) || []) {
    for (const m of (q && q.members) || []) add(m);
  }
  return out;
}

function schedRunsFilterLocal(runs, opts) {
  const o = opts || {};
  const task = String(o.task == null ? '' : o.task).trim();
  const outcome = String(o.outcome == null ? '' : o.outcome).trim().toLowerCase();
  const queueId = String(o.queueId == null ? '' : o.queueId).trim();
  return (Array.isArray(runs) ? runs : []).filter((run) => {
    if (!run) return false;
    if (task && String(run.task == null ? '' : run.task) !== task) return false;
    if (outcome) {
      // Match the persisted outcome (Done/Failed/...) or the result class
      // (success/resource/...); the two live in separate run fields.
      const runOutcome = String(run.outcome == null ? '' : run.outcome).trim().toLowerCase();
      const runClass = String(run.class == null ? '' : run.class).trim().toLowerCase();
      if (runOutcome !== outcome && runClass !== outcome) return false;
    }
    if (queueId && String(run.queue_id == null ? '' : run.queue_id) !== queueId) return false;
    return true;
  });
}

function schedRunsFilterActive(filter) {
  const f = filter || {};
  return !!String(f.task == null ? '' : f.task).trim() ||
    !!String(f.outcome == null ? '' : f.outcome).trim() ||
    !!String(f.queueId == null ? '' : f.queueId).trim();
}

function schedRunsListStatusText(opts) {
  const o = opts || {};
  if (o.error) return String(o.error);
  const data = o.data || null;
  const filter = o.filter || {};
  if (!data) return '运行记录：—';
  const rows = Array.isArray(o.rows) ? o.rows : null;
  const total = Number.isFinite(Number(data.count))
    ? Number(data.count)
    : (Array.isArray(data.runs) ? data.runs.length : 0);
  const parts = ['共 ' + total + ' 条'];
  if (filter.task) parts.push('任务=' + String(filter.task));
  if (filter.outcome) {
    const classWords = ['success', 'resource', 'blocked', 'retryable', 'fatal'];
    const isClassWord = classWords.indexOf(String(filter.outcome).trim().toLowerCase()) >= 0;
    parts.push('结果=' + schedRunOutcomeLabel(filter.outcome) + (isClassWord ? '（本地筛选）' : ''));
  }
  if (filter.queueId) parts.push('队列=' + String(filter.queueId) + '（本地筛选）');
  if (rows && rows.length !== total && schedRunsFilterActive(filter)) {
    parts.push('显示 ' + rows.length + ' 条');
  }
  return '运行记录：' + parts.join(' · ');
}

// --- P3: 调度页/对话框（DOM 层） --------------------------------------------
function setScheduleBusy(busy) {
  scheduleBusy = !!busy;
  for (const el of [pageSchedSave, pageSchedReload, pageSchedAddEntry, pageSchedAddQueue,
    pageSchedJsonSave, pageSchedJsonReload, pageSchedRunsRefresh]) {
    if (el) el.disabled = !!busy;
  }
}

function scheduleLocked() {
  return !!(scheduleConfigMeta && scheduleConfigMeta.running);
}

// One raw GET /scheduler/config. Never mutates page state: page loading and the
// P1-4 pre-save check decide separately how a failure is surfaced.
async function fetchScheduleConfigOnce() {
  try {
    const r = await fetch('/scheduler/config');
    let body = null;
    try { body = await r.json(); } catch (e) { body = null; }
    if (!r.ok || !body || body.ok === false) {
      const error = (body && Array.isArray(body.errors) && body.errors.length)
        ? body.errors.join('；')
        : (r.status === 404
          ? '服务端未提供 /scheduler/config（需要 P3A 后端）'
          : `HTTP ${r.status}`);
      return { ok: false, status: r.status, body, error };
    }
    return { ok: true, status: r.status, body, error: '' };
  } catch (e) {
    return { ok: false, status: 0, body: null, error: '网络错误：' + e };
  }
}

async function loadScheduleConfig(force) {
  if (scheduleConfig && !force) return scheduleConfig;
  const res = await fetchScheduleConfigOnce();
  if (!res.ok) {
    scheduleConfig = null;
    scheduleConfigMeta = null;
    scheduleConfigBase = '';
    scheduleConfigError = res.error;
    log('读取调度配置失败：' + scheduleConfigError, 'ERROR');
    return null;
  }
  scheduleConfigError = '';
  scheduleConfigMeta = res.body;
  scheduleConfig = schedNormalizeConfig(res.body.config);
  // P1-4: remember what this page state is based on, so a later save can refuse
  // to overwrite a file that changed in the meantime.
  scheduleConfigBase = schedConfigSnapshotText(res.body.config);
  if (!res.body.config) {
    log('调度配置文件不存在（' + (res.body.path || 'scheduler.json') + '）；保存后会创建', 'WARN');
  }
  return scheduleConfig;
}

async function ensureScheduleTaskChoices() {
  if (scheduleTasksCache.length) return scheduleTasksCache;
  try {
    const r = await fetch('/task/library');
    const s = await r.json();
    if (s && s.ok) scheduleTasksCache = (s.tasks || []).map((t) => t.name).filter(Boolean);
  } catch (e) {
    log('读取任务库失败：' + e, 'WARN');
  }
  return scheduleTasksCache;
}

async function postScheduleConfig(cfg) {
  try {
    const r = await fetch('/scheduler/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ config: cfg }),
    });
    let body = null;
    try { body = await r.json(); } catch (e) { body = null; }
    return { ok: r.ok && !!body && body.ok !== false, status: r.status, body };
  } catch (e) {
    return { ok: false, status: 0, body: { errors: [String(e)] }, network: true };
  }
}

function scheduleStatusEntryMap(status) {
  const map = {};
  for (const e of (status && status.entries) || []) map[e.entry_id] = e;
  return map;
}

function renderScheduleStatusCard(s) {
  const status = s || {};
  const meta = scheduleConfigMeta || {};
  // P1-3: with a failed /scheduler/status read the scheduler state is unknown.
  // Show `—` everywhere and put the error into the summary instead of rendering
  // the healthy-looking defaults (passes=0 / 锁=空闲 / quota：无限制).
  const statusError = String(scheduleStatusError || '');
  const known = !statusError;
  if (pageSchedMode) pageSchedMode.textContent = `模式：${known ? status.mode || '—' : '—'}`;
  if (pageSchedPasses) pageSchedPasses.textContent = `passes=${known ? (status.passes ?? 0) : '—'}`;
  if (pageSchedNextWake) {
    pageSchedNextWake.textContent = `下次唤醒=${known ? (status.next_wake_local || fmtLocal(status.next_wake_ms)) : '—'}`;
  }
  if (pageSchedQuota) pageSchedQuota.textContent = known ? schedQuotaSummary(status) : 'quota：—';
  if (pageSchedRunning) {
    if (!known) {
      pageSchedRunning.textContent = '锁=—';
      pageSchedRunning.className = 'badge';
    } else {
      const lock = scheduleLocked() ? '锁=调度器运行中' : '锁=空闲';
      const run = status.running
        ? ` · 正在执行 ${status.running.entry_id || '?'}/${status.running.task || '?'}`
        : '';
      pageSchedRunning.textContent = lock + run;
      pageSchedRunning.className = 'badge ' + (scheduleLocked() ? 'warn' : 'ok');
    }
  }
  const parts = [];
  if (!known) {
    parts.push(schedStatusErrorText(statusError));
  } else {
    const guards = status.guards || {};
    if (guards.blocked) {
      parts.push('护栏=已阻断' + (guards.blackout_reason ? '（' + guards.blackout_reason + '）' : ''));
    }
    if (status.last_stop) {
      parts.push('上次停止=' + (typeof status.last_stop === 'string' ? status.last_stop : JSON.stringify(status.last_stop)));
    }
  }
  if (scheduleConfigError) parts.push('配置读取失败：' + scheduleConfigError);
  if (pageSchedSummary) pageSchedSummary.textContent = parts.join(' · ') || '调度器状态正常';
  if (pageSchedConfigStatus) {
    const bits = [];
    if (meta.path) bits.push('路径=' + meta.path);
    if (meta.exists === false) bits.push('文件不存在（首次保存会创建）');
    if (meta.state_path) bits.push('state=' + meta.state_path);
    if (meta.schema_version) bits.push('schema=' + meta.schema_version);
    if (scheduleLocked()) bits.push('运行中：保存会被 409 拒绝，请先 stop-scheduler.bat');
    if (scheduleConfigError) bits.push('错误=' + scheduleConfigError);
    pageSchedConfigStatus.textContent = bits.join(' · ') || '配置：—';
  }
  // P4: latest run + failure count from the loaded list (soft: read failure
  // only changes this line; the rest of the card is unaffected).
  if (pageSchedRunsSummary) {
    pageSchedRunsSummary.textContent = schedRunsStatusCardText(scheduleRunsData, scheduleRunsError);
  }
}

function renderScheduleEntries(status, cfg) {
  if (!pageSchedEntries) return;
  pageSchedEntries.textContent = '';
  if (!cfg) {
    pageSchedEntries.textContent = '（调度配置不可用：' + (scheduleConfigError || '未知') + '）';
    return;
  }
  const entries = cfg.entries || [];
  if (!entries.length) {
    pageSchedEntries.textContent = '（还没有调度条目；到任务库点「调度设置…」或点「+ 添加条目…」）';
    return;
  }
  const stMap = scheduleStatusEntryMap(status);
  const queueStatus = {};
  for (const q of (status && status.queues) || []) queueStatus[q.queue_id] = q;
  entries.forEach((e, i) => {
    const card = document.createElement('div');
    card.className = 'task-card';
    const meta = document.createElement('div');
    meta.className = 'task-meta';
    const title = document.createElement('div');
    title.className = 'task-title';
    const isQueue = schedEntryKindLabel(e.kind) === 'queue';
    title.textContent = `${e.entry_id} · ${isQueue ? '队列' : '任务'} ${e.task}`;
    const sub = document.createElement('div');
    sub.className = 'task-sub';
    sub.textContent = schedEntrySummary(e, stMap[e.entry_id]);
    meta.append(title, sub);

    const badges = document.createElement('div');
    badges.className = 'row';
    badges.appendChild(schedBadge(isQueue ? 'queue' : 'task', 'kind'));
    badges.appendChild(schedBadge(e.lifecycle || 'periodic', 'kind'));
    if (e.enabled === false) badges.appendChild(schedBadge('已禁用', 'warn'));
    const st = stMap[e.entry_id];
    if (st && st.runnable_now) badges.appendChild(schedBadge('现在可跑', 'ok'));
    else if (st && st.blocked_reason) badges.appendChild(schedBadge('等待：' + st.blocked_reason, 'warn'));
    if (isQueue) {
      const progress = (st && st.queue) || queueStatus[e.task];
      if (progress) {
        badges.appendChild(schedBadge(schedQueueProgressText(progress, progress), 'kind'));
        if (progress.finished) badges.appendChild(schedBadge('队列已完成', 'warn'));
      }
    }
    meta.appendChild(badges);
    card.appendChild(meta);

    const enableLabel = document.createElement('label');
    enableLabel.className = 'opt small';
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.checked = e.enabled !== false;
    cb.title = '启用/禁用该调度条目（立即保存）';
    cb.addEventListener('change', () => toggleScheduleEntry(e.entry_id, cb.checked));
    enableLabel.append(cb, document.createTextNode(' 启用'));

    const up = document.createElement('button');
    up.type = 'button';
    up.textContent = '上移';
    up.disabled = i === 0;
    up.addEventListener('click', () => moveScheduleEntry(e.entry_id, -1));
    const down = document.createElement('button');
    down.type = 'button';
    down.textContent = '下移';
    down.disabled = i === entries.length - 1;
    down.addEventListener('click', () => moveScheduleEntry(e.entry_id, 1));
    const edit = document.createElement('button');
    edit.type = 'button';
    edit.textContent = '编辑';
    edit.addEventListener('click', () => openScheduleEntryDialog({ mode: 'edit', entryId: e.entry_id, entry: e }));
    // P4: one-click run history filtered to this entry (task or queue member).
    const runsBtn = document.createElement('button');
    runsBtn.type = 'button';
    runsBtn.textContent = '运行记录';
    runsBtn.title = isQueue ? '筛选该队列的运行记录' : '筛选该任务的运行记录';
    runsBtn.addEventListener('click', () => showScheduleRunsForEntry(e));
    const del = document.createElement('button');
    del.type = 'button';
    del.textContent = '删除';
    del.addEventListener('click', () => deleteScheduleEntry(e.entry_id, isQueue));
    card.append(enableLabel, up, down, runsBtn, edit, del);
    pageSchedEntries.appendChild(card);
  });
}

function renderScheduleQueues(status, cfg) {
  if (!pageSchedQueues) return;
  pageSchedQueues.textContent = '';
  if (!cfg) {
    pageSchedQueues.textContent = '（调度配置不可用：' + (scheduleConfigError || '未知') + '）';
    return;
  }
  const queues = cfg.queues || [];
  if (!queues.length) {
    pageSchedQueues.textContent = '（还没有队列；点「+ 新建队列…」把任务按顺序串起来）';
    return;
  }
  const progress = {};
  for (const q of (status && status.queues) || []) progress[q.queue_id] = q;
  for (const q of queues) {
    const card = document.createElement('div');
    card.className = 'task-card';
    const meta = document.createElement('div');
    meta.className = 'task-meta';
    const title = document.createElement('div');
    title.className = 'task-title';
    title.textContent = `${q.queue_id}${q.name && q.name !== q.queue_id ? ' · ' + q.name : ''}`;
    const sub = document.createElement('div');
    sub.className = 'task-sub';
    sub.textContent = schedQueueSummary(q, progress[q.queue_id]);
    meta.append(title, sub);
    const badges = document.createElement('div');
    badges.className = 'row';
    const entryId = schedQueueEntryConflict(cfg, q.queue_id);
    badges.appendChild(entryId
      ? schedBadge('触发条目：' + entryId, 'kind')
      : schedBadge('无触发条目（不会被调度）', 'warn'));
    const prog = progress[q.queue_id];
    if (prog && prog.finished) badges.appendChild(schedBadge('已完成', 'warn'));
    if (q.total_rounds === 0) badges.appendChild(schedBadge('无限轮', 'kind'));
    meta.appendChild(badges);
    card.appendChild(meta);
    const edit = document.createElement('button');
    edit.type = 'button';
    edit.textContent = '编辑';
    edit.addEventListener('click', () => openScheduleQueueDialog(q.queue_id));
    // P4: queue members' runs share queue_id; filter them locally.
    const runsBtn = document.createElement('button');
    runsBtn.type = 'button';
    runsBtn.textContent = '运行记录';
    runsBtn.title = '筛选该队列成员的运行记录（本地筛选）';
    runsBtn.addEventListener('click', () => showScheduleRunsForQueue(q.queue_id));
    const del = document.createElement('button');
    del.type = 'button';
    del.textContent = '删除';
    del.addEventListener('click', () => deleteScheduleQueue(q.queue_id));
    card.append(runsBtn, edit, del);
    pageSchedQueues.appendChild(card);
  }
}

// --- P4: 调度运行记录（列表 / 筛选 / 详情）DOM 层 ---------------------------
function populateScheduleRunsTaskOptions() {
  if (!pageSchedRunsTask) return;
  const choices = schedRunsTaskChoices(schedulerStatus, scheduleConfig);
  const current = String(scheduleRunsFilter.task || '');
  const values = choices.slice();
  if (current && values.indexOf(current) < 0) values.unshift(current);
  pageSchedRunsTask.textContent = '';
  const all = document.createElement('option');
  all.value = '';
  all.textContent = '全部任务';
  pageSchedRunsTask.appendChild(all);
  for (const value of values) {
    const opt = document.createElement('option');
    opt.value = value;
    opt.textContent = value;
    pageSchedRunsTask.appendChild(opt);
  }
  pageSchedRunsTask.value = values.indexOf(current) >= 0 ? current : '';
}

function syncScheduleRunsControls() {
  if (pageSchedRunsTask) {
    const task = String(scheduleRunsFilter.task || '');
    const options = Array.from(pageSchedRunsTask.options || []);
    if (task && !options.some((opt) => opt.value === task)) {
      const opt = document.createElement('option');
      opt.value = task;
      opt.textContent = task;
      pageSchedRunsTask.appendChild(opt);
    }
    pageSchedRunsTask.value = task;
  }
  if (pageSchedRunsOutcome) pageSchedRunsOutcome.value = String(scheduleRunsFilter.outcome || '');
  if (pageSchedRunsLimit) pageSchedRunsLimit.value = String(schedRunLimitValue(scheduleRunsFilter.limit));
  if (pageSchedRunsClear) pageSchedRunsClear.disabled = !schedRunsFilterActive(scheduleRunsFilter);
}

function scrollScheduleRunsIntoView() {
  if (!pageSchedRunsList || typeof pageSchedRunsList.scrollIntoView !== 'function') return;
  try {
    pageSchedRunsList.scrollIntoView({ block: 'start', behavior: 'smooth' });
  } catch (e) {
    /* ignore: scrolling is cosmetic */
  }
}

// Task-library shortcut: open the schedule page with the run list filtered to
// this task. The filter survives the page refresh because refreshSchedulePage
// reads `scheduleRunsFilter` instead of resetting it.
function showScheduleRunsForTask(task) {
  const name = String(task == null ? '' : task).trim();
  scheduleRunsFilter.task = name;
  scheduleRunsFilter.outcome = '';
  scheduleRunsFilter.queueId = '';
  activatePage('schedule');
  syncScheduleRunsControls();
  scrollScheduleRunsIntoView();
}

// Entry-row shortcut: task entries filter server-side by task; queue entries
// filter their members' runs locally by queue_id (the list endpoint has no
// queue filter parameter in the P4A contract).
function showScheduleRunsForEntry(entry) {
  const e = entry || {};
  const isQueue = String(e.kind == null ? '' : e.kind) === 'queue';
  const target = String(e.task == null ? '' : e.task).trim();
  scheduleRunsFilter.task = isQueue ? '' : target;
  scheduleRunsFilter.outcome = '';
  scheduleRunsFilter.queueId = isQueue ? target : '';
  syncScheduleRunsControls();
  refreshScheduleRuns();
  scrollScheduleRunsIntoView();
}

function showScheduleRunsForQueue(queueId) {
  scheduleRunsFilter.task = '';
  scheduleRunsFilter.outcome = '';
  scheduleRunsFilter.queueId = String(queueId == null ? '' : queueId).trim();
  syncScheduleRunsControls();
  refreshScheduleRuns();
  scrollScheduleRunsIntoView();
}

function makeSchedRunBadge(text, cls) {
  const span = document.createElement('span');
  span.className = 'run-badge ' + (cls || '');
  span.textContent = String(text == null ? '' : text);
  return span;
}

function renderScheduleRuns() {
  if (!pageSchedRunsList) return;
  const runs = (scheduleRunsData && Array.isArray(scheduleRunsData.runs)) ? scheduleRunsData.runs : [];
  const rows = schedRunsFilterLocal(runs, scheduleRunsFilter);
  if (pageSchedRunsStatus) {
    pageSchedRunsStatus.textContent = scheduleRunsLoading
      ? '读取中…'
      : schedRunsListStatusText({
        data: scheduleRunsData,
        error: scheduleRunsError,
        filter: scheduleRunsFilter,
        rows,
      });
  }
  pageSchedRunsList.textContent = '';
  if (scheduleRunsError) {
    pageSchedRunsList.textContent = scheduleRunsError;
    return;
  }
  if (!scheduleRunsData) {
    pageSchedRunsList.textContent = '（未加载）';
    return;
  }
  if (!rows.length) {
    pageSchedRunsList.textContent = schedRunsFilterActive(scheduleRunsFilter)
      ? '（没有符合当前筛选的运行记录；点「清除筛选」看全部）'
      : '（暂无调度运行记录；调度器跑过一次后这里会出现记录）';
    return;
  }
  const table = document.createElement('table');
  table.className = 'sched-runs-table';
  const thead = document.createElement('thead');
  const headRow = document.createElement('tr');
  for (const label of ['时间', '任务', 'kind', '轮次/成员', 'attempt', 'class/outcome',
    'exit/exit_code', 'duration', 'clicks', 'error']) {
    const th = document.createElement('th');
    th.textContent = label;
    headRow.appendChild(th);
  }
  thead.appendChild(headRow);
  const tbody = document.createElement('tbody');
  for (const run of rows) {
    const cells = schedRunRowCells(run, scheduleConfig);
    const tr = document.createElement('tr');
    tr.tabIndex = 0;
    tr.title = '点击查看运行详情' + (run && run.run_id ? '：' + run.run_id : '');
    const open = () => openScheduleRunDetail(run);
    tr.addEventListener('click', open);
    tr.addEventListener('keydown', (ev) => {
      if (!ev || (ev.key !== 'Enter' && ev.key !== ' ')) return;
      if (ev.preventDefault) ev.preventDefault();
      open();
    });
    const addCell = (text, cls) => {
      const td = document.createElement('td');
      td.textContent = String(text == null ? '' : text);
      if (cls) td.className = cls;
      tr.appendChild(td);
      return td;
    };
    addCell(cells.time);
    addCell(cells.task);
    const kindCell = document.createElement('td');
    kindCell.appendChild(makeSchedRunBadge(cells.kind, 'kind'));
    tr.appendChild(kindCell);
    addCell(cells.round_member);
    addCell(cells.attempt);
    const outcomeCell = document.createElement('td');
    if (cells.class !== '—') outcomeCell.appendChild(makeSchedRunBadge(cells.class, 'kind'));
    outcomeCell.appendChild(makeSchedRunBadge(cells.outcome, cells.outcome_class));
    tr.appendChild(outcomeCell);
    addCell(cells.exit);
    addCell(cells.duration);
    addCell(cells.clicks);
    addCell(cells.error || '—', cells.error ? 'run-error-cell' : '');
    tbody.appendChild(tr);
  }
  table.appendChild(thead);
  table.appendChild(tbody);
  pageSchedRunsList.appendChild(table);
}

// List read. Always resolves (never rejects): the runs section is a soft
// dependency of the schedule page, and its failure is shown verbatim + logged
// as WARN instead of being swallowed.
async function refreshScheduleRuns() {
  if (!pageSchedRunsList) return { ok: false, error: '运行记录区不可用' };
  if (scheduleRunsLoading) return { ok: false, error: '运行记录读取中' };
  scheduleRunsLoading = true;
  if (pageSchedRunsRefresh) pageSchedRunsRefresh.disabled = true;
  if (pageSchedRunsStatus) pageSchedRunsStatus.textContent = '读取中…';
  const url = schedRunsBuildUrl(scheduleRunsFilter);
  try {
    const r = await fetch(url);
    let body = null;
    try { body = await r.json(); } catch (e) { body = null; }
    if (!r.ok || !body || body.ok === false) {
      scheduleRunsData = null;
      scheduleRunsError = schedRunsResponseErrorText(r.status, body);
      log(scheduleRunsError, 'WARN');
      return { ok: false, error: scheduleRunsError };
    }
    scheduleRunsData = body;
    scheduleRunsError = '';
    return { ok: true, data: body };
  } catch (e) {
    scheduleRunsData = null;
    scheduleRunsError = schedRunsErrorText('网络错误：' + (e && e.message ? e.message : e));
    log(scheduleRunsError, 'WARN');
    return { ok: false, error: scheduleRunsError };
  } finally {
    scheduleRunsLoading = false;
    if (pageSchedRunsRefresh) pageSchedRunsRefresh.disabled = false;
    renderScheduleRuns();
    if (pageSchedRunsSummary) {
      pageSchedRunsSummary.textContent = schedRunsStatusCardText(scheduleRunsData, scheduleRunsError);
    }
  }
}

// Single-run read for the detail dialog (GET /runs/scheduler/run?run_id=).
async function fetchScheduleRunOnce(runId) {
  try {
    const r = await fetch(schedRunDetailUrl(runId));
    let body = null;
    try { body = await r.json(); } catch (e) { body = null; }
    if (!r.ok || !body || body.ok === false) {
      return { ok: false, status: r.status, body, error: schedRunDetailErrorText(r.status, body) };
    }
    const run = (body.run && typeof body.run === 'object') ? body.run : null;
    if (!run) return { ok: false, status: r.status, body, error: '详情读取失败：响应缺少 run 对象' };
    return { ok: true, status: r.status, body, run, error: '' };
  } catch (e) {
    return {
      ok: false,
      status: 0,
      body: null,
      error: schedRunDetailErrorText(0, { errors: [String(e)] }),
    };
  }
}

function renderScheduleRunDetail(run) {
  if (!schedRunDialogBody) return;
  const r = run || {};
  if (schedRunDialogTitle) schedRunDialogTitle.textContent = schedRunDetailTitle(r, scheduleConfig);
  schedRunDialogBody.textContent = '';
  const table = document.createElement('table');
  table.className = 'sched-runs-table run-detail-table';
  const tbody = document.createElement('tbody');
  for (const [key, value] of schedRunDetailRows(r)) {
    const tr = document.createElement('tr');
    const label = document.createElement('td');
    label.textContent = key;
    const cell = document.createElement('td');
    cell.textContent = value;
    if (key === 'error') cell.className = 'dialog-error';
    tr.append(label, cell);
    tbody.appendChild(tr);
  }
  table.appendChild(tbody);
  schedRunDialogBody.appendChild(table);
  const errorText = schedRunErrorText(r.error);
  if (errorText) {
    const label = document.createElement('div');
    label.className = 'muted small';
    label.textContent = '错误全文';
    const pre = document.createElement('pre');
    pre.className = 'dialog-error';
    pre.textContent = errorText;
    schedRunDialogBody.append(label, pre);
  }
  const details = document.createElement('details');
  const summary = document.createElement('summary');
  summary.textContent = '完整 JSON（所有字段，含 resume / clicks_file）';
  const pre = document.createElement('pre');
  try {
    pre.textContent = JSON.stringify(r, null, 2);
  } catch (e) {
    pre.textContent = String(r);
  }
  details.append(summary, pre);
  schedRunDialogBody.appendChild(details);
}

function closeScheduleRunDetail() {
  scheduleRunDetailRunId = '';
  if (schedRunDialog && schedRunDialog.open) schedRunDialog.close();
}

async function openScheduleRunDetail(run) {
  if (!schedRunDialog) return;
  const runId = typeof run === 'string'
    ? String(run).trim()
    : String((run && run.run_id) || '').trim();
  scheduleRunDetailRunId = runId;
  if (schedRunDialogTitle) schedRunDialogTitle.textContent = '运行详情' + (runId ? ' · ' + runId : '');
  if (schedRunDialogBody) schedRunDialogBody.textContent = '读取中…';
  if (schedRunDialogError) schedRunDialogError.textContent = '';
  if (typeof schedRunDialog.showModal === 'function' && !schedRunDialog.open) {
    try { schedRunDialog.showModal(); } catch (e) { /* already open */ }
  }
  if (!runId) {
    const msg = '详情读取失败：该运行记录缺少 run_id';
    if (schedRunDialogBody) schedRunDialogBody.textContent = '';
    if (schedRunDialogError) schedRunDialogError.textContent = msg;
    log(msg, 'WARN');
    return;
  }
  const res = await fetchScheduleRunOnce(runId);
  if (scheduleRunDetailRunId !== runId) return; // a newer row click supersedes this read
  if (!res.ok) {
    if (schedRunDialogBody) schedRunDialogBody.textContent = '';
    if (schedRunDialogError) schedRunDialogError.textContent = res.error;
    log(res.error, 'WARN');
    return;
  }
  if (schedRunDialogError) schedRunDialogError.textContent = '';
  renderScheduleRunDetail(res.run);
}

async function toggleScheduleEntry(entryId, enabled) {
  if (scheduleBusy) return;
  const cfg = await loadScheduleConfig(true);
  if (!cfg) {
    log('启用/禁用失败：调度配置不可用', 'ERROR');
    return;
  }
  const idx = (cfg.entries || []).findIndex((e) => e.entry_id === entryId);
  if (idx < 0) {
    log('启用/禁用失败：条目「' + entryId + '」不存在（可能已被外部修改）', 'WARN');
    await refreshSchedulePage();
    return;
  }
  cfg.entries[idx].enabled = !!enabled;
  const res = await postScheduleConfig(cfg);
  if (!res.ok) {
    const msg = schedResponseErrorText(res.status, res.body);
    log('保存调度条目失败：' + msg, 'WARN');
    if (pageSchedConfigStatus) pageSchedConfigStatus.textContent = msg;
    await refreshSchedulePage();
    return;
  }
  log(`已${enabled ? '启用' : '禁用'}调度条目「${entryId}」`);
  await refreshSchedulePage();
}

async function moveScheduleEntry(entryId, delta) {
  if (scheduleBusy) return;
  const cfg = await loadScheduleConfig(true);
  if (!cfg) {
    log('排序失败：调度配置不可用', 'ERROR');
    return;
  }
  const index = (cfg.entries || []).findIndex((e) => e.entry_id === entryId);
  if (index < 0) {
    log('排序失败：条目「' + entryId + '」不存在', 'WARN');
    return;
  }
  const next = schedMoveEntry(cfg, index, delta);
  if (next.entries[index] && next.entries[index].entry_id === entryId) return; // already at the edge
  const res = await postScheduleConfig(next);
  if (!res.ok) {
    const msg = schedResponseErrorText(res.status, res.body);
    log('保存条目顺序失败：' + msg, 'WARN');
    if (pageSchedConfigStatus) pageSchedConfigStatus.textContent = msg;
    return;
  }
  log(`已移动调度条目「${entryId}」（entries 顺序 = 执行顺序）`);
  await refreshSchedulePage();
}

async function deleteScheduleEntry(entryId, isQueue) {
  if (scheduleBusy) return;
  const note = isQueue ? '\n只删除触发条目，队列本身和成员任务不受影响。' : '';
  if (!window.confirm(`删除调度条目「${entryId}」？${note}`)) return;
  const cfg = await loadScheduleConfig(true);
  if (!cfg) {
    log('删除失败：调度配置不可用', 'ERROR');
    return;
  }
  if (!(cfg.entries || []).some((e) => e.entry_id === entryId)) {
    log('删除失败：条目「' + entryId + '」不存在', 'WARN');
    await refreshSchedulePage();
    return;
  }
  const res = await postScheduleConfig(schedRemoveEntryById(cfg, entryId));
  if (!res.ok) {
    const msg = schedResponseErrorText(res.status, res.body);
    log('删除调度条目失败：' + msg, 'WARN');
    if (pageSchedConfigStatus) pageSchedConfigStatus.textContent = msg;
    return;
  }
  log(`已删除调度条目「${entryId}」`);
  await refreshSchedulePage();
}

function populateScheduleEntryTargets(kind, selected) {
  if (!schedEntryTarget) return;
  const k = schedEntryKindLabel(kind);
  const value = String(selected || '');
  schedEntryTarget.textContent = '';
  const values = [];
  if (k === 'queue') {
    for (const q of (scheduleConfig && scheduleConfig.queues) || []) values.push(q.queue_id);
  } else {
    for (const t of scheduleTasksCache) values.push(t);
  }
  if (value && !values.includes(value)) values.unshift(value);
  if (!values.length) {
    const opt = document.createElement('option');
    opt.value = '';
    opt.textContent = k === 'queue' ? '（还没有队列：先到队列面板新建）' : '（任务库为空）';
    schedEntryTarget.appendChild(opt);
    return;
  }
  for (const v of values) {
    const opt = document.createElement('option');
    opt.value = v;
    opt.textContent = v;
    schedEntryTarget.appendChild(opt);
  }
  schedEntryTarget.value = value && values.includes(value) ? value : values[0];
}

function updateScheduleEntryTargetHint() {
  if (!schedEntryModeHint) return;
  const locked = schedSaveBlockedReason(scheduleLocked());
  if (locked) {
    schedEntryModeHint.textContent = '注意：' + locked;
    return;
  }
  const st = scheduleEntryDialogState;
  const kind = schedEntryKind ? schedEntryKind.value : 'task';
  const target = schedEntryTarget ? schedEntryTarget.value : '';
  const cfg = scheduleConfig || schedEmptyConfig();
  if (kind === 'queue' && target) {
    const other = schedQueueEntryConflict(cfg, target, st && st.entryId);
    if (other) {
      schedEntryModeHint.textContent =
        `队列「${target}」已有触发条目「${other}」：保存会被拒绝（一个队列最多一个 entry）。`;
      return;
    }
  }
  if (kind === 'task' && target) {
    const qid = schedFindTaskQueueMember(cfg, target);
    if (qid) {
      schedEntryModeHint.textContent = `任务「${target}」已在队列「${qid}」中：保存会被拒绝（防双跑）。`;
      return;
    }
  }
  schedEntryModeHint.textContent = st && st.mode === 'create'
    ? '新建条目：保存后追加到 entries 末尾（顺序 = 执行顺序）。'
    : '修改后保存会先 GET 当前配置再整体写回。';
}

function updateScheduleEntryLifecycleUi() {
  const temp = schedEntryLifecycle && schedEntryLifecycle.value === 'temp';
  for (const el of [schedEntryWindows, schedEntryWindowsJson, schedEntryQuotaPer, schedEntryQuotaTimes]) {
    if (el) el.disabled = temp;
  }
  if (schedEntryTempHint) {
    schedEntryTempHint.textContent = temp
      ? 'temp：每个 pass 按 entries 顺序执行一次；没有窗口/quota/完成记忆，保存时 windows 与 quota 会被清空。'
      : 'periodic：按窗口/quota/冷却触发；窗口留空 = 全天。';
  }
}

function readScheduleEntryForm() {
  return {
    kind: schedEntryKind ? schedEntryKind.value : 'task',
    task: schedEntryTarget ? schedEntryTarget.value : '',
    enabled: schedEntryEnabled ? schedEntryEnabled.checked : true,
    lifecycle: schedEntryLifecycle ? schedEntryLifecycle.value : 'periodic',
    windows_text: schedEntryWindows ? schedEntryWindows.value : '',
    windows_json: schedEntryWindowsJson ? schedEntryWindowsJson.value : '',
    quota_per: schedEntryQuotaPer ? schedEntryQuotaPer.value : '',
    quota_times: schedEntryQuotaTimes ? schedEntryQuotaTimes.value : '1',
    retry: schedEntryRetry ? schedEntryRetry.value : '1',
    cooldown: schedEntryCooldown ? schedEntryCooldown.value : '300',
    params_text: schedEntryParams ? schedEntryParams.value : '',
  };
}

function openScheduleEntryDialog(opts) {
  if (!schedEntryDialog) return;
  const o = opts || {};
  const cfg = scheduleConfig || schedEmptyConfig();
  const mode = o.mode === 'create' ? 'create' : 'edit';
  let entry = o.entry || null;
  if (!entry && o.entryId) {
    entry = (cfg.entries || []).find((e) => e.entry_id === o.entryId) || null;
  }
  if (mode === 'edit' && !entry) {
    log('打开调度条目失败：条目不存在', 'WARN');
    return;
  }
  if (!entry) entry = schedDefaultTaskEntry(o.task || scheduleTasksCache[0] || '', cfg, o.kind || 'task');
  scheduleEntryDialogState = {
    mode,
    entryId: mode === 'edit' ? entry.entry_id : '',
    baseEntry: entry,
  };
  const form = schedEntryToForm(entry, (cfg.config && cfg.config.defaults) || {});
  if (schedEntryTitle) {
    schedEntryTitle.textContent = o.title
      || (mode === 'create' ? '添加调度条目' : `编辑调度条目：${entry.entry_id}`);
  }
  if (schedEntrySave) schedEntrySave.textContent = mode === 'create' ? '加入调度' : '保存';
  if (schedEntryIdEl) {
    schedEntryIdEl.textContent = mode === 'create'
      ? 'entry_id：保存时自动生成（task-<任务名> / queue-<queue_id>）'
      : 'entry_id：' + entry.entry_id;
  }
  if (schedEntryEnabled) schedEntryEnabled.checked = form.enabled;
  if (schedEntryLifecycle) schedEntryLifecycle.value = form.lifecycle;
  if (schedEntryWindows) schedEntryWindows.value = (entry.windows && entry.windows.length) ? schedWindowsToText(entry.windows) : '';
  if (schedEntryWindowsJson) schedEntryWindowsJson.value = '';
  if (schedEntryQuotaPer) schedEntryQuotaPer.value = form.quota_per;
  if (schedEntryQuotaTimes) schedEntryQuotaTimes.value = form.quota_times;
  if (schedEntryRetry) schedEntryRetry.value = form.retry;
  if (schedEntryCooldown) schedEntryCooldown.value = form.cooldown;
  if (schedEntryParams) schedEntryParams.value = form.params_text;
  if (schedEntryKind) {
    schedEntryKind.value = form.kind;
    schedEntryKind.disabled = mode === 'edit';
  }
  populateScheduleEntryTargets(form.kind, form.task);
  if (schedEntryTarget) schedEntryTarget.disabled = mode === 'edit';
  updateScheduleEntryLifecycleUi();
  updateScheduleEntryTargetHint();
  if (schedEntryError) schedEntryError.textContent = '';
  if (schedEntryDialog.open) schedEntryDialog.close();
  schedEntryDialog.showModal();
}

async function openTaskScheduleDialog(task) {
  const t = String(task == null ? '' : task).trim();
  if (!t) return;
  await ensureScheduleTaskChoices();
  await loadScheduleConfig(true);
  if (!scheduleConfig) {
    window.alert('读取调度配置失败：' + (scheduleConfigError || '未知错误'));
    return;
  }
  const cfg = scheduleConfig;
  const hits = schedFindTaskEntries(cfg, t);
  const queueId = schedFindTaskQueueMember(cfg, t);
  if (!hits.length && queueId) {
    const msg = `任务「${t}」已是队列「${queueId}」的成员，不能再单独加入调度（会双跑）。\n请先在调度页调整队列成员。`;
    log(msg, 'WARN');
    window.alert(msg);
    return;
  }
  if (hits.length > 1) {
    const ids = hits.map((e) => e.entry_id).join('、');
    log(`任务「${t}」有 ${hits.length} 条调度条目（${ids}），只编辑第一条`, 'WARN');
    window.alert(`任务「${t}」有 ${hits.length} 条调度条目：${ids}。\n本次只编辑第一条；其余条目可在调度页查看/删除。`);
  }
  if (hits.length) {
    openScheduleEntryDialog({ mode: 'edit', entry: hits[0] });
  } else {
    openScheduleEntryDialog({
      mode: 'create',
      entry: schedDefaultTaskEntry(t, cfg, 'task'),
      title: `加入调度：${t}`,
    });
  }
}

async function openScheduleAddEntryDialog() {
  await ensureScheduleTaskChoices();
  await loadScheduleConfig(true);
  if (!scheduleConfig) {
    window.alert('读取调度配置失败：' + (scheduleConfigError || '未知错误'));
    return;
  }
  const target = scheduleTasksCache[0] || ((scheduleConfig.queues || [])[0] || {}).queue_id || '';
  openScheduleEntryDialog({
    mode: 'create',
    entry: schedDefaultTaskEntry(target, scheduleConfig, 'task'),
  });
}

async function saveScheduleEntryDialog() {
  const st = scheduleEntryDialogState;
  if (!st || !schedEntryDialog) return;
  if (schedEntryError) schedEntryError.textContent = '';
  const form = readScheduleEntryForm();
  const local = schedValidateEntryForm(form, scheduleConfig || schedEmptyConfig(), st.entryId, st.baseEntry);
  if (!local.ok) {
    if (schedEntryError) schedEntryError.textContent = local.errors.join('\n');
    return;
  }
  const fresh = await loadScheduleConfig(true);
  if (!fresh) {
    if (schedEntryError) schedEntryError.textContent = '读取调度配置失败：' + (scheduleConfigError || '未知错误');
    return;
  }
  const freshEntry = (fresh.entries || []).find((e) => e.entry_id === st.entryId) || st.baseEntry;
  const check = schedValidateEntryForm(form, fresh, st.entryId, freshEntry);
  if (!check.ok) {
    if (schedEntryError) schedEntryError.textContent = check.errors.join('\n');
    return;
  }
  let next;
  if (st.mode === 'create') {
    next = schedUpsertEntry(fresh, check.entry);
  } else {
    const idx = (fresh.entries || []).findIndex((e) => e.entry_id === st.entryId);
    if (idx < 0) {
      if (schedEntryError) schedEntryError.textContent = '条目已被外部删除，请关闭后重新打开。';
      return;
    }
    next = schedNormalizeConfig(fresh);
    next.entries[idx] = Object.assign({}, next.entries[idx], check.entry);
  }
  if (schedEntrySave) schedEntrySave.disabled = true;
  const res = await postScheduleConfig(next);
  if (schedEntrySave) schedEntrySave.disabled = false;
  if (!res.ok) {
    const msg = schedResponseErrorText(res.status, res.body);
    if (schedEntryError) schedEntryError.textContent = msg;
    log('保存调度条目失败：' + msg, res.status === 409 ? 'WARN' : 'ERROR');
    return;
  }
  schedEntryDialog.close();
  log(`已保存调度条目「${check.entry.entry_id}」${st.mode === 'create' ? '（新建）' : ''}`);
  await refreshSchedulePage();
}

function renderScheduleQueueMembers() {
  if (!schedQueueMembers) return;
  const st = scheduleQueueDialogState;
  const members = st ? st.members : [];
  schedQueueMembers.textContent = '';
  if (!members.length) {
    schedQueueMembers.textContent = '（还没有成员：从下方任务池勾选后点「添加勾选任务」）';
    return;
  }
  members.forEach((task, i) => {
    const row = document.createElement('div');
    row.className = 'row';
    const label = document.createElement('span');
    label.className = 'small';
    label.textContent = `${i + 1}. ${task}`;
    const up = document.createElement('button');
    up.type = 'button';
    up.textContent = '↑';
    up.disabled = i === 0;
    up.addEventListener('click', () => {
      st.members = schedQueueMemberMove(st.members, i, -1);
      renderScheduleQueueMembers();
      renderScheduleQueuePool();
    });
    const down = document.createElement('button');
    down.type = 'button';
    down.textContent = '↓';
    down.disabled = i === members.length - 1;
    down.addEventListener('click', () => {
      st.members = schedQueueMemberMove(st.members, i, 1);
      renderScheduleQueueMembers();
      renderScheduleQueuePool();
    });
    const rm = document.createElement('button');
    rm.type = 'button';
    rm.textContent = '移除';
    rm.addEventListener('click', () => {
      st.members = schedQueueMemberRemove(st.members, task);
      renderScheduleQueueMembers();
      renderScheduleQueuePool();
    });
    row.append(label, up, down, rm);
    schedQueueMembers.appendChild(row);
  });
}

function renderScheduleQueuePool() {
  if (!schedQueueTaskPool) return;
  const st = scheduleQueueDialogState;
  const members = st ? st.members : [];
  schedQueueTaskPool.textContent = '';
  const available = scheduleTasksCache.filter((t) => !members.includes(t));
  if (!available.length) {
    schedQueueTaskPool.textContent = '（任务库没有更多可添加的任务）';
    return;
  }
  for (const task of available) {
    const label = document.createElement('label');
    label.className = 'opt';
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.value = task;
    cb.dataset.task = task;
    label.append(cb, document.createTextNode(' ' + task));
    schedQueueTaskPool.appendChild(label);
  }
}

function updateScheduleQueueRoundsHint() {
  if (!schedQueueRoundsHint) return;
  const rounds = Number(schedQueueTotalRounds ? schedQueueTotalRounds.value : 0);
  schedQueueRoundsHint.textContent = rounds === 0
    ? 'total_rounds=0：无限轮，直到手动停止/重置。'
    : `total_rounds=${rounds}：跑满 ${rounds} 轮后 finished，不再触发（不自动重置）。`;
}

async function openScheduleQueueDialog(queueId) {
  await ensureScheduleTaskChoices();
  await loadScheduleConfig(true);
  if (!scheduleConfig) {
    window.alert('读取调度配置失败：' + (scheduleConfigError || '未知错误'));
    return;
  }
  const q = queueId
    ? (scheduleConfig.queues || []).find((x) => x.queue_id === queueId)
    : null;
  if (queueId && !q) {
    log('打开队列失败：队列「' + queueId + '」不存在', 'WARN');
    return;
  }
  scheduleQueueDialogState = {
    mode: q ? 'edit' : 'create',
    queueId: q ? q.queue_id : '',
    base: q || null,
    members: q && Array.isArray(q.members) ? q.members.slice() : [],
  };
  const form = schedQueueToForm(q || {});
  if (schedQueueTitle) schedQueueTitle.textContent = q ? `编辑队列：${q.queue_id}` : '新建队列';
  if (schedQueueId) {
    schedQueueId.value = form.queue_id;
    schedQueueId.disabled = !!q;
  }
  if (schedQueueName) schedQueueName.value = form.name;
  if (schedQueueTotalRounds) schedQueueTotalRounds.value = form.total_rounds;
  if (schedQueueOnFailure) schedQueueOnFailure.value = form.on_failure;
  if (schedQueueBetweenMs) schedQueueBetweenMs.value = form.between_ms;
  renderScheduleQueueMembers();
  renderScheduleQueuePool();
  updateScheduleQueueRoundsHint();
  if (schedQueueLockHint) {
    const locked = schedSaveBlockedReason(scheduleLocked());
    schedQueueLockHint.textContent = locked ? '注意：' + locked : '—';
  }
  if (schedQueueError) schedQueueError.textContent = '';
  if (schedQueueDialog.open) schedQueueDialog.close();
  schedQueueDialog.showModal();
}

async function saveScheduleQueueDialog() {
  const st = scheduleQueueDialogState;
  if (!st || !schedQueueDialog) return;
  if (schedQueueError) schedQueueError.textContent = '';
  const form = {
    queue_id: schedQueueId ? schedQueueId.value : '',
    name: schedQueueName ? schedQueueName.value : '',
    members: st.members.slice(),
    total_rounds: schedQueueTotalRounds ? schedQueueTotalRounds.value : '0',
    on_failure: schedQueueOnFailure ? schedQueueOnFailure.value : 'continue',
    between_ms: schedQueueBetweenMs ? schedQueueBetweenMs.value : '500',
  };
  const local = schedValidateQueueForm(form, scheduleConfig || schedEmptyConfig(), st.queueId, st.base);
  if (!local.ok) {
    if (schedQueueError) schedQueueError.textContent = local.errors.join('\n');
    return;
  }
  const fresh = await loadScheduleConfig(true);
  if (!fresh) {
    if (schedQueueError) schedQueueError.textContent = '读取调度配置失败：' + (scheduleConfigError || '未知错误');
    return;
  }
  const freshQueue = (fresh.queues || []).find((q) => q.queue_id === st.queueId) || st.base;
  const check = schedValidateQueueForm(form, fresh, st.queueId, freshQueue);
  if (!check.ok) {
    if (schedQueueError) schedQueueError.textContent = check.errors.join('\n');
    return;
  }
  let next;
  if (st.mode === 'create') {
    next = schedUpsertQueue(fresh, check.queue);
  } else {
    const idx = (fresh.queues || []).findIndex((q) => q.queue_id === st.queueId);
    if (idx < 0) {
      if (schedQueueError) schedQueueError.textContent = '队列已被外部删除，请关闭后重新打开。';
      return;
    }
    next = schedNormalizeConfig(fresh);
    next.queues[idx] = Object.assign({}, next.queues[idx], check.queue);
  }
  if (schedQueueSave) schedQueueSave.disabled = true;
  const res = await postScheduleConfig(next);
  if (schedQueueSave) schedQueueSave.disabled = false;
  if (!res.ok) {
    const msg = schedResponseErrorText(res.status, res.body);
    if (schedQueueError) schedQueueError.textContent = msg;
    log('保存队列失败：' + msg, res.status === 409 ? 'WARN' : 'ERROR');
    return;
  }
  schedQueueDialog.close();
  log(`已保存队列「${check.queue.queue_id}」${st.mode === 'create' ? '（新建）' : ''}`);
  await refreshSchedulePage();
}

async function deleteScheduleQueue(queueId) {
  if (scheduleBusy) return;
  const cfg = await loadScheduleConfig(true);
  if (!cfg) {
    log('删除队列失败：调度配置不可用', 'ERROR');
    return;
  }
  if (!(cfg.queues || []).some((q) => q.queue_id === queueId)) {
    log('删除队列失败：队列「' + queueId + '」不存在', 'WARN');
    return;
  }
  const removed = schedRemoveQueue(cfg, queueId);
  const entryNote = removed.removed_entry_ids.length
    ? `\n同时删除触发条目：${removed.removed_entry_ids.join('、')}`
    : '';
  if (!window.confirm(`删除队列「${queueId}」？${entryNote}\n成员任务目录不受影响。`)) return;
  const res = await postScheduleConfig(removed.config);
  if (!res.ok) {
    const msg = schedResponseErrorText(res.status, res.body);
    log('删除队列失败：' + msg, 'WARN');
    if (pageSchedConfigStatus) pageSchedConfigStatus.textContent = msg;
    return;
  }
  log(`已删除队列「${queueId}」` + (removed.removed_entry_ids.length
    ? `（同时删除条目 ${removed.removed_entry_ids.join('、')}）` : ''));
  await refreshSchedulePage();
}

async function saveSchedulePageConfig() {
  if (scheduleBusy) return;
  if (scheduleJsonDirty) {
    const msg = '高级 JSON 有未保存修改：请用「保存 JSON」写入，或先「重新加载」放弃修改。';
    log(msg, 'WARN');
    if (pageSchedConfigStatus) pageSchedConfigStatus.textContent = msg;
    return;
  }
  // P1-4: GET first. The toolbar save must not POST the cached document over an
  // edit made outside this page (hand edit / another tab / a script). Compare the
  // current file with the snapshot this page was loaded from; a mismatch aborts
  // the save and asks for a reload.
  const freshRes = await fetchScheduleConfigOnce();
  if (!freshRes.ok) {
    const msg = '保存配置失败：' + freshRes.error;
    log(msg, 'ERROR');
    if (pageSchedConfigStatus) pageSchedConfigStatus.textContent = msg;
    return;
  }
  const freshRaw = schedConfigSnapshotText(freshRes.body.config);
  if (schedConfigSnapshotChanged(scheduleConfigBase, freshRaw)) {
    const msg = schedExternalConfigChangeText();
    log(msg, 'WARN');
    if (pageSchedConfigStatus) pageSchedConfigStatus.textContent = msg;
    return;
  }
  const cfg = scheduleConfig || schedNormalizeConfig(freshRes.body.config);
  const res = await postScheduleConfig(cfg);
  if (!res.ok) {
    const msg = schedResponseErrorText(res.status, res.body);
    log('保存调度配置失败：' + msg, res.status === 409 ? 'WARN' : 'ERROR');
    if (pageSchedConfigStatus) pageSchedConfigStatus.textContent = msg;
    return;
  }
  const bytes = res.body && res.body.bytes != null ? `（${res.body.bytes} bytes）` : '';
  const backup = res.body && res.body.backup ? ` · 备份=${res.body.backup}` : '';
  log(`调度配置已保存${bytes}${backup}`);
  if (pageSchedConfigStatus) pageSchedConfigStatus.textContent = `已保存${bytes}${backup}`;
  await refreshSchedulePage();
}

async function saveScheduleJson() {
  if (scheduleBusy || !pageSchedJson) return;
  const parsed = schedParseConfigJson(pageSchedJson.value);
  if (!parsed.ok) {
    if (pageSchedJsonStatus) pageSchedJsonStatus.textContent = '本地校验失败：' + parsed.error;
    log('调度配置 JSON 本地校验失败：' + parsed.error, 'WARN');
    return;
  }
  const res = await postScheduleConfig(parsed.config);
  if (!res.ok) {
    const msg = schedResponseErrorText(res.status, res.body);
    if (pageSchedJsonStatus) pageSchedJsonStatus.textContent = msg;
    log('保存调度配置 JSON 失败：' + msg, res.status === 409 ? 'WARN' : 'ERROR');
    return;
  }
  scheduleJsonDirty = false;
  const bytes = res.body && res.body.bytes != null ? `（${res.body.bytes} bytes）` : '';
  log('调度配置 JSON 已保存' + bytes);
  await refreshSchedulePage();
  if (pageSchedJsonStatus) pageSchedJsonStatus.textContent = '已保存' + bytes;
}

async function reloadScheduleJson() {
  scheduleJsonDirty = false;
  if (pageSchedJsonStatus) pageSchedJsonStatus.textContent = '重新载入中…';
  await refreshSchedulePage();
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
    schedEntries.textContent = '（无调度条目：到「调度」页添加，或点上方「打开调度页 →」）';
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
    if (e.kind === 'queue') head.appendChild(schedBadge('queue', 'kind'));
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
  // P2-1: Esc leaves the canvas ROI editor (asset_ui.js already closes the
  // settings menu on Esc). Never steal Esc from an open modal dialog.
  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape' || !editingRecognitionAsset) return;
    if (document.querySelector && document.querySelector('dialog[open]')) return;
    cancelRecognitionRoiEdit('Esc');
  });
  taskName.addEventListener('change', async () => {
    // P0-2: the asset caches are about to be replaced with another task's rows.
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
  if (pageSchedRefresh) pageSchedRefresh.addEventListener('click', refreshSchedulePage);
  if (pageSchedReload) pageSchedReload.addEventListener('click', refreshSchedulePage);
  if (pageSchedReset) pageSchedReset.addEventListener('click', openResetMemoryDialog);
  // P3: editable scheduler page actions.
  if (pageSchedSave) pageSchedSave.addEventListener('click', saveSchedulePageConfig);
  if (pageSchedAddEntry) pageSchedAddEntry.addEventListener('click', openScheduleAddEntryDialog);
  if (pageSchedAddQueue) pageSchedAddQueue.addEventListener('click', () => openScheduleQueueDialog(''));
  if (pageSchedJsonSave) pageSchedJsonSave.addEventListener('click', saveScheduleJson);
  if (pageSchedJsonReload) pageSchedJsonReload.addEventListener('click', reloadScheduleJson);
  if (pageSchedJson) {
    pageSchedJson.addEventListener('input', () => {
      scheduleJsonDirty = true;
      if (pageSchedJsonStatus) pageSchedJsonStatus.textContent = '有未保存修改（用「保存 JSON」写入）';
    });
  }
  // P4: run history controls (list filters + detail dialog).
  if (pageSchedRunsRefresh) pageSchedRunsRefresh.addEventListener('click', () => refreshScheduleRuns());
  if (pageSchedRunsTask) {
    pageSchedRunsTask.addEventListener('change', () => {
      scheduleRunsFilter.task = pageSchedRunsTask.value;
      scheduleRunsFilter.queueId = '';
      syncScheduleRunsControls();
      refreshScheduleRuns();
    });
  }
  if (pageSchedRunsOutcome) {
    pageSchedRunsOutcome.addEventListener('change', () => {
      scheduleRunsFilter.outcome = pageSchedRunsOutcome.value;
      syncScheduleRunsControls();
      refreshScheduleRuns();
    });
  }
  if (pageSchedRunsLimit) {
    pageSchedRunsLimit.addEventListener('change', () => {
      scheduleRunsFilter.limit = schedRunLimitValue(pageSchedRunsLimit.value);
      syncScheduleRunsControls();
      refreshScheduleRuns();
    });
  }
  if (pageSchedRunsClear) {
    pageSchedRunsClear.addEventListener('click', () => {
      scheduleRunsFilter.task = '';
      scheduleRunsFilter.outcome = '';
      scheduleRunsFilter.queueId = '';
      syncScheduleRunsControls();
      refreshScheduleRuns();
    });
  }
  if (schedRunDialogClose) schedRunDialogClose.addEventListener('click', closeScheduleRunDetail);
  if (schedRunDialogRefresh) {
    schedRunDialogRefresh.addEventListener('click', () => {
      if (scheduleRunDetailRunId) openScheduleRunDetail(scheduleRunDetailRunId);
    });
  }
  if (btnSchedOpenPage) btnSchedOpenPage.addEventListener('click', () => activatePage('schedule'));
  if (schedEntryKind) {
    schedEntryKind.addEventListener('change', () => {
      populateScheduleEntryTargets(schedEntryKind.value, '');
      updateScheduleEntryTargetHint();
    });
  }
  if (schedEntryTarget) schedEntryTarget.addEventListener('change', updateScheduleEntryTargetHint);
  if (schedEntryLifecycle) schedEntryLifecycle.addEventListener('change', updateScheduleEntryLifecycleUi);
  if (schedEntryCancel) {
    schedEntryCancel.addEventListener('click', () => {
      if (schedEntryDialog) schedEntryDialog.close();
    });
  }
  if (schedEntrySave) schedEntrySave.addEventListener('click', saveScheduleEntryDialog);
  if (schedQueueMemberAddBtn) {
    schedQueueMemberAddBtn.addEventListener('click', () => {
      const st = scheduleQueueDialogState;
      if (!st || !schedQueueTaskPool) return;
      const checked = Array.from(
        schedQueueTaskPool.querySelectorAll('input[type=checkbox]:checked')
      ).map((cb) => cb.value);
      for (const task of checked) st.members = schedQueueMemberAdd(st.members, task);
      renderScheduleQueueMembers();
      renderScheduleQueuePool();
    });
  }
  if (schedQueueTotalRounds) schedQueueTotalRounds.addEventListener('input', updateScheduleQueueRoundsHint);
  if (schedQueueCancel) {
    schedQueueCancel.addEventListener('click', () => {
      if (schedQueueDialog) schedQueueDialog.close();
    });
  }
  if (schedQueueSave) schedQueueSave.addEventListener('click', saveScheduleQueueDialog);
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
