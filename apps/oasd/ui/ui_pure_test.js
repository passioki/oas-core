// Pure-function and static contract regression tests for the editor UI.
// No server, no bundler, no browser needed.
//
// Run from the repository root:
//   node apps/oasd/ui/ui_pure_test.js
//
// The test extracts the pure helpers from app.js itself (so it cannot drift
// from a copy) and evaluates them in a Node vm sandbox. It also performs static
// contract checks on index.html / app.js for:
//   - the two asset libraries sharing one search/category filter,
//   - zero static dead DOM ids (every getElementById target exists in HTML),
//   - the OCR/single save payloads carrying polygon + recognition metadata,
//   - the mutually-exclusive selection mode dispatch,
//   - ResizeObserver-driven canvas fitting staying rAF-coalesced + idempotent,
//   - PR1 Studio/Runner split: Studio nav has no schedule page, Runner nav has
//     exactly six pages and its own static bundle.
//   - PR2 task config: Studio edits meta.params (schema) while the Runner edits
//     local values (config/values.json); the two API surfaces stay isolated.
//   - PR5 packages: Studio publishes .oaspkg (POST /task/publish) while the
//     Runner installs/uninstalls/rolls back (POST /runner/install|uninstall|
//     packages/activate) and never touches the authoring surface.
//   - PR6 scheduler operations: the Runner starts/stops its scheduler thread,
//     runs one task on demand and edits scheduler.json; the Studio has no
//     scheduling page/DOM left and never calls Runner-only scheduler routes.
'use strict';

const fs = require('fs');
const path = require('path');
const vm = require('vm');
const assert = require('assert');

const appPath = path.join(__dirname, 'app.js');
const htmlPath = path.join(__dirname, 'index.html');
const builderPath = path.join(__dirname, 'builder.js');
const assetUiPath = path.join(__dirname, 'asset_ui.js');
const stylePath = path.join(__dirname, 'style.css');
const runnerHtmlPath = path.join(__dirname, 'runner.html');
const runnerJsPath = path.join(__dirname, 'runner.js');
const runnerCssPath = path.join(__dirname, 'runner.css');
const source = fs.readFileSync(appPath, 'utf8');
const html = fs.readFileSync(htmlPath, 'utf8');
const builderSource = fs.readFileSync(builderPath, 'utf8');
const assetUiSource = fs.readFileSync(assetUiPath, 'utf8');
const styleCss = fs.readFileSync(stylePath, 'utf8');
const runnerHtml = fs.readFileSync(runnerHtmlPath, 'utf8');
const runnerSource = fs.readFileSync(runnerJsPath, 'utf8');
const runnerCss = fs.readFileSync(runnerCssPath, 'utf8');

function extractFunctionFrom(text, name, label) {
  const re = new RegExp(`(?:^|\\n)[ \\t]*(?:async\\s+)?function\\s+${name}\\s*\\(`);
  const m = re.exec(text);
  if (!m) throw new Error(`function ${name} not found in ${label}`);
  const start = m.index + (m[0][0] === '\n' ? 1 : 0);
  let i = text.indexOf('{', m.index);
  let depth = 0;
  let quote = null;
  let esc = false;
  let lineComment = false;
  let blockComment = false;
  for (; i < text.length; i++) {
    const ch = text[i];
    const next = text[i + 1];
    if (lineComment) {
      if (ch === '\n') lineComment = false;
      continue;
    }
    if (blockComment) {
      if (ch === '*' && next === '/') {
        blockComment = false;
        i++;
      }
      continue;
    }
    if (quote) {
      if (esc) {
        esc = false;
        continue;
      }
      if (ch === '\\') {
        esc = true;
        continue;
      }
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === '/' && next === '/') {
      lineComment = true;
      i++;
      continue;
    }
    if (ch === '/' && next === '*') {
      blockComment = true;
      i++;
      continue;
    }
    if (ch === "'" || ch === '"' || ch === '`') {
      quote = ch;
      continue;
    }
    if (ch === '{') depth++;
    else if (ch === '}') {
      depth--;
      if (depth === 0) return text.slice(start, i + 1);
    }
  }
  throw new Error(`unterminated function ${name}`);
}

function extractFunction(name) {
  return extractFunctionFrom(source, name, 'app.js');
}

const PURE_FUNCTIONS = [
  'actionShapeBBox',
  'actionShapeContains',
  'actionShapeEffectiveContains',
  'actionShapeFromGeometry',
  'actionShapeEditState',
  'loadActionShapeForEdit',
  'currentActionShape',
  'regionDragStep',
  'assetTestModeLabel',
  'assetTestGestureRequest',
  'sendAssetTestGesture',
  'realAssetTestGuard',
  'confirmAssetTestGestures',
  'updateAssetTestModeButton',
  'assetCollectionMatchUrl',
  'assetCollectionClickPoint',
  'assetCollectionMatchLine',
  'segmentIntersects',
  'polygonOverlapsRect',
  'rectsOverlap',
  'actionValidity',
  'boxCenter',
  // MATCH-BUG: frame selection for single asset tests + match-box validity
  'testFramePlan',
  'matchBoxValid',
  'translateActionShape',
  'candidateActionPayload',
  'recognitionPolygonPayload',
  'ocrAssetPayload',
  'singleAssetPayload',
  'parseAssetThresholdText',
  // B: the canvas ROI editor owns the single set of ROI bounds (the old prompt
  // text parser was removed with the prompt editor).
  'assetRoiBoxValid',
  // P1-1/P1-2: the template replace payload + latest-asset lookup.
  'assetRoiWithinFrame',
  'assetRoiRectanglePolygon',
  'assetRoiPolygonForSave',
  'assetRoiPolygonPlan',
  'assetRoiReplacePayload',
  'findAssetById',
  // P1-3: batch test target = the visible library rows next to the button.
  'batchTestAssetSelection',
  'batchTestEmptyText',
  // E: threshold meaning differs per asset kind.
  'assetThresholdKind',
  'assetCategory',
  'assetCategoryNames',
  'assetMatchesFilter',
  'filterAssets',
  'globalAssetStatusText',
  'assetRowElement',
  'renderAssetRows',
  'renderGlobalAssets',
  'frameMatchesQuery',
  'filterFramesByQuery',
  'frameSearchStatusText',
  'assetHasFramePosition',
  'assetsForFrame',
  // C: per-row API scope for the merged frame-detail surface.
  'frameAssetScope',
  'frameAssetsSummary',
  'frameAssetIdsByFrame',
  // P1-1: cache-only repaint of the frame rows + current-frame detail after a
  // global-library change (save/delete/positions).
  'renderFrameAssetViews',
  'selectionModeFromFlags',
  // PR2: Studio-side meta.params schema editor (runner values stay on Runner).
  'taskConfigNormalizeParam',
  'taskConfigParamsToJson',
  'parseTaskConfigParamsText',
  'taskConfigSummary',
  // PR5: Studio-side TaskPackage publish body / summary / error rendering.
  'publishRequestBody',
  'formatPackageSummary',
  'publishErrorsText',
];

const sandbox = {};
vm.createContext(sandbox);
// app.js stores the unified-global scope label in a top-level const; the pure
// helpers compare against it (frameAssetScope -> GLOBAL_SCOPE_TASK), so the
// harness extracts the REAL literal instead of duplicating the string.
const GLOBAL_SCOPE_LITERAL = (/^const GLOBAL_SCOPE_TASK = '([^']*)';/m.exec(source) || [])[1];
vm.runInContext(
  `const GLOBAL_SCOPE_TASK = ${JSON.stringify(GLOBAL_SCOPE_LITERAL || '')};\n` +
    PURE_FUNCTIONS.map(extractFunction).join('\n\n'),
  sandbox,
  { filename: 'app.js#pure' }
);

const {
  actionShapeFromGeometry,
  actionShapeEditState,
  loadActionShapeForEdit,
  currentActionShape,
  regionDragStep,
  assetTestModeLabel,
  assetTestGestureRequest,
  sendAssetTestGesture,
  realAssetTestGuard,
  confirmAssetTestGestures,
  updateAssetTestModeButton,
  assetCollectionMatchUrl,
  assetCollectionClickPoint,
  assetCollectionMatchLine,
  actionValidity,
  polygonOverlapsRect,
  boxCenter,
  translateActionShape,
  recognitionPolygonPayload,
  ocrAssetPayload,
  singleAssetPayload,
  assetCategoryNames,
  assetMatchesFilter,
  filterAssets,
  globalAssetStatusText,
  selectionModeFromFlags,
  taskConfigNormalizeParam,
  taskConfigParamsToJson,
  parseTaskConfigParamsText,
  taskConfigSummary,
  publishRequestBody,
  formatPackageSummary,
  publishErrorsText,
} = sandbox;

let checks = 0;
function ok(cond, message) {
  checks += 1;
  assert.ok(cond, message);
}
function eq(actual, expected, message) {
  checks += 1;
  // Values returned from another vm context have a different Object prototype;
  // round-trip through JSON so deepStrictEqual compares plain data only.
  assert.deepStrictEqual(JSON.parse(JSON.stringify(actual)), expected, message);
}

// --- C: the unified-global scope label is one protocol literal ---
eq(GLOBAL_SCOPE_LITERAL, '（全局素材库）', 'GLOBAL_SCOPE_TASK stays the unified-global protocol label');

// --- boxCenter: selectedBox rows are objects ({box:[x,y,w,h],...}) ---
eq(boxCenter({ box: [10, 20, 31, 41] }), { x: 26, y: 41 }, 'boxCenter object row');
ok(
  (source.match(/(?:^|\n)function boxCenter\s*\(/g) || []).length === 1,
  'boxCenter must be declared exactly once (the old array override broke 生成点击指令)'
);

// --- polygon/rect overlap ---
const square = [[0, 0], [10, 0], [10, 10], [0, 10]];
ok(polygonOverlapsRect(square, [2, 2, 2, 2]), 'polygon contains rect');
ok(!polygonOverlapsRect(square, [20, 20, 5, 5]), 'polygon away from rect');
ok(polygonOverlapsRect(square, [9, 4, 5, 2]), 'rect edge crosses polygon edge');
ok(polygonOverlapsRect([[5, 0], [15, 0], [15, 20], [5, 20]], [3, 5, 4, 4]), 'rect corner inside polygon');

// --- actionValidity: recognition vs fixed contract (P0-2) ---
const recNoShape = { action_source: 'recognition' };
eq(actionValidity(recNoShape, [0, 0, 10, 10]).valid, true, 'recognition without shape is valid');

const fixedNoShape = { action_source: 'fixed' };
eq(actionValidity(fixedNoShape, [0, 0, 10, 10]).valid, false, 'fixed without shape is invalid');

const fixedRect = {
  action_source: 'fixed',
  action_shape: { kind: 'rect', roi: [100, 100, 10, 10], invert: false },
};
const fixedValidity = actionValidity(fixedRect, [0, 0, 10, 10]);
eq(fixedValidity.valid, true, 'fixed independent region is allowed');
eq(fixedValidity.independent, true, 'fixed independent region flagged');

const recRect = {
  action_source: 'recognition',
  action_shape: { kind: 'rect', roi: [100, 100, 10, 10], invert: false },
};
eq(actionValidity(recRect, [105, 105, 2, 2]).valid, true, 'recognition shape overlaps');
eq(actionValidity(recRect, [0, 0, 10, 10]).valid, false, 'recognition shape does not overlap');

const recPoly = {
  action_source: 'recognition',
  action_shape: { kind: 'polygon', points: square, invert: false },
};
eq(actionValidity(recPoly, [2, 2, 2, 2]).valid, true, 'recognition polygon overlaps');

const recInvert = {
  action_source: 'recognition',
  action_shape: { kind: 'rect', roi: [0, 0, 50, 50], invert: true },
};
eq(actionValidity(recInvert, [10, 10, 5, 5]).valid, false, 'invert: box fully inside excluded area');
eq(actionValidity(recInvert, [45, 45, 10, 10]).valid, true, 'invert: box partly outside excluded area');

// --- P1-1: template action_shape is translated by matched - roi before validity ---
const savedTemplate = {
  roi: [22, 9, 43, 39],
  action_source: 'fixed',
  action_shape: { kind: 'rect', roi: [20, 10, 10, 10], invert: false },
};
const match = { x: 40, y: 30 };
const translated = translateActionShape(
  savedTemplate.action_shape,
  match.x - savedTemplate.roi[0],
  match.y - savedTemplate.roi[1]
);
eq(translated.roi, [38, 31, 10, 10], 'translate rect action_shape');
const translatedValidity = actionValidity(savedTemplate, [40, 30, 10, 10], translated);
eq(translatedValidity.text, '有效（固定操作区与识别框重叠）', 'translated fixed shape overlaps match');
ok(!translatedValidity.independent, 'translated shape is not independent');
const staleValidity = actionValidity(savedTemplate, [40, 30, 10, 10]);
ok(staleValidity.independent, 'without translation the stale shape is independent (regression guard)');

eq(
  translateActionShape({ kind: 'polygon', points: [[0, 0], [4, 0], [0, 4]], invert: true }, 5, 7),
  { kind: 'polygon', points: [[5, 7], [9, 7], [5, 11]], invert: true },
  'translate polygon action_shape'
);
eq(translateActionShape(null, 1, 2), null, 'translate null shape');

// --- v1.2.4: action_shape invert travels with BOTH rect and polygon ---
const testPolyPoints = [{ x: 1, y: 2 }, { x: 9, y: 2 }, { x: 9, y: 8 }];
eq(
  actionShapeFromGeometry(null, false, { x: 10, y: 20, w: 30, h: 40 }, true),
  { kind: 'rect', roi: [10, 20, 30, 40], invert: true },
  'rect action_shape keeps invert=true'
);
eq(
  actionShapeFromGeometry(testPolyPoints, true, { x: 100, y: 100, w: 5, h: 5 }, true),
  { kind: 'polygon', points: [[1, 2], [9, 2], [9, 8]], invert: true },
  'closed polygon wins over rect and keeps invert=true'
);
eq(
  actionShapeFromGeometry(testPolyPoints, false, { x: 1, y: 2, w: 3, h: 4 }, true).kind,
  'rect',
  'open polygon falls back to the rect shape'
);
eq(actionShapeFromGeometry(testPolyPoints, false, null, true), null, 'open polygon without rect is not savable');
eq(actionShapeFromGeometry(null, false, { x: 1, y: 2, w: 0, h: 4 }, true), null, 'zero-size rect rejected');
eq(actionShapeFromGeometry(testPolyPoints, true, null, false).invert, false, 'invert defaults off');

// currentActionShape is what rect dragging / save payloads use. A rect drag with
// the invert toggle on must still produce invert=true.
sandbox.actionPoly = null;
sandbox.actionPolyClosed = false;
sandbox.roiBack = { x: 7, y: 8, w: 9, h: 10 };
sandbox.actionInvert = true;
eq(currentActionShape(), { kind: 'rect', roi: [7, 8, 9, 10], invert: true }, 'rect drag keeps invert');
sandbox.actionPoly = testPolyPoints;
sandbox.actionPolyClosed = true;
eq(
  currentActionShape(),
  { kind: 'polygon', points: [[1, 2], [9, 2], [9, 8]], invert: true },
  'polygon keeps invert'
);

// Loading an existing asset's action_shape (素材行「操作区」).
sandbox.btnActionInvert = { textContent: '' };
sandbox.actionPoly = null;
sandbox.actionPolyClosed = false;
sandbox.roiBack = null;
loadActionShapeForEdit({ kind: 'rect', roi: [3, 4, 5, 6], invert: true });
eq(sandbox.actionInvert, true, 'load rect restores invert');
eq(sandbox.roiBack, { x: 3, y: 4, w: 5, h: 6 }, 'load rect restores roiBack');
eq(sandbox.btnActionInvert.textContent, '操作区域反选：开', 'load rect syncs the invert button label');
loadActionShapeForEdit({ kind: 'polygon', points: [[0, 0], [10, 0], [10, 10]], invert: true });
eq(sandbox.actionInvert, true, 'load polygon restores invert');
eq(sandbox.actionPolyClosed, true, 'load polygon closes the shape');
eq(sandbox.actionPoly, [{ x: 0, y: 0 }, { x: 10, y: 0 }, { x: 10, y: 10 }], 'load polygon restores points');
eq(sandbox.roiBack, null, 'load polygon leaves no stray bbox rect over the polygon');
eq(
  actionShapeEditState({ kind: 'rect', roi: [1, 2, 3, 4], invert: true }),
  { kind: 'rect', points: null, roi: [1, 2, 3, 4], invert: true },
  'edit-state normalization for inverted rect'
);
eq(
  actionShapeEditState({ kind: 'polygon', points: [[0, 0], [1, 0], [0, 1]], invert: true }),
  { kind: 'polygon', points: [[0, 0], [1, 0], [0, 1]], roi: null, invert: true },
  'edit-state normalization for inverted polygon'
);
eq(actionShapeEditState(null), { kind: null, points: null, roi: null, invert: false }, 'edit-state for no shape');

// --- v1.2.4: asset test real/dry-run payload contract ---
eq(assetTestModeLabel(false), '演练', 'dry-run label');
eq(assetTestModeLabel(true), '真实点击', 'real mode label');
eq(assetTestGestureRequest(false, 'tap', { x: 1, y: 2 }), null, 'dry-run builds no device request');
eq(
  assetTestGestureRequest(true, 'tap', { x: 10.4, y: 20.6 }),
  { url: '/input/tap', payload: { x: 10, y: 21 }, gesture: 'tap', text: '真实已点击 (10, 21)' },
  'real tap payload is POST /input/tap {x,y}'
);
eq(
  assetTestGestureRequest(true, 'swipe', { x: 1, y: 2 }, { x: 30, y: 40 }, 123.4),
  {
    url: '/input/swipe',
    payload: { from: [1, 2], to: [30, 40], ms: 123 },
    gesture: 'swipe',
    text: '真实已滑动 (1,2) → (30,40) 123ms',
  },
  'real swipe payload is POST /input/swipe {from,to,ms}'
);
eq(
  assetTestGestureRequest(true, 'swipe', { x: 1, y: 2 }, { x: 3, y: 4 }, 9999).payload.ms,
  2000,
  'swipe duration clamps to the server range'
);
eq(assetTestGestureRequest(true, 'tap', { x: 'bad', y: 2 }), null, 'invalid coordinate builds no request');

// --- O-1: collection test real=1 URL / point / result-line contract ---
eq(
  assetCollectionMatchUrl('任务', '集合', '&live=1', false),
  '/task/asset/collection/match?task=%E4%BB%BB%E5%8A%A1&id=%E9%9B%86%E5%90%88&live=1',
  'collection dry-run URL is byte-identical to the old call (no real param)'
);
eq(
  assetCollectionMatchUrl('t', 'C', '&frame_id=3', true),
  '/task/asset/collection/match?task=t&id=C&frame_id=3&real=1',
  'collection real URL appends real=1 after the frame query'
);
eq(assetCollectionClickPoint([20, 20, 8, 8]), [24, 24], 'collection click point is the action-box centre');
eq(assetCollectionClickPoint([0, 0, 1, 1]), [0, 0], '1x1 action box centre');
eq(assetCollectionClickPoint(null), null, 'missing action box has no click point');
eq(assetCollectionClickPoint([1, 2, 3]), null, 'malformed action box has no click point');
eq(assetCollectionClickPoint([0, 0, 0, 4]), null, 'zero-width action box has no click point');
eq(
  assetCollectionMatchLine(
    '实时帧',
    { member: 'M', score: 0.98765, threshold: 0.8, pass: true, x: 20, y: 20, action_box: [20, 20, 8, 8] },
    'live'
  ),
  '实时帧: 最佳成员 M · score=0.9877 · threshold=0.800 · pass=true · 位置 (20,20) · 帧来源：live · 操作区域[20,20,8,8]',
  'collection match line keeps the dry-run wording and formats numbers'
);
eq(
  assetCollectionMatchLine(
    '',
    { member: 'M', score: 1, threshold: 0.8, pass: false, x: 1, y: 2 },
    'position:#3'
  ),
  '最佳成员 M · score=1.0000 · threshold=0.800 · pass=false · 位置 (1,2) · 帧来源：position:#3',
  'collection match line supports an empty label and a fallback frame source'
);

async function checkAssetTestWiring() {
  const calls = [];
  sandbox.postJson = async (url, payload) => {
    calls.push({ url, payload });
    return {
      ok: true,
      device: 'dev-1',
      gesture: url.endsWith('/swipe') ? 'swipe' : 'tap',
    };
  };
  const dry = await sendAssetTestGesture(false, 'tap', { x: 1, y: 2 });
  eq(calls.length, 0, 'dry-run sends zero device requests');
  eq(dry.dry, true, 'dry-run result is marked dry');
  const tap = await sendAssetTestGesture(true, 'tap', { x: 5, y: 6 });
  eq(calls.length, 1, 'real tap sends exactly one request');
  eq(calls[0], { url: '/input/tap', payload: { x: 5, y: 6 } }, 'real tap request contract');
  eq(tap.device, 'dev-1', 'real tap surfaces the server device');
  eq(tap.gesture, 'tap', 'real tap surfaces the server gesture');
  ok(/device=dev-1/.test(tap.text) && /gesture=tap/.test(tap.text), 'tap result shows device/gesture');
  const swipe = await sendAssetTestGesture(true, 'swipe', { x: 1, y: 2 }, { x: 30, y: 40 }, 500);
  eq(calls.length, 2, 'real swipe sends exactly one request');
  eq(
    calls[1],
    { url: '/input/swipe', payload: { from: [1, 2], to: [30, 40], ms: 500 } },
    'real swipe request contract'
  );
  eq(swipe.gesture, 'swipe', 'real swipe surfaces the server gesture');
  sandbox.postJson = async () => ({ ok: false, errors: ['boom'] });
  let failed = false;
  try {
    await sendAssetTestGesture(true, 'tap', { x: 1, y: 2 });
  } catch (e) {
    failed = /boom/.test(e.message);
  }
  ok(failed, 'real gesture surfaces server errors');
}

// Real-mode gate: no live preview -> explicit refusal with no request; with live
// preview -> allowed. This is the branch testAssetClick/testAssetDrag run first.
sandbox.resultEl = { textContent: '' };
sandbox.log = () => {};
sandbox.assetTestReal = true;
sandbox.livePreview = false;
const refused = realAssetTestGuard();
eq(refused.ok, false, 'real mode is refused while live preview is paused');
ok(/实时预览已暂停/.test(sandbox.resultEl.textContent), 'refusal tells the user to switch live preview on');
sandbox.livePreview = true;
eq(realAssetTestGuard().ok, true, 'real mode is allowed while live preview is on');
sandbox.assetTestReal = false;
eq(realAssetTestGuard().real, false, 'dry mode bypasses the live-preview gate');
eq(
  confirmAssetTestGestures('click', []),
  false,
  'empty gesture batch never asks for confirmation'
);
let confirmText = '';
sandbox.window = {
  confirm: (text) => { confirmText = text; return true; },
};
eq(
  confirmAssetTestGestures('drag', [{ label: '实时帧 滑动 (1,2) → (3,4)' }]),
  true,
  'non-empty real batch asks for confirmation'
);
ok(/这不是演练/.test(confirmText) && /实时帧 滑动/.test(confirmText), 'confirmation text names real mode and the gesture');
ok(
  /实时画面可能仍在变化，将按上面的定位发送。确认发送？/.test(confirmText),
  'default confirmation keeps the legacy caveat sentence'
);
// P2-6: collection real mode replaces the generic caveat with the actual
// two-request behaviour (server re-matches + samples inside the shape).
eq(
  confirmAssetTestGestures('click', [{ label: '实时帧 集合「COL」成员 M 点击 (24, 24)' }],
    '服务端将重新匹配并点击最新操作区（shape 内采样）；实际点击点以返回 click_point 为准。'),
  true,
  'collection confirmation with a note still asks'
);
ok(
  /服务端将重新匹配并点击最新操作区（shape 内采样）/.test(confirmText) &&
    /click_point/.test(confirmText),
  'collection confirmation explains the second match and the authoritative click_point'
);
ok(
  !/实时画面可能仍在变化，将按上面的定位发送/.test(confirmText),
  'the generic "above positioning" caveat is replaced for collection'
);

// Mode button label/aria reflects the state (no device request involved).
const fakeClassList = {
  values: new Set(),
  add(c) { this.values.add(c); },
  remove(c) { this.values.delete(c); },
  toggle(c, on) { if (on) this.values.add(c); else this.values.delete(c); },
};
sandbox.btnTestMode = {
  textContent: '',
  title: '',
  classList: fakeClassList,
  setAttribute(k, v) { this[k] = v; },
};
sandbox.assetTestReal = false;
updateAssetTestModeButton();
eq(sandbox.btnTestMode.textContent, '素材测试：演练', 'header button defaults to 演练');
eq(sandbox.btnTestMode['aria-pressed'], 'false', 'dry button aria-pressed=false');
sandbox.assetTestReal = true;
updateAssetTestModeButton();
eq(sandbox.btnTestMode.textContent, '素材测试：真实点击', 'header button shows 真实点击');
eq(sandbox.btnTestMode['aria-pressed'], 'true', 'real button aria-pressed=true');
sandbox.assetTestReal = false;

// --- P0-2: save payload branch (recognition vs fixed vs region) ---
eq(
  ocrAssetPayload(
    { kind: 'ocr_line', name: '按钮甲', recognize: { x: 1, y: 2, w: 30, h: 40 }, actionSource: 'recognition', actionShape: null },
    [3],
    null
  ),
  { id: '按钮甲', kind: 'ocr_line', roi: [1, 2, 30, 40], positions: [3], action_source: 'recognition' },
  'un-edited OCR candidate saves recognition without shape/polygon'
);
eq(
  ocrAssetPayload(
    {
      kind: 'ocr_line',
      name: '按钮乙',
      text: '开始',
      recognize: { x: 5, y: 6, w: 20, h: 10 },
      fromLines: true,
      sourceRoi: { x: 0, y: 0, w: 100, h: 50 },
      actionSource: 'fixed',
      actionShape: { kind: 'rect', roi: [1, 2, 3, 4], invert: false },
    },
    [7, 8],
    null
  ),
  {
    id: '按钮乙',
    kind: 'ocr_line',
    roi: [0, 0, 100, 50],
    positions: [7, 8],
    action_shape: { kind: 'rect', roi: [1, 2, 3, 4], invert: false },
    action_source: 'fixed',
    ocr_mode: 'lines',
    ocr_text: '开始',
  },
  'multi-line OCR keeps ocr_mode/ocr_text and fixed shape; no polygon when absent'
);
eq(
  singleAssetPayload(
    { kind: 'template', name: '模板甲', recognize: { x: 1, y: 2, w: 3, h: 4 }, actionSource: 'recognition', actionShape: null },
    [7],
    null
  ),
  { id: '模板甲', kind: 'template', roi: [1, 2, 3, 4], positions: [7], action_source: 'recognition', threshold: 0.85 },
  'template keeps threshold and recognition semantics'
);
eq(
  singleAssetPayload(
    { kind: 'region', name: '区域甲', recognize: { x: 1, y: 2, w: 3, h: 4 }, actionSource: 'recognition', actionShape: null },
    [9],
    null
  ),
  { id: '区域甲', kind: 'region', roi: [1, 2, 3, 4], positions: [9] },
  'region keeps its roi-as-operation-area semantics (no extra action fields)'
);

// --- C-07b: recognition polygon travels in BOTH save payloads ---
const recognitionPoly = [[1, 2], [9, 2], [9, 8], [1, 8]];
eq(recognitionPolygonPayload(null), null, 'no polygon state -> no payload field');
eq(recognitionPolygonPayload([[0, 0], [1, 1]]), null, 'degenerate polygon rejected');
eq(recognitionPolygonPayload(recognitionPoly), recognitionPoly, 'closed recognition polygon copied');

const ocrWithPoly = ocrAssetPayload(
  {
    kind: 'ocr_line',
    name: '按钮丙',
    text: '确定',
    recognize: { x: 2, y: 2, w: 4, h: 4 },
    fromLines: true,
    sourceRoi: { x: 1, y: 1, w: 8, h: 8 },
    actionSource: 'recognition',
    actionShape: null,
  },
  [5],
  recognitionPoly
);
eq(ocrWithPoly.polygon, recognitionPoly, 'OCR batch payload includes the recognition polygon');
eq(ocrWithPoly.ocr_mode, 'lines', 'OCR batch payload keeps ocr_mode');
eq(ocrWithPoly.ocr_text, '确定', 'OCR batch payload keeps ocr_text');

const singleWithPoly = singleAssetPayload(
  { kind: 'ocr_line', name: '按钮丁', recognize: { x: 3, y: 3, w: 2, h: 2 }, actionSource: 'recognition', actionShape: null },
  [6],
  recognitionPoly
);
eq(singleWithPoly.polygon, recognitionPoly, 'single save payload includes the recognition polygon');

const candidatePoly = [[0, 0], [5, 0], [5, 5]];
const candidatePolyWins = ocrAssetPayload(
  { kind: 'ocr_line', name: '按钮戊', recognize: { x: 0, y: 0, w: 1, h: 1 }, actionSource: 'recognition', actionShape: null, polygon: candidatePoly },
  [1],
  recognitionPoly
);
eq(candidatePolyWins.polygon, candidatePoly, 'candidate-level polygon wins over the canvas polygon');

// --- global asset library filter (same helper as the task panel) ---
const globalRows = [
  { id: '按钮甲', kind: 'ocr_line', category: '庭院' },
  { id: 'button-b', kind: 'template', category: '庭院' },
  { id: '弹窗丙', kind: 'template', category: '弹窗' },
  { id: '集合丁', kind: 'template_set', category: '素材集合', members: ['x'] },
  { id: '未标记', kind: 'ocr_line' },
];
eq(filterAssets(globalRows, '', '').length, 5, 'empty query/category shows all cached rows');
eq(filterAssets(globalRows, 'BUTTON', '').map((x) => x.id), ['button-b'], 'id search is case-insensitive');
eq(filterAssets(globalRows, '集', '').map((x) => x.id), ['集合丁'], 'collection rows participate in id search');
eq(filterAssets(globalRows, '', '弹窗').map((x) => x.id), ['弹窗丙'], 'category filter matches one category');
eq(filterAssets(globalRows, 'button', '庭院').map((x) => x.id), ['button-b'], 'search + category combine');
eq(filterAssets(globalRows, '', '素材集合').map((x) => x.id), ['集合丁'], 'collection category is filterable');
eq(assetMatchesFilter(globalRows[4], '', '未分类'), true, 'empty category maps to 未分类');
eq(filterAssets(globalRows, '找不到', '').length, 0, 'no match yields an empty list');
const categoryNames = assetCategoryNames(globalRows);
ok(categoryNames.includes('未分类') && categoryNames.includes('素材集合'), 'category list includes 未分类 and collection rows');
eq(categoryNames[categoryNames.length - 1], '未分类', '未分类 sorts last');
eq(
  globalAssetStatusText(34, 12),
  '全局素材库：共 34 / 显示 12',
  'global status shows 共 N / 显示 M'
);
eq(
  globalAssetStatusText(4, 1, { collections: 1, query: '按', category: '庭院' }),
  '全局素材库：共 4 / 显示 1（含 1 个集合） · 搜索：按 · 分类：庭院',
  'global status keeps filter context'
);

// Runtime contract for the actual global renderer: it must filter the cached
// rows (collections included), report 共 N / 显示 M, keep the input values, and
// produce the right empty state. Minimal DOM stubs stand in for the page.
function fakeElement() {
  return {
    textContent: '',
    children: [],
    appendChild(child) { this.children.push(child); },
    append(...children) { this.children.push(...children); },
  };
}

function installGlobalRenderDom(items) {
  const list = fakeElement();
  const status = fakeElement();
  const search = fakeElement();
  const category = fakeElement();
  search.value = '';
  category.value = '';
  const rendered = { ids: [] };
  sandbox.globalAssetsList = list;
  sandbox.globalAssetsStatus = status;
  sandbox.globalAssetSearch = search;
  sandbox.globalAssetCategoryFilter = category;
  sandbox.globalAssetsCache = items;
  sandbox.document = { createElement: () => fakeElement() };
  // renderGlobalAssets wires row actions from app.js; they are irrelevant here.
  for (const fn of [
    'pickAssetRoi', 'runAssetTest', 'editAssetPositions', 'deleteAsset',
    'editAssetActionRoi', 'testAssetClick', 'testAssetDrag', 'generateAssetStep',
    'testAssetCollection', 'deleteCollection',
  ]) {
    sandbox[fn] = () => {};
  }
  // The renderer now goes through app.js assetRowElement -> OASAssetUI.createRow.
  // The stub records every item that actually becomes a row so the existing
  // filter/status assertions keep testing real rendering, not just filtering.
  const OASAssetUI = {
    createRow(it) {
      rendered.ids.push(it.id);
      const row = fakeElement();
      row.dataset = { assetId: it.id };
      return row;
    },
  };
  sandbox.window = { OASAssetUI };
  // renderGlobalAssets clears + repaints the list on every call; the stub only
  // sees createRow calls, so reset the recorded ids at the same boundary.
  const renderOnce = sandbox.renderGlobalAssets;
  sandbox.renderGlobalAssets = () => {
    rendered.ids = [];
    return renderOnce();
  };
  return { list, status, search, category, rendered };
}

const gdom = installGlobalRenderDom(globalRows);
sandbox.renderGlobalAssets();
eq(gdom.rendered.ids.length, 5, 'global renderer renders every cached row by default');
eq(gdom.status.textContent, '全局素材库：共 5 / 显示 5（含 1 个集合）', 'global status total/visible on first render');

gdom.search.value = 'BUTTON';
sandbox.renderGlobalAssets();
eq(gdom.rendered.ids, ['button-b'], 'global id search filters the cached rows (case-insensitive)');
eq(gdom.search.value, 'BUTTON', 'renderer keeps the typed search value');
eq(gdom.category.value, '', 'renderer keeps the empty category');

gdom.search.value = '';
gdom.category.value = '庭院';
sandbox.renderGlobalAssets();
eq(gdom.rendered.ids, ['按钮甲', 'button-b'], 'global category filter applies');
eq(gdom.status.textContent, '全局素材库：共 5 / 显示 2（含 1 个集合） · 分类：庭院', 'global status shows category context');

gdom.search.value = '找不到';
gdom.category.value = '';
sandbox.renderGlobalAssets();
eq(gdom.list.textContent, '（没有匹配的全局素材）', 'filtered empty state is explicit');
eq(gdom.status.textContent, '全局素材库：共 5 / 显示 0（含 1 个集合） · 搜索：找不到', 'global status shows zero visible');

const gdomEmpty = installGlobalRenderDom([]);
sandbox.renderGlobalAssets();
eq(gdomEmpty.list.textContent, '（暂无全局素材）', 'unfiltered empty state stays 暂无全局素材');

// Requirement 5: the frame-detail source badge is GONE. assetRowElement must
// return the shared row untouched (no extra child even for a legacy
// `sourceLabel` option) and app.js must not contain the badge class anymore.
const savedWindow = sandbox.window;
const savedDocument = sandbox.document;
const sharedRow = { children: [], appendChild(child) { this.children.push(child); } };
sandbox.window = {
  OASAssetUI: { createRow: () => sharedRow },
};
sandbox.document = {
  createElement: () => ({ className: '', textContent: '', title: '', appendChild() {} }),
};
const rowWithLegacyLabel = sandbox.assetRowElement({ id: 'a' }, { sourceLabel: '全局/任务' });
eq(rowWithLegacyLabel.children.length, 0, 'no source badge is appended (requirement 5)');
ok(rowWithLegacyLabel === sharedRow, 'assetRowElement returns the shared OASAssetUI row unchanged');
ok(
  !/frame-asset-source/.test(source) && !/frame-asset-source/.test(styleCss),
  'the frame-asset source badge class is removed from app.js and style.css'
);
ok(
  !/function assetSourceLabel\s*\(/.test(source),
  'assetSourceLabel is deleted instead of left as dead code'
);
sandbox.window = savedWindow;
sandbox.document = savedDocument;

// Task library regression: it uses the same shared row wrapper, keeps its
// header, and keeps the old empty text. The row builder is stubbed so the
// recorded ids prove renderAssetRows actually produced the rows.
function installTaskRenderDom(items) {
  const list = fakeElement();
  const search = fakeElement();
  const category = fakeElement();
  search.value = '';
  category.value = '';
  const rendered = { ids: [], opts: null };
  sandbox.assetsList = list;
  sandbox.assetSearch = search;
  sandbox.assetCategoryFilter = category;
  sandbox.taskAssets = items;
  sandbox.taskAssetsMeta = { task: '任务甲', design: [1280, 720], count: items.length, scope: 'task' };
  sandbox.taskShowAllAssets = false;
  sandbox.taskName = { value: '任务甲' };
  sandbox.document = { createElement: () => fakeElement() };
  sandbox.attachAssetDragReorder = () => {};
  sandbox.assetRowElement = (it, opts) => {
    rendered.ids.push(it.id);
    rendered.opts = opts;
    const row = fakeElement();
    row.dataset = { assetId: it.id };
    return row;
  };
  for (const fn of ['pickAssetRoi', 'runAssetTest', 'editAssetPositions',
    'deleteAsset', 'editAssetActionRoi', 'testAssetClick', 'testAssetDrag',
    'generateAssetStep', 'testAssetCollection', 'deleteCollection']) {
    sandbox[fn] = () => {};
  }
  vm.runInContext(extractFunction('renderAssetList'), sandbox, {
    filename: 'app.js#task-assets-render',
  });
  const renderOnce = sandbox.renderAssetList;
  sandbox.renderAssetList = () => {
    rendered.ids = [];
    return renderOnce();
  };
  return { list, search, category, rendered };
}
const tdom = installTaskRenderDom(globalRows);
sandbox.renderAssetList();
eq(tdom.rendered.ids, ['按钮甲', 'button-b', '弹窗丙', '集合丁', '未标记'], 'task library renders every row through the shared builder');
eq(tdom.rendered.opts.task, '任务甲', 'task library passes the task scope to the shared row builder');
eq(tdom.list.children[0].textContent, '任务素材库 任务甲 · design 1280x720 · 5 个素材', 'task library keeps its header');
tdom.search.value = 'BUTTON';
sandbox.renderAssetList();
eq(tdom.rendered.ids, ['button-b'], 'task library search still uses the shared filter');
tdom.search.value = '找不到';
sandbox.renderAssetList();
eq(
  tdom.list.children[tdom.list.children.length - 1].textContent,
  '（没有匹配的素材）',
  'task library keeps its filtered empty text'
);

// --- frame list search: pure front-end filtering, never a /frames parameter ---
const searchFrames = [
  { id: 1, name: '宠物小屋', category: '宠物小屋' },
  { id: 12, name: '庭院-卷轴打开', category: '庭院' },
  { id: 21, name: 'Backyard', category: '庭院' },
  { id: 33, name: '未命名', category: '' },
];
ok(sandbox.frameMatchesQuery(searchFrames[0], ''), 'empty frame query matches all');
ok(sandbox.frameMatchesQuery(searchFrames[0], '   '), 'whitespace-only frame query matches all');
ok(sandbox.frameMatchesQuery(searchFrames[1], '12'), 'plain numeric query matches the frame id');
ok(sandbox.frameMatchesQuery(searchFrames[1], '#12'), '#id query matches the frame id');
ok(sandbox.frameMatchesQuery(searchFrames[1], '  #12  '), '#id query is trimmed');
ok(sandbox.frameMatchesQuery(searchFrames[1], '卷轴'), 'frame name substring is searchable');
ok(sandbox.frameMatchesQuery(searchFrames[1], '庭院'), 'frame category is searchable');
ok(sandbox.frameMatchesQuery(searchFrames[2], 'back'), 'frame search is case-insensitive');
ok(!sandbox.frameMatchesQuery(searchFrames[1], '#1'), 'numeric #id match is exact, not a prefix');
ok(!sandbox.frameMatchesQuery(searchFrames[1], '找不到'), 'unmatched frame query is false');
eq(sandbox.filterFramesByQuery(searchFrames, '').map((f) => f.id), [1, 12, 21, 33], 'empty query keeps all frames');
eq(sandbox.filterFramesByQuery(searchFrames, '#12').map((f) => f.id), [12], 'filterFramesByQuery keeps only the matching frame');
eq(sandbox.filterFramesByQuery(searchFrames, '庭院').map((f) => f.id), [12, 21], 'category filter preserves frame order');
eq(sandbox.frameSearchStatusText(30, 3), '共 30 帧 · 匹配 3 帧', 'frame search status shows the match count');
eq(sandbox.frameSearchStatusText(30, 0), '共 30 帧 · 没有匹配的帧', 'frame search status shows the empty state');
eq(sandbox.frameSearchStatusText(0, 0), '共 0 帧', 'frame search status handles no frames');

// --- frame detail assets: global + task, deduped by id, sources merged ---
const frameGlobalAssets = [
  { id: 'a', kind: 'ocr_line', positions: [1, 2], category: '庭院' },
  { id: 'b', kind: 'template', positions: [2], category: '庭院' },
  { id: 'empty', kind: 'ocr_line', positions: [] },
  { id: 'other', kind: 'region', positions: [3] },
  { id: 'nopos', kind: 'ocr_line' },
];
const frameTaskAssets = [
  { id: 'a', kind: 'ocr_line', positions: [2], category: '庭院' },
  { id: 'c', kind: 'region', positions: [2, 3], category: '庭院' },
  { id: 'd', kind: 'ocr_line', positions: [] },
];
const frame2Rows = sandbox.assetsForFrame(frameGlobalAssets, frameTaskAssets, 2);
eq(frame2Rows.map((r) => r.id), ['a', 'b', 'c'], 'assetsForFrame keeps only frame records and dedupes by id');
eq(frame2Rows[0].sources, ['global', 'task'], 'asset present in both libraries reports both sources');
eq(frame2Rows[1].sources, ['global'], 'global-only asset reports global source');
eq(frame2Rows[2].sources, ['task'], 'task-only asset reports task source');
ok(frame2Rows.every((r) => Array.isArray(r.positions) && r.positions.includes(2)), 'every frame row keeps its positions');
eq(sandbox.assetsForFrame(frameGlobalAssets, frameTaskAssets, 3).map((r) => r.id), ['other', 'c'], 'frame 3 combines global and task rows');
eq(sandbox.assetsForFrame(frameGlobalAssets, frameTaskAssets, 99), [], 'a frame with no records yields an empty list');
eq(sandbox.assetsForFrame(null, undefined, 2), [], 'missing libraries are safe');
ok(sandbox.assetHasFramePosition({ positions: ['2'] }, 2), 'string positions compare numerically');
ok(!sandbox.assetHasFramePosition({ positions: [2] }, 99), 'asset without the frame is rejected');
ok(!sandbox.assetHasFramePosition({}, 2), 'missing positions are rejected');
// C: one API scope per frame-detail row. global (either alone or together with
// task) writes to the unified store; task-only rows write to the current task.
eq(sandbox.frameAssetScope({ sources: ['global', 'task'] }, '任务甲'), '（全局素材库）', 'both-source row is global-scoped');
eq(sandbox.frameAssetScope({ sources: ['global'] }, '任务甲'), '（全局素材库）', 'global-only row is global-scoped');
eq(sandbox.frameAssetScope({ sources: ['task'] }, '任务甲'), '任务甲', 'task-only row uses the current task');
eq(sandbox.frameAssetScope({ sources: ['task'] }, '  任务乙  '), '任务乙', 'task scope is trimmed');
eq(sandbox.frameAssetScope({}, '任务甲'), '任务甲', 'a row without sources falls back to the current task');
eq(sandbox.frameAssetScope({ sources: ['task'] }, ''), '', 'no task + task-only row yields an empty scope');
eq(sandbox.frameAssetScope(null, '任务甲'), '任务甲', 'a missing row falls back to the current task');
const frame2Summary = sandbox.frameAssetsSummary(frame2Rows, 2);
eq(frame2Summary.total, 3, 'frame summary counts every row');
eq(frame2Summary.global, 2, 'frame summary counts global rows');
eq(frame2Summary.task, 2, 'frame summary counts task rows');
eq(frame2Summary.text, '该帧素材 3 条（全局 2 · 任务 2）', 'frame summary text keeps the source split');
eq(sandbox.frameAssetsSummary([], 2).text, '该帧没有素材记录', 'frame summary empty state');

// Runtime smoke test for the frame detail list: it must go through the same
// assetRowElement row builder, expose the SAME management menu as the global
// library (manage:true) and pass the per-row API scope (C).
function installFrameAssetsRenderHarness() {
  const status = fakeElement();
  const list = fakeElement();
  const calls = [];
  const actions = [];
  const manage = [];
  sandbox.frameAssetsStatus = status;
  sandbox.frameAssetsList = list;
  sandbox.taskName = { value: '任务甲' };
  sandbox.assetRowElement = (it, opts) => {
    calls.push({ id: it.id, opts });
    return fakeElement();
  };
  // MATCH-BUG: record which frame id each frame-detail row action targets, and
  // C: which API scope each management action hands over.
  sandbox.runAssetTest = (it, frameId, scope) => actions.push({ action: 'test', id: it.id, frameId, scope });
  sandbox.testAssetClick = (it, frameId, scope) => actions.push({ action: 'click', id: it.id, frameId, scope });
  sandbox.testAssetDrag = (it, frameId, scope) => actions.push({ action: 'drag', id: it.id, frameId, scope });
  sandbox.testAssetCollection = (it, frameId, scope) =>
    actions.push({ action: 'collection', id: it.id, frameId, scope });
  sandbox.pickAssetRoi = () => {};
  sandbox.generateAssetStep = () => {};
  sandbox.deleteCollection = () => {};
  for (const fn of [
    'editAssetRecognitionRoi', 'renameAsset', 'editAssetThreshold',
    'editAssetPositions', 'deleteAsset', 'editAssetActionRoi',
  ]) {
    sandbox[fn] = (it, scope) => actions.push({ action: fn, id: it.id, scope });
  }
  vm.runInContext(extractFunction('renderFrameAssets'), sandbox, {
    filename: 'app.js#frame-assets-render',
  });
  return { status, list, calls, actions, manage };
}
const frameDom = installFrameAssetsRenderHarness();
const frameDomRows = sandbox.assetsForFrame(frameGlobalAssets, frameTaskAssets, 2);
sandbox.renderFrameAssets({ id: 2 }, frameDomRows);
eq(frameDom.status.textContent, '该帧素材 3 条（全局 2 · 任务 2）', 'frame detail count line');
eq(frameDom.calls.map((c) => c.id), ['a', 'b', 'c'], 'frame detail builds one shared row per asset');
eq(frameDom.calls[0].opts.task, '', 'global rows use the global thumbnail scope');
eq(frameDom.calls[1].opts.task, '', 'global-only rows use the global thumbnail scope');
eq(frameDom.calls[2].opts.task, '任务甲', 'task-only rows use the task thumbnail scope');
ok(
  frameDom.calls.every((c) => c.opts.sourceLabel === undefined),
  'frame rows no longer carry a source label/badge'
);
ok(
  frameDom.calls.every((c) => c.opts.manage === true),
  'frame rows are fully manageable, exactly like the global library'
);
eq(frameDom.list.children.length, 3, 'frame detail appends every shared row');
// MATCH-BUG: every frame-detail row action must pin the frame it was rendered
// for; otherwise this new surface would silently re-run the saved-position bug.
const frame2RowOpts = frameDom.calls[0].opts;
frame2RowOpts.onTest({ id: 'a' });
frame2RowOpts.onClickTest({ id: 'a' });
frame2RowOpts.onDragTest({ id: 'a' });
frame2RowOpts.onCollectionTest({ id: 'a' });
eq(
  frameDom.actions,
  [
    { action: 'test', id: 'a', frameId: 2, scope: '（全局素材库）' },
    { action: 'click', id: 'a', frameId: 2, scope: '（全局素材库）' },
    { action: 'drag', id: 'a', frameId: 2, scope: '（全局素材库）' },
    { action: 'collection', id: 'a', frameId: 2, scope: '（全局素材库）' },
  ],
  'frame detail row actions all pass the rendered frame id + the global scope'
);
// C: management callbacks must carry the row scope too (task-only row = 任务甲).
frameDom.actions.length = 0;
const taskRowOpts = frameDom.calls[2].opts;
for (const name of [
  'onEditRoi', 'onRename', 'onThreshold', 'onPosition', 'onDelete', 'onAction',
]) {
  taskRowOpts[name]({ id: 'c' });
}
eq(
  frameDom.actions.map((a) => `${a.action}:${a.scope}`),
  [
    'editAssetRecognitionRoi:任务甲',
    'renameAsset:任务甲',
    'editAssetThreshold:任务甲',
    'editAssetPositions:任务甲',
    'deleteAsset:任务甲',
    'editAssetActionRoi:任务甲',
  ],
  'task-only frame rows pass the current task to every management callback'
);
sandbox.renderFrameAssets({ id: 2 }, []);
eq(frameDom.list.textContent, '该帧没有素材记录', 'frame detail empty state');
eq(frameDom.status.textContent, '该帧没有素材记录', 'frame detail empty count line');
sandbox.renderFrameAssets(null, []);
eq(frameDom.list.textContent, '（选择一帧查看该帧素材）', 'no current frame placeholder');
eq(frameDom.status.textContent, '—', 'no current frame leaves count as —');

// Runtime smoke test for the filtered frame list: search is applied to the
// cached rows, category filtering comes afterwards, and the status count
// follows the visible rows.
function fakeFrameDomElement() {
  const el = {
    className: '',
    title: '',
    draggable: false,
    dataset: {},
    style: {},
    children: [],
    classList: { add() {}, remove() {}, toggle() {} },
    appendChild(child) { el.children.push(child); return child; },
    append(...children) { el.children.push(...children); },
    addEventListener() {},
  };
  Object.defineProperty(el, 'textContent', {
    get() { return el._text || ''; },
    set(v) { el._text = String(v); el.children = []; },
  });
  return el;
}

function installFrameListRenderHarness() {
  const list = fakeFrameDomElement();
  const status = fakeFrameDomElement();
  sandbox.framesList = list;
  sandbox.frameSearch = { value: '' };
  sandbox.frameSearchStatus = status;
  sandbox.framesCache = searchFrames;
  sandbox.categoryFilterValue = '';
  sandbox.globalAssetsCache = [];
  sandbox.taskAssets = [];
  sandbox.document = { createElement: () => fakeFrameDomElement() };
  for (const fn of ['moveFrameToCategory', 'moveFrameBefore', 'selectFrame',
    'renameFrame', 'setFrameCategory', 'deleteFrame']) {
    sandbox[fn] = () => {};
  }
  vm.runInContext(extractFunction('renderFramesList'), sandbox, {
    filename: 'app.js#frames-list-render',
  });
  const ids = () => {
    const out = [];
    for (const group of list.children) {
      for (const child of group.children || []) {
        if (child.dataset && child.dataset.id) out.push(child.dataset.id);
      }
    }
    return out;
  };
  return { list, status, ids };
}
const framesDom = installFrameListRenderHarness();
sandbox.renderFramesList();
eq(framesDom.status.textContent, '共 4 帧 · 匹配 4 帧', 'frame list renders all frames without a query');
eq(framesDom.status.style.display, 'none', 'an empty search hides the status line so the frame list keeps its height');
eq(framesDom.ids(), ['1', '12', '21', '33'], 'frame list keeps grouping order');
sandbox.frameSearch.value = '庭院';
sandbox.renderFramesList();
eq(framesDom.status.textContent, '共 4 帧 · 匹配 2 帧', 'frame list updates the count while typing');
eq(framesDom.status.style.display, '', 'typing a query reveals the search status line');
eq(framesDom.ids(), ['12', '21'], 'frame list shows only matching frames');
sandbox.frameSearch.value = '找不到';
sandbox.renderFramesList();
eq(framesDom.list.textContent, '没有匹配的帧', 'frame list shows the no-match message');
eq(framesDom.status.textContent, '共 4 帧 · 没有匹配的帧', 'frame list status shows zero matches');
sandbox.frameSearch.value = '庭院';
sandbox.categoryFilterValue = '宠物小屋';
sandbox.renderFramesList();
eq(framesDom.list.textContent, '没有匹配的帧', 'category + search with no overlap stays explicit');
sandbox.categoryFilterValue = '';
sandbox.frameSearch.value = '  #12  ';
sandbox.renderFramesList();
eq(framesDom.ids(), ['12'], 'search survives a category reset and trims #id');
sandbox.frameSearch.value = '';
sandbox.renderFramesList();
eq(framesDom.ids(), ['1', '12', '21', '33'], 'clearing the query restores the cached list');

// --- P1-1: a global-library change must repaint the frame surfaces ---
// Every asset CRUD path refreshes the task panel first (whose refreshFrames()
// still reads the OLD globalAssetsCache) and only then calls
// refreshGlobalAssets(). renderFrameAssetViews() is the cache-only repaint that
// closes the gap, so the frame row "素材 N" and the frame detail list must
// follow a swapped globalAssetsCache. This drives the real renderers.
function installFrameAssetViewsHarness() {
  const detail = fakeFrameDomElement();
  const assetsStatus = fakeFrameDomElement();
  const assetsList = fakeFrameDomElement();
  const rowIds = [];
  sandbox.frameDetail = detail;
  sandbox.frameAssetsStatus = assetsStatus;
  sandbox.frameAssetsList = assetsList;
  sandbox.assetRowElement = (it) => {
    rowIds.push(it.id);
    return fakeFrameDomElement();
  };
  sandbox.framesCache = [
    { id: 7, name: '帧甲', category: '庭院', width: 1280, height: 720, current: true },
  ];
  sandbox.currentFrameId = 7;
  sandbox.categoryFilterValue = '';
  sandbox.frameSearch.value = '';
  sandbox.globalAssetsCache = [];
  sandbox.taskAssets = [];
  // A second /frames round trip after an asset save would be redundant and is
  // exactly what the fix avoids; count any accidental call.
  sandbox.refreshFramesCalls = 0;
  sandbox.refreshFrames = () => {
    sandbox.refreshFramesCalls += 1;
    return Promise.resolve();
  };
  vm.runInContext(extractFunction('renderFrameDetail'), sandbox, {
    filename: 'app.js#frame-detail-render',
  });
  const pickText = () => {
    for (const group of sandbox.framesList.children) {
      for (const child of group.children || []) {
        if (child.dataset && child.dataset.id === '7') {
          const pick = (child.children || [])[0];
          return pick ? pick.textContent : '';
        }
      }
    }
    return '';
  };
  return { detail, assetsStatus, assetsList, rowIds, pickText };
}
const frameViews = installFrameAssetViewsHarness();
sandbox.renderFrameAssetViews();
ok(frameViews.pickText().includes('素材 0'), 'frame row starts with zero linked assets');
ok(frameViews.detail.textContent.includes('该帧素材：0 条'), 'frame detail starts with zero assets');
eq(frameViews.assetsStatus.textContent, '该帧没有素材记录', 'frame asset count starts empty');

// Save-asset round trip: loadGlobalAssets() replaces the cache array wholesale.
sandbox.globalAssetsCache = [
  { id: 'a', kind: 'ocr_line', positions: [7], category: '庭院' },
  { id: 'b', kind: 'template', positions: [7, 8], category: '庭院' },
];
sandbox.renderFrameAssetViews();
ok(frameViews.pickText().includes('素材 2'), 'a saved global asset updates the frame row count');
ok(
  frameViews.detail.textContent.includes('该帧素材：2 条'),
  'a saved global asset updates the frame detail count'
);
ok(
  frameViews.detail.textContent.includes('关联素材：a、b'),
  'the frame detail lists the refreshed asset ids'
);
eq(
  frameViews.assetsStatus.textContent,
  '该帧素材 2 条（全局 2 · 任务 0）',
  'the frame asset list follows the refreshed global cache'
);
eq(frameViews.assetsList.children.length, 2, 'the frame asset list rebuilds one row per refreshed asset');
eq(frameViews.rowIds, ['a', 'b'], 'the frame asset rows are built from the refreshed cache');

// Edit-positions round trip: the asset moves away from frame 7.
sandbox.globalAssetsCache = [{ id: 'b', kind: 'template', positions: [8], category: '庭院' }];
sandbox.renderFrameAssetViews();
ok(frameViews.pickText().includes('素材 0'), 'moving positions drops the asset from the frame row');
ok(
  frameViews.detail.textContent.includes('该帧素材：0 条'),
  'moving positions drops the asset from the frame detail'
);
eq(frameViews.assetsStatus.textContent, '该帧没有素材记录', 'moving positions empties the frame asset list');

// No current frame: the detail surfaces keep their placeholders.
sandbox.currentFrameId = null;
sandbox.renderFrameAssetViews();
eq(frameViews.detail.textContent, '（选择一帧查看详情）', 'no current frame keeps the detail placeholder');
eq(
  frameViews.assetsList.textContent,
  '（选择一帧查看该帧素材）',
  'no current frame keeps the asset list placeholder'
);
eq(sandbox.refreshFramesCalls, 0, 'the frame repaint never issues a second /frames request');

// --- asset row: exactly 识别 + 设置, with a nine-item inline settings menu ---
// asset_ui.js is a browser IIFE, so run the REAL file against a tiny DOM stub.
// This covers createRow()/assetMenuState() instead of a copied expectation.
function assetDomElement(tagName) {
  const el = {
    tagName: String(tagName || 'div').toUpperCase(),
    className: '',
    title: '',
    type: '',
    disabled: false,
    draggable: false,
    dataset: {},
    style: {},
    attrs: {},
    children: [],
    listeners: {},
    textContent: '',
    appendChild(child) { el.children.push(child); return child; },
    append(...children) { el.children.push(...children); },
    addEventListener(type, fn) {
      el.listeners[type] = el.listeners[type] || [];
      el.listeners[type].push(fn);
    },
    dispatch(type, event) {
      const ev = event || { stopPropagation() {}, preventDefault() {} };
      for (const fn of el.listeners[type] || []) fn(ev);
    },
    setAttribute(name, value) { el.attrs[name] = String(value); },
    getAttribute(name) { return el.attrs[name]; },
    focus() { el.focused = true; },
  };
  el.classList = {
    _set: new Set(),
    add(...names) { names.forEach((n) => el.classList._set.add(n)); },
    remove(...names) { names.forEach((n) => el.classList._set.delete(n)); },
    contains(name) { return el.classList._set.has(name); },
    toggle(name, force) {
      const next = force == null ? !el.classList._set.has(name) : !!force;
      if (next) el.classList._set.add(name);
      else el.classList._set.delete(name);
      return next;
    },
  };
  return el;
}

function installAssetUiHarness() {
  const uiSandbox = {
    window: {},
    document: { createElement: (tag) => assetDomElement(tag) },
    Image: function AssetRowTestImage() {},
  };
  vm.createContext(uiSandbox);
  vm.runInContext(assetUiSource, uiSandbox, { filename: 'asset_ui.js#row' });
  return { ui: uiSandbox.window.OASAssetUI, sandbox: uiSandbox };
}

const assetUiHarness = installAssetUiHarness();
const assetUi = assetUiHarness.ui;
const ASSET_MENU_LABELS = ['点测', '拖测', '识别区', '操作区', '重命名', '阈值', '位置帧', '删除', '步骤'];

function assetChildren(el, className) {
  return (el.children || []).filter((child) =>
    String(child.className || '').split(/\s+/).includes(className));
}
function assetRowActionText(row) {
  return assetChildren(row, 'asset-row-action').map((b) => b.textContent);
}
function assetRowMenu(row) {
  return assetChildren(row, 'asset-row-menu')[0] || null;
}
function assetMenuButton(row, key) {
  const menu = assetRowMenu(row);
  return menu ? menu.children.find((b) => b.dataset.menuKey === key) : null;
}
function assetRowOpts(overrides) {
  const calls = [];
  const record = (name) => (it) => calls.push({ action: name, id: it.id });
  const opts = {
    task: '任务甲',
    manage: true,
    actions: true,
    onTest: record('test'),
    onPosition: record('position'),
    onDelete: record('delete'),
    onAction: record('action'),
    onClickTest: record('clickTest'),
    onDragTest: record('dragTest'),
    onStep: record('step'),
    onEditRoi: record('editRoi'),
    onRename: record('rename'),
    onThreshold: record('threshold'),
    onCollectionTest: record('collectionTest'),
    onCollectionDelete: record('collectionDelete'),
  };
  return { opts: Object.assign(opts, overrides || {}), calls };
}

const rowTemplate = { id: '模板甲', kind: 'template', threshold: 0.9, roi: [10, 20, 30, 40], positions: [7] };
const rowOcr = { id: '文字乙', kind: 'ocr_line', roi: [1, 2, 3, 4], positions: [7] };
const rowRegion = { id: '区域丙', kind: 'region', roi: [5, 6, 7, 8] };
const rowSet = { id: '集合丁', kind: 'template_set', members: ['a', 'b'] };

eq(
  assetUi.ASSET_MENU_ITEMS.map((m) => m.label),
  ASSET_MENU_LABELS,
  'the settings menu is the agreed nine-item order'
);
eq(
  assetUi.assetMenuState(rowTemplate, { manage: true }).filter((m) => m.disabled).map((m) => m.label),
  [],
  'a manageable template enables all nine items (识别区 now re-crops through replace)'
);
eq(
  assetUi.assetMenuState(rowTemplate, { manage: true }).filter((m) => !m.disabled).map((m) => m.label),
  ['点测', '拖测', '识别区', '操作区', '重命名', '阈值', '位置帧', '删除', '步骤'],
  'every template settings item is actionable on a manageable surface'
);
const templateRoiMenu = assetUi.assetMenuState(rowTemplate, { manage: true }).find((m) => m.key === 'roi');
ok(
  /画布/.test(templateRoiMenu.title) && !/服务端拒绝修改模板 ROI/.test(templateRoiMenu.title),
  'template 识别区 is now a canvas editor, not the old rejected-ROI warning'
);
ok(
  !/m\.key === 'roi' && kind === 'template'/.test(assetUiSource),
  'asset_ui.js no longer carries the template 识别区 disable rule'
);
const readOnlyMenu = assetUi.assetMenuState(rowTemplate, { manage: false });
eq(
  readOnlyMenu.filter((m) => m.disabled).map((m) => m.label),
  ['识别区', '操作区', '重命名', '阈值', '位置帧', '删除'],
  'manage:false disables every management item'
);
eq(
  readOnlyMenu.filter((m) => !m.disabled).map((m) => m.label),
  ['点测', '拖测', '步骤'],
  'manage:false keeps the dry-run tests usable'
);
// E: threshold is enabled for template + OCR lines, disabled for region/set.
const ocrThresholdMenu = assetUi.assetMenuState(rowOcr, { manage: true }).find((m) => m.key === 'threshold');
eq(ocrThresholdMenu.disabled, false, 'OCR text assets can edit a threshold');
ok(
  /OCR 最低置信度（0~1；不设或 0 = 不限制）/.test(ocrThresholdMenu.title),
  'the OCR threshold title names the minimum-confidence meaning'
);
const ocrLinesThresholdMenu = assetUi
  .assetMenuState({ id: '多行丁', kind: 'ocr_lines', roi: [1, 2, 3, 4] }, { manage: true })
  .find((m) => m.key === 'threshold');
eq(ocrLinesThresholdMenu.disabled, false, 'ocr_lines also exposes the threshold');
const templateThresholdMenu = assetUi.assetMenuState(rowTemplate, { manage: true }).find((m) => m.key === 'threshold');
eq(templateThresholdMenu.disabled, false, 'template assets can edit a threshold');
ok(/模板阈值/.test(templateThresholdMenu.title), 'the template threshold title names the match threshold');
const regionThresholdMenu = assetUi.assetMenuState(rowRegion, { manage: true }).find((m) => m.key === 'threshold');
eq(regionThresholdMenu.disabled, true, 'region assets cannot edit a threshold');
const regionActionMenu = assetUi.assetMenuState(rowRegion, { manage: true }).find((m) => m.key === 'action');
eq(regionActionMenu.disabled, true, 'region assets disable 操作区');
ok(/region/.test(regionActionMenu.title), 'the disabled 操作区 title explains the region rule');
eq(
  assetUi.assetMenuState(rowRegion, { manage: true }).find((m) => m.key === 'roi').disabled,
  false,
  'region assets keep 识别区 enabled (the server accepts region ROI updates)'
);
const setMenuState = assetUi.assetMenuState(rowSet, { manage: true });
eq(
  setMenuState.filter((m) => !m.disabled).map((m) => m.label),
  ['删除'],
  'a template_set settings menu only enables 删除'
);
eq(setMenuState.filter((m) => m.disabled).length, 8, 'the other eight collection entries stay visible but disabled');
ok(
  setMenuState.filter((m) => m.disabled).every((m) => /集合不支持/.test(m.title)),
  'disabled collection entries explain that sets do not support them'
);

const tplRowHarness = assetRowOpts();
const tplRow = assetUi.createRow(rowTemplate, tplRowHarness.opts);
eq(assetRowActionText(tplRow), ['识别', '设置'], 'a normal asset row exposes exactly 识别 + 设置');
const tplMenu = assetRowMenu(tplRow);
ok(tplMenu && !tplMenu.classList.contains('open'), 'the settings menu starts collapsed');
eq(tplMenu.children.map((b) => b.textContent), ASSET_MENU_LABELS, 'the settings menu renders all nine items in order');
eq(
  assetChildren(tplRow, 'asset-row-action')[0].title,
  '识别：在当前帧用该素材测试模板匹配',
  'template 识别 keeps the template-match meaning'
);
eq(
  assetRowActionText(assetUi.createRow(rowOcr, assetRowOpts().opts))[0],
  '识别',
  'OCR rows use the same 识别 label'
);
ok(
  /试识别/.test(assetChildren(assetUi.createRow(rowOcr, assetRowOpts().opts), 'asset-row-action')[0].title),
  'OCR 识别 title names the text trial'
);
ok(
  /操作区域/.test(assetChildren(assetUi.createRow(rowRegion, assetRowOpts().opts), 'asset-row-action')[0].title),
  'region 识别 title names the operation region'
);
eq(
  assetMenuButton(tplRow, 'roi').disabled,
  false,
  'the rendered template row keeps 识别区 enabled'
);
eq(
  assetMenuButton(assetUi.createRow(rowOcr, assetRowOpts().opts), 'roi').disabled,
  false,
  'OCR rows keep 识别区 enabled'
);
const tplRoiHarness = assetRowOpts();
const tplRoiRow = assetUi.createRow(rowTemplate, tplRoiHarness.opts);
assetChildren(tplRoiRow, 'asset-row-action')[1].dispatch('click');
assetMenuButton(tplRoiRow, 'roi').dispatch('click');
eq(
  tplRoiHarness.calls,
  [{ action: 'editRoi', id: '模板甲' }],
  'clicking template 识别区 opens the canvas ROI editor'
);
const tplSettings = assetChildren(tplRow, 'asset-row-action')[1];
tplSettings.dispatch('click');
ok(tplMenu.classList.contains('open'), 'clicking 设置 opens the inline menu');
tplSettings.dispatch('click');
ok(!tplMenu.classList.contains('open'), 'clicking 设置 again collapses the menu');
tplSettings.dispatch('click');
assetMenuButton(tplRow, 'rename').dispatch('click');
eq(tplRowHarness.calls, [{ action: 'rename', id: '模板甲' }], 'a menu item runs the split callback for its row');
ok(
  tplMenu.classList.contains('open'),
  'the settings panel stays open after running an item (requirement 1)'
);

const readOnlyHarness = assetRowOpts({ manage: false });
const readOnlyRow = assetUi.createRow(rowTemplate, readOnlyHarness.opts);
const readOnlyRename = assetMenuButton(readOnlyRow, 'rename');
eq(readOnlyRename.disabled, true, 'the rendered menu disables management items on read-only rows');
assetChildren(readOnlyRow, 'asset-row-action')[1].dispatch('click');
readOnlyRename.dispatch('click');
eq(readOnlyHarness.calls, [], 'clicking a disabled menu item never calls its callback');

const setHarness = assetRowOpts();
const setRow = assetUi.createRow(rowSet, setHarness.opts);
eq(assetRowActionText(setRow), ['识别', '设置'], 'collection rows use the same two action buttons');
eq(
  assetRowMenu(setRow).children.filter((b) => !b.disabled).map((b) => b.textContent),
  ['删除'],
  'the rendered collection menu enables only 删除'
);
assetChildren(setRow, 'asset-row-action')[0].dispatch('click');
eq(setHarness.calls.map((c) => c.action), ['collectionTest'], 'collection 识别 tests the collection');
assetChildren(setRow, 'asset-row-action')[1].dispatch('click');
assetMenuButton(setRow, 'delete').dispatch('click');
eq(setHarness.calls.map((c) => c.action), ['collectionTest', 'collectionDelete'], 'collection 删除 uses onCollectionDelete');

const noActionRow = assetUi.createRow(rowOcr, assetRowOpts({ actions: false }).opts);
eq(assetRowActionText(noActionRow), [], 'actions:false hides 识别 and 设置');
eq(assetRowMenu(noActionRow), null, 'actions:false renders no settings menu');

const compactRow = assetUi.createRow(rowOcr, assetRowOpts({ compact: true }).opts);
eq(
  assetRowMenu(compactRow).children.map((b) => b.textContent),
  ['识别区', '操作区', '重命名', '阈值', '位置帧', '删除'],
  'compact keeps the old hiding of 点测/拖测/步骤'
);

// --- A: selected asset row + "settings must not collapse" (requirement 1) ---
// A fresh asset_ui instance so the module-level selection starts clean.
const selHarness = installAssetUiHarness();
const selUi = selHarness.ui;
eq(selUi.SELECTED_CLASS, 'selected', 'the selected-row modifier class is "selected"');
ok(
  /\.asset-row\.selected\b/.test(styleCss),
  'style.css defines .asset-row.selected (visible highlight in the dark theme)'
);
eq(typeof selUi.selectAsset, 'function', 'OASAssetUI.selectAsset is exported');
eq(typeof selUi.getSelectedAsset, 'function', 'OASAssetUI.getSelectedAsset is exported');
eq(typeof selUi.closeMenus, 'function', 'OASAssetUI.closeMenus is exported');
eq(selUi.getSelectedAsset(), '', 'nothing is selected before the first click');
const selRowA = selUi.createRow({ id: '甲', kind: 'ocr_line', roi: [1, 2, 3, 4] }, assetRowOpts().opts);
const selRowB = selUi.createRow({ id: '乙', kind: 'ocr_line', roi: [1, 2, 3, 4] }, assetRowOpts().opts);
ok(!selRowA.classList.contains('selected'), 'a fresh row is not selected');

// 识别 selects the row.
assetChildren(selRowA, 'asset-row-action')[0].dispatch('click');
eq(selUi.getSelectedAsset(), '甲', '识别 selects the row');
ok(selRowA.classList.contains('selected'), 'the clicked row carries the selected class');
ok(!selRowB.classList.contains('selected'), 'other rows stay unselected');

// 设置 selects + opens; an item click keeps it open (requirement 1).
assetChildren(selRowA, 'asset-row-action')[1].dispatch('click');
const selMenuA = assetRowMenu(selRowA);
ok(selMenuA.classList.contains('open'), '设置 opens the panel for the selected asset');
assetMenuButton(selRowA, 'rename').dispatch('click');
ok(selMenuA.classList.contains('open'), 'a settings item never collapses the panel');
eq(selUi.getSelectedAsset(), '甲', 'using a settings item keeps the row selected');

// Choosing ANOTHER asset closes the old panel and moves the highlight.
assetChildren(selRowB, 'asset-row-action')[0].dispatch('click');
ok(!selMenuA.classList.contains('open'), 'selecting another asset closes the previous panel');
eq(selUi.getSelectedAsset(), '乙', 'the newly clicked asset is selected');
ok(selRowB.classList.contains('selected') && !selRowA.classList.contains('selected'), 'the highlight moved');

// Re-render (save/refresh) restores the highlight from module state.
const selRowB2 = selUi.createRow({ id: '乙', kind: 'ocr_line', roi: [1, 2, 3, 4] }, assetRowOpts().opts);
ok(selRowB2.classList.contains('selected'), 'a re-rendered row restores the selection by asset id');
const selRowA2 = selUi.createRow({ id: '甲', kind: 'ocr_line', roi: [1, 2, 3, 4] }, assetRowOpts().opts);
ok(!selRowA2.classList.contains('selected'), 'unselected assets stay unselected after a re-render');

// 设置 again / Esc collapse it; closeMenus() is the public escape hatch.
assetChildren(selRowB, 'asset-row-action')[1].dispatch('click');
const selMenuB = assetRowMenu(selRowB);
ok(selMenuB.classList.contains('open'), '设置 opens the panel again');
assetChildren(selRowB, 'asset-row-action')[1].dispatch('click');
ok(!selMenuB.classList.contains('open'), 'clicking 设置 again collapses the panel');
assetChildren(selRowB, 'asset-row-action')[1].dispatch('click');
assetChildren(selRowB, 'asset-row-action')[1].dispatch('keydown', { key: 'Escape' });
ok(!selMenuB.classList.contains('open'), 'Esc collapses the settings panel');
assetChildren(selRowB, 'asset-row-action')[1].dispatch('click');
selUi.closeMenus();
ok(!selMenuB.classList.contains('open'), 'closeMenus() collapses the open panel');

// A disabled item must neither act nor change the selection.
const roHarness = assetRowOpts({ manage: false });
const selRoRow = selUi.createRow({ id: '丙', kind: 'template', roi: [1, 2, 3, 4] }, roHarness.opts);
selUi.selectAsset('乙');
const roDisabledRename = assetMenuButton(selRoRow, 'rename');
eq(roDisabledRename.disabled, true, 'read-only rows render disabled management items');
roDisabledRename.dispatch('click');
eq(roHarness.calls, [], 'a disabled settings item runs no callback');
eq(selUi.getSelectedAsset(), '乙', 'a disabled settings item never changes the selection');
eq(selUi.selectAsset(null), '', 'selectAsset(null) clears the selection');
eq(selUi.selectAsset(123), '123', 'numeric asset ids are stored as strings');

// P2-3: a list re-render (save/rename/threshold/positions) replaces the DOM row
// that owned the open settings menu; the fresh row must reopen the panel for
// the still-selected asset instead of silently collapsing it.
const restoreHarness = installAssetUiHarness();
const restoreUi = restoreHarness.ui;
const restoreRowA = restoreUi.createRow({ id: '甲', kind: 'ocr_line', roi: [1, 2, 3, 4] }, assetRowOpts().opts);
restoreUi.selectAsset('甲');
assetChildren(restoreRowA, 'asset-row-action')[1].dispatch('click');
ok(assetRowMenu(restoreRowA).classList.contains('open'), 'P2-3 setup: settings menu is open');
const restoreRowA2 = restoreUi.createRow({ id: '甲', kind: 'ocr_line', roi: [1, 2, 3, 4] }, assetRowOpts().opts);
ok(
  assetRowMenu(restoreRowA2).classList.contains('open'),
  'a re-rendered row restores the open settings menu for the selected asset (P2-3)'
);
restoreUi.closeMenus();
const restoreRowA3 = restoreUi.createRow({ id: '甲', kind: 'ocr_line', roi: [1, 2, 3, 4] }, assetRowOpts().opts);
ok(
  !assetRowMenu(restoreRowA3).classList.contains('open'),
  'after closeMenus a re-render does not reopen the menu'
);

// The browser path defers the restore by one tick so rows are connected and a
// hidden rail-panel copy cannot steal the menu from the visible surface.
function installDeferredAssetUiHarness() {
  const timers = [];
  const uiSandbox = {
    window: {},
    document: { createElement: (tag) => assetDomElement(tag) },
    Image: function AssetRowTestImage() {},
    setTimeout: (fn) => { timers.push(fn); return timers.length; },
  };
  vm.createContext(uiSandbox);
  vm.runInContext(assetUiSource, uiSandbox, { filename: 'asset_ui.js#deferred-menu' });
  return { ui: uiSandbox.window.OASAssetUI, timers };
}
const deferredHarness = installDeferredAssetUiHarness();
const deferredUi = deferredHarness.ui;
const deferredRow = deferredUi.createRow({ id: '甲', kind: 'ocr_line', roi: [1, 2, 3, 4] }, assetRowOpts().opts);
deferredUi.selectAsset('甲');
assetChildren(deferredRow, 'asset-row-action')[1].dispatch('click');
ok(assetRowMenu(deferredRow).classList.contains('open'), 'deferred restore setup: menu is open');
const deferredRow2 = deferredUi.createRow({ id: '甲', kind: 'ocr_line', roi: [1, 2, 3, 4] }, assetRowOpts().opts);
ok(
  !assetRowMenu(deferredRow2).classList.contains('open'),
  'the menu restore is deferred until the re-rendered row is connected'
);
eq(deferredHarness.timers.length, 1, 'one coalesced restore timer is scheduled per re-render');
deferredHarness.timers.shift()();
ok(
  assetRowMenu(deferredRow2).classList.contains('open'),
  'the deferred restore reopens the menu on the new row'
);

// Static contracts for the split editors: every surface wires the split
// callbacks, the row never calls a combined onEdit, and the legacy combined
// editor is gone (it was dead code: nothing outside the IIFE could reach it).
for (const name of ['renderAssetList', 'renderGlobalAssets', 'renderFrameAssets']) {
  const fn = extractFunction(name);
  ok(
    /onEditRoi:/.test(fn) && /onRename:/.test(fn) && /onThreshold:/.test(fn) &&
      /onPosition:/.test(fn) && /onAction:/.test(fn) && /onStep:/.test(fn),
    `${name} wires the nine-item settings callbacks`
  );
}
ok(
  !/m\.key === 'roi' && kind === 'template'/.test(assetUiSource) &&
    !/模板无识别区/.test(assetUiSource),
  'asset_ui.js has no template 识别区 disable rule left'
);
const createRowFn = extractFunctionFrom(assetUiSource, 'createRow', 'asset_ui.js');
ok(!/o\.onEdit\b/.test(createRowFn) && !/onEdit:/.test(createRowFn), 'createRow never invokes a combined onEdit');
ok(!/\bonEdit\b/.test(source), 'app.js no longer wires the legacy combined onEdit');
ok(!/function editAsset\s*\(/.test(source), 'the dead editAsset editor is removed from app.js');
ok(
  !/makeButton\('(?:改|帧|删|操作区|点测|拖测|步骤|测试|测集合)'/.test(assetUiSource),
  'the old per-action asset buttons are gone from asset_ui.js'
);
const renameAssetFn = extractFunction('renameAsset');
ok(/\/task\/asset\/rename/.test(renameAssetFn), 'renameAsset posts /task/asset/rename');
ok(
  /await refreshTaskAssetsForScope\(task\);/.test(renameAssetFn) &&
    /await refreshGlobalAssets\(\);/.test(renameAssetFn),
  'renameAsset refreshes both asset libraries'
);
const thresholdAssetFn = extractFunction('editAssetThreshold');
ok(
  /postJson\('\/task\/asset\/update', \{ task, id: it\.id, threshold \}\)/.test(thresholdAssetFn),
  'editAssetThreshold sends only the threshold to /task/asset/update'
);
ok(
  /await refreshTaskAssetsForScope\(task\);/.test(thresholdAssetFn) &&
    /await refreshGlobalAssets\(\);/.test(thresholdAssetFn),
  'editAssetThreshold refreshes both asset libraries'
);
// E: template + OCR share the POST, but the prompt/title must say which value it
// is; region/template_set are rejected before prompting.
ok(
  /assetThresholdKind\(it\)/.test(thresholdAssetFn) &&
    /OCR 最低置信度 0~1（当前/.test(thresholdAssetFn) &&
    /模板阈值 0~1（当前/.test(thresholdAssetFn),
  'editAssetThreshold distinguishes the template threshold from the OCR confidence'
);
ok(
  /assetRowScopeTask\(scopeTask, true\)/.test(thresholdAssetFn),
  'editAssetThreshold accepts the row-scope override'
);
const thresholdKindFn = extractFunction('assetThresholdKind');
ok(
  /return 'template'/.test(thresholdKindFn) && /kind === 'ocr_line' \|\| kind === 'ocr_lines'/.test(thresholdKindFn),
  'assetThresholdKind enables exactly template / ocr_line / ocr_lines'
);
eq(sandbox.assetThresholdKind({ kind: 'template' }), 'template', 'template assets use the template threshold');
eq(sandbox.assetThresholdKind({ kind: 'ocr_line' }), 'ocr', 'ocr_line assets use the OCR confidence');
eq(sandbox.assetThresholdKind({ kind: 'ocr_lines' }), 'ocr', 'ocr_lines assets use the OCR confidence');
eq(sandbox.assetThresholdKind({ kind: 'region' }), '', 'region assets have no threshold');
eq(sandbox.assetThresholdKind({ kind: 'template_set' }), '', 'template_set assets have no threshold');
eq(sandbox.assetThresholdKind(null), '', 'a missing asset has no threshold');
// --- B: 识别区 is a CANVAS editor now (no prompt, no typed coordinates) ---
const roiEditorFn = extractFunction('editAssetRecognitionRoi');
ok(
  /resetSelection\(/.test(roiEditorFn) && /pickAssetRoi\(it\)/.test(roiEditorFn),
  'editAssetRecognitionRoi leaves action modes and loads the asset ROI on the canvas'
);
ok(
  /editingRecognitionAsset = \{ it, scopeTask: task \}/.test(roiEditorFn),
  'editAssetRecognitionRoi records the asset + row scope being edited'
);
ok(/updateAssetRoiSaveButton\(\)/.test(roiEditorFn), 'entering ROI edit enables the save button');
ok(
  !/activatePanel\('panel-assets'\)/.test(roiEditorFn),
  'entering ROI edit never forces the 素材 panel (the button is shared, P0-1)'
);
ok(
  /btnSaveAssetRoi\.addEventListener\('click'/.test(source),
  'the footer save button keeps its click listener'
);
ok(!/window\.prompt/.test(roiEditorFn), 'the old prompt-based ROI editor is gone');
ok(!/function editAssetRoi\s*\(/.test(source), 'editAssetRoi (prompt version) is deleted');
const roiSaveFn = extractFunction('saveAssetRecognitionRoi');
ok(
  /postJson\('\/task\/save_asset', payload\)/.test(roiSaveFn) &&
    /assetRoiReplacePayload\(task, it, valid\)/.test(roiSaveFn),
  'saving a template ROI overwrites through /task/save_asset + the replace payload builder'
);
ok(
  /覆盖模板「\$\{it\.id\}」（rev\+1/.test(roiSaveFn),
  'the template overwrite confirmation names the asset id and rev+1 (P2-7)'
);
ok(
  /postJson\('\/task\/asset\/update', payload\)/.test(roiSaveFn) &&
    /if \(it\.kind === 'region'\) payload\.keep_action_shape = true;/.test(roiSaveFn),
  'OCR/region ROI saves patch /task/asset/update (a region keeps its action_shape)'
);
ok(
  /assetRoiPolygonPlan\(it\.polygon, valid, it\.roi\)/.test(roiSaveFn) &&
    /polygonPlan\.fallback/.test(roiSaveFn),
  'the template overwrite plans a translated polygon and reports a rectangle fallback (leftover 2)'
);
ok(
  /assetRoiBoxValid\(box\)/.test(roiSaveFn) &&
    /assetRoiWithinFrame\(valid, canvas\.width, canvas\.height\)/.test(roiSaveFn),
  'the canvas box is validated (w/h > 0 and inside the frame) before any request'
);
ok(
  /findAssetById\(editing\.scopeTask, editingId/.test(roiSaveFn),
  'the save re-reads the latest asset by scope+id instead of replaying the entry snapshot (P1-2)'
);
ok(
  /editingRecognitionAsset = null;/.test(roiSaveFn),
  'a successful save still clears the edit state'
);
// P0-2: every way out of the ROI editor goes through the shared cancel helper.
for (const name of [
  'pickAssetRoi', 'editAssetRecognitionRoi', 'editAssetActionRoi', 'renameAsset',
  'editAssetThreshold', 'editAssetPositions', 'deleteAsset', 'runAssetTest',
  'testAssetClick', 'testAssetDrag', 'generateAssetStep', 'testAssetCollection',
  'deleteCollection', 'selectFrame', 'deleteFrame', 'clearFrames',
  'pauseLivePreview', 'startLivePreview', 'saveAssetUnified',
]) {
  ok(
    new RegExp(`cancelRecognitionRoiEdit\\(`).test(extractFunction(name)),
    `${name} leaves the recognition-ROI editor through cancelRecognitionRoiEdit (P0-2)`
  );
}
const cancelRoiFn = extractFunction('cancelRecognitionRoiEdit');
ok(
  /editingRecognitionAsset = null/.test(cancelRoiFn) &&
    /resetSelection\(\{ clearInvert: true, clearRoi: true, clearBoxes: false/.test(cancelRoiFn) &&
    /updateAssetRoiSaveButton\(\)/.test(cancelRoiFn),
  'cancelRecognitionRoiEdit clears the edit target, the canvas ROI and the save button'
);
for (const name of ['renderAssetList', 'renderGlobalAssets']) {
  ok(
    /onEditRoi: \(it\) => editAssetRecognitionRoi\(it\)/.test(extractFunction(name)),
    `${name} opens the canvas ROI editor`
  );
}
ok(
  /onEditRoi: \(it\) => editAssetRecognitionRoi\(it, scopeTask\)/.test(extractFunction('renderFrameAssets')),
  'frame detail opens the canvas ROI editor with the row scope'
);

// Behaviour harness for the save path (template overwrite vs OCR patch).
async function installRoiSaveHarness() {
  const posts = [];
  const sb = {
    confirmText: '',
    alertText: '',
    refreshTask: false,
    refreshGlobal: false,
    errors: null,
    buttonUpdates: 0,
    canceled: null,
  };
  vm.createContext(sb);
  vm.runInContext(
    'const GLOBAL_SCOPE_TASK = ' + JSON.stringify(GLOBAL_SCOPE_LITERAL) + ';\n' +
      'let assetsGlobalMode = false;\n' +
      'let taskName = { value: "任务甲", focus() {} };\n' +
      'let editingRecognitionAsset = null;\n' +
      'let roi = null;\n' +
      'let globalAssetsCache = [];\n' +
      'let taskAssets = [];\n' +
      'const canvas = { width: 1280, height: 720 };\n' +
      'globalThis.resultEl = { textContent: "" };\n' +
      'const window = {\n' +
      '  confirm(m) { globalThis.confirmText = m; return globalThis.confirmAnswer !== false; },\n' +
      '  alert(m) { globalThis.alertText = m; },\n' +
      '};\n' +
      'globalThis.postJson = async (url, payload) => {\n' +
      '  globalThis.posts.push({ url, payload });\n' +
      '  return { ok: true };\n' +
      '};\n' +
      'globalThis.refreshTaskAssetsForScope = async () => { globalThis.refreshTask = true; };\n' +
      'globalThis.refreshGlobalAssets = async () => { globalThis.refreshGlobal = true; };\n' +
      'globalThis.log = () => {};\n' +
      'globalThis.setStatus = () => {};\n' +
      'globalThis.showErrors = (p, e) => { globalThis.errors = { p, e }; };\n' +
      'globalThis.updateAssetRoiSaveButton = () => { globalThis.buttonUpdates += 1; };\n' +
      'globalThis.cancelRecognitionRoiEdit = (reason) => {\n' +
      '  globalThis.canceled = reason;\n' +
      '  editingRecognitionAsset = null;\n' +
      '  globalThis.buttonUpdates += 1;\n' +
      '};\n' +
      [assetRowScopeTaskFn(), assetScopeTaskFn(), assetRoiBoxValidFn(),
        extractFunction('assetRoiWithinFrame'),
        extractFunction('assetRoiRectanglePolygon'),
        extractFunction('assetRoiPolygonPlan'),
        extractFunction('assetRoiPolygonForSave'),
        extractFunction('assetRoiReplacePayload'),
        extractFunction('findAssetById'),
        roiSaveFn].join('\n\n'),
    sb,
    { filename: 'app.js#asset-roi-save' }
  );
  sb.posts = posts;
  sb.run = async (asset, box, opts) => {
    const o = opts || {};
    const latest = o.latest !== undefined ? o.latest : asset;
    const globalItems = o.globalItems !== undefined ? o.globalItems : [latest];
    const taskItems = o.taskItems !== undefined ? o.taskItems : [latest];
    if (o.frameWidth != null) vm.runInContext(`canvas.width = ${Number(o.frameWidth)};`, sb);
    if (o.frameHeight != null) vm.runInContext(`canvas.height = ${Number(o.frameHeight)};`, sb);
    vm.runInContext(
      `editingRecognitionAsset = { it: ${JSON.stringify(asset)}, scopeTask: ${JSON.stringify(o.scope || '')} };` +
        `roi = ${JSON.stringify(box)};` +
        `globalAssetsCache = ${JSON.stringify(globalItems)};` +
        `taskAssets = ${JSON.stringify(taskItems)};` +
        'globalThis.canceled = null;',
      sb
    );
    const edited = await sb.saveAssetRecognitionRoi();
    const stillEditing = vm.runInContext('editingRecognitionAsset', sb);
    return { edited, stillEditing };
  };
  return sb;
}

function assetScopeTaskFn() {
  return extractFunction('assetScopeTask');
}
function assetRowScopeTaskFn() {
  return extractFunction('assetRowScopeTask');
}
function assetRoiBoxValidFn() {
  return extractFunction('assetRoiBoxValid');
}

// The save path is async, so it runs in the promise chain at the bottom.
async function checkAssetRoiSavePath() {
  const roiSave = await installRoiSaveHarness();
  const tplAsset = {
    id: '模板甲',
    kind: 'template',
    roi: [1, 2, 3, 4],
    threshold: 0.9,
    positions: [7, 8],
    action_source: 'fixed',
    action_shape: { kind: 'rect', roi: [5, 6, 7, 8] },
  };
  const saved = await roiSave.run(tplAsset, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙' });
  eq(
    roiSave.confirmText,
    '用当前帧重新截图并覆盖模板「模板甲」（rev+1，不可撤销；位置帧 2 个）？',
    'template overwrite asks the exact confirmation with the asset id (P2-7)'
  );
  eq(roiSave.posts.length, 1, 'template ROI save posts exactly once');
  eq(roiSave.posts[0].url, '/task/save_asset', 'template ROI save uses the overwrite endpoint');
  eq(
    roiSave.posts[0].payload,
    {
      task: '任务乙',
      replace: true,
      id: '模板甲',
      kind: 'template',
      roi: [10, 20, 30, 40],
      positions: [7, 8],
      action_source: 'fixed',
      threshold: 0.9,
      action_shape: { kind: 'rect', roi: [5, 6, 7, 8] },
    },
    'template overwrite payload preserves id/positions/threshold/action_shape and replaces roi'
  );
  ok(roiSave.refreshTask && roiSave.refreshGlobal, 'template ROI save refreshes both libraries');
  eq(saved.stillEditing, null, 'saving clears the recognition-edit state');
  ok(roiSave.buttonUpdates >= 1, 'saving re-disables the save button');

  // Cancelling the confirm must not POST (and must stay in edit mode).
  roiSave.posts.length = 0;
  roiSave.confirmAnswer = false;
  const cancelled = await roiSave.run(tplAsset, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙' });
  roiSave.confirmAnswer = true;
  eq(roiSave.posts.length, 0, 'cancelling the template overwrite posts nothing');
  ok(cancelled.stillEditing && cancelled.stillEditing.it.id === '模板甲', 'cancelling keeps the ROI editor open');

  // P1-2: threshold/positions were changed in the still-open settings menu after
  // the ROI editor captured its snapshot. The save must use the latest object.
  roiSave.posts.length = 0;
  const drifted = Object.assign({}, tplAsset, { threshold: 0.6, positions: [9] });
  await roiSave.run(tplAsset, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙', latest: drifted });
  eq(
    roiSave.posts[0].payload,
    {
      task: '任务乙',
      replace: true,
      id: '模板甲',
      kind: 'template',
      roi: [10, 20, 30, 40],
      positions: [9],
      action_source: 'fixed',
      threshold: 0.6,
      action_shape: { kind: 'rect', roi: [5, 6, 7, 8] },
    },
    'template ROI save uses the latest threshold/positions, never the entry snapshot (P1-2)'
  );

  // P1-1: a template polygon whose bbox equals the new ROI is preserved as-is.
  roiSave.posts.length = 0;
  const matchingPolygon = [[10, 20], [39, 20], [39, 59], [10, 59]];
  const polygonTpl = Object.assign({}, tplAsset, { roi: [10, 20, 30, 40], polygon: matchingPolygon });
  await roiSave.run(polygonTpl, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙', latest: polygonTpl });
  eq(
    roiSave.posts[0].payload.polygon,
    matchingPolygon,
    'template overwrite keeps an existing recognition polygon (P1-1)'
  );

  // P1-1: a stale polygon must NOT be dropped (that loses the mask) and must not
  // make the server derive the old ROI either. It is re-synced to the new box.
  roiSave.posts.length = 0;
  const stalePolygon = [[1, 2], [3, 2], [3, 5], [1, 5]];
  const staleTpl = Object.assign({}, tplAsset, { polygon: stalePolygon });
  await roiSave.run(staleTpl, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙', latest: staleTpl });
  eq(
    roiSave.posts[0].payload.polygon,
    [[10, 20], [39, 20], [39, 59], [10, 59]],
    'a stale template polygon is re-synced to the new ROI rectangle (not dropped, not the old bbox)'
  );
  eq(roiSave.posts[0].payload.roi, [10, 20, 30, 40], 'the new ROI still wins over the stale polygon bbox');

  // Leftover 2: MOVING the ROI must translate the stored polygon (same vertex
  // count, same shape, bbox = new ROI), not replace it with the new rectangle.
  roiSave.posts.length = 0;
  roiSave.alertText = '';
  const shapeTpl = Object.assign({}, tplAsset, {
    roi: [10, 20, 30, 40],
    polygon: [[10, 20], [39, 26], [39, 59], [10, 53]],
  });
  await roiSave.run(shapeTpl, { x: 20, y: 30, w: 30, h: 40 }, { scope: '任务乙', latest: shapeTpl });
  eq(
    roiSave.posts[0].payload.polygon,
    [[20, 30], [49, 36], [49, 69], [20, 63]],
    'moving a template ROI translates every polygon vertex (leftover 2)'
  );
  eq(roiSave.posts[0].payload.roi, [20, 30, 30, 40], 'the translated polygon bbox equals the new ROI');
  eq(roiSave.posts[0].payload.polygon.length, 4, 'the translated polygon keeps its point count');
  eq(roiSave.alertText, '', 'a successful polygon translation shows no warning');
  ok(!/形状丢失/.test(roiSave.resultEl.textContent), 'a successful translation reports no shape loss');

  // Leftover 2 fallback: a resized ROI cannot be represented by a translation,
  // so the rectangle is sent and the shape loss is explained to the user.
  roiSave.posts.length = 0;
  roiSave.alertText = '';
  await roiSave.run(shapeTpl, { x: 20, y: 30, w: 20, h: 20 }, { scope: '任务乙', latest: shapeTpl });
  eq(
    roiSave.posts[0].payload.polygon,
    [[20, 30], [39, 30], [39, 49], [20, 49]],
    'a resized template ROI falls back to the new rectangle'
  );
  ok(/多边形/.test(roiSave.alertText) && /矩形/.test(roiSave.alertText),
    'a rectangle fallback is explained before the user continues');
  ok(/形状丢失/.test(roiSave.resultEl.textContent),
    'the save result notes that the polygon shape was lost');

  // P2-6: a template without positions must never reach the server.
  roiSave.posts.length = 0;
  roiSave.alertText = '';
  const noPos = Object.assign({}, tplAsset, { positions: [] });
  const noPosRun = await roiSave.run(noPos, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙', latest: noPos });
  eq(roiSave.posts.length, 0, 'a template with no positions never POSTs');
  ok(/位置帧/.test(roiSave.alertText), 'a template with no positions explains the missing 位置帧');
  ok(noPosRun.stillEditing, 'the ROI editor stays open so the user can add positions');

  // P2-9: the box must fit the current frame.
  roiSave.posts.length = 0;
  roiSave.alertText = '';
  await roiSave.run(
    tplAsset,
    { x: 90, y: 90, w: 30, h: 30 },
    { scope: '任务乙', frameWidth: 100, frameHeight: 100 }
  );
  eq(roiSave.posts.length, 0, 'an out-of-frame box never reaches the server');
  ok(/超出当前帧 100x100/.test(roiSave.alertText), 'an out-of-frame box explains the frame bounds');

  // P0-2/P1-2: a deleted/renamed asset cancels instead of writing the old id.
  roiSave.posts.length = 0;
  await roiSave.run(tplAsset, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙', latest: null });
  eq(roiSave.posts.length, 0, 'a missing asset never POSTs');
  eq(roiSave.canceled, '素材已不存在', 'a missing asset cancels the ROI editor');
  ok(/已不存在/.test(roiSave.resultEl.textContent), 'a missing asset explains why the save was cancelled');

  // OCR path: patch the ROI directly, no confirm (latest lookup still applies).
  roiSave.posts.length = 0;
  roiSave.confirmText = '';
  await roiSave.run(
    { id: '文字乙', kind: 'ocr_line', roi: [1, 2, 3, 4], positions: [7] },
    { x: 5, y: 6, w: 7, h: 8 }
  );
  eq(roiSave.posts.length, 1, 'OCR ROI save posts exactly once');
  eq(roiSave.posts[0].url, '/task/asset/update', 'OCR ROI save patches the asset');
  eq(
    roiSave.posts[0].payload,
    { task: '任务甲', id: '文字乙', roi: [5, 6, 7, 8] },
    'OCR ROI save sends only {task,id,roi}'
  );
  eq(roiSave.confirmText, '', 'OCR ROI save never asks the template overwrite question');

  // Leftover 1: a region's 识别区 save must keep the operation polygon. The
  // payload carries keep_action_shape and the message says the action area is
  // untouched; the OCR payload above stays flag-free.
  roiSave.posts.length = 0;
  roiSave.resultEl.textContent = '';
  const regionAsset = {
    id: '区域丙',
    kind: 'region',
    roi: [1, 2, 3, 4],
    action_source: 'fixed',
    action_shape: { kind: 'polygon', points: [[1, 2], [3, 2], [3, 5], [1, 5]], invert: false },
  };
  await roiSave.run(regionAsset, { x: 5, y: 6, w: 7, h: 8 });
  eq(roiSave.posts.length, 1, 'region ROI save posts exactly once');
  eq(roiSave.posts[0].url, '/task/asset/update', 'region ROI save patches the asset');
  eq(
    roiSave.posts[0].payload,
    { task: '任务甲', id: '区域丙', roi: [5, 6, 7, 8], keep_action_shape: true },
    'region ROI save sends keep_action_shape so the operation polygon survives (leftover 1)'
  );
  ok(/操作区 action_shape 保持不变/.test(roiSave.resultEl.textContent),
    'region ROI save tells the user the operation area was not changed');

  // Invalid canvas box (w/h <= 0) must be rejected before any request.
  roiSave.posts.length = 0;
  await roiSave.run({ id: '文字乙', kind: 'ocr_line', roi: [1, 2, 3, 4] }, { x: 5, y: 6, w: 0, h: 8 });
  eq(roiSave.posts.length, 0, 'an empty canvas box never reaches the server');
  ok(/识别区无效/.test(roiSave.alertText), 'an empty canvas box explains what to do');
}

// P1-3 behaviour: the batch handler must POST the visible library rows and the
// matching API scope (global store vs task bundle), and render a surface-specific
// message when the visible set is empty.
async function checkBatchTestScope() {
  const calls = [];
  const sb = {
    assetsGlobalMode: true,
    globalAssetsCache: [
      { id: '甲', kind: 'template' },
      { id: '乙', kind: 'ocr_line' },
      { id: '集合', kind: 'template_set', members: ['甲'] },
    ],
    taskAssets: [{ id: '任务子集', kind: 'template' }],
    globalAssetSearch: { value: '' },
    globalAssetCategoryFilter: { value: '' },
    assetSearch: { value: '' },
    assetCategoryFilter: { value: '' },
    taskName: { value: '任务甲', focus() {} },
    livePreview: false,
    resultEl: { textContent: '' },
    postJson: async (url, payload) => {
      calls.push({ url, payload });
      return { ok: true, results: [], passed: 0, total: 0 };
    },
    log: () => {},
  };
  vm.createContext(sb);
  vm.runInContext(
    'const GLOBAL_SCOPE_TASK = ' + JSON.stringify(GLOBAL_SCOPE_LITERAL) + ';\n' +
    [assetScopeTaskFn(), extractFunction('assetCategory'), extractFunction('assetMatchesFilter'),
      extractFunction('filterAssets'), extractFunction('batchTestAssetSelection'),
      extractFunction('batchTestEmptyText'), extractFunction('batchTestAssets')].join('\n\n'),
    sb,
    { filename: 'app.js#batch-test' }
  );

  await sb.batchTestAssets();
  eq(calls.length, 1, 'global batch test posts once');
  eq(calls[0].url, '/task/assets/batch_test', 'global batch test uses /task/assets/batch_test');
  eq(calls[0].payload.task, GLOBAL_SCOPE_LITERAL, 'global batch test uses the unified-global API scope');
  eq(calls[0].payload.assets, ['甲', '乙'], 'global batch test posts the visible global ids (not the task subset)');
  eq(calls[0].payload.live, false, 'batch test forwards the live flag');

  sb.globalAssetSearch.value = '乙';
  await sb.batchTestAssets();
  eq(calls[1].payload.assets, ['乙'], 'global batch test respects the visible id-search filter');

  sb.globalAssetSearch.value = '';
  sb.globalAssetCategoryFilter.value = '不存在的分类';
  await sb.batchTestAssets();
  eq(calls.length, 2, 'an empty visible global set never posts');
  ok(/全局素材库/.test(sb.resultEl.textContent), 'the empty global set message names the global library');

  sb.globalAssetCategoryFilter.value = '';
  sb.taskName.value = '';
  calls.length = 0;
  await sb.batchTestAssets();
  eq(calls.length, 1, 'global batch test does not require a task name');
  eq(calls[0].payload.task, GLOBAL_SCOPE_LITERAL, 'global batch test keeps the global scope without a task');
  eq(calls[0].payload.assets, ['甲', '乙'], 'global batch test still posts the visible global ids');

  sb.taskName.value = '任务甲';
  sb.assetsGlobalMode = false;
  await sb.batchTestAssets();
  eq(calls[1].payload.task, '任务甲', 'task-mode batch test uses the selected task name');
  eq(calls[1].payload.assets, ['任务子集'], 'task-mode batch test posts the visible task ids');

  sb.taskName.value = '';
  await sb.batchTestAssets();
  eq(calls.length, 2, 'task mode without a task posts nothing');
  ok(/请先填任务名/.test(sb.resultEl.textContent), 'task mode without a task still asks for one');
}

// --- E: threshold prompts for template AND OCR, never for region ---
async function checkAssetThresholdPrompts() {
  const posts = [];
  const sb = {
    promptAnswer: null,
    promptText: '',
    promptDefault: '',
    alertText: '',
  };
  vm.createContext(sb);
  vm.runInContext(
    'const GLOBAL_SCOPE_TASK = ' + JSON.stringify(GLOBAL_SCOPE_LITERAL) + ';\n' +
      'let assetsGlobalMode = false;\n' +
      'let taskName = { value: "任务甲", focus() {} };\n' +
      'globalThis.resultEl = { textContent: "" };\n' +
      'const window = {\n' +
      '  prompt(m, def) { globalThis.promptText = m; globalThis.promptDefault = def; return globalThis.promptAnswer; },\n' +
      '  alert(m) { globalThis.alertText = m; },\n' +
      '};\n' +
      'globalThis.postJson = async (url, payload) => {\n' +
      '  globalThis.posts.push({ url, payload });\n' +
      '  return { ok: true };\n' +
      '};\n' +
      'globalThis.refreshTaskAssetsForScope = async () => {};\n' +
      'globalThis.refreshGlobalAssets = async () => {};\n' +
      'globalThis.log = () => {};\n' +
      'globalThis.setStatus = () => {};\n' +
      'globalThis.showErrors = () => {};\n' +
      'globalThis.cancelRecognitionRoiEdit = () => {};\n' +
      [assetScopeTaskFn(), assetRowScopeTaskFn(), extractFunction('parseAssetThresholdText'),
        extractFunction('assetThresholdKind'), extractFunction('editAssetThreshold')].join('\n\n'),
    sb,
    { filename: 'app.js#asset-threshold' }
  );
  sb.posts = posts;

  sb.promptAnswer = '0.75';
  await sb.editAssetThreshold({ id: '模板甲', kind: 'template', threshold: 0.9 });
  ok(/模板阈值 0~1/.test(sb.promptText), 'the template threshold prompt says 模板阈值');
  eq(sb.promptDefault, '0.9', 'the template prompt starts from the stored threshold');
  eq(sb.posts[0], { url: '/task/asset/update', payload: { task: '任务甲', id: '模板甲', threshold: 0.75 } },
    'the template threshold POSTs {task,id,threshold}');

  // P2-4: an unset template has no "unlimited" mode; the effective default is
  // 0.85 (oas-task DEFAULT_THRESHOLD), and the prompt must say so.
  sb.posts.length = 0;
  sb.promptAnswer = '0.85';
  await sb.editAssetThreshold({ id: '模板丁', kind: 'template' });
  ok(/0\.85/.test(sb.promptText), 'an unset template prompt shows the effective default 0.85');
  ok(/未保存，按默认 0\.85/.test(sb.promptText), 'an unset template prompt explains the default is not "unlimited"');
  eq(sb.promptDefault, '0.85', 'an unset template prompt pre-fills 0.85');

  sb.posts.length = 0;
  sb.promptAnswer = '0.6';
  await sb.editAssetThreshold({ id: '文字乙', kind: 'ocr_line', roi: [1, 2, 3, 4] });
  ok(/OCR 最低置信度 0~1/.test(sb.promptText), 'the OCR prompt says OCR 最低置信度');
  ok(/0 = 不限制/.test(sb.promptText), 'the OCR prompt documents the 0/unset unlimited semantics (P1-4)');
  eq(sb.promptDefault, '', 'an unset OCR threshold starts empty');
  eq(sb.posts[0], { url: '/task/asset/update', payload: { task: '任务甲', id: '文字乙', threshold: 0.6 } },
    'the OCR confidence POSTs {task,id,threshold}');

  // P1-4: the runtime normalizes an explicit 0 to "no filter"; the UI must say
  // so and let the user clear the OCR threshold by writing 0.
  sb.posts.length = 0;
  sb.promptAnswer = '0';
  await sb.editAssetThreshold({ id: '文字乙', kind: 'ocr_line', roi: [1, 2, 3, 4], threshold: 0 });
  ok(/0（不限制）/.test(sb.promptText), 'a stored OCR 0 reads back as unlimited');
  eq(sb.promptDefault, '0', 'a stored OCR 0 keeps its literal value in the prompt');
  eq(sb.posts[0].payload.threshold, 0, 'writing 0 is the documented way to clear the OCR threshold');

  sb.posts.length = 0;
  sb.promptText = '';
  await sb.editAssetThreshold({ id: '区域丙', kind: 'region', roi: [1, 2, 3, 4] });
  eq(sb.promptText, '', 'region assets never open a threshold prompt');
  ok(/不支持阈值/.test(sb.resultEl.textContent), 'region assets explain that they have no threshold');
  eq(sb.posts.length, 0, 'region assets never POST a threshold');

  sb.posts.length = 0;
  sb.promptAnswer = '1.5';
  await sb.editAssetThreshold({ id: '文字乙', kind: 'ocr_line', roi: [1, 2, 3, 4] });
  eq(sb.posts.length, 0, 'an out-of-range OCR threshold is never sent');
  ok(/阈值必须是 0~1/.test(sb.alertText), 'an out-of-range threshold is reported to the user');
}


// The DOM button helper: hidden+disabled until an edit starts.
const roiBtn = { hidden: true, disabled: true, title: '' };
const roiBtnLabel = { hidden: true, textContent: '' };
const roiBtnSb = {
  btnSaveAssetRoi: roiBtn,
  assetRoiEditLabel: roiBtnLabel,
  editingRecognitionAsset: null,
};
vm.createContext(roiBtnSb);
vm.runInContext(extractFunction('updateAssetRoiSaveButton'), roiBtnSb);
roiBtnSb.editingRecognitionAsset = { it: { id: '模板甲', kind: 'template' }, scopeTask: '任务甲' };
roiBtnSb.updateAssetRoiSaveButton();
eq([roiBtn.hidden, roiBtn.disabled], [false, false], 'an active ROI edit shows and enables the save button');
ok(/模板/.test(roiBtn.title) && /rev\+1/.test(roiBtn.title), 'the button title explains the template overwrite');
eq([roiBtnLabel.hidden, roiBtnLabel.textContent], [false, '识别区编辑中：模板甲'],
  'the footer label names the asset being edited (P0-1)');
roiBtnSb.editingRecognitionAsset = null;
roiBtnSb.updateAssetRoiSaveButton();
eq([roiBtn.hidden, roiBtn.disabled], [true, true], 'leaving ROI edit hides and disables the save button');
eq([roiBtnLabel.hidden, roiBtnLabel.textContent], [true, ''], 'leaving ROI edit hides the asset label');

// Guard: no ROI editor for template_set / assets without roi.
const roiEditSb = {
  resultEl: { textContent: '' },
  window: {},
  livePreview: false,
  resetSelection: () => { throw new Error('template_set must not start a canvas edit'); },
  pickAssetRoi: () => {},
  cancelRecognitionRoiEdit: () => {},
  updateAssetRoiSaveButton: () => {},
  updateRoiLabel: () => {},
  draw: () => {},
  log: () => {},
  assetScopeTask: () => '任务甲',
};
vm.createContext(roiEditSb);
vm.runInContext(assetRowScopeTaskFn() + '\n' + roiEditorFn, roiEditSb);
roiEditSb.editAssetRecognitionRoi({ id: '集合丁', kind: 'template_set', members: ['a'] });
eq(roiEditSb.resultEl.textContent, '素材集合「集合丁」没有识别区域', 'template_set cannot enter ROI edit');
roiEditSb.editAssetRecognitionRoi({ id: '无区域', kind: 'ocr_line' });
eq(
  roiEditSb.resultEl.textContent,
  '素材「无区域」没有识别区域，无法在画布上编辑',
  'an asset without roi cannot enter the canvas editor'
);

// --- P0-2 behaviour: selecting another asset cancels the ROI editor ---
function installRoiCancelHarness() {
  const sb = {
    editingRecognitionAsset: null,
    roi: null,
    dragStart: null,
    dragTarget: 'roi',
    lastLinesRoi: null,
    polygon: null,
    polyClosed: false,
    resetSelectionCalls: 0,
    roiLabels: 0,
    draws: 0,
    statusText: '',
    btnSaveAssetRoi: { hidden: true, disabled: true, title: '' },
    assetRoiEditLabel: { hidden: true, textContent: '' },
    resultEl: { textContent: '' },
    loadActionShapeForEdit: () => {},
    actionShapeBBox: () => null,
  };
  sb.resetSelection = (opts) => {
    sb.resetSelectionCalls += 1;
    if (opts && opts.clearRoi) sb.roi = null;
  };
  sb.updateRoiLabel = () => { sb.roiLabels += 1; };
  sb.draw = () => { sb.draws += 1; };
  sb.setStatus = (text) => { sb.statusText = text; };
  sb.log = () => {};
  vm.createContext(sb);
  vm.runInContext(
    [extractFunction('updateAssetRoiSaveButton'), cancelRoiFn,
      extractFunction('pickAssetRoi')].join('\n\n'),
    sb,
    { filename: 'app.js#asset-roi-cancel' }
  );
  return sb;
}

const roiCancel = installRoiCancelHarness();
roiCancel.editingRecognitionAsset = { it: { id: '甲', kind: 'ocr_line' }, scopeTask: '任务甲' };
roiCancel.roi = { x: 1, y: 2, w: 3, h: 4 };
roiCancel.updateAssetRoiSaveButton();
eq(
  [roiCancel.btnSaveAssetRoi.hidden, roiCancel.btnSaveAssetRoi.disabled],
  [false, false],
  'P0-2 setup: the save button is armed for asset 甲'
);
roiCancel.pickAssetRoi({ id: '乙', kind: 'ocr_line', roi: [5, 6, 7, 8] });
eq(roiCancel.editingRecognitionAsset, null, 'selecting another asset clears the recognition-edit target');
eq(
  [roiCancel.btnSaveAssetRoi.hidden, roiCancel.btnSaveAssetRoi.disabled],
  [true, true],
  'selecting another asset disables/hides the save button (P0-2)'
);
eq(roiCancel.assetRoiEditLabel.hidden, true, 'selecting another asset hides the editing label');
eq(roiCancel.roi, { x: 5, y: 6, w: 7, h: 8 }, 'the canvas ROI then belongs to the newly selected asset');
eq(roiCancel.resetSelectionCalls, 1, 'cancelling resets the canvas selection exactly once');
eq(roiCancel.dragTarget, 'roi', 'cancelling restores the default drag target');
eq(roiCancel.lastLinesRoi, null, 'cancelling drops the stale line-ROI cache');
eq(
  roiCancel.statusText,
  '已取消素材「甲」的识别区编辑（选中其它素材）',
  'cancelling explains which asset was left and why'
);

roiCancel.editingRecognitionAsset = { it: { id: '丙', kind: 'template' }, scopeTask: '任务甲' };
roiCancel.updateAssetRoiSaveButton();
const escCancelled = roiCancel.cancelRecognitionRoiEdit('Esc');
eq(escCancelled, true, 'cancelRecognitionRoiEdit reports that it cancelled an active edit');
ok(/Esc/.test(roiCancel.statusText), 'the cancel reason is surfaced to the user');
eq(
  [roiCancel.btnSaveAssetRoi.hidden, roiCancel.btnSaveAssetRoi.disabled],
  [true, true],
  'the Esc path also disables/hides the save button'
);
eq(roiCancel.cancelRecognitionRoiEdit('再次取消'), false, 'cancelling without an active edit is a no-op');

eq(sandbox.assetRoiBoxValid([10, 20, 30, 40]), [10, 20, 30, 40], 'a valid canvas box is normalized');
eq(sandbox.assetRoiBoxValid([10, 20, 0, 40]), null, 'a zero-width canvas box is rejected');
eq(sandbox.assetRoiBoxValid([10, 20, 30]), null, 'a canvas box needs four values');
eq(sandbox.assetRoiBoxValid([-1, 20, 30, 40]), null, 'a negative canvas origin is rejected');
eq(sandbox.assetRoiBoxValid([0, 0, 100001, 40]), null, 'a canvas box above the 100000 cap is rejected');
eq(sandbox.assetRoiBoxValid([1.5, 2, 3, 4]), null, 'a fractional canvas box is rejected');
ok(!/function parseAssetRoiText\s*\(/.test(source), 'the dead prompt ROI text parser is removed');
eq(sandbox.parseAssetThresholdText('0.8'), 0.8, 'threshold text parses 0.8');
eq(sandbox.parseAssetThresholdText('0'), 0, 'threshold text accepts 0');
eq(sandbox.parseAssetThresholdText('1'), 1, 'threshold text accepts 1');
eq(sandbox.parseAssetThresholdText('1.2'), null, 'threshold text rejects values above 1');
eq(sandbox.parseAssetThresholdText('-0.1'), null, 'threshold text rejects negative values');
eq(sandbox.parseAssetThresholdText('  '), null, 'empty threshold text is cancelled, not 0');

// --- P1-1/P1-2/P2-9: template replace payload, polygon sync, latest lookup ---
eq(sandbox.assetRoiWithinFrame([10, 20, 30, 40], 1280, 720).ok, true, 'an in-frame ROI passes the frame check');
eq(sandbox.assetRoiWithinFrame([1270, 700, 30, 40], 1280, 720).ok, false, 'a ROI crossing the frame edge is rejected');
ok(
  /超出当前帧 1280x720/.test(sandbox.assetRoiWithinFrame([1270, 700, 30, 40], 1280, 720).text),
  'an out-of-frame ROI explains the frame bounds'
);
eq(sandbox.assetRoiWithinFrame([10, 20, 30, 40], 0, 0).ok, true, 'unknown frame size skips the frame-bounds check');
eq(sandbox.assetRoiWithinFrame([10, 20, 0, 40], 1280, 720).ok, false, 'an invalid box also fails the frame check');
eq(
  sandbox.assetRoiRectanglePolygon([10, 20, 30, 40]),
  [[10, 20], [39, 20], [39, 59], [10, 59]],
  'rectangle polygon uses inclusive max = origin + size - 1'
);
eq(sandbox.assetRoiPolygonForSave(null, [10, 20, 30, 40]), null, 'no stored polygon stays polygon-free');
eq(
  sandbox.assetRoiPolygonForSave([[10, 20], [39, 20], [39, 59], [10, 59]], [10, 20, 30, 40]),
  [[10, 20], [39, 20], [39, 59], [10, 59]],
  'a polygon that still bounds the new ROI is preserved'
);
eq(
  sandbox.assetRoiPolygonForSave([[1, 2], [3, 2], [3, 5], [1, 5]], [10, 20, 30, 40]),
  [[10, 20], [39, 20], [39, 59], [10, 59]],
  'a stale polygon is re-synced to the new ROI rectangle instead of being dropped'
);
// Leftover 2: the polygon plan is a pure translation when the ROI merely moves.
const movedPlan = sandbox.assetRoiPolygonPlan(
  [[10, 20], [39, 26], [39, 59], [10, 53]],
  [20, 30, 30, 40],
  [10, 20, 30, 40]
);
eq(
  movedPlan.polygon,
  [[20, 30], [49, 36], [49, 69], [20, 63]],
  'a moved ROI translates the polygon instead of replacing it'
);
eq(movedPlan.fallback, false, 'a pure ROI move keeps the polygon shape');
eq(movedPlan.reason, '', 'a pure ROI move has no fallback reason');
eq(
  sandbox.assetRoiPolygonForSave(
    [[10, 20], [39, 26], [39, 59], [10, 53]],
    [20, 30, 30, 40],
    [10, 20, 30, 40]
  ),
  [[20, 30], [49, 36], [49, 69], [20, 63]],
  'the polygon wrapper forwards the translated shape'
);
const resizePlan = sandbox.assetRoiPolygonPlan(
  [[10, 20], [39, 26], [39, 59], [10, 53]],
  [20, 30, 20, 40],
  [10, 20, 30, 40]
);
eq(
  resizePlan.polygon,
  [[20, 30], [39, 30], [39, 69], [20, 69]],
  'a resized ROI falls back to the new rectangle'
);
eq(resizePlan.fallback, true, 'a resized ROI is a fallback, not a translation');
ok(/尺寸|越界/.test(resizePlan.reason), 'the resize fallback explains why the shape was lost');
// Out-of-frame / inconsistent stored data: the translated bbox cannot be the new
// ROI, so the rectangle is the only safe payload (server derives roi from bbox).
const strayPlan = sandbox.assetRoiPolygonPlan(
  [[45, 45], [74, 45], [74, 74], [45, 74]],
  [0, 0, 30, 30],
  [50, 50, 30, 30]
);
eq(
  strayPlan.polygon,
  [[0, 0], [29, 0], [29, 29], [0, 29]],
  'a translation that would leave the frame falls back to the rectangle'
);
eq(strayPlan.fallback, true, 'an out-of-frame translation is a fallback');
ok(/越界|尺寸/.test(strayPlan.reason), 'the out-of-frame fallback explains why');
const noPolyPlan = sandbox.assetRoiPolygonPlan(null, [10, 20, 30, 40], [1, 2, 3, 4]);
eq(noPolyPlan.polygon, null, 'an asset without a polygon stays polygon-free');
eq(noPolyPlan.fallback, false, 'no polygon is not a rectangle fallback');
const payloadAsset = {
  id: '模板甲',
  kind: 'template',
  roi: [1, 2, 3, 4],
  positions: [7, 8],
  action_source: 'fixed',
  action_shape: { kind: 'rect', roi: [5, 6, 7, 8] },
  threshold: 0.9,
  polygon: [[1, 2], [3, 2], [3, 5], [1, 5]],
  ocr_mode: null,
  ocr_text: null,
};
eq(
  sandbox.assetRoiReplacePayload('任务甲', payloadAsset, [10, 20, 30, 40]),
  {
    task: '任务甲',
    replace: true,
    id: '模板甲',
    kind: 'template',
    roi: [10, 20, 30, 40],
    positions: [7, 8],
    action_source: 'fixed',
    threshold: 0.9,
    action_shape: { kind: 'rect', roi: [5, 6, 7, 8] },
    polygon: [[10, 20], [39, 20], [39, 59], [10, 59]],
  },
  'the replace payload carries every persisted field and only replaces roi (P1-1)'
);
// Leftover 2: the payload for a pure ROI move carries the TRANSLATED polygon
// (bbox = new ROI), not the new ROI rectangle.
const movedPayloadAsset = Object.assign({}, payloadAsset, {
  roi: [10, 20, 30, 40],
  polygon: [[10, 20], [39, 26], [39, 59], [10, 53]],
});
eq(
  sandbox.assetRoiReplacePayload('任务甲', movedPayloadAsset, [20, 30, 30, 40]).polygon,
  [[20, 30], [49, 36], [49, 69], [20, 63]],
  'the replace payload sends the translated polygon for a moved ROI (leftover 2)'
);

const globalRow = { id: '甲', kind: 'ocr_line', threshold: 0.9 };
const taskRow = { id: '甲', kind: 'ocr_line', threshold: 0.4 };
eq(
  sandbox.findAssetById(GLOBAL_SCOPE_LITERAL, '甲', { global: [globalRow], task: [taskRow] }),
  globalRow,
  'global scope resolves the asset from the global cache first'
);
eq(
  sandbox.findAssetById('任务乙', '甲', { global: [globalRow], task: [taskRow] }),
  taskRow,
  'task scope resolves the asset from the task cache first'
);
eq(
  sandbox.findAssetById('任务乙', '不存在', { global: [globalRow], task: [taskRow] }),
  null,
  'a missing asset id resolves to null (save must cancel)'
);
eq(
  sandbox.findAssetById('任务乙', '甲', { global: [globalRow], task: [] }),
  globalRow,
  'an unloaded scoped cache may fall back to the other cache'
);
eq(
  sandbox.findAssetById('任务乙', '甲', { global: [globalRow], task: [{ id: '乙' }] }),
  null,
  'a loaded scoped cache never cross-matches an unrelated store'
);

// --- P1-3: batch test target = the visible rows next to the button ---
const batchGlobalItems = [
  { id: '甲', kind: 'template', category: '主界面' },
  { id: '乙', kind: 'ocr_line', category: '主界面' },
  { id: '集合', kind: 'template_set', members: ['甲'], category: '素材集合' },
];
const batchGlobal = sandbox.batchTestAssetSelection({
  globalMode: true,
  globalItems: batchGlobalItems,
  taskItems: [{ id: '任务子集' }],
});
eq(batchGlobal.ids, ['甲', '乙'], 'global batch test covers the visible global rows, not the task flow subset');
eq(batchGlobal.scope, GLOBAL_SCOPE_LITERAL, 'global batch test uses the global API scope');
eq(
  sandbox.batchTestAssetSelection({ globalMode: true, globalItems: batchGlobalItems, query: '乙' }).ids,
  ['乙'],
  'global batch test respects the id search filter'
);
eq(
  sandbox.batchTestAssetSelection({ globalMode: true, globalItems: batchGlobalItems, category: '主界面' }).ids,
  ['甲', '乙'],
  'global batch test respects the category filter'
);
eq(
  sandbox.batchTestAssetSelection({ globalMode: false, taskItems: batchGlobalItems, query: '' }).ids,
  ['甲', '乙'],
  'task mode batch test covers the visible task rows (show-all already applied)'
);
ok(
  /全局素材库/.test(sandbox.batchTestEmptyText(true)),
  'the empty global batch-test message names the global library'
);
ok(
  /任务素材库/.test(sandbox.batchTestEmptyText(false)) &&
    /显示全部全局素材/.test(sandbox.batchTestEmptyText(false)),
  'the empty task batch-test message points at show-all + filters'
);

// --- MATCH-BUG: a single asset test targets the frame the user is viewing ---
// Regression: every saved asset has positions>=1, so the old "positions first"
// order made currentFrameId unreachable and tested a static screenshot (the
// template is still at its saved ROI there, so the drawn box never moved).
const { testFramePlan, matchBoxValid } = sandbox;
const liveFramePlan = testFramePlan({ positions: [23] }, {
  livePreview: true,
  currentFrameId: 41,
  frameId: 26,
});
eq(liveFramePlan.map((f) => f.query), ['&live=1'], 'live preview always tests the live frame');
eq(liveFramePlan[0].label, '实时帧', 'live frame label');
eq(liveFramePlan[0].source, 'live', 'live frame source');
eq(liveFramePlan[0].frameId, null, 'live frame carries no frame id');
eq(liveFramePlan[0].live, true, 'live frame is flagged live');

const currentFramePlan = testFramePlan({ positions: [23] }, {
  livePreview: false,
  currentFrameId: 41,
});
eq(currentFramePlan.length, 1, 'the current frame is a single request, not one per position');
eq(
  currentFramePlan[0].query,
  '&frame_id=41',
  'current frame wins over the saved position frame (pre-fix this was &frame_id=23)'
);
eq(currentFramePlan[0].label, '当前帧 #41', 'current frame label names the frame');
eq(currentFramePlan[0].source, 'current:#41', 'current frame source names the frame');
eq(currentFramePlan[0].frameId, 41, 'current frame keeps the numeric frame id');
eq(currentFramePlan[0].live, false, 'current frame is not live');

const explicitFramePlan = testFramePlan({ positions: [23] }, {
  livePreview: false,
  currentFrameId: 41,
  frameId: 26,
});
eq(explicitFramePlan[0].query, '&frame_id=26', 'an explicit frame id overrides the current frame');
eq(explicitFramePlan[0].label, '当前帧 #26', 'explicit frame label names the frame');
eq(explicitFramePlan[0].source, 'current:#26', 'explicit frame source names the frame');

const positionFramePlan = testFramePlan({ positions: [1, 2] }, {
  livePreview: false,
  currentFrameId: null,
});
eq(
  positionFramePlan.map((f) => f.query),
  ['&frame_id=1', '&frame_id=2'],
  'without a current frame every position frame is still tested'
);
eq(positionFramePlan.map((f) => f.label), ['位置帧 #1', '位置帧 #2'], 'position frames keep their old labels');
eq(positionFramePlan.map((f) => f.source), ['position:#1', 'position:#2'], 'position frames keep their old sources');

const fallbackFramePlan = testFramePlan({}, { livePreview: false, currentFrameId: null });
eq(fallbackFramePlan.length, 1, 'no current frame and no positions yields one request');
eq(fallbackFramePlan[0].query, '', 'the fallback lets the server use its current frame');
eq(fallbackFramePlan[0].label, '当前帧', 'the fallback keeps the old 当前帧 label');
eq(fallbackFramePlan[0].source, 'current', 'the fallback keeps the old current source');
eq(fallbackFramePlan[0].live, false, 'the fallback is not live');
eq(
  testFramePlan({ positions: ['9', 9, 0, -1, 'x'] }, { livePreview: false, currentFrameId: null })
    .map((f) => f.query),
  ['&frame_id=9'],
  'position ids are normalized (deduped, positive integers only)'
);
eq(
  testFramePlan({ positions: [5] }, { livePreview: false, currentFrameId: '41' }).map((f) => f.query),
  ['&frame_id=41'],
  'a numeric-string current frame id is accepted'
);
eq(
  testFramePlan({}, { livePreview: false, currentFrameId: 41, frameId: 0 }).map((f) => f.query),
  ['&frame_id=41'],
  'frame id 0 is not a valid override'
);

// --- P2-4: the full-screen OCR fallback follows the frame it was called for ---
// It used to call testFrameQuery(it)/testFrameLabel(it) without the frame id, so
// a frame-detail row could fail on its own frame and then retry /lines on the
// editor's current frame. The fetch URL is produced synchronously before the
// first await, so a never-settling fetch captures it without any async test.
function installFallbackFrameHarness() {
  sandbox.detReady = true;
  sandbox.canvas = { width: 1280, height: 720 };
  sandbox.img = null;
  sandbox.roi = null;
  sandbox.resultEl = fakeElement();
  sandbox.logs = [];
  sandbox.log = (msg, level, data) => sandbox.logs.push({ msg, level, data });
  sandbox.renderOcrLines = () => {};
  sandbox.livePreview = false;
  sandbox.currentFrameId = 41;
  const urls = [];
  sandbox.fetch = (url) => {
    urls.push(url);
    return new Promise(() => {});
  };
  for (const fn of ['testFrames', 'testFrameQuery', 'testFrameLabel', 'fallbackFullscreenLines']) {
    vm.runInContext(extractFunction(fn), sandbox, { filename: `app.js#${fn}` });
  }
  return { urls };
}
const fallbackHarness = installFallbackFrameHarness();
void sandbox.fallbackFullscreenLines('ROI 内没有文字', { id: 'a', positions: [23] }, 26);
ok(
  fallbackHarness.urls[0] && fallbackHarness.urls[0].includes('&frame_id=26'),
  'the full-screen fallback asks for the explicitly tested frame, not the current one'
);
eq(
  sandbox.logs[0].data.frames,
  '当前帧 #26',
  'the full-screen fallback logs the same frame it requests'
);
void sandbox.fallbackFullscreenLines('ROI 内没有文字', { id: 'a' }, undefined);
ok(
  fallbackHarness.urls[1] && fallbackHarness.urls[1].includes('&frame_id=41'),
  'without an explicit frame the fallback still resolves the current frame'
);

// A /task/asset/match miss is x=y=-1 while w/h still equal the template size.
ok(matchBoxValid([10, 20, 43, 39]), 'a real match box is valid');
ok(matchBoxValid([0, 0, 1, 1]), 'a match at the origin is valid');
ok(!matchBoxValid([-1, -1, 43, 39]), 'a miss at (-1,-1) is not drawn even though w/h stay positive');
ok(!matchBoxValid([10, 20, 0, 39]), 'zero width is invalid');
ok(!matchBoxValid([10, 20, 43, 0]), 'zero height is invalid');
ok(!matchBoxValid(null), 'a missing match box is invalid');
ok(!matchBoxValid(['x', 1, 2, 3]), 'a non-numeric origin is invalid');

// --- P1-8: one selection mode at a time ---
eq(selectionModeFromFlags(false, false, false), 'none', 'no flags -> no selection mode');
eq(selectionModeFromFlags(true, false, false), 'recognitionPolygon', 'recognition polygon mode');
eq(selectionModeFromFlags(false, true, false), 'actionPolygon', 'action polygon mode');
eq(selectionModeFromFlags(false, false, true), 'actionRect', 'action rect mode');
eq(selectionModeFromFlags(true, true, true), 'actionPolygon', 'action polygon wins the single dispatch order');
ok(
  (source.match(/\bfunction setSelectionMode\s*\(/g) || []).length === 1,
  'setSelectionMode is the single mode-entry helper'
);
for (const flag of ['polyMode', 'actionPolyMode', 'backMode']) {
  // declaration + assignment inside setSelectionMode only; any other direct
  // assignment re-opens the old cross-mode state bug.
  const assignments = source.match(new RegExp(`\\b${flag}\\s*=`, 'g')) || [];
  eq(assignments.length, 2, `${flag} is only assigned in its declaration and setSelectionMode`);
}
const resetSelectionFn = extractFunction('resetSelection');
const resetActionFn = extractFunction('resetActionGeometry');
ok(/setSelectionMode\('none'\)/.test(resetSelectionFn), 'resetSelection clears every mode through setSelectionMode');
ok(
  /setSelectionMode\(selectionModeFromFlags\(/.test(resetActionFn),
  'resetActionGeometry drops action modes while preserving the recognition mode flag'
);
ok(
  /btnClear\.addEventListener\('click',[\s\S]{0,600}resetSelection\(/.test(source),
  'the clear button uses the unified resetSelection path'
);
const replaceResultsFn = extractFunction('replaceResults');
ok(
  /resetActionGeometry\(\{[^}]*clearInvert: true/.test(replaceResultsFn) &&
    /resetActionGeometry\(\{[^}]*clearRect: true/.test(replaceResultsFn),
  'new recognition results reset action polygon/invert/rect'
);

// --- static UI contract checks (P0-1 / P0-3 / dead bindings) ---
ok(/id="btn-library-refresh"/.test(html), 'index.html provides a real #btn-library-refresh');
const assetsPanelHtml = html.slice(html.indexOf('id="panel-assets"'), html.indexOf('id="panel-images"'));
const taskPanelHtml = html.slice(
  html.indexOf('id="panel-task"'),
  html.indexOf('</aside>', html.indexOf('id="panel-task"'))
);
// P0-1: the canvas 识别区 save button + editing label live in the shared
// #canvas-footer (outside every .rail-panels .panel), so the task library and
// frame-detail surfaces can reach them without switching to the 素材 panel.
ok(
  /id="btn-save-asset-roi"[^>]*type="button"[^>]*disabled hidden/.test(html),
  'index.html provides a disabled+hidden #btn-save-asset-roi'
);
const footerStart = html.indexOf('<div id="canvas-footer">');
const saveBtnPos = html.indexOf('id="btn-save-asset-roi"');
const footerEnd = saveBtnPos > footerStart ? html.indexOf('</div>', saveBtnPos) : -1;
const footerHtml = footerStart >= 0 && footerEnd > footerStart
  ? html.slice(footerStart, footerEnd)
  : '';
ok(/id="btn-save-asset-roi"/.test(footerHtml), '#btn-save-asset-roi lives inside #canvas-footer (P0-1)');
ok(
  /id="asset-roi-edit-label"/.test(footerHtml),
  'the "editing <asset>" label sits next to the save button in #canvas-footer'
);
ok(
  footerStart > 0 && footerStart < html.indexOf('<aside'),
  '#canvas-footer is outside the sidebar panels (visible from every asset surface)'
);
ok(
  !/id="btn-save-asset-roi"/.test(assetsPanelHtml) && !/id="btn-save-asset-roi"/.test(taskPanelHtml),
  'the ROI save button is no longer confined to the 素材 / 任务 panels (P0-1)'
);
ok(/保存识别区（覆盖素材）/.test(html), 'the ROI save button carries the agreed label');
ok(
  /#btn-save-asset-roi\[hidden\] \{ display: none; \}/.test(styleCss),
  'the hidden ROI save button is really hidden despite button display rules'
);
// D: 批量帧测试 moved from the task panel to the assets panel, next to 刷新全局库.
ok(
  /id="btn-assets-batch-test"/.test(assetsPanelHtml),
  '批量帧测试 lives in the 素材 panel'
);
ok(
  !/id="btn-assets-batch-test"/.test(taskPanelHtml),
  '批量帧测试 is gone from the 任务 panel'
);
ok(
  /id="btn-library-refresh"[^>]*>[^<]*<\/button>\s*<button id="btn-assets-batch-test"/.test(assetsPanelHtml),
  '批量帧测试 sits on the same toolbar row, right after 刷新全局库'
);
// P1-3: the handler tests the visible rows it sits next to, not the task subset.
const batchTestAssetsFn = extractFunction('batchTestAssets');
ok(
  /assetsGlobalMode/.test(batchTestAssetsFn) &&
    /batchTestAssetSelection\(\{/.test(batchTestAssetsFn),
  'batchTestAssets picks the visible library rows through batchTestAssetSelection (P1-3)'
);
ok(
  /GLOBAL_SCOPE_TASK/.test(batchTestAssetsFn),
  'global-mode batch test uses the unified-global API scope, not the flow subset'
);
ok(
  /const body = \{ task, assets: selection\.ids, live: !!livePreview \};/.test(batchTestAssetsFn),
  'batch test always sends the explicit visible id list to /task/assets/batch_test'
);
ok(
  /batchTestEmptyText\(globalMode\)/.test(batchTestAssetsFn),
  'an empty batch test uses the surface-specific empty message'
);
ok(
  !/当前任务 flow 未引用素材/.test(batchTestAssetsFn),
  'the stale "flow 未引用素材" message for the global-library button is gone'
);
ok(/id="task-show-all-assets"/.test(html), 'index.html provides the show-all-global switch');
ok(/id="task-assets-hint"/.test(html), 'index.html provides the unreferenced hint');
ok(/id="global-asset-search"/.test(html), 'index.html provides the global asset id search');
ok(
  /id="global-asset-category-filter"/.test(html) && /<option value="">全部分类<\/option>/.test(html),
  'index.html provides the global category filter with 全部分类'
);
ok(/id="global-assets-list"/.test(html), 'index.html provides the global asset list');
// Frame search + frame-detail asset list are real DOM nodes (static audit keeps
// getElementById references honest).
ok(/id="frame-search"/.test(html), 'index.html provides the frame search input');
ok(/id="frame-search-clear"/.test(html), 'index.html provides the frame search clear button');
ok(/id="frame-search-status"/.test(html), 'index.html provides the frame search status line');
ok(/id="frame-assets-status"/.test(html), 'index.html provides the frame asset count line');
ok(/id="frame-assets-list"/.test(html), 'index.html provides the frame asset list');
// LAYOUT-REGRESSION: the frame detail/assets block must be collapsible so the
// category-grouped frame list keeps its height (it collapsed to 0px at 1440x900
// when these nodes were rendered inline).
ok(/id="frame-details"/.test(html), 'index.html provides the frame-details container');
ok(/<details id="frame-details"/.test(html), 'frame detail/assets live in a <details> element');
ok(/<summary>帧详情 \/ 该帧素材/.test(html), 'the frame-details summary documents what it contains');
ok(
  /placeholder="搜索帧：名称 \/ #id \/ 分类"/.test(html),
  'the frame search placeholder documents name/#id/category'
);
ok(/该帧素材：\$\{ids/.test(source), 'frame detail text reports 该帧素材 N 条');
// DRY-3: all three asset-library surfaces build their rows through one helper.
const renderGlobalFn = extractFunction('renderGlobalAssets');
const renderTaskFn = extractFunction('renderAssetList');
const renderFrameAssetsFn = extractFunction('renderFrameAssets');
const renderAssetRowsFn = extractFunction('renderAssetRows');
ok(/renderAssetRows\(/.test(renderGlobalFn), 'global library renders through renderAssetRows');
ok(/renderAssetRows\(/.test(renderTaskFn), 'task library renders through renderAssetRows');
ok(/assetRowElement\(/.test(renderAssetRowsFn), 'renderAssetRows builds rows through assetRowElement');
ok(/assetRowElement\(/.test(renderFrameAssetsFn), 'frame detail builds rows through assetRowElement');
ok(
  (source.match(/(?:^|\n)function assetRowElement\s*\(/g) || []).length === 1,
  'assetRowElement is declared exactly once'
);
const refreshFramesFn = extractFunction('refreshFrames');
ok(/assetsForFrame\(/.test(refreshFramesFn), 'refreshFrames combines global + task records for the current frame');
ok(/renderFrameAssets\(/.test(refreshFramesFn), 'refreshFrames paints the frame asset list');
ok(/renderFramesList\(/.test(refreshFramesFn), 'refreshFrames paints the filtered frame list');
// P1-1: every global-library change must repaint the frame surfaces, from the
// caches only (a second /frames request would be redundant and `refreshFrames`
// is already called by the task-panel refresh that precedes this one).
const refreshGlobalFn = extractFunction('refreshGlobalAssets');
ok(
  // Match a real statement line, not a comment: a commented-out or merely
  // mentioned call must not satisfy this contract.
  /^[ \t]*renderFrameAssetViews\(\);[ \t]*$/m.test(refreshGlobalFn),
  'refreshGlobalAssets repaints the frame rows/detail after a global-library change'
);
ok(
  /^[ \t]*await refreshGlobalAssets\(\);[ \t]*$/m.test(extractFunction('saveAssetOrder')),
  'saving the asset order refreshes the shared global cache too'
);
const renderFrameAssetViewsFn = extractFunction('renderFrameAssetViews');
ok(
  /assetsForFrame\(globalAssetsCache, taskAssets, f\.id\)/.test(renderFrameAssetViewsFn) &&
    /renderFrameDetail\(f, rows\.map/.test(renderFrameAssetViewsFn) &&
    /renderFrameAssets\(f, rows\)/.test(renderFrameAssetViewsFn),
  'renderFrameAssetViews recomputes the current frame from the refreshed caches'
);
ok(
  !/refreshFrames\(/.test(renderFrameAssetViewsFn),
  'renderFrameAssetViews stays cache-only (no duplicate GET /frames)'
);
ok(
  !/refreshGlobalAssets\(/.test(renderFrameAssetViewsFn) && !/loadGlobalAssets\(/.test(renderFrameAssetViewsFn),
  'renderFrameAssetViews cannot recurse into the global-library refresh'
);
ok(
  /frameSearch\.addEventListener\('input', \(\) => renderFramesList\(\)\)/.test(source),
  'frame search input re-renders the cached list without fetching /frames'
);
ok(!/saveSelectedCandidates/.test(source), 'dead saveSelectedCandidates is gone');
ok(!/btn-batch-save/.test(source), 'dead #btn-batch-save binding is gone');
ok(!/action_roi/.test(source), 'no stale action_roi text remains in app.js');

// --- MATCH-BUG static contracts: frame-id plumbing + no stale result boxes ---
const testFramesFn = extractFunction('testFrames');
ok(
  /testFramePlan\(it, \{ livePreview, currentFrameId, frameId \}\)/.test(testFramesFn),
  'testFrames is a thin wrapper over the pure testFramePlan (globals passed explicitly)'
);
const runAssetTestFn = extractFunction('runAssetTest');
ok(
  /testAssetMatch\(it, frameId, scopeTask\)/.test(runAssetTestFn),
  'runAssetTest passes the optional frame id + row scope to the template test'
);
ok(
  /testAssetOcr\(it, frameId, scopeTask\)/.test(runAssetTestFn),
  'runAssetTest passes the optional frame id + row scope to the OCR test'
);
for (const fn of ['testAssetMatch', 'testAssetClick', 'testAssetDrag', 'testAssetOcr', 'testAssetCollection']) {
  const body = extractFunction(fn);
  ok(
    new RegExp(`function ${fn}\\(it, frameId, scopeTask\\)`).test(body),
    `${fn} accepts an optional frame id + scope override`
  );
  ok(/testFrames\(it, frameId\)/.test(body), `${fn} resolves frames through testFrames(it, frameId)`);
  ok(/assetRowScopeTask\(scopeTask/.test(body), `${fn} resolves the API scope through assetRowScopeTask`);
}
ok(
  /function testFrameLabel\(it, frameId\)/.test(extractFunction('testFrameLabel')),
  'testFrameLabel can name an explicitly requested frame'
);
for (const fn of ['testAssetClick', 'testAssetDrag', 'testAssetOcr']) {
  ok(
    /testFrameLabel\(it, frameId\)/.test(extractFunction(fn)),
    `${fn} reports the frame it actually tested`
  );
}
ok(
  /onTest: \(it\) => runAssetTest\(it, f\.id, scopeTask\)/.test(renderFrameAssetsFn),
  'frame detail 测试 forces the rendered frame id + row scope'
);
ok(
  /onClickTest: \(it\) => testAssetClick\(it, f\.id, scopeTask\)/.test(renderFrameAssetsFn) &&
    /onDragTest: \(it\) => testAssetDrag\(it, f\.id, scopeTask\)/.test(renderFrameAssetsFn) &&
    /onCollectionTest: \(it\) => testAssetCollection\(it, f\.id, scopeTask\)/.test(renderFrameAssetsFn),
  'frame detail click/drag/collection tests force the rendered frame id + row scope too'
);
// C static contract: frame rows are manageable and carry no source label.
ok(/manage: true/.test(renderFrameAssetsFn), 'renderFrameAssets passes manage:true (same as the global library)');
ok(!/sourceLabel/.test(renderFrameAssetsFn), 'renderFrameAssets passes no sourceLabel (来源列 deleted)');
ok(
  /frameAssetScope\(row, taskName/.test(renderFrameAssetsFn),
  'renderFrameAssets derives each row API scope through frameAssetScope'
);
ok(
  ['onRename', 'onThreshold', 'onPosition', 'onDelete', 'onAction'].every((k) =>
    new RegExp(`${k}: \\(it\\) => \\w+\\(it, scopeTask\\)`).test(renderFrameAssetsFn)),
  'every frame-detail management callback forwards the row scope'
);
const selectFrameFn = extractFunction('selectFrame');
ok(
  /boxes = \[\];/.test(selectFrameFn) && /selectedBox = null;/.test(selectFrameFn),
  'switching frames drops result boxes measured on the previous image'
);
// LAYOUT-REGRESSION: clicking a frame reveals the collapsed detail/asset section.
ok(/frameDetails\.open = true/.test(selectFrameFn), 'selectFrame opens the frame detail/assets section');
ok(
  /frameSearchStatus\.style\.display/.test(extractFunction('renderFramesList')),
  'the empty frame-search status line is hidden so the frame list keeps its height'
);
// P2-2: deleting the CURRENT frame falls back to another one; the old boxes must
// not be repainted over it (same illusion the selectFrame clearing fixes).
const deleteFrameFn = extractFunction('deleteFrame');
ok(
  /const wasCurrent = currentFrameId != null/.test(deleteFrameFn) &&
    /if \(wasCurrent\) \{\s*boxes = \[\];\s*selectedBox = null;/.test(deleteFrameFn),
  'deleting the current frame drops result boxes measured on the removed image'
);
// P2-4: the single-frame full-screen fallback must stay on the frame it was
// called for instead of silently reading testFrameQuery(it) on the current frame.
const fallbackFullscreenFn = extractFunction('fallbackFullscreenLines');
ok(
  /function fallbackFullscreenLines\(reason, it, frameId\)/.test(fallbackFullscreenFn) &&
    /testFrameQuery\(it, frameId\)/.test(fallbackFullscreenFn) &&
    /testFrameLabel\(it, frameId\)/.test(fallbackFullscreenFn),
  'the full-screen OCR fallback carries the tested frame id through'
);
ok(
  /fallbackFullscreenLines\(probe\.error, it, frame\.frameId\)/.test(extractFunction('testAssetOcr')),
  'the OCR probe hands its own frame id to the full-screen fallback'
);
ok(
  /matchBoxValid\(recognizedBox\)/.test(extractFunction('testAssetMatch')) &&
    /matchBoxValid\(recognizedBox\)/.test(extractFunction('testAssetClick')) &&
    /matchBoxValid\(recognizedBox\)/.test(extractFunction('testAssetDrag')),
  'template tests reject a miss (-1,-1) through matchBoxValid before drawing'
);
const probeOcrFrameFn = extractFunction('probeAssetOcrFrame');
ok(/recognizedBox: null/.test(probeOcrFrameFn), 'single-line OCR reports no independent detection box');
ok(
  /roiBox: \[rec\[0\], rec\[1\], rec\[2\], rec\[3\]\]/.test(probeOcrFrameFn),
  'single-line OCR keeps the stored ROI as roiBox only'
);
ok(
  !/recognizedBox: \[rec\[0\]/.test(probeOcrFrameFn),
  'the stored ROI is no longer labelled a recognized box'
);
ok(
  /识别区域（存储 ROI，无独立检测框）/.test(extractFunction('testAssetClick')),
  'the click test names the stored ROI a recognition area, not a detection box'
);

// Boot regression (E2E O1): #task-select must be populated BEFORE the task
// panels load. The old boot refreshed assets while the select still had only
// the empty placeholder, then refreshTaskList() auto-selected tasks[0]; the
// asset panel stayed on "未选择任务" until the user switched tasks manually.
ok(
  /await refreshTaskList\(taskName\.value\.trim\(\)\);\s*let bootTask = [\s\S]{0,500}await refreshTaskAssets\(bootTask \|\| undefined\);/.test(source),
  'boot resolves #task-select before loading the task asset panel'
);
ok(
  /if \(!bootTask && deepTask\) bootTask = deepTask;/.test(source),
  'boot keeps a ?task= deep link usable when /task/list has no matching option'
);

const saveUnified = extractFunction('saveAssetUnified');
ok(
  /postJson\('\/task\/save_assets'/.test(saveUnified) && /postJson\('\/task\/save_asset'/.test(saveUnified),
  'both save endpoints are still used (batch OCR + single)'
);

// End-to-end wiring of saveAssetUnified (still no server): stub the network and
// UI effects, then assert the exact JSON bodies sent to the two endpoints.
async function checkSavePathWiring(saveSource) {
  vm.runInContext(saveSource, sandbox);
  const calls = [];
  sandbox.assetsGlobalMode = true;
  sandbox.GLOBAL_SCOPE_TASK = '（全局素材库）';
  sandbox.taskName = { value: '' };
  sandbox.resultEl = { textContent: '' };
  sandbox.pendingAssetPositions = [5];
  sandbox.candidateNameValid = () => true;
  sandbox.selectedAssetPositions = () => [5];
  sandbox.currentRecognitionPolygon = () => recognitionPoly;
  sandbox.postJson = async (url, payload) => {
    calls.push({ url, payload });
    return { ok: true, saved: payload.assets ? payload.assets.map((a) => ({ id: a.id })) : [{ id: payload.id }] };
  };
  sandbox.refreshTaskAssetsForScope = async () => {};
  sandbox.refreshGlobalAssets = async () => {};
  sandbox.refreshAssetPositionFrames = async () => {};
  sandbox.renderPendingPositions = () => {};
  sandbox.replaceResults = () => {};
  sandbox.log = () => {};
  sandbox.setStatus = () => {};
  sandbox.cancelRecognitionRoiEdit = () => {};

  sandbox.saveCandidates = [{
    selected: true,
    kind: 'ocr_line',
    name: '按钮甲',
    text: '开始',
    recognize: { x: 2, y: 2, w: 4, h: 4 },
    fromLines: true,
    sourceRoi: { x: 1, y: 1, w: 8, h: 8 },
    actionSource: 'recognition',
    actionShape: null,
  }];
  await sandbox.saveAssetUnified();
  eq(calls.length, 1, 'OCR save posts exactly once');
  eq(calls[0].url, '/task/save_assets', 'OCR rows use the batch endpoint');
  eq(calls[0].payload.assets[0].polygon, recognitionPoly, 'batch OCR save payload carries polygon');
  eq(calls[0].payload.assets[0].ocr_mode, 'lines', 'batch OCR save payload keeps ocr_mode');
  eq(calls[0].payload.assets[0].ocr_text, '开始', 'batch OCR save payload keeps ocr_text');
  eq(calls[0].payload.assets[0].action_source, 'recognition', 'batch OCR keeps recognition semantics');

  sandbox.saveCandidates = [{
    selected: true,
    kind: 'template',
    name: '模板甲',
    recognize: { x: 1, y: 2, w: 3, h: 4 },
    actionSource: 'recognition',
    actionShape: null,
  }];
  await sandbox.saveAssetUnified();
  eq(calls.length, 2, 'single save posts exactly once');
  eq(calls[1].url, '/task/save_asset', 'non-OCR rows use the single endpoint');
  eq(calls[1].payload.polygon, recognitionPoly, 'single save payload carries polygon');
  eq(calls[1].payload.threshold, 0.85, 'template threshold preserved');
}
ok(
  /ocrRows\.map\(\(c\) => ocrAssetPayload\(c, positions, recognitionPolygon\)\)/.test(saveUnified),
  'batch OCR rows are built by ocrAssetPayload with the recognition polygon'
);
ok(
  /singleAssetPayload\(c, positions, recognitionPolygon\)/.test(saveUnified),
  'single rows are built by singleAssetPayload with the recognition polygon'
);
ok(
  !/action_shape:\s*c\.actionShape\s*\|\|/.test(saveUnified),
  'save path no longer falls back to a hard-coded rect action_shape'
);
const renderGlobal = extractFunction('renderGlobalAssets');
ok(
  /globalAssetsCache/.test(renderGlobal) && /filterAssets\(all, q, cat\)/.test(renderGlobal),
  'global rendering filters globalAssetsCache (not a second copy of the list)'
);
ok(
  /globalAssetSearch\.addEventListener\('input'/.test(source) &&
    /globalAssetCategoryFilter\.addEventListener\('change'/.test(source),
  'global search/category controls are wired'
);
ok(
  /globalAssetStatusText\(all\.length, items\.length/.test(source),
  'global status reports total and visible count'
);
ok(
  (source.match(/await refreshGlobalAssets\(\)/g) || []).length >= 4,
  'save/delete/position/action-ROI paths refresh the global library'
);
ok(/btn-library-refresh/.test(html), 'real global refresh button exists');

// --- v1.2.4: static contracts for overlay removal, invert rendering, mode ---
const drawOverlayFn = extractFunction('drawClickOverlay');
ok(!/drawClickStats/.test(source), 'on-canvas click-stats helper is removed');
ok(!/clicks=/.test(drawOverlayFn), 'click overlay no longer draws a clicks=N plate');
ok(!/稀疏 → 密集/.test(source), 'old on-canvas legend caption is gone');
ok(/drawImage\(clickOverlay/.test(drawOverlayFn), 'click overlay still paints heat/scatter');
ok(/id="click-stats"/.test(html) && /id="clicks-legend"/.test(html), 'sidebar click stats and legend remain');
ok(/id="btn-test-mode"[\s\S]{0,400}id="btn-live-preview"|id="btn-live-preview"[\s\S]{0,400}id="btn-test-mode"/.test(html), 'test-mode button sits next to #btn-live-preview');
ok(/id="run-dry"/.test(html), 'task-run #run-dry stays untouched');
ok(/let assetTestReal = false;/.test(source), 'asset-test mode defaults to dry-run');

const drawActionFn = extractFunction('drawActionShapeOverlay');
ok(
  /ACTION_SHAPE_DASH/.test(drawActionFn) &&
    /ACTION_EFFECTIVE_FILL/.test(drawActionFn) &&
    /ACTION_EXCLUDED_FILL/.test(drawActionFn),
  'action renderer paints different effective/excluded fills and a shared dash'
);
ok(/const ACTION_SHAPE_DASH = \[8, 5\];/.test(source), 'action rect+polygon use the dashed [8,5] style');
ok(!/setLineDash\(\[10, 6\]\)/.test(source), 'old polygon-only invert dash is gone');
ok(/drawActionShapeOverlay\(\)/.test(extractFunction('draw')), 'draw() uses the unified action renderer');

const editActionFn = extractFunction('editAssetActionRoi');
ok(/loadActionShapeForEdit\(it\.action_shape\)/.test(editActionFn), 'asset-row 操作区 loads the saved shape (incl. invert)');
ok(!/roiBack = actionBox/.test(editActionFn), 'polygon edit no longer paints a stray bbox rect');
const updateActionFn = extractFunction('updateAssetActionRoi');
ok(
  /action_shape: shape/.test(updateActionFn) && /currentActionShape\(\)/.test(updateActionFn),
  'save payload uses currentActionShape (invert included)'
);

const clickTestFn = extractFunction('testAssetClick');
ok(/realAssetTestGuard\(\)/.test(clickTestFn), 'click test is gated by the real/dry-run mode');
ok(/confirmAssetTestGestures\('click'/.test(clickTestFn), 'real click confirms before sending');
ok(/sendAssetTestGesture\(true, 'tap'/.test(clickTestFn), 'real click sends POST /input/tap');
ok(/真实点击模式/.test(clickTestFn) && /演练/.test(clickTestFn), 'click result text distinguishes 演练/真实');
// --- O-2: drag steps carry the operation shape (polygon / invert) ---
// The 区域拖动 button and the asset drag test share regionDragStep, so this is
// the single place the DSL shape contract is pinned.
const polyDragShape = { kind: 'polygon', points: [[10, 10], [70, 10], [70, 50]], invert: true };
const polyDrag = regionDragStep([0, 0, 100, 100], polyDragShape, 'up');
eq(polyDrag.op, 'drag', 'regionDragStep builds a drag step');
eq(polyDrag.args.from.kind, 'region', 'drag `from` stays a region target');
eq(
  polyDrag.args.from.roi,
  [10, 10, 61, 41],
  'from.roi is derived from the shape bbox, not the recognition ROI'
);
eq(polyDrag.args.from.shape.kind, 'polygon', 'polygon shape is inlined into the flow');
eq(polyDrag.args.from.shape.invert, true, 'invert is preserved on the flow shape');
eq(polyDrag.args.direction, 'up', 'direction is passed through');
eq(polyDrag.args.distance, [150, 500], 'default distance stays [150,500]');
eq(polyDrag.args.duration_ms, [300, 650], 'default duration stays [300,650]');
eq(polyDrag.args.margin, 0.15, 'default margin stays 0.15');

const polyNoInvertDrag = regionDragStep(
  [0, 0, 100, 100],
  { kind: 'polygon', points: [[0, 0], [20, 0], [0, 20]], invert: false },
  'up'
);
eq(
  polyNoInvertDrag.args.from.roi,
  [0, 0, 21, 21],
  'polygon bbox is derived from its points (recognition ROI ignored)'
);
eq(
  polyNoInvertDrag.args.from.shape,
  { kind: 'polygon', points: [[0, 0], [20, 0], [0, 20]], invert: false },
  'a non-inverted polygon is still inlined (rect-only bbox would lose the shape)'
);

const rectInvertDrag = regionDragStep(
  [0, 0, 100, 100],
  { kind: 'rect', roi: [4, 5, 6, 7], invert: true },
  'left'
);
eq(
  rectInvertDrag.args.from.roi,
  [4, 5, 6, 7],
  'rect+invert uses the action rect as roi (recognition ROI ignored)'
);
eq(
  rectInvertDrag.args.from.shape,
  { kind: 'rect', roi: [4, 5, 6, 7], invert: true },
  'rect+invert is inlined too (invert changes the effective region)'
);

const bareDrag = regionDragStep([1, 2, 3, 4], null, 'down');
eq(bareDrag.args.from, { kind: 'region', roi: [1, 2, 3, 4] }, 'no shape => legacy bbox-only `from`');
ok(!('shape' in bareDrag.args.from), 'a missing shape never grows a shape key');

// P1-3: the plain-rect case used to keep the recognition ROI and drop the
// user's roiBack. A differing roi/shape input is what makes that visible.
const plainRectDrag = regionDragStep(
  [0, 0, 100, 100],
  { kind: 'rect', roi: [200, 200, 50, 50], invert: false },
  'right'
);
eq(
  plainRectDrag.args.from,
  { kind: 'region', roi: [200, 200, 50, 50] },
  'plain rect: from.roi is the user action rect, not the recognition ROI'
);
ok(
  !('shape' in plainRectDrag.args.from),
  'plain rect stays bbox-only (runtime samples roi with margin; no shape key)'
);

const legacyPlainRectDrag = regionDragStep(
  [9, 8, 7, 6],
  { kind: 'rect', roi: [9, 8, 7, 6], invert: false },
  'right'
);
eq(
  legacyPlainRectDrag.args.from,
  { kind: 'region', roi: [9, 8, 7, 6] },
  'plain rect where roi === shape bbox stays byte-identical to the old output'
);

const customDistanceDrag = regionDragStep([0, 0, 5, 5], null, 'up', [150, 350]);
eq(customDistanceDrag.args.distance, [150, 350], 'the asset drag test keeps its [150,350] distance');

const dragTestFn = extractFunction('testAssetDrag');
ok(/realAssetTestGuard\(\)/.test(dragTestFn), 'drag test is gated by the real/dry-run mode');
ok(/confirmAssetTestGestures\('drag'/.test(dragTestFn), 'real drag confirms before sending');
ok(/sendAssetTestGesture\(true, 'swipe'/.test(dragTestFn), 'real drag sends POST /input/swipe');
ok(/真实滑动模式/.test(dragTestFn) && /演练/.test(dragTestFn), 'drag result text distinguishes 演练/真实');
ok(/let stepShape = null;/.test(dragTestFn), 'asset drag snapshots a frame shape');
ok(
  /if \(!stepShape && effShape\) stepShape = effShape;/.test(dragTestFn),
  'shape snapshot is taken on the first successful frame (template shape already translated)'
);
ok(
  /regionDragStep\(stepAction, stepShape, direction, \[150, 350\]\)/.test(dragTestFn),
  'asset drag step is built by regionDragStep with the snapshot'
);
const genDragFn = extractFunction('generateRegionDrag');
ok(/regionDragStep\(/.test(genDragFn), '区域拖动 goes through the pure regionDragStep builder');
ok(/currentActionShape\(\)/.test(genDragFn), '区域拖动 inlines the current polygon/invert shape');
ok(!/roi: stepAction/.test(source), 'the old shape-less asset drag `from` is gone');
ok(
  !/from: \{ kind: 'region', roi: r \}/.test(source),
  'the old shape-less 区域拖动 `from` is gone'
);
// builder.js must not eat the new field when the user edits roi.
const structuredDragFn = extractFunctionFrom(builderSource, 'structuredDrag', 'builder.js');
ok(/const shape = from\.shape;/.test(structuredDragFn), 'builder keeps the drag from.shape');
ok(
  /shape\s*\n?\s*\? \{ kind: 'region', roi, shape \}/.test(structuredDragFn),
  'editing roi re-emits from.shape instead of eating it'
);
ok(
  /改 roi 不会改 shape/.test(structuredDragFn),
  'builder tells the user that roi and shape are edited separately'
);
ok(
  /polygon\/反选 shape：runtime 按 shape 采样/.test(structuredDragFn) &&
    /margin 忽略/.test(structuredDragFn),
  'builder says polygon/invert is sampled from shape and ignores margin'
);
ok(
  /runtime 实际只使用上面的 roi/.test(structuredDragFn),
  'builder says a plain rect is sampled from roi (shape is ignored by runtime)'
);
ok(/可选 shape/.test(structuredDragFn), 'builder drag form title mentions the optional shape');
const collectionTestFn = extractFunction('testAssetCollection');
ok(/realAssetTestGuard\(\)/.test(collectionTestFn), 'collection test is gated by the real/dry-run mode');
ok(
  /assetCollectionMatchUrl\(/.test(collectionTestFn),
  'collection test builds both requests through the shared URL helper'
);
ok(
  /real=1/.test(extractFunction('assetCollectionMatchUrl')),
  'collection real request carries real=1'
);
ok(
  /confirmAssetTestGestures\('click'/.test(collectionTestFn),
  'collection real mode confirms the concrete click before sending'
);
ok(
  collectionTestFn.indexOf("confirmAssetTestGestures('click'") <
    collectionTestFn.indexOf('frame.query, true'),
  'collection real request is sent only after the confirmation'
);
ok(/真实点击（已点击/.test(collectionTestFn), 'collection real result names the actual clicked point');
ok(/演练（未点击）/.test(collectionTestFn), 'collection dry-run result says 演练（未点击）');
ok(
  /assetCollectionMatchUrl\(task, it\.id, frame\.query, false\)/.test(collectionTestFn),
  'collection dry preview explicitly builds a real=0 URL'
);
ok(
  /response\.status === 409/.test(collectionTestFn) &&
    /真实点击未执行：重新匹配未过阈值（未点击）/.test(collectionTestFn),
  'collection real below-threshold 409 is a refused click, not a generic failure'
);
ok(
  /服务端将重新匹配并点击最新操作区（shape 内采样）/.test(collectionTestFn) &&
    /click_point 为准/.test(collectionTestFn),
  'collection confirmation explains the server re-match and click_point authority'
);
ok(
  !/\/input\/tap/.test(collectionTestFn),
  'collection test never duplicates the tap route (the backend owns the click)'
);
const guardFn = extractFunction('realAssetTestGuard');
ok(/livePreview/.test(guardFn) && /实时预览已暂停/.test(guardFn), 'real mode refuses when live preview is off');

// --- PR2: Studio meta.params schema editor ----------------------------------
// Studio is the only schema editor: GET/POST /task/config write meta.params.
// Runner values are a separate surface and must never be called from here.
eq(
  taskConfigParamsToJson([
    { key: 'b', type: 'string', label: '乙', enum: ['x', 'y'], required: true },
    { type: 'int', key: 'a', default: 3, min: 1, max: 9, description: '轮数' },
  ]),
  '[\n' +
    '  {\n' +
    '    "key": "b",\n' +
    '    "type": "string",\n' +
    '    "label": "乙",\n' +
    '    "enum": [\n' +
    '      "x",\n' +
    '      "y"\n' +
    '    ],\n' +
    '    "required": true\n' +
    '  },\n' +
    '  {\n' +
    '    "key": "a",\n' +
    '    "type": "int",\n' +
    '    "default": 3,\n' +
    '    "min": 1,\n' +
    '    "max": 9,\n' +
    '    "description": "轮数"\n' +
    '  }\n' +
    ']',
  'taskConfigParamsToJson uses pretty 2-space JSON with a stable field order'
);
eq(taskConfigParamsToJson(null), '[]', 'missing params serialize to an empty JSON array');
eq(
  taskConfigNormalizeParam({ zz: 1, group: '高级', key: 'k', type: 'int' }),
  { key: 'k', type: 'int', group: '高级', zz: 1 },
  'unknown meta.params fields are preserved after the known fields'
);
const schemaText = taskConfigParamsToJson([
  { key: '识别关键词', type: 'string', default: '突破', label: '识别关键词' },
  { key: 'max_rounds', type: 'int', default: 5 },
]);
const parsedSchema = parseTaskConfigParamsText(schemaText);
eq(parsedSchema.ok, true, 'pretty JSON round-trips through parseTaskConfigParamsText');
eq(parsedSchema.params.length, 2, 'round-trip keeps both params');
eq(parsedSchema.params[0].key, '识别关键词', 'round-trip keeps non-ASCII keys');
eq(parsedSchema.error, '', 'successful parse has no error text');
eq(parseTaskConfigParamsText('').ok, false, 'empty textarea is a local validation error');
eq(parseTaskConfigParamsText('{broken').ok, false, 'broken JSON is a local validation error');
ok(/JSON 解析失败/.test(parseTaskConfigParamsText('{broken').error), 'broken JSON error is explicit');
eq(parseTaskConfigParamsText('{"params":[]}').ok, false, 'top-level object is rejected (params must be an array)');
eq(parseTaskConfigParamsText('[{"key":"a"}]').ok, false, 'param without type is rejected locally');
eq(parseTaskConfigParamsText('[{"key":"a","type":"int"},{"key":"a","type":"int"}]').ok, false, 'duplicate keys are rejected locally');
eq(parseTaskConfigParamsText('[]'), { ok: true, params: [], error: '' }, 'empty array is a valid schema (no config items)');
eq(
  taskConfigSummary([{ key: 'a', type: 'int' }, { key: 'b', type: 'string' }]),
  '2 项配置：a(int)、b(string)',
  'taskConfigSummary lists count + key(type)'
);
eq(taskConfigSummary([]), '0 项配置', 'empty schema summary');
eq(taskConfigSummary([{ type: 'int' }]), '1 项配置：（未命名）(int)', 'summary marks a missing key');

// Static wiring: dialog markup + API call sites + the card button placement.
for (const id of ['task-config-dialog', 'task-config-title', 'task-config-json',
  'task-config-status', 'task-config-error', 'task-config-save', 'task-config-cancel']) {
  ok(new RegExp(`id="${id}"`).test(html), `index.html provides #${id}`);
}
ok(/id="task-config-dialog"[\s\S]{0,900}id="task-config-json" class="builder-code/.test(html), 'schema editor uses the builder-code textarea');
ok(/这里定义任务需要哪些配置（key\/type\/default\/label\/description\/enum\/required\/group）/.test(html), 'dialog explains the schema fields');
ok(/终端用户在 Runner 里填运行值，本处只定义 schema/.test(html), 'dialog draws the Studio/Runner boundary');
ok(/id="task-config-save"[^>]*>保存配置项</.test(html), 'save button is labelled 保存配置项');
ok(
  /taskConfigTitle\.textContent = '任务配置项 · ' \+ name/.test(source),
  'dialog title becomes 任务配置项 · <task>'
);
ok(
  /fetch\('\/task\/config\?task=' \+ encodeURIComponent\(task\)\)/.test(source),
  'dialog loads the schema from GET /task/config?task=<encoded>'
);
ok(
  /postJson\('\/task\/config', \{ task: name, params: parsed\.params \}\)/.test(source),
  'dialog saves with POST /task/config {task, params}'
);
ok(
  /const configBtn = document\.createElement\('button'\);[\s\S]{0,260}configBtn\.textContent = '配置项…'/.test(source),
  'task library card creates the 配置项… button'
);
ok(
  /configBtn\.title = '编辑该任务的 meta\.params 配置项（发布后随任务包下发）'/.test(source),
  'config button title explains the published-schema semantics'
);
ok(
  /card\.append\(meta, open, configBtn, publishBtn, runsBtn, del\)/.test(source),
  '配置项… / 发布… sit between 打开 and 运行记录'
);
ok(!/\/runner\/tasks\//.test(source), 'Studio never calls the Runner-only task config routes');

// --- PR5: Studio publishes .oaspkg (install stays Runner-only) --------------
eq(
  publishRequestBody('任务 2', {}),
  { task: '任务 2', include_deps: true },
  'publish body defaults to include_deps=true and omits out/force'
);
eq(
  publishRequestBody(' 任务 2 ', { out: '  ', force: false, include_deps: true }),
  { task: '任务 2', include_deps: true },
  'blank out is omitted and force=false does not travel'
);
eq(
  publishRequestBody('任务2', { out: ' dist/custom ', force: true, include_deps: false }),
  { task: '任务2', out: 'dist/custom', force: true, include_deps: false },
  'publish body carries custom out + explicit false/true flags'
);
eq(publishRequestBody(null, null), { task: '', include_deps: true }, 'missing task -> empty name, no crash');
eq(
  publishRequestBody('t', { reproducible: true }),
  { task: 't', include_deps: true, reproducible: true },
  'reproducible travels for CLI parity'
);

eq(
  formatPackageSummary({
    path: 'dist/packages/任务-v1.oaspkg',
    version: 1,
    package_id: 'pkg-1',
    sha256: 'a'.repeat(64),
  }),
  'dist/packages/任务-v1.oaspkg · v1 · pkg-1 · sha256 aaaaaaaaaaaa…',
  'package summary shows path + version + package_id + short sha'
);
eq(formatPackageSummary({ path: 'x.oaspkg' }), 'x.oaspkg', 'missing optional package fields are omitted');
eq(
  formatPackageSummary({ version: 'v2', package_id: 'p'.repeat(40) }),
  '（服务端未返回 path） · v2 · ' + 'p'.repeat(32) + '…',
  'summary never doubles the v prefix and truncates long ids'
);
eq(formatPackageSummary(null), '（服务端未返回 path）', 'null package summary is still renderable');

eq(
  publishErrorsText(['T3: 未声明的参数 k', 'T7: 子任务缺失']),
  '· T3: 未声明的参数 k\n· T7: 子任务缺失',
  'publish errors render one bullet per server line'
);
eq(publishErrorsText([]), '（服务端未返回错误详情）', 'empty publish error list still says something');
eq(publishErrorsText('单一错误'), '· 单一错误', 'a single error string is bulleted too');
eq(publishErrorsText([null, '  ', 'x']), '· x', 'blank publish errors are filtered');

for (const id of ['task-publish-dialog', 'task-publish-out', 'task-publish-list',
  'task-publish-status', 'task-publish-error', 'task-publish-submit']) {
  ok(new RegExp(`id="${id}"`).test(html), `index.html provides #${id}`);
}
ok(/id="task-publish-include-deps"[^>]*type="checkbox" checked/.test(html), 'include_deps is checked by default');
ok(/id="task-publish-force"[^>]*type="checkbox" \/>/.test(html), 'force is unchecked by default');
ok(
  /发布为 <code>\.oaspkg<\/code>：仅包含任务、子任务依赖与引用素材快照；终端用户在 Runner 安装。/.test(html),
  'publish dialog states the .oaspkg boundary'
);
ok(/<summary class="muted small">输出目录（可选，默认 dist\/packages）<\/summary>/.test(html), 'out input is collapsed behind a summary');
ok(/id="task-publish-submit"[^>]*>发布</.test(html), 'publish dialog has the 发布 button');
ok(
  /taskPublishDialog\.querySelector\('h3'\)[\s\S]{0,80}'发布任务包 · ' \+ name/.test(source),
  'dialog title becomes 发布任务包 · <task>'
);
ok(
  /fetch\('\/task\/package\?task=' \+ encodeURIComponent\(task\)\)/.test(source),
  'published list loads from GET /task/package?task=<encoded>'
);
ok(
  /postJson\('\/task\/publish', body\)/.test(source),
  'publish button POSTs /task/publish with the pure request body'
);
ok(/包已存在，勾选 force 或提升版本。/.test(source), '409/package-exists shows the force/version hint');
ok(
  /lines\.push\('sha256：' \+ \(s\.sha256 \|\| '—'\)\)/.test(source),
  'success panel shows the sha256 (copyable textContent)'
);
ok(
  /warnings\.map\(\(w\) => '· ' \+ String\(w\)\)\.join\('\\n'\)/.test(source),
  'warnings are rendered one per line'
);
ok(
  /taskPublishSubmit\.disabled = true/.test(source) && /taskPublishSubmit\.disabled = false/.test(source),
  'publish button is disabled while publishing and re-enabled afterwards'
);
ok(
  !/innerHTML|outerHTML|insertAdjacentHTML/.test(extractFunction('submitTaskPublish')) &&
    !/innerHTML|outerHTML|insertAdjacentHTML/.test(extractFunction('loadPublishedPackages')),
  'publish dialog renders through textContent only'
);
ok(
  !/\/runner\/(?:install|uninstall|packages)/.test(source),
  'Studio never calls the Runner-only install/uninstall/packages routes'
);

// DOM id audit: the whole point of this round is ZERO static dead ids. Every
// getElementById target must exist in index.html; if an id is genuinely needed,
// add a real element to the HTML instead of keeping a fake reference.
const referencedIds = [...new Set(
  [...source.matchAll(/getElementById\(['"]([^'"]+)['"]\)/g)].map((m) => m[1])
)];
const missingIds = referencedIds.filter((id) => !new RegExp(`id=["']${id}["']`).test(html));
const fixedIds = [
  'btn-library-refresh',
  'task-show-all-assets',
  'task-assets-hint',
  'btn-assets-batch-test',
  'assets-refresh',
  'global-assets-list',
  'global-asset-search',
  'global-asset-category-filter',
  'lines-list',
];
for (const id of fixedIds) {
  ok(!missingIds.includes(id), `fixed DOM id #${id} exists in index.html`);
}
ok(
  missingIds.length === 0,
  `dead DOM ids must be 0 (still dead: ${missingIds.join(', ') || 'none'})`
);
ok(
  referencedIds.length >= 159,
  `Studio DOM id audit covers the PR6 cleanup (got ${referencedIds.length}, need >= 159)`
);

// --- PR1/PR2/PR6: Studio / Runner surface split (static UI contract) --------
// Studio owns no scheduling page at all any more: PR6 removed the deferred
// #page-schedule DOM, the sched-entry/queue/run dialogs and the reset-memory
// dialog, and Studio only hands the user over to the Runner. The Runner is a
// separate usage surface with exactly six pages, its own static bundle and its
// own local write paths (task values, scheduler start/stop, temp run,
// scheduler.json).
const studioNavBlock = /<nav[^>]*id="app-nav"[^>]*>([\s\S]*?)<\/nav>/.exec(html);
ok(studioNavBlock, 'index.html still has the Studio app nav');
if (studioNavBlock) {
  const studioNav = studioNavBlock[1];
  ok(!/data-page="schedule"/.test(studioNav), 'Studio nav has no schedule page button');
  ok(!/>调度</.test(studioNav), 'Studio nav has no 调度 label');
  eq((studioNav.match(/class="app-nav-btn/g) || []).length, 5, 'Studio nav exposes exactly 5 page buttons');
}
ok(!/id="page-schedule"/.test(html), 'PR6 removed the legacy #page-schedule DOM');
ok(!/id="panel-schedule"/.test(html) && !/page-sched-/.test(html), 'PR6 removed the legacy schedule panel + page ids');
ok(!/id="sched-entry-dialog"/.test(html) && !/id="sched-queue-dialog"/.test(html), 'PR6 removed the legacy entry/queue dialogs');
ok(
  !/id="reset-memory-dialog"/.test(html) && !/id="sched-summary"/.test(html) &&
    !/id="sched-entries"/.test(html),
  'PR6 removed the reset-memory dialog + rail placeholders'
);
ok(!/sched-runs-table|sched-run-dialog/.test(html), 'PR6 removed the schedule-page-only CSS');
ok(!/panel-schedule|page-sched-|sched-entry-|sched-queue-|sched-summary|sched-entries/.test(source),
  'app.js no longer references any legacy schedule DOM id/panel');
ok(/id="studio-runner-hint"/.test(html) && /Runner（8789）/.test(html), 'Studio shows the Runner migration hint');
ok(/Studio 不参与调度|调度\/运行运维请用 Runner/.test(html), 'Studio still tells the user to use the Runner');
ok(/const RUNNER_BASE_URL = 'http:\/\/127\.0\.0\.1:8789\/'/.test(source), 'Studio knows the Runner default URL (8789)');
ok(/function openRunnerPage\(hash\)/.test(source), 'Studio has a single hand-off helper to the Runner');
ok(
  /openRunnerPage\('#runs'\)/.test(source) && /openRunnerPage\('#queue'\)/.test(source),
  'Studio run-history + legacy schedule deep links open the Runner'
);
ok(/wantPage === 'schedule'/.test(source), '?page=schedule is redirected to the Runner queue page');
ok(/if \(new URLSearchParams\(location\.search\)\.get\('reset'\) === '1'\)/.test(source) &&
  /重置记忆属于运行运维，请在 Runner（8789）中操作。/.test(source),
  '?reset=1 keeps its Runner-only hint');
ok(!/openTaskScheduleDialog\(t\.name\)/.test(source) && !/调度设置…/.test(source),
  'task library card no longer opens a Studio scheduler dialog');
ok(!/id="btn-sched-open-page"/.test(html), 'Studio no longer provides the workbench scheduling-page button');
// Studio must never call the Runner-only scheduling/run routes (they answer 404
// on the Studio surface). Read-only scheduling view is not used by Studio UI.
ok(!/\/runner\/scheduler\/(?:start|stop|status)/.test(source) && !/\/runner\/run['"]/.test(source),
  'Studio never calls /runner/scheduler/* or /runner/run');
ok(!/fetch\('\/scheduler\/config'\)/.test(source) && !/'\/scheduler\/reset'/.test(source),
  'Studio never writes scheduler config/reset');
ok(!/fetch\('\/runs\/scheduler/.test(source), 'Studio never reads /runs/scheduler (Runner-only)');
ok(!/fetch\('\/scheduler\/status'\)/.test(source) && !/fetch\('\/scheduler\/config'\)/.test(source),
  'Studio no longer polls /scheduler/status or config (the page is gone)');
const htmlIds = [...html.matchAll(/id=["']([^"']+)["']/g)].map((m) => m[1]);
const dupHtmlIds = [...new Set(htmlIds.filter((id, i) => htmlIds.indexOf(id) !== i))];
ok(dupHtmlIds.length === 0, `index.html has no duplicate DOM ids (duplicates: ${dupHtmlIds.join(', ') || 'none'})`);
ok(
  (html.match(/<dialog\b/g) || []).length === (html.match(/<\/dialog>/g) || []).length,
  'index.html dialog tags are balanced'
);
ok(/fetch\('\/runs'\)/.test(source) && /loadClicks\(/.test(source), 'legacy /runs click-distribution path is untouched');
ok(/runsBtn\.addEventListener\('click', \(\) => openRunnerPage\('#runs'\)\)/.test(source), 'task library row links to the Runner run history');

const runnerNavBlock = /<nav[^>]*id="runner-nav"[^>]*>([\s\S]*?)<\/nav>/.exec(runnerHtml);
ok(runnerNavBlock, 'runner.html has the Runner app nav');
if (runnerNavBlock) {
  const runnerNav = runnerNavBlock[1];
  const runnerPages = [...runnerNav.matchAll(/data-page="([^"]+)"/g)].map((m) => m[1]);
  eq(runnerPages, ['overview', 'tasks', 'packages', 'queue', 'runs', 'settings'], 'Runner nav has exactly the six usage pages');
  eq((runnerNav.match(/class="app-nav-btn/g) || []).length, 6, 'Runner nav renders exactly 6 buttons');
  for (const label of ['概览', '已安装任务', '安装包', '队列与调度', '运行记录', '设置']) {
    ok(new RegExp(`>${label}<`).test(runnerNav), `Runner nav contains 「${label}」`);
  }
  ok(!/任务构建|新建任务|素材|编辑|创作/.test(runnerNav), 'Runner nav has no authoring entry');
}
for (const id of ['page-overview', 'page-tasks', 'page-packages', 'page-queue', 'page-runs', 'page-settings']) {
  ok(new RegExp(`id="${id}"`).test(runnerHtml), `runner.html provides #${id}`);
}
for (const token of ['/runner/health', '/runner/tasks', '/runner/packages', '/runner/install',
  '/runner/uninstall', '/runner/packages/activate', '/scheduler/status', '/runs/scheduler',
  '/config', '/instance']) {
  ok(runnerSource.includes(token), `runner.js reads ${token}`);
}
for (const bad of ['/task/save_flow', '/task/save_asset', '/frames', '/task/', '/library/', '/relations/',
  'builder.js', 'flow_model.js', 'asset_ui.js', 'app.js']) {
  ok(!runnerSource.includes(bad), `runner.js does not reference authoring surface «${bad}»`);
  ok(!runnerHtml.includes(bad), `runner.html does not reference authoring surface «${bad}»`);
}
ok(/href="\/style.css"/.test(runnerHtml) && /href="\/runner.css"/.test(runnerHtml), 'Runner loads the shared + Runner stylesheets');
ok(/src="\/runner.js"/.test(runnerHtml), 'Runner loads its own script only');
ok(!/(?:src|href)=["'](?:https?:)?\/\//i.test(runnerHtml), 'runner.html has no external/CDN asset');
ok(!/@import|url\(\s*['"]?https?:/i.test(runnerCss), 'runner.css has no external font/CDN import');
ok(/name="viewport"/.test(runnerHtml), 'Runner is mobile/WebView friendly (viewport meta)');
ok(/@media\s*\(max-width/.test(runnerCss), 'runner.css has a narrow-screen single-column layout');
ok(/只读/.test(runnerHtml), 'Runner still labels the pages that remain read-only');
ok(/还没有任务：请到「安装包」页安装 \.oaspkg/.test(runnerSource), 'Runner task empty state points at the install page');
ok(
  /id="runner-sched-control"/.test(runnerHtml) && /id="runner-run-card"/.test(runnerHtml),
  'Runner queue page hosts the scheduler control + temp-run cards'
);
ok(
  /运行值保存在 Runner 本机（config\/values\.json）；单任务运行时使用这里的值；\s*被其他任务作为子任务调用时使用调用方的配置（PR4 生效）。/.test(runnerHtml),
  'Runner config dialog explains local values + call_task inheritance (PR4)'
);
ok(/安装\/卸载\/回滚请到「安装包」页/.test(runnerHtml), 'Runner tasks page points at the package page');
ok(/修改配置请编辑 <code>config\/live\.json<\/code> 后重启 Runner；编辑端不随使用端分发/.test(runnerHtml), 'Runner settings keeps the manual-config + no-Studio-distribution note');
ok(/runner.js/.test(runnerHtml) && !/dashboard\.js/.test(runnerHtml), 'Runner does not load Studio-only scripts');

ok(!/innerHTML|outerHTML|insertAdjacentHTML/.test(runnerSource), 'runner.js never writes raw HTML');
ok(!/\beval\s*\(|new Function\s*\(/.test(runnerSource), 'runner.js never evaluates dynamic code');
ok(!/document\.write\s*\(/.test(runnerSource), 'runner.js never uses document.write');
eq((runnerSource.match(/fetch\s*\(/g) || []).length, 1, 'all Runner requests go through the single fetch helper');
ok(/RUNNER_FETCH_TIMEOUT_MS/.test(runnerSource) && /AbortController/.test(runnerSource), 'Runner requests have an abort/timeout guard');
ok(
  /const method = options\.method \? String\(options\.method\)\.toUpperCase\(\) : 'GET'/.test(runnerSource),
  'Runner requests default to GET and only explicit writes opt into POST/PUT'
);
ok(/method: method,/.test(runnerSource), 'the fetch helper sends the resolved method');
ok(
  /runnerFetchJson\(RUNNER_API\.install, \{ method: 'POST', body: body \}\)/.test(runnerSource) &&
    /runnerFetchJson\(RUNNER_API\.uninstall, \{ method: 'POST', body: \{ task_id: task \} \}\)/.test(runnerSource) &&
    /runnerFetchJson\(RUNNER_API\.packagesActivate, \{ method: 'POST', body: \{ task_id: task \} \}\)/.test(runnerSource),
  'the package POST writes are /runner/install, /runner/uninstall and /runner/packages/activate'
);
ok(/'PUT'/.test(runnerSource), 'Runner sends PUT for task config values');
ok(!/localhost|127\.0\.0\.1|8789/.test(runnerSource), 'runner.js does not hardcode host/port');
ok(!/localhost|127\.0\.0\.1|8789/.test(runnerHtml), 'runner.html does not hardcode host/port');
ok(!/[A-Za-z]:\\/.test(runnerSource) && !/[A-Za-z]:\\/.test(runnerHtml), 'no hardcoded Windows drive path');
ok(/runnerRunsErrorText/.test(runnerSource) && /运行记录读取失败：/.test(runnerSource), 'Runner run-read errors keep the 运行记录读取失败 prefix');

const runnerHtmlIds = [...runnerHtml.matchAll(/id=["']([^"']+)["']/g)].map((m) => m[1]);
const runnerDupIds = [...new Set(runnerHtmlIds.filter((id, i) => runnerHtmlIds.indexOf(id) !== i))];
ok(runnerDupIds.length === 0, `runner.html has no duplicate DOM ids (duplicates: ${runnerDupIds.join(', ') || 'none'})`);
const runnerReferencedIds = [...new Set(
  [...runnerSource.matchAll(/getElementById\(['"]([^'"]+)['"]\)/g)].map((m) => m[1])
)];
const runnerMissingIds = runnerReferencedIds.filter((id) => !new RegExp(`id=["']${id}["']`).test(runnerHtml));
ok(runnerReferencedIds.length >= 20, 'runner.js references the real Runner DOM ids');
ok(
  runnerMissingIds.length === 0,
  `Runner dead DOM ids must be 0 (still dead: ${runnerMissingIds.join(', ') || 'none'})`
);
ok(
  runnerReferencedIds.length >= 75,
  `Runner DOM id audit covers the PR6 scheduler control (got ${runnerReferencedIds.length}, need >= 75)`
);

// Behavioural checks for the extractable Runner formatting helpers.
const runnerHelperNames = [
  'runnerValueText', 'runnerDurationText', 'runnerParamKeys', 'runnerFlagText',
  'runnerTaskStatusText', 'runnerOutcomeClass', 'runnerLockText', 'runnerLockClass',
  'runnerRunsErrorText', 'runnerQueueRoundsText', 'runnerEntryStateText', 'runnerMemberText',
  'runnerErrorText', 'runnerErrorSummary',
  // PR5: package install body / list row / task badge / error mapping.
  'runnerInstallBody', 'formatPackageRow', 'runnerTaskPackageBadge', 'runnerPackagesErrorText',
  // PR6: scheduler start/stop text, temp-run body/status, scheduler.json body.
  'runnerRunBody', 'runnerRunStatusText', 'runnerSchedStatusText', 'runnerSchedLockPidText',
  'runnerSchedThreadText', 'runnerSchedJsonBody', 'runnerErrorLines',
  // PR6 fix: 409 code mapping, stop timeout text, scheduler history rows.
  'runnerErrorCodeLabel', 'runnerBodyErrors', 'runnerStopText', 'runnerStopPending',
  'runnerSchedHistoryRows',
];
const runnerHelperCode = runnerHelperNames
  .map((name) => extractFunctionFrom(runnerSource, name, 'runner.js'))
  .join('\n');
const runnerSb = {};
vm.createContext(runnerSb);
vm.runInContext(runnerHelperCode, runnerSb, { filename: 'runner.js#pure' });
eq(runnerSb.runnerParamKeys(['a', 'b']), ['a', 'b'], 'Runner task params keep the string-list contract');
eq(runnerSb.runnerParamKeys([{ key: 'max_rounds' }, { key: '关键词' }]), ['max_rounds', '关键词'], 'Runner params also accept key objects');
eq(runnerSb.runnerParamKeys({ a: 1 }), ['a'], 'Runner params accept a map');
eq(runnerSb.runnerParamKeys(null), [], 'Runner params missing -> empty list');
eq(runnerSb.runnerFlagText(true), '有', 'Runner flag true -> 有');
eq(runnerSb.runnerFlagText(false), '无', 'Runner flag false -> 无');
eq(runnerSb.runnerTaskStatusText({ unmanaged: true }), '未托管/本机目录', 'PR1 tasks are unmanaged/local');
eq(runnerSb.runnerTaskStatusText({ unmanaged: false }), '已安装', 'managed tasks render 已安装');
eq(runnerSb.runnerLockText(null), '调度锁 空闲', 'missing lock -> idle');
eq(runnerSb.runnerLockText({ held: true, pid: 42 }), '调度锁 占用（pid 42）', 'held lock shows its pid');
eq(runnerSb.runnerLockClass({ held: true, pid: 42 }), 'warn', 'held lock uses the warning badge');
eq(runnerSb.runnerLockClass({ held: false }), 'ok', 'free lock uses the ok badge');
eq(runnerSb.runnerDurationText(500), '500ms', 'sub-second duration');
eq(runnerSb.runnerDurationText(1500), '1.5s', 'second duration');
eq(runnerSb.runnerDurationText(null), '—', 'missing duration is a dash');
eq(runnerSb.runnerRunsErrorText('HTTP 500'), '运行记录读取失败：HTTP 500', 'Runner run-read error text');
eq(runnerSb.runnerOutcomeClass('Failed'), 'err', 'Failed outcome is an error badge');
eq(runnerSb.runnerOutcomeClass('success'), 'ok', 'success outcome is an ok badge');
eq(runnerSb.runnerQueueRoundsText({ total_rounds: 0, rounds_completed: 3 }), '3/∞', 'unlimited queue rounds');
eq(runnerSb.runnerQueueRoundsText({ total_rounds: 2, rounds_completed: 1 }), '1/2', 'finite queue rounds');
eq(runnerSb.runnerEntryStateText({ enabled: false }), '停用', 'disabled entry state');
ok(/等待 冷却/.test(runnerSb.runnerEntryStateText({ enabled: true, blocked_reason: '冷却' })), 'blocked entry explains the wait');
eq(runnerSb.runnerMemberText(0), '成员 1', 'member_index is rendered one-based for humans');
eq(runnerSb.runnerMemberText(null), '—', 'missing member index is a dash');
eq(runnerSb.runnerValueText(0), '0', 'zero is preserved');
eq(runnerSb.runnerValueText(null), '—', 'null renders a dash');

// --- PR5: Runner package helpers (install body / list row / badge / errors) --
eq(
  runnerSb.runnerInstallBody('  packages\\任务-v1.oaspkg  ', true),
  { path: 'packages\\任务-v1.oaspkg', force: true },
  'install body trims the local path and keeps force'
);
eq(runnerSb.runnerInstallBody(null, null), { path: '', force: false }, 'missing install path is an empty body');
eq(runnerSb.runnerInstallBody('x.oaspkg', 'yes'), { path: 'x.oaspkg', force: false }, 'force is a strict boolean');

const longPid = 'p'.repeat(40);
const pkgRow = runnerSb.formatPackageRow({
  package_id: longPid,
  task_id: '任务2',
  version: 3,
  sha256: 'a'.repeat(64),
  installed_at_ms: 1700000000000,
  backup: 'packages/backup/任务2-2-1700000000000',
});
eq(pkgRow.task_id, '任务2', 'package row keeps the task id');
eq(pkgRow.version, '3', 'package row keeps the version as text');
eq(pkgRow.package_id, longPid, 'package row keeps the full package id for the tooltip');
eq(pkgRow.package_id_short, 'p'.repeat(28) + '…', 'package row shortens the package id for display');
eq(pkgRow.sha256, 'a'.repeat(64), 'package row keeps the full sha for the tooltip');
eq(pkgRow.sha_short, 'aaaaaaaaaaaa…', 'package row shows a 12-char sha prefix');
eq(pkgRow.installed_at_ms, 1700000000000, 'package row keeps installed_at_ms for local time formatting');
eq(pkgRow.backup, 'packages/backup/任务2-2-1700000000000', 'package row shows the backup path');
const emptyPkgRow = runnerSb.formatPackageRow(null);
eq(emptyPkgRow.task_id, '—', 'missing package fields render a dash');
eq(emptyPkgRow.sha_short, '—', 'missing sha renders a dash');
eq(emptyPkgRow.installed_at_ms, null, 'missing installed_at_ms stays null');
eq(emptyPkgRow.backup, '—', 'missing backup renders a dash');

eq(
  runnerSb.runnerTaskPackageBadge({ unmanaged: true, version: 3 }).text,
  '未纳管',
  'bare tasks show 未纳管 even when meta.version exists'
);
eq(runnerSb.runnerTaskPackageBadge({ unmanaged: true }).cls, 'warn', '未纳管 badge is a warning');
eq(
  runnerSb.runnerTaskPackageBadge({ unmanaged: false, version: 3, package_id: 'pkg-1' }),
  { text: '已安装 v3', cls: 'ok', title: 'package_id：pkg-1' },
  'installed tasks show 已安装 vN + package id'
);
eq(runnerSb.runnerTaskPackageBadge({ package_id: 'pkg-1' }).text, '已安装', 'package_id alone marks a managed task');
eq(
  runnerSb.runnerTaskPackageBadge({ installed: true, version: 2 }).text,
  '已安装 v2',
  'installed:true is honored even without unmanaged/package_id'
);
eq(runnerSb.runnerTaskPackageBadge({}).text, '—', 'package info missing -> neutral badge');
eq(runnerSb.runnerTaskPackageBadge({}).cls, 'kind', 'neutral badge uses the kind class');

ok(
  /dirty_task/.test(runnerSb.runnerPackagesErrorText('HTTP 409：dirty_task 任务被修改')) &&
    /force/.test(runnerSb.runnerPackagesErrorText('HTTP 409：dirty_task 任务被修改')),
  'dirty_task error explains force'
);
ok(
  /flow_ops/.test(runnerSb.runnerPackagesErrorText('manifest.engine.flow_ops unsupported')),
  'flow_ops error names the manifest gate'
);
ok(
  /checksum/.test(runnerSb.runnerPackagesErrorText({ message: 'checksum mismatch' })),
  'checksum error is explained'
);
ok(
  /找不到安装包文件/.test(runnerSb.runnerPackagesErrorText('ENOENT: no such file')),
  'missing file error is explained'
);
ok(
  /版本不满足/.test(runnerSb.runnerPackagesErrorText('version 3 required')),
  'version mismatch is explained'
);
eq(runnerSb.runnerPackagesErrorText('HTTP 500：boom'), 'HTTP 500：boom', 'unknown errors keep the server text');
eq(runnerSb.runnerPackagesErrorText(null), '未知错误', 'null package error is never blank');

// Static package-page wiring.
for (const id of ['page-packages', 'runner-packages-summary', 'runner-packages-refresh',
  'runner-packages-status', 'runner-packages-count', 'runner-packages-list',
  'runner-packages-unmanaged-status', 'runner-packages-unmanaged', 'runner-install-path',
  'runner-install-force', 'runner-install-submit', 'runner-install-status',
  'runner-install-result', 'runner-install-warnings', 'runner-install-error']) {
  ok(new RegExp(`id="${id}"`).test(runnerHtml), `runner.html provides #${id}`);
}
ok(/id="runner-install-submit"[^>]*>安装</.test(runnerHtml), 'install card has the 安装 button');
ok(/id="runner-install-path"[^>]*type="text"/.test(runnerHtml), 'install path is a plain local path input');
ok(/id="runner-install-force"[^>]*type="checkbox"/.test(runnerHtml), 'install card has the force checkbox');
ok(/<h3>已安装包<\/h3>/.test(runnerHtml) && /<h3>未纳管（裸目录）<\/h3>/.test(runnerHtml), 'package page separates installed + unmanaged');
ok(/runnerFetchJson\(RUNNER_API\.packages\)/.test(runnerSource), 'package list is loaded from GET /runner/packages');
ok(/runnerInstallBody\(\s*el\.installPath \? el\.installPath\.value : '',\s*el\.installForce \? el\.installForce\.checked : false\s*\)/.test(runnerSource), 'install reads path + force through the pure body builder');
ok(/renderKv\(el\.installResult, \[/.test(runnerSource), 'install success renders package_id/task/version/replaced/backup');
ok(/b\.replaced === true \? '是（旧版本已备份）'/.test(runnerSource), 'replaced is rendered as yes/no, not raw JSON');
ok(/warnings\.map\(function \(w\) \{ return '· ' \+ String\(w\); \}\)/.test(runnerSource), 'install warnings are listed line by line');
ok(/window\.confirm\('卸载任务包「' \+ task \+ '」/.test(runnerSource), 'uninstall asks for a second confirmation');
ok(/runnerFetchJson\(RUNNER_API\.uninstall, \{ method: 'POST', body: \{ task_id: task \} \}\)/.test(runnerSource), 'uninstall POSTs {task_id}');
ok(/runnerFetchJson\(RUNNER_API\.packagesActivate, \{ method: 'POST', body: \{ task_id: task \} \}\)/.test(runnerSource), 'rollback POSTs /runner/packages/activate {task_id}');
ok(/uninstall\.textContent = '卸载'/.test(runnerSource) && /activate\.textContent = '回滚'/.test(runnerSource), 'each installed row offers 卸载 + 回滚');
ok(/runnerTaskPackageBadge\(t\)/.test(runnerSource), 'installed-task rows render the package badge');
ok(/el\.installSubmit\.disabled = !!busy/.test(runnerSource), 'install button is disabled while a package operation is in flight');
ok(/packages\/installed\.json/.test(runnerSource), 'package list names packages/installed.json');
ok(/state\.page === 'packages'/.test(runnerSource), 'page switch loads the package page');
ok(/RUNNER_PAGES = \['overview', 'tasks', 'packages', 'queue', 'runs', 'settings'\]/.test(runnerSource), 'packages is the 3rd nav page in the JS page list');
ok(/#runner-packages-list/.test(runnerCss) && /#runner-install-path/.test(runnerCss), 'runner.css styles the install/package surfaces');

// --- PR2: Runner task config form (local values, config/values.json) --------
for (const id of ['runner-config-dialog', 'runner-config-title', 'runner-config-form',
  'runner-config-fields', 'runner-config-status', 'runner-config-error',
  'runner-config-save', 'runner-config-close']) {
  ok(new RegExp(`id="${id}"`).test(runnerHtml), `runner.html provides #${id}`);
}
ok(
  /id="runner-config-dialog"[\s\S]{0,1800}<form id="runner-config-form"/.test(runnerHtml),
  'the config dialog owns the real form element'
);
ok(
  /id="runner-config-form"[\s\S]{0,420}id="runner-config-fields"/.test(runnerHtml),
  'the form hosts the generated field container'
);
ok(/id="runner-config-save"[^>]*>保存</.test(runnerHtml), 'the config form has a 保存 button');
ok(
  /el\.configTitle\.textContent = '任务配置 · ' \+ name/.test(runnerSource),
  'dialog title becomes 任务配置 · <task>'
);
ok(/该任务没有配置项/.test(runnerSource), 'empty schema shows 该任务没有配置项 instead of a blank form');
eq(
  (runnerHtml.match(/<dialog\b/g) || []).length,
  (runnerHtml.match(/<\/dialog>/g) || []).length,
  'runner.html dialog tags are balanced'
);
ok(/runner-config-dialog/.test(runnerCss), 'runner.css styles the config dialog');
ok(/\.runner-config-field/.test(runnerCss), 'runner.css styles the generated field rows');
ok(/#runner-config-fields[\s\S]{0,80}max-height/.test(runnerCss), 'long config forms scroll instead of overflowing');

ok(
  /const body = await runnerFetchJson\(runnerConfigUrl\(task\)\);/.test(runnerSource),
  'dialog loads values with GET runnerConfigUrl(task)'
);
ok(
  /runnerFetchJson\(runnerConfigUrl\(task\), \{\s*method: 'PUT',\s*body: \{ values: collected\.values \},\s*\}\)/.test(runnerSource),
  'dialog saves with PUT /runner/tasks/<id>/config {values}'
);
ok(
  /const controlId = 'runner-cfg-' \+ i;/.test(runnerSource) &&
    /control\.setAttribute\('data-key', key\)/.test(runnerSource),
  'config controls use index-based DOM ids + data-key (non-ASCII/space-safe)'
);
ok(/runnerConfigCollectValues\(configParams, fieldValues\)/.test(runnerSource), 'save collects typed values from the form');
ok(/configBtn\.textContent = '配置'/.test(runnerSource), 'installed task rows render a 配置 button');
ok(
  /t\.has_meta === true \? configBtn : \{ text: '—' \}/.test(runnerSource),
  'only tasks with a readable meta.json offer 配置'
);
ok(/const emptyOption = document\.createElement\('option'\)/.test(runnerSource), 'enum params render a select with an empty option');
ok(/e\.details\.map\(function \(line\)/.test(runnerSource), '400 errors are displayed line by line (backend messages keep the key)');
ok(/任务已不存在：/.test(runnerSource), '404 from the config PUT reports a vanished task');
ok(/配置读取失败：/.test(runnerSource), 'config GET failures keep the 配置读取失败 prefix');
ok(!runnerSource.includes('/task/config'), 'Runner never calls the Studio-only /task/config schema route');
ok(/config\/values\.json/.test(runnerHtml), 'Runner UI tells the user where local values live');

// Behavioural checks for the extractable Runner config helpers.
const runnerConfigNames = [
  'runnerConfigFieldKind', 'runnerConfigIsIntegerType', 'runnerConfigInitialValue',
  'runnerConfigCollectValues', 'runnerConfigUrl',
];
const runnerConfigCode = runnerConfigNames
  .map((name) => extractFunctionFrom(runnerSource, name, 'runner.js'))
  .join('\n');
const cfgSb = {};
vm.createContext(cfgSb);
vm.runInContext(runnerConfigCode, cfgSb, { filename: 'runner.js#config' });
eq(cfgSb.runnerConfigFieldKind({ key: 'n', type: 'int' }), 'number', 'int -> number input');
eq(cfgSb.runnerConfigFieldKind({ key: 'n', type: 'float' }), 'number', 'float -> number input');
eq(cfgSb.runnerConfigFieldKind({ key: 'n', type: 'number' }), 'number', 'number -> number input');
eq(cfgSb.runnerConfigFieldKind({ key: 'b', type: 'bool' }), 'bool', 'bool -> checkbox');
eq(cfgSb.runnerConfigFieldKind({ key: 's', type: 'string', enum: ['突破', '结界'] }), 'select', 'string + enum -> select');
eq(cfgSb.runnerConfigFieldKind({ key: 's', type: 'string' }), 'text', 'plain string -> text input');
eq(cfgSb.runnerConfigIsIntegerType({ type: 'int' }), true, 'int is integer-typed');
eq(cfgSb.runnerConfigIsIntegerType({ type: 'float' }), false, 'float allows decimals');
eq(cfgSb.runnerConfigInitialValue({ key: 'k', default: 5 }, { k: 7 }), 7, 'saved value wins over default');
eq(cfgSb.runnerConfigInitialValue({ key: 'k', default: 5 }, {}), 5, 'default fills a missing saved value');
eq(cfgSb.runnerConfigInitialValue({ key: 'b', type: 'bool' }, {}), false, 'bool without value starts unchecked');
eq(cfgSb.runnerConfigInitialValue({ key: 's', type: 'string' }, {}), '', 'string without value starts empty');
eq(cfgSb.runnerConfigInitialValue({ key: 'k', default: 0 }, { k: 0 }), 0, 'zero saved value is preserved');
eq(
  cfgSb.runnerConfigCollectValues([{ key: 'flag', type: 'bool' }], { flag: true }).values,
  { flag: true },
  'bool checkbox collects a boolean true'
);
eq(
  cfgSb.runnerConfigCollectValues([{ key: 'flag', type: 'bool' }], { flag: false }).values,
  { flag: false },
  'bool checkbox collects a boolean false'
);
eq(
  cfgSb.runnerConfigCollectValues([{ key: 'n', type: 'int' }], { n: '7' }).values,
  { n: 7 },
  'number input is converted to a JSON number'
);
eq(
  cfgSb.runnerConfigCollectValues([{ key: 'n', type: 'int' }], { n: '' }).values,
  {},
  'blank optional number is omitted (server default applies)'
);
const requiredRes = cfgSb.runnerConfigCollectValues(
  [{ key: 'k', type: 'string', required: true, label: '关键词' }], { k: '' }
);
eq(requiredRes.ok, false, 'blank required string is a validation error');
ok(/「关键词」是必填项/.test(requiredRes.errors[0]), 'required error names the label/key');
eq(
  cfgSb.runnerConfigCollectValues(
    [{ key: 'k', type: 'string', required: true, default: '突破' }], { k: '' }
  ).ok,
  true,
  'required with a default may stay blank'
);
const rangeRes = cfgSb.runnerConfigCollectValues(
  [{ key: 'n', type: 'int', min: 1, max: 5, label: '轮数' }], { n: '9' }
);
eq(rangeRes.ok, true, 'out-of-range value is submitted so the server 400 can name key + range');
eq(rangeRes.values, { n: 9 }, 'out-of-range value keeps its JSON number type');
const nanRes = cfgSb.runnerConfigCollectValues([{ key: 'n', type: 'int', label: '轮数' }], { n: 'x' });
eq(nanRes.ok, false, 'non-numeric number input is rejected locally');
ok(/必须是数字/.test(nanRes.errors[0]), 'non-numeric error explains the expected type');
const enumRes = cfgSb.runnerConfigCollectValues(
  [{ key: 's', type: 'string', enum: ['突破', '结界'], label: '模式' }], { s: '其它' }
);
eq(enumRes.ok, false, 'value outside enum is rejected locally');
ok(/必须是以下之一/.test(enumRes.errors[0]), 'enum error lists the options');
eq(
  cfgSb.runnerConfigCollectValues([{ key: 'n', type: 'int', enum: [1, 3, 5] }], { n: '3' }).values,
  { n: 3 },
  'numeric enum accepts a declared member'
);
eq(
  cfgSb.runnerConfigCollectValues(
    [{ key: 'n', type: 'int', enum: [1, 3, 5], label: '轮数' }], { n: '4' }
  ).values,
  { n: 4 },
  'numeric enum membership stays server-authoritative (400 lists the allowed values)'
);
eq(
  cfgSb.runnerConfigCollectValues(
    [{ key: 'n', type: 'int' }], { n: 1, stray: 2 }
  ).values,
  { n: 1 },
  'field values outside the schema are never submitted'
);
eq(
  cfgSb.runnerConfigUrl('任务 1'),
  '/runner/tasks/%E4%BB%BB%E5%8A%A1%201/config',
  'config URL encodes non-ASCII + spaces'
);
eq(
  cfgSb.runnerConfigUrl('a/b'),
  '/runner/tasks/a%2Fb/config',
  'config URL encodes slashes so a task name cannot escape the path'
);


// --- PR6: Runner scheduler control / temp run / scheduler.json ---------------
// The Runner owns scheduling operations now: start/stop its in-process thread,
// run one task on demand (body params override the saved values) and edit the
// raw scheduler.json as the advanced escape hatch (V5 step 13).
for (const id of ['runner-sched-control', 'runner-sched-badge', 'runner-sched-meta',
  'runner-sched-start', 'runner-sched-stop', 'runner-sched-refresh', 'runner-sched-status',
  'runner-sched-error', 'runner-run-card', 'runner-run-task', 'runner-run-params',
  'runner-run-submit', 'runner-run-status', 'runner-run-error', 'runner-run-log',
  'runner-sched-json', 'runner-sched-json-save', 'runner-sched-json-status',
  'runner-sched-json-error', 'overview-scheduler', 'overview-goto-queue']) {
  ok(new RegExp(`id="${id}"`).test(runnerHtml), `runner.html provides #${id}`);
}
ok(/id="runner-sched-start"[^>]*>启动调度</.test(runnerHtml), 'scheduler control has the 启动调度 button');
ok(/id="runner-sched-stop"[^>]*>停止调度</.test(runnerHtml), 'scheduler control has the 停止调度 button');
ok(/id="runner-run-submit"[^>]*>立即运行</.test(runnerHtml), 'temp-run card has the 立即运行 button');
ok(/id="runner-sched-json-save"[^>]*>保存 JSON</.test(runnerHtml), 'advanced card has the 保存 JSON button');
ok(
  /留空或 <code>\{\}<\/code> 时使用 Runner\s*已保存的运行值/.test(runnerHtml),
  'temp run explains saved values + params override'
);
ok(/JSON 结构见文档；保存后需启动\/重启调度生效/.test(runnerHtml), 'scheduler.json card explains the restart requirement');
ok(/去「队列与调度」页/.test(runnerHtml), 'overview links to the queue page');
ok(/id="runner-sched-json"[^>]*spellcheck="false"/.test(runnerHtml), 'advanced JSON is a plain textarea');

ok(
  /schedulerStart: '\/runner\/scheduler\/start'/.test(runnerSource) &&
    /schedulerStop: '\/runner\/scheduler\/stop'/.test(runnerSource),
  'Runner API map names the scheduler start/stop routes'
);
ok(
  /schedulerThreadStatus: '\/runner\/scheduler\/status'/.test(runnerSource) &&
    /runStatus: '\/runner\/run\/status'/.test(runnerSource),
  'Runner API map names the scheduler-thread + run-status routes'
);
ok(/schedulerConfig: '\/scheduler\/config'/.test(runnerSource), 'Runner reads/writes the shared scheduler config route');
ok(
  /runnerFetchJson\(RUNNER_API\.schedulerStart, \{ method: 'POST' \}\)/.test(runnerSource),
  '启动调度 POSTs /runner/scheduler/start'
);
ok(
  /runnerFetchJson\(RUNNER_API\.schedulerStop, \{ method: 'POST' \}\)/.test(runnerSource),
  '停止调度 POSTs /runner/scheduler/stop'
);
ok(
  /runnerFetchJson\(RUNNER_API\.run, \{ method: 'POST', body: parsed\.body \}\)/.test(runnerSource),
  '立即运行 POSTs /runner/run with the pure body'
);
ok(/runnerFetchJson\(RUNNER_API\.runStatus\)/.test(runnerSource), 'temp run polls GET /runner/run/status');
ok(/runnerFetchJson\(RUNNER_API\.schedulerThreadStatus\)/.test(runnerSource), 'queue page reads GET /runner/scheduler/status');
ok(/runnerFetchJson\(RUNNER_API\.schedulerStatus\)/.test(runnerSource), 'queue page still reads the shared GET /scheduler/status');
ok(
  /runnerFetchJson\(RUNNER_API\.schedulerConfig, \{ method: 'POST', body: parsed\.body \}\)/.test(runnerSource),
  '保存 JSON POSTs {config} to /scheduler/config'
);
ok(
  /runnerSchedJsonBody\(el\.schedJson \? el\.schedJson\.value : ''\)/.test(runnerSource),
  '保存 JSON parses the textarea through the pure body builder'
);
ok(
  /el\.schedJson\.addEventListener\('input'/.test(runnerSource) &&
    /schedJsonDirty = true/.test(runnerSource) &&
    /if \(!schedJsonDirty\)/.test(runnerSource),
  'advanced JSON tracks unsaved edits and a refresh never clobbers them'
);
ok(
  /RUNNER_RUN_POLL_MS = 500;/.test(runnerSource) && /RUNNER_RUN_POLL_MAX_MS = 60000;/.test(runnerSource),
  'temp run polls every 500ms for at most 60s'
);
ok(/RUNNER_SCHED_POLL_MS = 5000;/.test(runnerSource), 'queue page polls the scheduler status every 5s');
ok(
  /queuePollTimer = setInterval\(function \(\) \{/.test(runnerSource),
  'queue polling is a real interval'
);
ok(
  /if \(page === 'queue'\) startQueuePolling\(\);/.test(runnerSource) &&
    /else stopQueuePolling\(\);/.test(runnerSource),
  'entering the queue page starts polling and leaving it stops'
);
ok(
  /if \(status && status\.running === true\) \{\s*scheduleRunnerRunPoll\(\)/.test(runnerSource),
  'the run poll continues only while the run is still running'
);
ok(
  /status && Array\.isArray\(status\.log_tail\)/.test(runnerSource) &&
    /el\.runLog\.textContent = lines\.join/.test(runnerSource),
  'the run log tail is rendered via textContent'
);
ok(
  /const err = status \? status\.error : null;/.test(runnerSource) &&
    /el\.runError\.textContent/.test(runnerSource),
  'run errors are rendered verbatim'
);
ok(
  /el\.schedError\.textContent = runnerErrorLines\(e\)\.join\('\\n'\)/.test(runnerSource),
  'scheduler errors are listed line by line'
);
ok(
  /body\.already_running === true/.test(runnerSource) &&
    /b\.already_stopped === true/.test(runnerSource),
  'already-running/stopped responses are handled'
);
ok(/b\.timeout === true|body\.timeout === true/.test(runnerSource), 'a stop timeout is surfaced as a warning');
ok(
  /state\.health && state\.health\.scheduler/.test(runnerSource),
  'Runner overview reuses health.scheduler'
);
ok(
  /renderOverviewScheduler\(\)/.test(runnerSource) && /el\.overviewGotoQueue/.test(runnerSource),
  'overview renders the scheduler card and the go-to-queue button'
);
ok(/e && e\.status === 409/.test(runnerSource), '409 busy/lease conflicts are named in the temp-run status');
ok(
  /#runner-sched-control/.test(runnerCss) && /#runner-sched-json/.test(runnerCss) &&
    /#runner-run-log/.test(runnerCss),
  'runner.css styles the scheduler control / JSON editor / run log'
);
ok(!/\/scheduler\/reset/.test(runnerSource), 'Runner UI never calls the ops-only /scheduler/reset');

// PR6 fixes: 409 code mapping, stop timeout polling, scheduler history rows,
// fresh task dropdown.
ok(
  /err\.code = String\(body\.error\)/.test(runnerSource),
  'runnerFetchJson keeps the backend error code on the Error object'
);
ok(
  /code === 'device_busy'/.test(runnerSource) && /code === 'run_busy'/.test(runnerSource),
  'the 409 branches use the machine code, not a regex over the human summary'
);
ok(
  /runnerStopText\(body\)/.test(runnerSource) && /runnerStopPending\(body\)/.test(runnerSource),
  'the stop handler maps the response through the pure stop helpers'
);
ok(
  /RUNNER_STOP_POLL_MS = 1000;/.test(runnerSource) &&
    /RUNNER_STOP_POLL_MAX_MS = 30000;/.test(runnerSource) &&
    /function pollStopOnce\(\)/.test(runnerSource) &&
    /startStopPolling\(\)/.test(runnerSource) &&
    /setSchedStopping\(pending\)/.test(runnerSource) &&
    /el\.schedStart\.disabled = schedBusy \|\| schedStopping/.test(runnerSource),
  'a stop timeout polls the scheduler status until running=false (<=30s) and locks the start button'
);
ok(
  /if \(s\.last_stop !== undefined/.test(runnerSource) &&
    /if \(s\.last_error !== undefined/.test(runnerSource) &&
    /runnerSchedHistoryRows\(sched\)/.test(runnerSource),
  'the scheduler cards render last_stop / last_error when present'
);
ok(
  /state\.tasks = await runnerFetchJson\(RUNNER_API\.tasks\)/.test(runnerSource) &&
    !/if \(!state\.tasks\) \{/.test(runnerSource),
  'the queue page refreshes the task dropdown on every load'
);

// Behavioural checks for the PR6 pure helpers.
eq(
  runnerSb.runnerRunBody(' 任务1 ', ''),
  { ok: true, body: { task: '任务1' }, error: '' },
  'blank params -> use the Runner saved values'
);
eq(
  runnerSb.runnerRunBody('任务1', '  {}  '),
  { ok: true, body: { task: '任务1' }, error: '' },
  '{} params -> use the Runner saved values'
);
eq(
  runnerSb.runnerRunBody('任务1', '{"触发文字":"突破"}'),
  { ok: true, body: { task: '任务1', params: { 触发文字: '突破' } }, error: '' },
  'params JSON overrides the saved values'
);
eq(runnerSb.runnerRunBody('', '{}').ok, false, 'missing task is rejected locally');
ok(/选择/.test(runnerSb.runnerRunBody('', '{}').error), 'missing task error says to choose a task');
eq(runnerSb.runnerRunBody('T', '[1]').ok, false, 'array params are rejected');
ok(/JSON 对象/.test(runnerSb.runnerRunBody('T', '[1]').error), 'non-object params error names the expected shape');
eq(runnerSb.runnerRunBody('T', 'null').ok, false, 'null params are rejected');
eq(runnerSb.runnerRunBody('T', '{bad}').ok, false, 'broken JSON is rejected locally');
ok(/合法 JSON/.test(runnerSb.runnerRunBody('T', '{bad}').error), 'broken JSON error explains the parse failure');

eq(runnerSb.runnerRunStatusText({ running: true, task: '任务1' }), '运行中：任务1', 'running status names the task');
eq(
  runnerSb.runnerRunStatusText({ running: false, task: '任务1', outcome: 'Done' }),
  '已结束：任务1 · outcome=Done',
  'finished status shows the outcome'
);
ok(
  /^失败：任务1/.test(runnerSb.runnerRunStatusText({ running: false, task: '任务1', error: 'boom' })),
  'failed status names the task'
);
eq(runnerSb.runnerRunStatusText({}), '—', 'missing run status is a dash');

eq(runnerSb.runnerSchedStatusText({ running: true }), '运行中', 'running scheduler badge');
eq(runnerSb.runnerSchedStatusText({ running: true, stop_requested: true }), '停止中', 'stop-requested scheduler badge');
eq(
  runnerSb.runnerSchedStatusText({ running: false, stop_requested: true, thread_alive: true }),
  '停止中',
  'stopping thread badge'
);
eq(runnerSb.runnerSchedStatusText({ running: false, thread_alive: false }), '已停止', 'stopped scheduler badge');
eq(runnerSb.runnerSchedStatusText(null), '—', 'unknown scheduler status is a dash');
eq(runnerSb.runnerSchedLockPidText({ lock_pid: 1234 }), '1234', 'scheduling lock pid is shown');
eq(runnerSb.runnerSchedLockPidText({}), '—', 'missing lock pid is a dash');
eq(runnerSb.runnerSchedThreadText({ thread_alive: true }), '存活', 'alive scheduler thread');
eq(runnerSb.runnerSchedThreadText({ thread_alive: false }), '已退出', 'dead scheduler thread');
eq(runnerSb.runnerSchedThreadText({}), '—', 'unknown scheduler thread is a dash');

eq(
  runnerSb.runnerSchedJsonBody('{"entries":[]}'),
  { ok: true, body: { config: { entries: [] } }, error: '' },
  'scheduler.json body wraps the raw config document'
);
eq(runnerSb.runnerSchedJsonBody('').ok, false, 'empty scheduler JSON is rejected locally');
ok(/JSON 为空/.test(runnerSb.runnerSchedJsonBody('').error), 'empty JSON error explains the requirement');
eq(runnerSb.runnerSchedJsonBody('[1]').ok, false, 'array scheduler JSON is rejected');
eq(runnerSb.runnerSchedJsonBody('{bad}').ok, false, 'broken scheduler JSON is rejected locally');
eq(runnerSb.runnerErrorLines({ details: ['a', 'b'] }), ['· a', '· b'], 'server errors become one line each');
eq(runnerSb.runnerErrorLines('boom'), ['· boom'], 'plain errors become one line');

// P1-1: the 409 machine code must survive in the summary even when the backend
// also sends a human `errors` array (the pre-fix code preferred `errors`).
const deviceBusyBody = {
  ok: false,
  error: 'device_busy',
  errors: ['Runner 正在调度（pid 1234，surface=runner）'],
  message: 'Runner 正在调度（pid 1234，surface=runner）',
};
eq(
  runnerSb.runnerErrorCodeLabel('device_busy'),
  '设备被占用',
  'device_busy has a fixed Chinese label'
);
eq(
  runnerSb.runnerErrorCodeLabel('scheduler_locked'),
  '调度锁被占用',
  'scheduler_locked has a fixed Chinese label'
);
eq(
  runnerSb.runnerErrorCodeLabel('run_busy'),
  '已有运行在进行',
  'run_busy has a fixed Chinese label'
);
eq(
  runnerSb.runnerBodyErrors(deviceBusyBody),
  '设备被占用：Runner 正在调度（pid 1234，surface=runner）',
  'device_busy summary keeps the code label + message, not the errors array'
);
eq(
  runnerSb.runnerBodyErrors({
    ok: false,
    error: 'scheduler_locked',
    errors: ['another scheduler is already running (lock held by pid 9)'],
    message: 'another scheduler is already running (lock held by pid 9)',
  }),
  '调度锁被占用：another scheduler is already running (lock held by pid 9)',
  'scheduler_locked summary names the lock'
);
eq(
  runnerSb.runnerBodyErrors({
    ok: false,
    error: 'run_busy',
    errors: ['已有任务正在运行：任务1'],
    message: '已有任务正在运行：任务1',
  }),
  '已有运行在进行：已有任务正在运行：任务1',
  'run_busy summary is actionable'
);
eq(
  runnerSb.runnerBodyErrors({ ok: false, error: 'weird', message: 'boom' }),
  'boom',
  'unknown codes fall back to the human message'
);
eq(
  runnerSb.runnerBodyErrors({ ok: false, errors: ['a', 'b'] }),
  'a；b',
  'a body without a code still joins errors'
);
eq(
  runnerSb.runnerBodyErrors({
    ok: false,
    error: 'device_busy',
    message: 'x',
    errors: ['y'],
  }).indexOf('device_busy'),
  -1,
  'the raw machine code is not shown to the user'
);

// P1-2: stop timeout bodies are a "still stopping" state, never a failure.
eq(
  runnerSb.runnerStopText({
    ok: true,
    running: true,
    stopping: true,
    timeout: true,
    message: '调度正在停止（当前 attempt 跑完后退出）',
  }),
  '停止中（当前任务跑完后退出）',
  'stop timeout renders as 停止中'
);
eq(
  runnerSb.runnerStopPending({ ok: true, running: true, stopping: true, timeout: true }),
  true,
  'a stop timeout keeps the status poll running'
);
eq(
  runnerSb.runnerStopText({ ok: true, running: false, stopped: true }),
  '调度已停止',
  'a completed stop renders 调度已停止'
);
eq(
  runnerSb.runnerStopPending({ ok: true, running: false, stopped: true }),
  false,
  'a completed stop ends the status poll'
);
eq(
  runnerSb.runnerStopText({ ok: true, running: false, stopped: true, already_stopped: true }),
  '调度本来就没在运行',
  'a repeated stop on a stopped scheduler is a no-op'
);

// P2: last_error / last_stop are shown only when the backend has them.
eq(runnerSb.runnerSchedHistoryRows({}), [], 'a clean scheduler has no history rows');
eq(
  runnerSb.runnerSchedHistoryRows({ last_stop: 'api_stop', last_error: null }),
  [['最后停止', 'api_stop']],
  'last_stop is rendered when present'
);
eq(
  runnerSb.runnerSchedHistoryRows({ last_error: 'plan missing', last_stop: '' }),
  [['最后错误', 'plan missing']],
  'last_error is rendered when present'
);
eq(
  runnerSb.runnerSchedHistoryRows({ last_stop: 'api_stop', last_error: 'boom' }),
  [['最后停止', 'api_stop'], ['最后错误', 'boom']],
  'both history rows render in order'
);

// --- ResizeObserver / canvas fit contract (no synchronous layout writes) ---
// #canvas-stage is observed, while fitCanvasToStage() writes inline CSS on its
// child canvas. A synchronous fit inside the observer callback re-queues the
// observer in the same frame and Chromium reports
// "ResizeObserver loop completed with undelivered notifications". The callback
// must only schedule a coalesced rAF pass, and a fit that computes the same
// width/height must not touch style at all.
const roBlock = /new ResizeObserver\(([\s\S]*?)\)\.observe\(canvasStage\)/.exec(source);
ok(roBlock, '#canvas-stage ResizeObserver is still registered');
if (roBlock) {
  ok(/scheduleCanvasFit/.test(roBlock[1]), 'ResizeObserver callback defers through scheduleCanvasFit');
  ok(
    !/fitCanvasToStage\s*\(/.test(roBlock[1]),
    'ResizeObserver callback never calls fitCanvasToStage synchronously'
  );
}
ok(
  /window\.addEventListener\('resize', \(\) => scheduleCanvasFit\(\)\)/.test(source),
  'window resize also goes through the rAF scheduler'
);
const scheduleCanvasFitFn = extractFunction('scheduleCanvasFit');
ok(/requestAnimationFrame/.test(scheduleCanvasFitFn), 'scheduleCanvasFit uses requestAnimationFrame');
ok(/canvasFitScheduled/.test(scheduleCanvasFitFn), 'scheduleCanvasFit coalesces pending requests');
const fitCanvasFn = extractFunction('fitCanvasToStage');
ok(
  /if \(w === canvasFitW && h === canvasFitH\) return;/.test(fitCanvasFn),
  'fitCanvasToStage early-returns when the target size did not change'
);
ok(
  /canvas\.style\.width !== wpx/.test(fitCanvasFn) && /canvas\.style\.height !== hpx/.test(fitCanvasFn),
  'fitCanvasToStage only assigns style values that actually changed'
);
const loadFrameFn = extractFunction('loadFrame');
ok(
  /setCanvasFrameSize\(im\.width, im\.height\)/.test(loadFrameFn) &&
    !/fitCanvasToStage\s*\(/.test(loadFrameFn),
  'loadFrame only requests a fit when the intrinsic frame size changes'
);
const applyLiveBlobFn = extractFunction('applyLiveBlob');
ok(
  /setCanvasFrameSize\(im\.width, im\.height\)/.test(applyLiveBlobFn) &&
    !/fitCanvasToStage\s*\(/.test(applyLiveBlobFn),
  'applyLiveBlob does not fit on every live frame'
);

// Behavioural idempotency: stub canvas/stage and count style writes. Two fits
// with identical stage + intrinsic sizes must produce exactly one write pair.
function makeFitHarness() {
  const writes = [];
  const style = {
    _w: '',
    _h: '',
    get width() { return this._w; },
    set width(v) { this._w = v; writes.push(['width', v]); },
    get height() { return this._h; },
    set height(v) { this._h = v; writes.push(['height', v]); },
  };
  const canvas = { width: 1280, height: 720, style };
  const stage = { clientWidth: 800, clientHeight: 600 };
  const sb = { canvasStage: stage, canvas };
  vm.createContext(sb);
  vm.runInContext(
    'let canvasFitW = 0;\nlet canvasFitH = 0;\n' +
      extractFunction('fitCanvasToStage') + '\n' +
      extractFunction('setCanvasFrameSize'),
    sb,
    { filename: 'app.js#canvas-fit' }
  );
  return { sb, canvas, style, stage, writes };
}

const fitH = makeFitHarness();
fitH.sb.fitCanvasToStage();
eq(fitH.writes.map((w) => w.join('=')), ['width=800px', 'height=450px'], 'first fit writes one width/height pair');
fitH.sb.fitCanvasToStage();
eq(fitH.writes.length, 2, 'same stage size never rewrites canvas style (idempotent)');
fitH.stage.clientWidth = 400;
fitH.stage.clientHeight = 600;
fitH.sb.fitCanvasToStage();
eq(fitH.writes.length, 4, 'a real stage resize writes exactly one new width/height pair');
fitH.stage.clientWidth = 10;
fitH.stage.clientHeight = 10;
fitH.sb.fitCanvasToStage();
eq(fitH.writes.length, 4, 'a degenerate hidden stage never writes style');
// Intrinsic size change must be requested explicitly (live frame path).
eq(fitH.sb.setCanvasFrameSize(1280, 720), false, 'same intrinsic frame size does not request a refit');
eq(fitH.sb.setCanvasFrameSize(1920, 1080), true, 'new intrinsic frame size requests a refit');
eq(fitH.sb.setCanvasFrameSize(1920, 1080), false, 'repeated intrinsic size does not request a refit');
fitH.stage.clientWidth = 800;
fitH.stage.clientHeight = 600;
fitH.canvas.width = 720;
fitH.canvas.height = 1280;
fitH.sb.fitCanvasToStage();
eq(fitH.writes.length, 6, 'aspect change writes a new width/height pair');
eq(fitH.style.width, '337px', 'portrait aspect fits the stage height');
eq(fitH.style.height, '600px', 'portrait aspect fills the available height');

// Behavioural coalescing: many schedule calls before the frame produce one fit.
const rafQueue = [];
const schedSb = {
  __fitCalls: 0,
  rafQueue,
  window: {
    requestAnimationFrame(cb) {
      rafQueue.push(cb);
      return rafQueue.length;
    },
  },
};
vm.createContext(schedSb);
vm.runInContext(
  'let canvasFitScheduled = false;\n' +
    'function fitCanvasToStage() { globalThis.__fitCalls += 1; }\n' +
    extractFunction('scheduleCanvasFit'),
  schedSb,
  { filename: 'app.js#canvas-schedule' }
);
schedSb.scheduleCanvasFit();
schedSb.scheduleCanvasFit();
schedSb.scheduleCanvasFit();
eq(rafQueue.length, 1, 'repeated fit requests are coalesced into one rAF callback');
rafQueue.shift()();
eq(schedSb.__fitCalls, 1, 'the coalesced callback runs fit once');
schedSb.scheduleCanvasFit();
eq(rafQueue.length, 1, 'a later request schedules a fresh rAF callback');
rafQueue.shift()();
eq(schedSb.__fitCalls, 2, 'the later fit still runs');

checkAssetTestWiring()
  .then(() => checkSavePathWiring(saveUnified))
  .then(() => checkAssetRoiSavePath())
  .then(() => checkBatchTestScope())
  .then(() => checkAssetThresholdPrompts())
  .then(() => {
    console.log(
      `dom_id_audit: ${referencedIds.length} referenced ids, ${missingIds.length} dead ids` +
      (missingIds.length ? ` (${missingIds.join(', ')})` : '')
    );
    console.log(
      `runner_ui_audit: ${runnerReferencedIds.length} referenced ids, ${runnerMissingIds.length} dead ids, ` +
      `${(runnerNavBlock ? (runnerNavBlock[1].match(/data-page="/g) || []).length : 0)} nav pages`
    );
    console.log(`ui_pure_test: ${checks} checks passed`);
  })
  .catch((err) => {
    console.error(err);
    process.exitCode = 1;
  });
