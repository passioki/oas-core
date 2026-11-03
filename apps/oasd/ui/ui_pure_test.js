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
//   - ResizeObserver-driven canvas fitting staying rAF-coalesced + idempotent.
'use strict';

const fs = require('fs');
const path = require('path');
const vm = require('vm');
const assert = require('assert');

const appPath = path.join(__dirname, 'app.js');
const htmlPath = path.join(__dirname, 'index.html');
const builderPath = path.join(__dirname, 'builder.js');
const source = fs.readFileSync(appPath, 'utf8');
const html = fs.readFileSync(htmlPath, 'utf8');
const builderSource = fs.readFileSync(builderPath, 'utf8');

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
  'assetSourceLabel',
  'frameAssetsSummary',
  'frameAssetIdsByFrame',
  // P1-1: cache-only repaint of the frame rows + current-frame detail after a
  // global-library change (save/delete/positions).
  'renderFrameAssetViews',
  'selectionModeFromFlags',
  // P3: scheduler config editing (pure JSON transforms / builders / validation)
  'schedEntryKindLabel',
  'schedSanitizeIdSegment',
  'schedHasUnsafeNameChar',
  'schedUniqueId',
  'schedEntryIdForTarget',
  'schedEmptyConfig',
  'schedNormalizeConfig',
  'schedDefaultTaskEntry',
  'schedWindowsToText',
  'schedParseWindowsText',
  'schedParseWindowsJson',
  'schedParseParamsText',
  'schedEntryToForm',
  'schedBuildEntryFromForm',
  'schedValidateEntryForm',
  'schedFindTaskEntries',
  'schedFindTaskQueueMember',
  'schedQueueEntryConflict',
  'schedUpsertEntry',
  'schedRemoveEntryById',
  'schedMoveEntry',
  'schedQueueToForm',
  'schedQueueMemberAdd',
  'schedQueueMemberRemove',
  'schedQueueMemberMove',
  'schedBuildQueueFromForm',
  'schedValidateQueueForm',
  'schedUpsertQueue',
  'schedRemoveQueue',
  'schedQueueProgressText',
  'schedQueueSummary',
  'schedEntrySummary',
  'schedQuotaSummary',
  'schedStatusErrorText',
  'schedCanonicalJson',
  'schedConfigSnapshotText',
  'schedConfigSnapshotChanged',
  'schedExternalConfigChangeText',
  'schedResponseErrorText',
  'schedSaveBlockedReason',
  'schedParseConfigJson',
  // P4: scheduler run history (list URL / row formatting / detail / errors)
  'schedRunValueText',
  'schedRunLimitValue',
  'schedRunsBuildQuery',
  'schedRunsBuildUrl',
  'schedRunDetailUrl',
  'schedRunTimeText',
  'schedRunDurationMs',
  'schedRunDurationText',
  'schedRunOutcomeLabel',
  'schedRunOutcomeBadgeClass',
  'schedRunStatusBadgeClass',
  'schedRunKindText',
  'schedRunIsQueueRun',
  'schedQueueDisplayName',
  'schedRunTaskLabel',
  'schedRunRoundMemberText',
  'schedRunAttemptText',
  'schedRunExitText',
  'schedRunClicksText',
  'schedRunErrorText',
  'schedRunErrorSummary',
  'schedRunResumeText',
  'schedRunDetailRows',
  'schedRunDetailTitle',
  'schedRunRowCells',
  'schedRunIsFailure',
  'schedRunsFailureCount',
  'schedRunsStatusCardText',
  'schedRunBodyErrors',
  'schedRunsErrorText',
  'schedRunsResponseErrorText',
  'schedRunDetailErrorText',
  'schedRunsTaskChoices',
  'schedRunsFilterLocal',
  'schedRunsFilterActive',
  'schedRunsListStatusText',
];

const sandbox = {};
vm.createContext(sandbox);
vm.runInContext(PURE_FUNCTIONS.map(extractFunction).join('\n\n'), sandbox, {
  filename: 'app.js#pure',
});

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
  schedEntryKindLabel,
  schedSanitizeIdSegment,
  schedHasUnsafeNameChar,
  schedUniqueId,
  schedEntryIdForTarget,
  schedEmptyConfig,
  schedNormalizeConfig,
  schedDefaultTaskEntry,
  schedWindowsToText,
  schedParseWindowsText,
  schedParseWindowsJson,
  schedParseParamsText,
  schedEntryToForm,
  schedBuildEntryFromForm,
  schedValidateEntryForm,
  schedFindTaskEntries,
  schedFindTaskQueueMember,
  schedQueueEntryConflict,
  schedUpsertEntry,
  schedRemoveEntryById,
  schedMoveEntry,
  schedQueueToForm,
  schedQueueMemberAdd,
  schedQueueMemberRemove,
  schedQueueMemberMove,
  schedBuildQueueFromForm,
  schedValidateQueueForm,
  schedUpsertQueue,
  schedRemoveQueue,
  schedQueueProgressText,
  schedQueueSummary,
  schedEntrySummary,
  schedQuotaSummary,
  schedStatusErrorText,
  schedCanonicalJson,
  schedConfigSnapshotText,
  schedConfigSnapshotChanged,
  schedExternalConfigChangeText,
  schedResponseErrorText,
  schedSaveBlockedReason,
  schedParseConfigJson,
  schedRunValueText,
  schedRunLimitValue,
  schedRunsBuildQuery,
  schedRunsBuildUrl,
  schedRunDetailUrl,
  schedRunTimeText,
  schedRunDurationMs,
  schedRunDurationText,
  schedRunOutcomeLabel,
  schedRunOutcomeBadgeClass,
  schedRunStatusBadgeClass,
  schedRunKindText,
  schedRunIsQueueRun,
  schedQueueDisplayName,
  schedRunTaskLabel,
  schedRunRoundMemberText,
  schedRunAttemptText,
  schedRunExitText,
  schedRunClicksText,
  schedRunErrorText,
  schedRunErrorSummary,
  schedRunResumeText,
  schedRunDetailRows,
  schedRunDetailTitle,
  schedRunRowCells,
  schedRunIsFailure,
  schedRunsFailureCount,
  schedRunsStatusCardText,
  schedRunBodyErrors,
  schedRunsErrorText,
  schedRunsResponseErrorText,
  schedRunDetailErrorText,
  schedRunsTaskChoices,
  schedRunsFilterLocal,
  schedRunsFilterActive,
  schedRunsListStatusText,
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
    'pickAssetRoi', 'runAssetTest', 'editAsset', 'editAssetPositions', 'deleteAsset',
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

// assetRowElement adds the frame-detail source badge without changing the row
// returned by the shared OASAssetUI.createRow (and adds nothing without it).
const savedWindow = sandbox.window;
const savedDocument = sandbox.document;
sandbox.window = {
  OASAssetUI: {
    createRow: () => ({ children: [], appendChild(child) { this.children.push(child); } }),
  },
};
sandbox.document = {
  createElement: () => ({ className: '', textContent: '', title: '', appendChild() {} }),
};
const rowWithBadge = sandbox.assetRowElement({ id: 'a' }, { sourceLabel: '全局/任务' });
eq(rowWithBadge.children.length, 1, 'source badge is appended to the shared row');
eq(rowWithBadge.children[0].textContent, '全局/任务', 'source badge text merges both libraries');
eq(rowWithBadge.children[0].className, 'run-badge kind frame-asset-source', 'source badge reuses the run-badge style');
const rowWithoutBadge = sandbox.assetRowElement({ id: 'b' }, {});
eq(rowWithoutBadge.children.length, 0, 'library rows stay badge-free');
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
  for (const fn of ['pickAssetRoi', 'runAssetTest', 'editAsset', 'editAssetPositions',
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
eq(sandbox.assetSourceLabel(['global', 'task']), '全局/任务', 'source label merges global and task');
eq(sandbox.assetSourceLabel(['task']), '任务', 'source label handles task only');
eq(sandbox.assetSourceLabel([]), '', 'source label is empty without sources');
const frame2Summary = sandbox.frameAssetsSummary(frame2Rows, 2);
eq(frame2Summary.total, 3, 'frame summary counts every row');
eq(frame2Summary.global, 2, 'frame summary counts global rows');
eq(frame2Summary.task, 2, 'frame summary counts task rows');
eq(frame2Summary.text, '该帧素材 3 条（全局 2 · 任务 2）', 'frame summary text keeps the source split');
eq(sandbox.frameAssetsSummary([], 2).text, '该帧没有素材记录', 'frame summary empty state');

// Runtime smoke test for the frame detail list: it must go through the same
// assetRowElement row builder and pass the per-source scope + source label.
function installFrameAssetsRenderHarness() {
  const status = fakeElement();
  const list = fakeElement();
  const calls = [];
  const actions = [];
  sandbox.frameAssetsStatus = status;
  sandbox.frameAssetsList = list;
  sandbox.taskName = { value: '任务甲' };
  sandbox.assetRowElement = (it, opts) => {
    calls.push({ id: it.id, opts });
    return fakeElement();
  };
  // MATCH-BUG: record which frame id each frame-detail row action targets.
  sandbox.runAssetTest = (it, frameId) => actions.push({ action: 'test', id: it.id, frameId });
  sandbox.testAssetClick = (it, frameId) => actions.push({ action: 'click', id: it.id, frameId });
  sandbox.testAssetDrag = (it, frameId) => actions.push({ action: 'drag', id: it.id, frameId });
  sandbox.testAssetCollection = (it, frameId) => actions.push({ action: 'collection', id: it.id, frameId });
  for (const fn of ['pickAssetRoi', 'generateAssetStep', 'deleteCollection']) {
    sandbox[fn] = () => {};
  }
  vm.runInContext(extractFunction('renderFrameAssets'), sandbox, {
    filename: 'app.js#frame-assets-render',
  });
  return { status, list, calls, actions };
}
const frameDom = installFrameAssetsRenderHarness();
const frameDomRows = sandbox.assetsForFrame(frameGlobalAssets, frameTaskAssets, 2);
sandbox.renderFrameAssets({ id: 2 }, frameDomRows);
eq(frameDom.status.textContent, '该帧素材 3 条（全局 2 · 任务 2）', 'frame detail count line');
eq(frameDom.calls.map((c) => c.id), ['a', 'b', 'c'], 'frame detail builds one shared row per asset');
eq(frameDom.calls[0].opts.task, '', 'global rows use the global thumbnail scope');
eq(frameDom.calls[0].opts.sourceLabel, '全局/任务', 'both-source row is badged 全局/任务');
eq(frameDom.calls[1].opts.sourceLabel, '全局', 'global-only row is badged 全局');
eq(frameDom.calls[2].opts.task, '任务甲', 'task-only rows use the task thumbnail scope');
eq(frameDom.calls[2].opts.sourceLabel, '任务', 'task-only row is badged 任务');
ok(frameDom.calls.every((c) => c.opts.manage === false), 'frame rows keep management actions in the libraries');
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
    { action: 'test', id: 'a', frameId: 2 },
    { action: 'click', id: 'a', frameId: 2 },
    { action: 'drag', id: 'a', frameId: 2 },
    { action: 'collection', id: 'a', frameId: 2 },
  ],
  'frame detail row actions all pass the rendered frame id'
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
ok(/testAssetMatch\(it, frameId\)/.test(runAssetTestFn), 'runAssetTest passes the optional frame id to the template test');
ok(/testAssetOcr\(it, frameId\)/.test(runAssetTestFn), 'runAssetTest passes the optional frame id to the OCR test');
for (const fn of ['testAssetMatch', 'testAssetClick', 'testAssetDrag', 'testAssetOcr', 'testAssetCollection']) {
  const body = extractFunction(fn);
  ok(new RegExp(`function ${fn}\\(it, frameId\\)`).test(body), `${fn} accepts an optional frame id`);
  ok(/testFrames\(it, frameId\)/.test(body), `${fn} resolves frames through testFrames(it, frameId)`);
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
  /onTest: \(it\) => runAssetTest\(it, f\.id\)/.test(renderFrameAssetsFn),
  'frame detail 测试 forces the rendered frame id'
);
ok(
  /onClickTest: \(it\) => testAssetClick\(it, f\.id\)/.test(renderFrameAssetsFn) &&
    /onDragTest: \(it\) => testAssetDrag\(it, f\.id\)/.test(renderFrameAssetsFn) &&
    /onCollectionTest: \(it\) => testAssetCollection\(it, f\.id\)/.test(renderFrameAssetsFn),
  'frame detail click/drag/collection tests force the rendered frame id too'
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

// --- P3: scheduler config editing (entry/queue payload + validation) ---
// Entry ids are generated from the task/queue target, sanitized and kept unique.
eq(schedEntryKindLabel('queue'), 'queue', 'entry kind queue');
eq(schedEntryKindLabel(undefined), 'task', 'entry kind defaults to task');
eq(schedSanitizeIdSegment(''), 'entry', 'empty id segment falls back to entry');
eq(schedSanitizeIdSegment('../A:B'), 'A-B', 'path/control characters are replaced and dots trimmed');
eq(schedEntryIdForTarget('task', '每日探索', schedEmptyConfig()), 'task-每日探索', 'task entry id is task-<任务名>');
eq(schedEntryIdForTarget('queue', 'q1', schedEmptyConfig()), 'queue-q1', 'queue entry id is queue-<queue_id>');
const idCfg = schedEmptyConfig();
idCfg.entries = [{ entry_id: 'task-T' }];
eq(schedEntryIdForTarget('task', 'T', idCfg), 'task-T-2', 'generated entry id avoids collisions');
eq(schedUniqueId('x', ['x', 'x-2']), 'x-3', 'unique id keeps incrementing');

const emptyCfg = schedEmptyConfig();
eq(emptyCfg.schema_version, 4, 'empty config is schema 4');
eq(emptyCfg.config.mode, 'loop', 'empty config defaults to loop mode (P2-2)');
eq(emptyCfg.config.loop_interval_s, 60, 'empty config keeps a 60s loop interval');
eq(emptyCfg.entries, [], 'empty config has no entries');
eq(emptyCfg.queues, [], 'empty config has no queues');
eq(schedNormalizeConfig(null), JSON.parse(JSON.stringify(emptyCfg)), 'normalizing a missing config yields the empty document');

const v3Raw = {
  schema_version: 3,
  config: { mode: 'loop', loop_interval_s: 5 },
  unknown_top_key: { keep: true },
  entries: [{ entry_id: 'e1', task: '任务1', lifecycle: 'periodic' }],
};
const v3Norm = schedNormalizeConfig(v3Raw);
eq(v3Norm.schema_version, 4, 'v3 document is normalized to schema 4');
eq(v3Norm.entries[0].kind, 'task', 'v3 entry gains kind=task');
eq(v3Norm.entries[0].windows, [], 'v3 entry gains an empty windows array');
eq(v3Norm.unknown_top_key, { keep: true }, 'normalization preserves unknown top-level keys');
eq(schedNormalizeConfig(v3Norm), JSON.parse(JSON.stringify(v3Norm)), 'config normalization is idempotent');

const defaultEntry = schedDefaultTaskEntry('T', emptyCfg);
eq(defaultEntry, {
  entry_id: 'task-T',
  task: 'T',
  kind: 'task',
  enabled: true,
  lifecycle: 'periodic',
  windows: [],
  quota: { per: 'day', times: 1 },
  params: {},
  retry: 1,
  cooldown: 300,
}, 'new task entry uses the documented defaults (full-day window, quota day/1)');
const customDefaultsCfg = schedEmptyConfig();
customDefaultsCfg.config.defaults = { retry: 2, cooldown: 60 };
eq(schedDefaultTaskEntry('T', customDefaultsCfg).retry, 2, 'new entry inherits config.defaults.retry');
eq(schedDefaultTaskEntry('T', customDefaultsCfg).cooldown, 60, 'new entry inherits config.defaults.cooldown');
eq(schedDefaultTaskEntry('q1', emptyCfg, 'queue').kind, 'queue', 'queue entry builder sets kind=queue');
eq(schedDefaultTaskEntry('q1', emptyCfg, 'queue').entry_id, 'queue-q1', 'queue entry id is queue-<queue_id>');

eq(schedParseWindowsText(''), { ok: true, windows: [], error: '' }, 'empty window text means full day');
eq(schedParseWindowsText('全天').windows, [], '全天 is accepted for an unrestricted window');
eq(schedParseWindowsText('08:00-12:00, 18:00-23:00').windows, [
  { days: [], start: '08:00', end: '12:00' },
  { days: [], start: '18:00', end: '23:00' },
], 'comma separated windows parse');
eq(schedParseWindowsText('[1,3]08:00-12:00; [5]23:00-02:00').windows, [
  { days: [1, 3], start: '08:00', end: '12:00' },
  { days: [5], start: '23:00', end: '02:00' },
], 'weekday prefix and cross-midnight windows parse');
eq(schedParseWindowsText('8:00-9:00').windows, [{ days: [], start: '08:00', end: '09:00' }], 'single digit hours are padded');
eq(schedParseWindowsText('8-9').ok, false, 'malformed window text is rejected');
eq(schedParseWindowsText('[7]08:00-09:00').ok, false, 'weekday 7 is rejected');
eq(schedParseWindowsText('[1]25:00-26:00').ok, false, 'hour 25 is rejected');
const winList = [
  { days: [1, 3], start: '08:00', end: '12:00' },
  { days: [], start: '18:00', end: '23:00' },
];
eq(schedWindowsToText(winList), '[1,3]08:00-12:00, 18:00-23:00', 'windows render back to the simple text');
eq(schedWindowsToText([]), '全天', 'no windows renders as 全天');
eq(schedParseWindowsText(schedWindowsToText(winList)).windows, winList, 'windows text round-trips');
eq(
  schedParseWindowsJson('[{"days":[1],"start":"8:00","end":"9:00"}]').windows,
  [{ days: [1], start: '08:00', end: '09:00' }],
  'windows JSON parses and normalizes HH:MM'
);
eq(schedParseWindowsJson('{}').ok, false, 'windows JSON must be an array');
eq(schedParseWindowsJson('[{"days":[9],"start":"08:00","end":"09:00"}]').ok, false, 'windows JSON validates days');
eq(schedParseWindowsJson('[{"days":[],"start":"bad","end":"09:00"}]').ok, false, 'windows JSON validates HH:MM');

eq(schedParseParamsText(''), { ok: true, params: {}, error: '' }, 'empty params is an empty object');
eq(schedParseParamsText('{"a":1}').params, { a: 1 }, 'params JSON parses');
eq(schedParseParamsText('[1]').ok, false, 'params JSON must be an object');
eq(schedParseParamsText('{bad}').ok, false, 'broken params JSON is rejected');

eq(schedEntryToForm(defaultEntry), {
  entry_id: 'task-T', kind: 'task', task: 'T', enabled: true, lifecycle: 'periodic',
  windows_text: '全天', windows_json: '', quota_per: 'day', quota_times: '1',
  retry: '1', cooldown: '300', params_text: '',
}, 'entry -> form maps every editable field');
eq(schedEntryToForm({ lifecycle: 'temp', entry_id: 'x', task: 'T', enabled: false }).quota_per, '', 'temp form has no quota');

const tempBuilt = schedBuildEntryFromForm({
  kind: 'task', task: 'T', enabled: false, lifecycle: 'temp',
  windows: [{ days: [], start: '08:00', end: '09:00' }], quota_per: 'day', quota_times: '3',
  retry: '2', cooldown: '60', params: { a: 1 },
}, { entry_id: 'e-temp' }, emptyCfg);
eq(tempBuilt.windows, [], 'temp entries clear windows');
eq(tempBuilt.quota, null, 'temp entries clear quota');
eq(tempBuilt.enabled, false, 'entry enabled flag is preserved');
eq(tempBuilt.entry_id, 'e-temp', 'editing keeps the entry_id');
eq(tempBuilt.retry, 2, 'entry form retry is numeric');
eq(tempBuilt.cooldown, 60, 'entry form cooldown is numeric');
eq(tempBuilt.params, { a: 1 }, 'entry form params are preserved');

const periodicBuilt = schedBuildEntryFromForm({
  kind: 'task', task: 'T', enabled: true, lifecycle: 'periodic',
  windows: [{ days: [2], start: '10:00', end: '11:00' }], quota_per: 'week', quota_times: '2',
  retry: '1', cooldown: '300', params: {},
}, {}, emptyCfg);
eq(periodicBuilt.windows, [{ days: [2], start: '10:00', end: '11:00' }], 'periodic keeps windows');
eq(periodicBuilt.quota, { per: 'week', times: 2 }, 'periodic keeps quota');

function schedEntryForm(over) {
  return Object.assign({
    kind: 'task', task: 'T', enabled: true, lifecycle: 'periodic',
    windows_text: '', windows_json: '', quota_per: 'day', quota_times: '1',
    retry: '1', cooldown: '300', params_text: '',
  }, over || {});
}
const validNewEntry = schedValidateEntryForm(schedEntryForm(), emptyCfg, '', null);
eq(validNewEntry.ok, true, 'default form validates');
eq(validNewEntry.entry.entry_id, 'task-T', 'new entry id is generated on save');
eq(validNewEntry.entry.windows, [], 'new entry defaults to a full-day window');
const changedTarget = schedValidateEntryForm(
  schedEntryForm({ task: 'U' }), emptyCfg, '', schedDefaultTaskEntry('T', emptyCfg)
);
eq(changedTarget.entry.entry_id, 'task-U', 'create re-generates the entry_id when the target changes');
const tempValid = schedValidateEntryForm(
  schedEntryForm({ lifecycle: 'temp', windows_text: '08:00-09:00', quota_per: 'day', quota_times: '2' }),
  emptyCfg, '', null
);
eq(tempValid.ok, true, 'temp entry validates even with stale window/quota form values');
eq(tempValid.entry.windows, [], 'validated temp entry drops windows');
eq(tempValid.entry.quota, null, 'validated temp entry drops quota');
eq(schedValidateEntryForm(schedEntryForm({ windows_text: '8-9' }), emptyCfg, '', null).ok, false, 'bad window text fails validation');
eq(schedValidateEntryForm(schedEntryForm({ quota_per: 'day', quota_times: '0' }), emptyCfg, '', null).ok, false, 'quota times 0 fails validation');
eq(schedValidateEntryForm(schedEntryForm({ retry: '11' }), emptyCfg, '', null).ok, false, 'retry > 10 fails validation');
eq(schedValidateEntryForm(schedEntryForm({ cooldown: '604801' }), emptyCfg, '', null).ok, false, 'cooldown above the max fails validation');
eq(schedValidateEntryForm(schedEntryForm({ params_text: '[1]' }), emptyCfg, '', null).ok, false, 'non-object params fail validation');
eq(schedValidateEntryForm(schedEntryForm({ windows_json: '{bad}' }), emptyCfg, '', null).ok, false, 'broken windows JSON fails validation');

const queueCfgForEntry = schedEmptyConfig();
queueCfgForEntry.queues = [{ queue_id: 'q1', name: '日常', members: ['T1'], total_rounds: 0, on_failure: 'continue', between_ms: 500 }];
const unknownQueue = schedValidateEntryForm(schedEntryForm({ kind: 'queue', task: 'nope' }), queueCfgForEntry, '', null);
eq(unknownQueue.ok, false, 'queue entry requires an existing queue');
ok(unknownQueue.errors.some((e) => /不存在/.test(e)), 'unknown queue error names the missing queue');
const firstQueueEntry = { entry_id: 'qe', task: 'q1', kind: 'queue', enabled: true, lifecycle: 'periodic', windows: [], quota: null, params: {}, retry: 0, cooldown: 0 };
queueCfgForEntry.entries = [firstQueueEntry];
const secondQueueEntry = schedValidateEntryForm(schedEntryForm({ kind: 'queue', task: 'q1' }), queueCfgForEntry, '', null);
eq(secondQueueEntry.ok, false, 'a queue cannot get a second trigger entry');
ok(secondQueueEntry.errors.some((e) => e.includes('qe')), 'queue conflict names the existing entry');
eq(schedQueueEntryConflict(queueCfgForEntry, 'q1', 'qe'), '', 'the edited entry does not conflict with itself');
eq(schedQueueEntryConflict(queueCfgForEntry, 'q1', ''), 'qe', 'queue conflict finds the other entry');
const overlappingCfg = schedEmptyConfig();
overlappingCfg.entries = [{ entry_id: 'te', task: 'T', kind: 'task' }];
const overlap = schedValidateEntryForm(schedEntryForm(), overlappingCfg, '', null);
eq(overlap.ok, true, 'a second task entry for the same task is allowed (only queue overlap is forbidden)');
const queueMemberOverlap = schedEmptyConfig();
queueMemberOverlap.queues = [{ queue_id: 'q1', members: ['T'], total_rounds: 0, on_failure: 'continue', between_ms: 0 }];
const memberOverlap = schedValidateEntryForm(schedEntryForm(), queueMemberOverlap, '', null);
ok(memberOverlap.ok === false && memberOverlap.errors.some((e) => e.includes('q1')), 'task entry cannot duplicate a queue member');

eq(schedFindTaskEntries(queueCfgForEntry, 'T1'), [], 'queue entry is not a task entry');
eq(schedFindTaskEntries(overlappingCfg, 'T').map((e) => e.entry_id), ['te'], 'find task entries by task');
eq(schedFindTaskQueueMember(queueMemberOverlap, 'T'), 'q1', 'find the queue containing a task');
eq(schedFindTaskQueueMember(queueMemberOverlap, 'other'), '', 'task not in any queue');

let upsertCfg = schedEmptyConfig();
upsertCfg = schedUpsertEntry(upsertCfg, firstQueueEntry);
upsertCfg = schedUpsertEntry(upsertCfg, Object.assign({}, firstQueueEntry, { enabled: false }));
eq(upsertCfg.entries.length, 1, 'upsert replaces by entry_id instead of appending');
eq(upsertCfg.entries[0].enabled, false, 'upsert applies the new entry body');
const untouchedCfg = schedEmptyConfig();
schedUpsertEntry(untouchedCfg, firstQueueEntry);
eq(untouchedCfg.entries.length, 0, 'upsert does not mutate the input config');
eq(schedRemoveEntryById(upsertCfg, 'qe').entries, [], 'remove entry by id');

function schedTestEntry(id) {
  return { entry_id: id, task: id, kind: 'task', enabled: true, lifecycle: 'periodic', windows: [], quota: null, params: {}, retry: 1, cooldown: 300 };
}
const moveCfg = schedEmptyConfig();
moveCfg.entries = [schedTestEntry('a'), schedTestEntry('b'), schedTestEntry('c')];
eq(schedMoveEntry(moveCfg, 0, 1).entries.map((e) => e.entry_id), ['b', 'a', 'c'], 'move entry down swaps order');
eq(schedMoveEntry(moveCfg, 2, -1).entries.map((e) => e.entry_id), ['a', 'c', 'b'], 'move entry up swaps order');
eq(schedMoveEntry(moveCfg, 0, -1).entries.map((e) => e.entry_id), ['a', 'b', 'c'], 'move at the edge is a no-op');
eq(moveCfg.entries.map((e) => e.entry_id), ['a', 'b', 'c'], 'move entry does not mutate the input');

eq(schedQueueToForm({ queue_id: 'q1', name: '日常', members: ['A'], total_rounds: 2, on_failure: 'fail_fast', between_ms: 500 }), {
  queue_id: 'q1', name: '日常', members: ['A'], total_rounds: '2', on_failure: 'fail_fast', between_ms: '500',
}, 'queue -> form maps every editable field');
eq(schedQueueMemberAdd(['A'], 'B'), ['A', 'B'], 'queue member add appends');
eq(schedQueueMemberAdd(['A'], 'A'), ['A'], 'queue member add ignores duplicates');
eq(schedQueueMemberRemove(['A', 'B'], 'A'), ['B'], 'queue member remove');
eq(schedQueueMemberMove(['A', 'B', 'C'], 2, -1), ['A', 'C', 'B'], 'queue member move up');
eq(schedQueueMemberMove(['A', 'B', 'C'], 0, 1), ['B', 'A', 'C'], 'queue member move down');
eq(schedQueueMemberMove(['A'], 0, -1), ['A'], 'queue member move at the edge is a no-op');
eq(schedBuildQueueFromForm({
  queue_id: ' q1 ', name: '', members: [' A ', 'A', ''], total_rounds: '3', on_failure: 'bogus', between_ms: '700',
}, {}), {
  queue_id: 'q1', name: '', members: ['A', 'A'], total_rounds: 3, on_failure: 'continue', between_ms: 700,
}, 'queue form trims ids/members and defaults on_failure to continue');

const queueForm = { queue_id: 'q1', name: '日常', members: ['T1', 'T2'], total_rounds: '2', on_failure: 'continue', between_ms: '500' };
const validQueue = schedValidateQueueForm(queueForm, schedEmptyConfig(), '', null);
eq(validQueue.ok, true, 'queue form validates');
eq(validQueue.queue.total_rounds, 2, 'queue total_rounds is numeric');
const noMembers = schedValidateQueueForm(Object.assign({}, queueForm, { members: [] }), schedEmptyConfig(), '', null);
eq(noMembers.ok, false, 'queue needs at least one member');
const dupQueueCfg = schedEmptyConfig();
dupQueueCfg.queues = [{ queue_id: 'q1', members: ['T1'] }];
eq(schedValidateQueueForm(queueForm, dupQueueCfg, '', null).ok, false, 'duplicate queue_id fails validation');
const queueOverlapCfg = schedEmptyConfig();
queueOverlapCfg.entries = [{ entry_id: 'te', task: 'T1', kind: 'task' }];
const queueOverlap = schedValidateQueueForm(queueForm, queueOverlapCfg, '', null);
ok(queueOverlap.ok === false && queueOverlap.errors.some((e) => e.includes('te')), 'queue member already scheduled as a task entry is rejected');
eq(schedValidateQueueForm(Object.assign({}, queueForm, { total_rounds: '-1' }), schedEmptyConfig(), '', null).ok, false, 'negative total_rounds fails validation');
eq(schedValidateQueueForm(Object.assign({}, queueForm, { between_ms: '700000' }), schedEmptyConfig(), '', null).ok, false, 'between_ms above the max fails validation');
eq(schedValidateQueueForm(Object.assign({}, queueForm, { queue_id: 'q2' }), schedEmptyConfig(), 'q1', null).ok, false, 'queue_id cannot change while editing');
eq(schedValidateQueueForm(Object.assign({}, queueForm, { members: ['T1', 'T1'] }), schedEmptyConfig(), '', null).ok, false, 'duplicate members fail validation');

const queueUpsert = schedUpsertQueue(schedEmptyConfig(), { queue_id: 'q1', name: 'A', members: ['T1'], total_rounds: 0, on_failure: 'continue', between_ms: 0 });
eq(schedUpsertQueue(queueUpsert, { queue_id: 'q1', name: 'B', members: ['T2'], total_rounds: 1, on_failure: 'fail_fast', between_ms: 100 }).queues.length, 1, 'queue upsert replaces by queue_id');
const removeQueueCfg = schedEmptyConfig();
removeQueueCfg.queues = [{ queue_id: 'q1', members: ['T1'], total_rounds: 0, on_failure: 'continue', between_ms: 0 }];
removeQueueCfg.entries = [
  { entry_id: 'qe', task: 'q1', kind: 'queue' },
  { entry_id: 'te', task: 'T2', kind: 'task' },
];
const removedQueue = schedRemoveQueue(removeQueueCfg, 'q1');
eq(removedQueue.removed_entry_ids, ['qe'], 'removing a queue reports the removed trigger entry');
eq(removedQueue.config.queues, [], 'removing a queue drops it');
eq(removedQueue.config.entries.map((e) => e.entry_id), ['te'], 'removing a queue keeps unrelated entries');
eq(schedRemoveQueue(removeQueueCfg, 'q1').config.entries.length, 1, 'removing a queue leaves one unrelated entry');
eq(removeQueueCfg.entries.length, 2, 'remove queue does not mutate the input config');

eq(schedQueueProgressText({ total_rounds: 0 }, { rounds_completed: 3, empty_rounds: 1, current_round: { index: 4, member_index: 1 } }),
  '轮次 3/无限 · 空轮 1 · 进行中：第 4 轮 · 成员 2', 'queue progress shows unlimited rounds + current member');
eq(schedQueueProgressText({ total_rounds: 2 }, { rounds_completed: 2, empty_rounds: 0, finished: true }),
  '轮次 2/2 · 已完成', 'queue progress shows finished');
const queueSummaryText = schedQueueSummary(
  { queue_id: 'q1', members: ['A', 'B'], total_rounds: 2, on_failure: 'fail_fast', between_ms: 500 },
  { rounds_completed: 1, empty_rounds: 0 }
);
ok(/成员 2：A → B/.test(queueSummaryText), 'queue summary lists ordered members');
ok(/失败策略 fail_fast/.test(queueSummaryText), 'queue summary names the failure policy');
ok(/轮次 1\/2/.test(queueSummaryText), 'queue summary includes round progress');

eq(schedEntrySummary({ enabled: true, lifecycle: 'temp', retry: 0, cooldown: 0 }, {}),
  '已启用 · temp（每 pass 一次） · retry 0 · 冷却 0s', 'temp entry summary omits windows/quota');
eq(
  schedEntrySummary(
    { enabled: false, lifecycle: 'periodic', windows: [], quota: { per: 'day', times: 2 }, retry: 1, cooldown: 300, params: { a: 1 } },
    { last_result: 'success', last_outcome: 'Done', blocked_reason: 'quota_exhausted' }
  ),
  '已禁用 · periodic · 窗口 全天 · quota 日 2 · retry 1 · 冷却 300s · params {"a":1} · 上次 success(Done) · 等待 quota_exhausted',
  'periodic entry summary includes window/quota/last result'
);

eq(schedQuotaSummary({ entries: [] }), 'quota：无限制', 'quota summary with no quota');
eq(schedQuotaSummary({ entries: [
  { quota: { per: 'day', times: 2, used: 1 } },
  { quota: { per: 'week', times: 3, used: 2 } },
  { quota: null },
] }), 'quota：2 条限制 · 本周期已用 3/5', 'quota summary aggregates used/times');

// P1-3: status read failures have a fixed message and must never look healthy.
eq(schedStatusErrorText('HTTP 500'), '调度器状态读取失败：HTTP 500', 'status error text keeps the detail');
eq(schedStatusErrorText(''), '调度器状态读取失败：未知错误', 'empty status error falls back to 未知错误');
eq(schedStatusErrorText(null), '调度器状态读取失败：未知错误', 'missing status error falls back to 未知错误');

// P1-4: canonical snapshots ignore object key order but see any value change.
eq(
  schedConfigSnapshotText({ b: 1, a: { d: 2, c: [3, { y: 1, x: 2 }] } }),
  schedConfigSnapshotText({ a: { c: [3, { x: 2, y: 1 }], d: 2 }, b: 1 }),
  'config snapshot is canonical over object key order'
);
eq(schedConfigSnapshotText(null), '', 'a missing config file snapshots as an empty string');
ok(
  !schedConfigSnapshotChanged(
    schedConfigSnapshotText({ schema_version: 4, entries: [] }),
    schedConfigSnapshotText({ entries: [], schema_version: 4 })
  ),
  'same content in a different key order is not an external change'
);
ok(
  schedConfigSnapshotChanged(
    schedConfigSnapshotText({ entries: [] }),
    schedConfigSnapshotText({ entries: [{ entry_id: 'e1' }] })
  ),
  'a value change is detected as an external change'
);
ok(!schedConfigSnapshotChanged('', ''), 'two missing files do not count as an external change');
ok(
  schedConfigSnapshotChanged('', schedConfigSnapshotText({ entries: [] })),
  'a file created outside the page counts as an external change'
);
ok(/配置已被外部修改/.test(schedExternalConfigChangeText()), 'external change text says the config was modified');
ok(/重新加载/.test(schedExternalConfigChangeText()), 'external change text asks the user to reload');

ok(/调度器运行中/.test(schedResponseErrorText(409, { errors: ['lock held'] })), '409 text says the scheduler is running');
ok(/stop-scheduler\.bat/.test(schedResponseErrorText(409, null)), '409 text tells the user to stop the daemon');
ok(/lock held/.test(schedResponseErrorText(409, { errors: ['lock held'] })), '409 text includes server errors');
ok(/配置校验失败/.test(schedResponseErrorText(400, { errors: ['entries[0].task must not be empty'] })), '400 text includes validation errors');
ok(/写入失败/.test(schedResponseErrorText(500, null)), '500 text says the write failed');
ok(/\/scheduler\/config/.test(schedResponseErrorText(404, null)), '404 text names the missing route');
ok(/网络错误/.test(schedResponseErrorText(0, null)), 'network error text');
eq(schedSaveBlockedReason(false), '', 'no blocked-save reason when the scheduler is idle');
ok(/409/.test(schedSaveBlockedReason(true)) && /stop-scheduler/.test(schedSaveBlockedReason(true)), 'running scheduler warns that save will 409');

eq(schedParseConfigJson('').ok, false, 'empty JSON text is rejected');
eq(schedParseConfigJson('{bad}').ok, false, 'broken JSON text is rejected');
eq(schedParseConfigJson('[1]').ok, false, 'JSON array config is rejected');
eq(schedParseConfigJson('{"entries":{}}').ok, false, 'entries must be an array');
const roundTripCfg = schedNormalizeConfig(v3Raw);
eq(
  schedParseConfigJson(JSON.stringify(roundTripCfg)).config,
  JSON.parse(JSON.stringify(roundTripCfg)),
  'config JSON round-trips through parse/normalize'
);

// --- P4: scheduler run history (list URL / row formatting / detail / errors) ---
const runCfg = schedEmptyConfig();
runCfg.queues = [{
  queue_id: 'q1', name: '日常', members: ['任务2', '任务3'], total_rounds: 2,
  on_failure: 'continue', between_ms: 500,
}];
runCfg.entries = [
  schedDefaultTaskEntry('任务1', runCfg),
  schedDefaultTaskEntry('q1', runCfg, 'queue'),
];
const taskRun = {
  run_id: 'run-1', file: 'run-1.json', task: '任务1', kind: 'task', entry_id: 'task-任务1',
  queue_id: '', round: null, member_index: null, attempt: 1,
  started_ms: 1762100000000, finished_ms: 1762100001234, duration_ms: 1234,
  class: 'success', outcome: 'Done', exit: 'done', exit_code: 0, clicks: 12,
  error: '', resume: true, clicks_file: 'runs/clicks/run-1.clicks.jsonl',
};
const queueRun = {
  run_id: 'run-2', file: 'run-2.json', task: '任务2', kind: 'queue_member', entry_id: 'queue-q1',
  queue_id: 'q1', round: 2, member_index: 0, attempt: 2,
  started_ms: 1762100100000, finished_ms: 1762100102500, duration_ms: 2500,
  class: 'success', outcome: 'Done', exit: 'done', exit_code: 0, clicks: 3,
  error: '', resume: false, clicks_file: 'runs/clicks/run-2.clicks.jsonl',
};

// limit 校验 / URL 构造（task/outcome 编码，空筛选省略）。
eq(schedRunLimitValue(undefined), 200, 'run list limit defaults to 200');
eq(schedRunLimitValue('500'), 500, 'a string limit value is accepted');
eq(schedRunLimitValue(300), 200, 'an unsupported limit falls back to 200');
eq(schedRunLimitValue(0), 200, 'limit 0 falls back to 200');
eq(schedRunsBuildQuery({}), 'limit=200', 'empty run filter only sends the default limit');
eq(
  schedRunsBuildQuery({ limit: 1000, task: ' 任务 A&1 ', outcome: 'Failed' }),
  'limit=1000&task=' + encodeURIComponent('任务 A&1') + '&outcome=Failed',
  'task is trimmed and URL-encoded; limit/outcome appended'
);
eq(schedRunsBuildQuery({ task: '', outcome: '' }), 'limit=200', 'empty task/outcome are omitted');
eq(schedRunsBuildQuery({ outcome: 'weird' }), 'limit=200', 'an unknown outcome is not sent');
eq(
  schedRunsBuildQuery({ outcome: 'FATAL' }),
  'limit=200',
  'class words are filtered locally (the list endpoint only filters persisted outcome)'
);
eq(schedRunsBuildQuery({ outcome: 'failed' }), 'limit=200&outcome=Failed', 'persisted outcome keeps its server casing');
eq(schedRunsBuildQuery({ outcome: 'Done' }), 'limit=200&outcome=Done', 'Done outcome is sent verbatim');
eq(schedRunsBuildQuery({ queueId: 'q1' }), 'limit=200', 'the local queue filter is never sent to the server');
eq(schedRunsBuildUrl({ limit: 500, task: 'x' }), '/runs/scheduler?limit=500&task=x', 'run list URL points at /runs/scheduler');
eq(
  schedRunDetailUrl('run 1/2'),
  '/runs/scheduler/run?run_id=' + encodeURIComponent('run 1/2'),
  'detail URL encodes run_id'
);
eq(schedRunDetailUrl(null), '/runs/scheduler/run?run_id=', 'empty detail run_id stays empty');

// 时间 / duration 格式化。
eq(schedRunTimeText(0), '—', 'missing run time renders as a dash');
eq(schedRunTimeText(null), '—', 'null run time renders as a dash');
eq(schedRunTimeText(''), '—', 'empty run time renders as a dash');
eq(
  schedRunTimeText(1762100000000),
  new Date(1762100000000).toLocaleString('zh-CN', { hour12: false }),
  'run time is localized'
);
eq(schedRunDurationText(850), '850ms', 'sub-second duration stays in ms');
eq(schedRunDurationText(1234), '1.2s', 'seconds duration keeps one decimal');
eq(schedRunDurationText(65000), '1m5s', 'minute duration is human readable');
eq(schedRunDurationText(3725000), '1h2m', 'hour duration is human readable');
eq(schedRunDurationText(null), '—', 'missing duration renders as a dash');
eq(schedRunDurationText(-5), '—', 'negative duration renders as a dash');
eq(schedRunDurationMs({ duration_ms: 0 }), 0, 'duration_ms=0 is kept (not treated as missing)');
eq(schedRunDurationMs({ started_ms: 1000, finished_ms: 3500 }), 2500, 'duration falls back to finished-started');
eq(schedRunDurationMs({}), null, 'missing duration and timestamps yield null');

// outcome 徽章标签/颜色 + kind。
eq(schedRunOutcomeLabel('success'), '成功', 'success label');
eq(schedRunOutcomeLabel('resource'), '资源不足', 'resource label');
eq(schedRunOutcomeLabel('blocked'), '被阻断', 'blocked label');
eq(schedRunOutcomeLabel('retryable'), '可重试', 'retryable label');
eq(schedRunOutcomeLabel('fatal'), '致命', 'fatal label');
eq(schedRunOutcomeLabel('Done'), '完成', 'persisted Done outcome label');
eq(schedRunOutcomeLabel('Failed'), '失败', 'persisted Failed outcome label');
eq(schedRunOutcomeLabel('Exhausted'), '耗尽', 'persisted Exhausted outcome label');
eq(schedRunOutcomeLabel('error'), '错误', 'persisted error outcome label');
eq(schedRunOutcomeLabel(''), '未知', 'missing outcome label');
eq(schedRunOutcomeLabel('weird'), 'weird', 'unknown outcome keeps its raw value');
eq(schedRunOutcomeBadgeClass('success'), 'ok', 'success badge is green');
eq(schedRunOutcomeBadgeClass('fatal'), 'err', 'fatal badge is red');
eq(schedRunOutcomeBadgeClass('retryable'), 'warn', 'retryable badge is yellow');
eq(schedRunOutcomeBadgeClass('Failed'), 'err', 'persisted Failed outcome badge is red');
eq(schedRunOutcomeBadgeClass('Done'), 'ok', 'persisted Done outcome badge is green');
eq(schedRunOutcomeBadgeClass('Exhausted'), 'warn', 'persisted Exhausted outcome badge is yellow');
eq(schedRunOutcomeBadgeClass(''), 'kind', 'unknown badge uses the neutral class');
eq(schedRunStatusBadgeClass({ class: 'fatal', outcome: 'Failed' }), 'err', 'class wins over outcome for colour');
eq(schedRunStatusBadgeClass({ class: '', outcome: 'Failed' }), 'err', 'outcome colours older runs without class');
eq(schedRunKindText('queue'), 'queue', 'queue run kind');
eq(schedRunKindText('queue_member'), 'queue_member', 'P4A queue_member kind is preserved');
eq(schedRunKindText(''), 'task', 'missing kind defaults to task');
eq(schedRunIsQueueRun({ kind: 'queue' }), true, 'kind=queue is a queue run');
eq(schedRunIsQueueRun({ kind: 'queue_member' }), true, 'kind=queue_member is a queue run');
eq(schedRunIsQueueRun({ kind: 'task' }), false, 'kind=task is not a queue run');

// queue member naming: 队列名/成员名, queue_id fallback.
eq(schedQueueDisplayName(runCfg, 'q1'), '日常', 'queue display name comes from config');
eq(schedQueueDisplayName(runCfg, 'nope'), 'nope', 'unknown queue falls back to its id');
eq(schedRunTaskLabel({ task: '任务1', kind: 'task' }, runCfg), '任务1', 'task run label is the task name');
eq(schedRunTaskLabel(queueRun, runCfg), '日常/任务2', 'queue member label is 队列名/成员名');
eq(
  schedRunTaskLabel({ task: '任务2', kind: 'queue', queue_id: 'qX' }, runCfg),
  'qX/任务2',
  'queue member label falls back to queue_id'
);
eq(schedRunRoundMemberText(queueRun), '第2轮 · 成员1', 'queue round/member is 1-based for display');
eq(schedRunRoundMemberText({ kind: 'queue', round: 1, member_index: 2 }), '第1轮 · 成员3', 'member_index is 0-based');
eq(schedRunRoundMemberText(taskRun), '—', 'task runs have no round/member');
eq(schedRunAttemptText({ attempt: 2 }), '2', 'attempt is rendered');
eq(schedRunAttemptText({}), '—', 'missing attempt is a dash');
eq(schedRunExitText({ exit_code: 0, exit: 'done' }), '0', 'exit_code wins over exit string');
eq(schedRunExitText({ exit: 'timeout' }), 'timeout', 'exit string is used when exit_code is absent');
eq(schedRunExitText({}), '—', 'missing exit is a dash');
eq(schedRunClicksText({ clicks: 12 }), '12', 'clicks count is rendered');
eq(schedRunClicksText({}), '—', 'missing clicks is a dash');

// 错误文本 / 列表与详情读取错误。
eq(schedRunErrorText('  boom \n'), 'boom', 'error text is trimmed');
eq(schedRunErrorText({ code: 1 }), '{"code":1}', 'object error is JSON encoded');
eq(schedRunErrorSummary(null), '', 'missing error summary is empty');
eq(schedRunErrorSummary('a\nb\tc'), 'a b c', 'error summary collapses whitespace');
eq(schedRunErrorSummary('x'.repeat(20), 10), 'xxxxxxxxx…', 'error summary truncates with an ellipsis');
eq(schedRunsErrorText('HTTP 500'), '运行记录读取失败：HTTP 500', 'list read error has the required prefix');
eq(
  schedRunsResponseErrorText(500, { errors: ['disk full'] }),
  '运行记录读取失败：HTTP 500：disk full',
  'list HTTP error includes server errors'
);
ok(/P4A/.test(schedRunsResponseErrorText(404, null)), '404 list error names the missing P4A route');
eq(
  schedRunDetailErrorText(404, null),
  '详情读取失败：运行记录不存在或已被删除（HTTP 404）',
  'detail 404 explains the missing run'
);
eq(schedRunDetailErrorText(400, null), '详情读取失败：run_id 无效（HTTP 400）', 'detail 400 explains bad run_id');
ok(/网络错误/.test(schedRunDetailErrorText(0, { errors: ['x'] })), 'detail network error is explicit');

// 行格式化：时间/任务/kind/轮次/attempt/outcome/exit/duration/clicks/error。
const taskCells = schedRunRowCells(taskRun, runCfg);
eq(taskCells.task, '任务1', 'row task label');
eq(taskCells.kind, 'task', 'row kind badge');
eq(taskCells.round_member, '—', 'task row has no round/member');
eq(taskCells.attempt, '1', 'row attempt');
eq(taskCells.class, 'success', 'row class');
eq(taskCells.outcome, '完成', 'row outcome label uses the persisted Done value');
eq(taskCells.outcome_class, 'ok', 'row outcome color');
eq(taskCells.exit, '0', 'row exit');
eq(taskCells.duration, '1.2s', 'row duration');
eq(taskCells.clicks, '12', 'row clicks');
eq(taskCells.error, '', 'success row has no error summary');
const queueCells = schedRunRowCells(queueRun, runCfg);
eq(queueCells.task, '日常/任务2', 'queue row task label');
eq(queueCells.kind, 'queue_member', 'P4A queue_member row kind');
eq(queueCells.round_member, '第2轮 · 成员1', 'queue row round/member');
eq(queueCells.outcome, '完成', 'queue row outcome uses the persisted Done value');
const fatalRun = {
  run_id: 'run-3', task: '任务1', kind: 'task', attempt: 3, duration_ms: 900,
  class: 'fatal', outcome: 'fatal', exit: 'error', exit_code: 1, clicks: 0,
  error: '第一行\n第二行',
};
const fatalCells = schedRunRowCells(fatalRun, runCfg);
eq(fatalCells.outcome, '致命', 'fatal row outcome');
eq(fatalCells.outcome_class, 'err', 'fatal row is red');
eq(fatalCells.exit, '1', 'fatal row exit_code');
eq(fatalCells.error, '第一行 第二行', 'fatal row error summary is one line');
eq(fatalCells.error_full, '第一行\n第二行', 'fatal row keeps the full error text');

// 详情字段（含 resume / clicks_file / started / finished / extras）。
const detailRows = schedRunDetailRows(taskRun);
const detailMap = {};
for (const [key, value] of detailRows) detailMap[key] = value;
eq(detailMap.run_id, 'run-1', 'detail run_id');
eq(detailMap.kind, 'task', 'detail kind');
eq(detailMap.attempt, '1', 'detail attempt');
eq(detailMap.outcome, '完成（Done）', 'detail outcome label + raw persisted value');
eq(detailMap.duration_ms, '1234（1.2s）', 'detail duration shows raw + readable');
ok(detailMap.started_ms.includes('（') && detailMap.started_ms.startsWith('1762100000000'), 'detail started_ms shows raw + local');
ok(detailMap.finished_ms.includes('（') && detailMap.finished_ms.startsWith('1762100001234'), 'detail finished_ms shows raw + local');
eq(detailMap.resume, '是', 'detail resume=true is 是');
eq(schedRunResumeText({ resume: false }), '否', 'resume=false is 否');
eq(schedRunResumeText({}), '—', 'missing resume is a dash');
eq(detailMap.clicks_file, 'runs/clicks/run-1.clicks.jsonl', 'detail clicks_file');
eq(detailMap.error, '—', 'detail empty error is a dash');
const extraRows = schedRunDetailRows(Object.assign({}, taskRun, { future_field: 'kept' }));
ok(extraRows.some(([k, v]) => k === 'future_field' && v === 'kept'), 'unknown run fields are appended to the detail');
ok(
  /日常\/任务2/.test(schedRunDetailTitle(queueRun, runCfg)) &&
    /完成/.test(schedRunDetailTitle(queueRun, runCfg)),
  'detail title summarizes task + persisted outcome'
);

// 任务下拉来源：status entries/task_choices + config entries/queue members。
eq(
  schedRunsTaskChoices({
    task_choices: ['b', 'a'],
    entries: [{ entry_id: 'task-a', kind: 'task', task: 'a' }, { entry_id: 'queue-q1', kind: 'queue', task: 'q1' }],
  }, runCfg),
  ['b', 'a', '任务1', '任务2', '任务3'],
  'task filter choices are merged from status/config without duplicates'
);

// 本地筛选（排序保持） + 状态文本。
const orderedRuns = [queueRun, fatalRun, taskRun];
eq(
  schedRunsFilterLocal(orderedRuns, {}).map((r) => r.run_id),
  ['run-2', 'run-3', 'run-1'],
  'local filtering preserves newest→oldest order'
);
eq(
  schedRunsFilterLocal(orderedRuns, { task: ' 任务1 ' }).map((r) => r.run_id),
  ['run-3', 'run-1'],
  'local task filter trims and preserves order'
);
eq(
  schedRunsFilterLocal(orderedRuns, { outcome: 'fatal' }).map((r) => r.run_id),
  ['run-3'],
  'local outcome filter'
);
eq(
  schedRunsFilterLocal([taskRun, fatalRun, Object.assign({}, queueRun, { class: 'success' })], { outcome: 'success' })
    .map((r) => r.run_id),
  ['run-1', 'run-2'],
  'local class word matches class=success even when the persisted outcome is Done'
);
eq(
  schedRunsFilterLocal([taskRun, Object.assign({}, fatalRun, { outcome: 'Failed' })], { outcome: 'Failed' })
    .map((r) => r.run_id),
  ['run-3'],
  'local filter matches the persisted outcome too'
);
eq(
  schedRunsFilterLocal(orderedRuns, { queueId: 'q1' }).map((r) => r.run_id),
  ['run-2'],
  'local queue filter selects queue members'
);
eq(schedRunsFilterActive({}), false, 'empty filter is inactive');
eq(schedRunsFilterActive({ task: 'x' }), true, 'task filter is active');
eq(schedRunsFilterActive({ queueId: 'q1' }), true, 'queue filter is active');
eq(schedRunIsFailure({ outcome: 'success' }), false, 'success is not a failure');
eq(schedRunIsFailure({ outcome: 'fatal' }), true, 'fatal is a failure');
eq(schedRunIsFailure({ class: 'success', outcome: 'Failed' }), false, 'class=success wins over an odd outcome');
eq(schedRunIsFailure({ class: 'resource' }), true, 'a non-success class is a failure');
eq(schedRunIsFailure({ outcome: 'Done' }), false, 'persisted Done is not a failure');
eq(schedRunIsFailure({ outcome: 'Failed' }), true, 'persisted Failed is a failure');
eq(schedRunIsFailure({}), false, 'missing outcome is not counted as a failure');
eq(schedRunsFailureCount([taskRun, fatalRun, { outcome: 'retryable' }, {}]), 2, 'failure count excludes success/missing');
eq(schedRunsStatusCardText(null, ''), '运行记录：—', 'status card shows the unloaded state');
eq(schedRunsStatusCardText({ runs: [] }, ''), '运行记录：暂无', 'status card shows the empty state');
eq(
  schedRunsStatusCardText(null, '运行记录读取失败：HTTP 500'),
  '运行记录：读取失败（见下方运行记录区）',
  'status card shows the read failure state'
);
ok(
  /最近 完成/.test(schedRunsStatusCardText({ runs: [taskRun, fatalRun] }, '')) &&
    /失败 1\/2/.test(schedRunsStatusCardText({ runs: [taskRun, fatalRun] }, '')),
  'status card summarizes the latest persisted outcome and failure count'
);
eq(schedRunsListStatusText({ data: null }), '运行记录：—', 'list status shows the unloaded state');
eq(schedRunsListStatusText({ data: { count: 0, runs: [] }, rows: [] }), '运行记录：共 0 条', 'list status shows the empty count');
ok(
  /共 2 条/.test(schedRunsListStatusText({ data: { count: 2, runs: orderedRuns }, rows: orderedRuns, filter: {} })),
  'list status shows the total count'
);
ok(
  /任务=任务1/.test(schedRunsListStatusText({ data: { count: 2, runs: orderedRuns }, rows: orderedRuns, filter: { task: '任务1' } })),
  'list status echoes the task filter'
);
ok(
  /结果=致命（本地筛选）/.test(schedRunsListStatusText({ data: { count: 2, runs: orderedRuns }, rows: orderedRuns, filter: { outcome: 'fatal' } })),
  'list status labels a class filter as local'
);
ok(
  /结果=失败/.test(schedRunsListStatusText({ data: { count: 2, runs: orderedRuns }, rows: orderedRuns, filter: { outcome: 'Failed' } })),
  'list status shows the persisted outcome label'
);
ok(
  /队列=q1（本地筛选）/.test(schedRunsListStatusText({ data: { count: 2, runs: orderedRuns }, rows: [queueRun], filter: { queueId: 'q1' } })) &&
    /显示 1 条/.test(schedRunsListStatusText({ data: { count: 2, runs: orderedRuns }, rows: [queueRun], filter: { queueId: 'q1' } })),
  'list status explains the local queue filter and visible rows'
);
eq(
  schedRunsListStatusText({ error: '运行记录读取失败：HTTP 500', data: null }),
  '运行记录读取失败：HTTP 500',
  'list status surfaces the read error verbatim'
);

// Behavioural render smoke test: the new schedule page renderers must produce
// one card per entry/queue without touching a real browser. A tiny DOM stub
// mirrors the fakeElement pattern used for the global asset renderer.
function fakeSchedElement() {
  return {
    textContent: '',
    className: '',
    children: [],
    disabled: false,
    checked: false,
    value: '',
    appendChild(child) { this.children.push(child); return child; },
    append(...children) { this.children.push(...children); },
    addEventListener() {},
  };
}

function installScheduleRenderHarness() {
  sandbox.document = {
    createElement: () => fakeSchedElement(),
    createTextNode: (text) => ({ textContent: String(text) }),
  };
  sandbox.schedBadge = (text, cls) => {
    const node = fakeSchedElement();
    node.className = 'badge ' + (cls || '');
    node.textContent = text;
    return node;
  };
  sandbox.pageSchedEntries = fakeSchedElement();
  sandbox.pageSchedQueues = fakeSchedElement();
  sandbox.pageSchedRunsList = fakeSchedElement();
  sandbox.pageSchedRunsStatus = fakeSchedElement();
  sandbox.pageSchedRunsSummary = fakeSchedElement();
  sandbox.schedRunDialogBody = fakeSchedElement();
  sandbox.schedRunDialogTitle = fakeSchedElement();
  sandbox.scheduleRunsData = null;
  sandbox.scheduleRunsError = '';
  sandbox.scheduleRunsLoading = false;
  sandbox.scheduleRunsFilter = { limit: 200, task: '', outcome: '', queueId: '' };
  sandbox.openScheduleRunDetail = () => {};
  for (const name of ['pageSchedMode', 'pageSchedPasses', 'pageSchedNextWake',
    'pageSchedRunning', 'pageSchedQuota', 'pageSchedSummary', 'pageSchedConfigStatus']) {
    sandbox[name] = fakeSchedElement();
  }
  sandbox.scheduleConfigMeta = { running: true, path: 'scheduler.json', exists: true, state_path: 'scheduler.state.json', schema_version: 4 };
  sandbox.scheduleConfigError = '';
  sandbox.scheduleStatusError = '';
  sandbox.scheduleLocked = () => true;
  sandbox.schedEntryKindLabel = schedEntryKindLabel;
  sandbox.schedStatusErrorText = schedStatusErrorText;
  const sourceFns = [
    extractFunction('scheduleStatusEntryMap'),
    extractFunction('renderScheduleStatusCard'),
    extractFunction('renderScheduleEntries'),
    extractFunction('renderScheduleQueues'),
    extractFunction('makeSchedRunBadge'),
    extractFunction('renderScheduleRuns'),
    extractFunction('renderScheduleRunDetail'),
  ].join('\n\n');
  vm.runInContext(sourceFns, sandbox, { filename: 'app.js#schedule-render' });
}

installScheduleRenderHarness();
const renderCfg = schedEmptyConfig();
renderCfg.queues = [{
  queue_id: 'q1', name: '日常', members: ['B'], total_rounds: 2,
  on_failure: 'continue', between_ms: 500,
}];
renderCfg.entries = [
  schedDefaultTaskEntry('A', renderCfg),
  schedDefaultTaskEntry('q1', renderCfg, 'queue'),
];
const renderStatus = {
  mode: 'loop',
  passes: 7,
  next_wake_local: '2026-11-03 09:00:00',
  guards: { blocked: false },
  entries: [
    { entry_id: 'task-A', runnable_now: true, last_result: 'success', last_outcome: 'Done' },
    { entry_id: 'queue-q1', queue: { total_rounds: 2, rounds_completed: 1, empty_rounds: 0, finished: false, current_round: { index: 2, member_index: 0 } } },
  ],
  queues: [{
    queue_id: 'q1', name: '日常', members: ['B'], total_rounds: 2, rounds_completed: 1,
    empty_rounds: 0, finished: false, on_failure: 'continue', between_ms: 500,
    current_round: { index: 2, member_index: 0 },
  }],
};
// P1-3: a failed /scheduler/status read renders an unknown state, not "正常".
sandbox.scheduleStatusError = 'HTTP 500';
sandbox.renderScheduleStatusCard(null);
eq(sandbox.pageSchedMode.textContent, '模式：—', 'status read failure shows an unknown mode');
eq(sandbox.pageSchedPasses.textContent, 'passes=—', 'status read failure does not fake passes=0');
eq(sandbox.pageSchedNextWake.textContent, '下次唤醒=—', 'status read failure shows an unknown next wake');
eq(sandbox.pageSchedQuota.textContent, 'quota：—', 'status read failure shows an unknown quota');
eq(sandbox.pageSchedRunning.textContent, '锁=—', 'status read failure does not claim an idle lock');
ok(
  /调度器状态读取失败：HTTP 500/.test(sandbox.pageSchedSummary.textContent),
  'status summary carries the read error'
);
ok(
  !/调度器状态正常/.test(sandbox.pageSchedSummary.textContent),
  'status summary never claims everything is normal after a read failure'
);
sandbox.scheduleStatusError = '';

sandbox.renderScheduleStatusCard(renderStatus);
eq(sandbox.pageSchedMode.textContent, '模式：loop', 'status card shows mode');
eq(sandbox.pageSchedPasses.textContent, 'passes=7', 'status card shows passes');
eq(sandbox.pageSchedNextWake.textContent, '下次唤醒=2026-11-03 09:00:00', 'status card shows next wake');
ok(/调度器运行中/.test(sandbox.pageSchedRunning.textContent), 'status card shows the running lock');
eq(sandbox.pageSchedQuota.textContent, 'quota：无限制', 'status card shows the quota summary');
ok(/stop-scheduler\.bat/.test(sandbox.pageSchedConfigStatus.textContent), 'config status warns how to save while locked');

sandbox.renderScheduleEntries(renderStatus, renderCfg);
eq(sandbox.pageSchedEntries.children.length, 2, 'entry renderer emits one card per entry');
const firstEntryCard = sandbox.pageSchedEntries.children[0];
eq(firstEntryCard.children[0].children[0].textContent, 'task-A · 任务 A', 'task entry card title');
ok(/quota 日 1/.test(firstEntryCard.children[0].children[1].textContent), 'task entry card summary includes quota');
eq(firstEntryCard.children[2].disabled, true, 'first entry cannot move up');
eq(firstEntryCard.children[3].disabled, false, 'first entry can move down');
const secondEntryCard = sandbox.pageSchedEntries.children[1];
eq(secondEntryCard.children[0].children[0].textContent, 'queue-q1 · 队列 q1', 'queue entry card title');
ok(
  secondEntryCard.children[0].children[2].children.some((b) => /轮次 1\/2/.test(b.textContent)),
  'queue entry card shows round progress'
);

sandbox.renderScheduleQueues(renderStatus, renderCfg);
eq(sandbox.pageSchedQueues.children.length, 1, 'queue renderer emits one card per queue');
const queueCard = sandbox.pageSchedQueues.children[0];
eq(queueCard.children[0].children[0].textContent, 'q1 · 日常', 'queue card title');
ok(/成员 1：B/.test(queueCard.children[0].children[1].textContent), 'queue card lists members in order');
ok(/进行中：第 2 轮/.test(queueCard.children[0].children[1].textContent), 'queue card shows the current round');
ok(
  queueCard.children[0].children[2].children.some((b) => /触发条目：queue-q1/.test(b.textContent)),
  'queue card shows the trigger entry id'
);

sandbox.renderScheduleEntries(renderStatus, null);
ok(/调度配置不可用/.test(sandbox.pageSchedEntries.textContent), 'entry renderer explains a missing config');
sandbox.renderScheduleQueues(renderStatus, null);
ok(/调度配置不可用/.test(sandbox.pageSchedQueues.textContent), 'queue renderer explains a missing config');

// P4: run list renderer smoke test (DOM stub, no browser).
sandbox.scheduleConfig = renderCfg;
function resetFake(el) {
  el.textContent = '';
  el.children.length = 0;
  return el;
}
sandbox.scheduleRunsData = { ok: true, dir: 'runs/scheduler', count: 2, runs: [queueRun, fatalRun] };
sandbox.scheduleRunsError = '';
sandbox.scheduleRunsLoading = false;
sandbox.scheduleRunsFilter = { limit: 200, task: '', outcome: '', queueId: '' };
resetFake(sandbox.pageSchedRunsList);
resetFake(sandbox.pageSchedRunsStatus);
sandbox.renderScheduleRuns();
eq(sandbox.pageSchedRunsList.children.length, 1, 'run renderer emits one table for a non-empty list');
const runTable = sandbox.pageSchedRunsList.children[0];
eq(runTable.children.length, 2, 'run table has thead + tbody');
eq(runTable.children[1].children.length, 2, 'run table emits one row per run');
const firstRunRow = runTable.children[1].children[0];
eq(firstRunRow.children.length, 10, 'run row has all ten columns');
eq(firstRunRow.children[1].textContent, '日常/任务2', 'run row task uses 队列名/成员名');
eq(firstRunRow.children[2].children[0].textContent, 'queue_member', 'run row kind badge');
eq(firstRunRow.children[3].textContent, '第2轮 · 成员1', 'run row shows round/member');
eq(firstRunRow.children[4].textContent, '2', 'run row shows attempt');
eq(firstRunRow.children[5].children[1].textContent, '完成', 'run row outcome badge uses the persisted Done label');
ok(/ok/.test(firstRunRow.children[5].children[1].className), 'run row outcome badge color class');
eq(firstRunRow.children[7].textContent, '2.5s', 'run row duration is human readable');
eq(firstRunRow.children[8].textContent, '3', 'run row clicks');
eq(firstRunRow.children[9].textContent, '—', 'successful run row has no error');
const fatalRunRow = runTable.children[1].children[1];
ok(/第一行 第二行/.test(fatalRunRow.children[9].textContent), 'fatal run row shows the error summary');
eq(fatalRunRow.children[9].className, 'run-error-cell', 'fatal run row error cell styling');
ok(/共 2 条/.test(sandbox.pageSchedRunsStatus.textContent), 'run status shows the total count');
// Status card: latest run + failure count from the loaded list.
resetFake(sandbox.pageSchedRunsSummary);
sandbox.renderScheduleStatusCard(renderStatus);
ok(/最近 完成/.test(sandbox.pageSchedRunsSummary.textContent), 'status card shows the latest persisted outcome');
ok(/失败 1\/2/.test(sandbox.pageSchedRunsSummary.textContent), 'status card shows the failure count');
// Empty + failed read states stay explicit.
sandbox.scheduleRunsData = { ok: true, count: 0, runs: [] };
sandbox.scheduleRunsError = '';
resetFake(sandbox.pageSchedRunsList);
sandbox.renderScheduleRuns();
ok(/暂无调度运行记录/.test(sandbox.pageSchedRunsList.textContent), 'run renderer shows the empty state');
sandbox.scheduleRunsData = null;
sandbox.scheduleRunsError = '运行记录读取失败：HTTP 500';
resetFake(sandbox.pageSchedRunsList);
sandbox.renderScheduleRuns();
eq(sandbox.pageSchedRunsList.textContent, '运行记录读取失败：HTTP 500', 'run renderer shows the read error verbatim');
ok(/运行记录读取失败/.test(sandbox.pageSchedRunsStatus.textContent), 'run status shows the read error');
sandbox.scheduleRunsError = '';

// P4: run detail renderer smoke test (field rows + full error + raw JSON).
sandbox.scheduleConfig = renderCfg;
resetFake(sandbox.schedRunDialogBody);
resetFake(sandbox.schedRunDialogTitle);
sandbox.renderScheduleRunDetail(queueRun);
eq(sandbox.schedRunDialogBody.children.length, 2, 'detail body has the field table + full JSON');
const detailTable = sandbox.schedRunDialogBody.children[0];
const detailSmokeMap = {};
for (const row of detailTable.children[0].children) detailSmokeMap[row.children[0].textContent] = row.children[1].textContent;
eq(detailSmokeMap.resume, '否', 'detail dialog shows resume');
eq(detailSmokeMap.clicks_file, 'runs/clicks/run-2.clicks.jsonl', 'detail dialog shows clicks_file');
ok(/^1762100100000（/.test(detailSmokeMap.started_ms), 'detail dialog shows raw + local started_ms');
ok(/^1762100102500（/.test(detailSmokeMap.finished_ms), 'detail dialog shows raw + local finished_ms');
ok(/日常\/任务2/.test(sandbox.schedRunDialogTitle.textContent), 'detail dialog title includes 队列名/成员名');
resetFake(sandbox.schedRunDialogBody);
sandbox.renderScheduleRunDetail(fatalRun);
eq(sandbox.schedRunDialogBody.children.length, 4, 'fatal detail adds the full-error block');
ok(/第一行\n第二行/.test(sandbox.schedRunDialogBody.children[2].textContent), 'detail dialog shows the error verbatim');
resetFake(sandbox.schedRunDialogBody);
sandbox.renderScheduleRunDetail(Object.assign({}, queueRun, { future_field: 'kept' }));
ok(
  /future_field/.test(sandbox.schedRunDialogBody.children[1].children[1].textContent),
  'detail full JSON includes unknown fields'
);

// Static wiring for the new editable scheduler UI.
ok(/id="page-sched-add-entry"/.test(html), 'index.html provides the add-entry button');
ok(/id="page-sched-add-queue"/.test(html), 'index.html provides the add-queue button');
ok(/id="page-sched-json"/.test(html) && /id="page-sched-json-save"/.test(html), 'index.html provides the advanced JSON editor');
ok(/id="page-sched-queues"/.test(html), 'index.html provides the queue panel container');
ok(/id="page-sched-running"/.test(html), 'index.html provides the running/lock badge');
ok(/id="sched-entry-dialog"/.test(html) && /id="sched-entry-windows"/.test(html) && /id="sched-entry-params"/.test(html), 'entry dialog fields exist');
ok(/id="sched-queue-dialog"/.test(html) && /id="sched-queue-members"/.test(html) && /id="sched-queue-task-pool"/.test(html), 'queue dialog fields exist');
ok(/id="btn-sched-open-page"/.test(html), 'index.html provides the workbench open-page button');
const htmlIds = [...html.matchAll(/id=["']([^"']+)["']/g)].map((m) => m[1]);
const dupHtmlIds = [...new Set(htmlIds.filter((id, i) => htmlIds.indexOf(id) !== i))];
ok(dupHtmlIds.length === 0, `index.html has no duplicate DOM ids (duplicates: ${dupHtmlIds.join(', ') || 'none'})`);
ok(
  (html.match(/<dialog\b/g) || []).length === (html.match(/<\/dialog>/g) || []).length,
  'index.html dialog tags are balanced'
);
ok(/调度设置…/.test(source) && /openTaskScheduleDialog\(t\.name\)/.test(source), 'task library card opens the scheduler dialog');
ok(/fetch\('\/scheduler\/config'\)/.test(source), 'GET /scheduler/config is wired');
ok(/body: JSON\.stringify\(\{ config: cfg \}\)/.test(source), 'POST /scheduler/config sends {config}');
ok(/btnSchedOpenPage\.addEventListener\('click', \(\) => activatePage\('schedule'\)\)/.test(source), 'workbench button opens the schedule page');
ok(/if \(pageSchedJson\)[\s\S]{0,200}scheduleJsonDirty = true/.test(source), 'advanced JSON tracks unsaved edits');
ok(/id="page-sched-entries"[^>]*>（未加载）<\/div>/.test(html), 'entry list container starts in the unloaded state');
ok(/id="page-sched-queues"[^>]*>（未加载）<\/div>/.test(html), 'queue list container starts in the unloaded state');

// P4 static contracts: real DOM ids for the run history + API call sites.
for (const id of [
  'page-sched-runs-summary', 'page-sched-runs-refresh', 'page-sched-runs-task',
  'page-sched-runs-outcome', 'page-sched-runs-limit', 'page-sched-runs-clear',
  'page-sched-runs-status', 'page-sched-runs-list', 'page-sched-runs-dialog',
  'page-sched-runs-detail-title', 'page-sched-runs-detail-body',
  'page-sched-runs-detail-error', 'page-sched-runs-detail-refresh',
  'page-sched-runs-detail-close',
]) {
  ok(new RegExp(`id="${id}"`).test(html), `index.html provides #${id}`);
}
ok(/id="page-sched-runs-list"[^>]*>（未加载）<\/div>/.test(html), 'run list starts in the unloaded state');
ok(/id="page-sched-runs-outcome"[\s\S]{0,900}<option value="fatal">/.test(html), 'outcome filter offers fatal');
ok(/id="page-sched-runs-outcome"[\s\S]{0,1400}<option value="Failed">/.test(html), 'outcome filter also offers the persisted Failed value');
ok(/id="page-sched-runs-limit"[\s\S]{0,200}<option value="200" selected>/.test(html), 'limit defaults to 200');
ok(/const url = schedRunsBuildUrl\(scheduleRunsFilter\);/.test(source), 'run list builds the /runs/scheduler URL from the filter');
ok(/fetch\(schedRunDetailUrl\(runId\)\)/.test(source), 'detail fetch uses schedRunDetailUrl(runId)');
ok(/\/runs\/scheduler\/run\?run_id=/.test(source), 'detail route is /runs/scheduler/run?run_id=');
ok(/log\(scheduleRunsError, 'WARN'\)/.test(source), 'run list read failures are logged as WARN');
ok(/log\(res\.error, 'WARN'\)/.test(source), 'run detail read failures are logged as WARN');
ok(/schedRunsErrorText\(/.test(source), 'run list error uses the 运行记录读取失败 prefix');
ok(/fetch\('\/runs'\)/.test(source) && /loadClicks\(/.test(source), 'legacy /runs click-distribution path is untouched');
ok(/showScheduleRunsForTask\(t\.name\)/.test(source), 'task library row links to the run history');
ok(/showScheduleRunsForEntry\(e\)/.test(source), 'schedule entry row links to the run history');
ok(/showScheduleRunsForQueue\(q\.queue_id\)/.test(source), 'queue row links to the run history');
ok(/scheduleRunsFilter\.queueId/.test(source), 'queue shortcut uses the local queue filter');

// P2: the whole P4 run-history block (pure helpers + list/detail renderers +
// the detail dialog) must stay DOM-safe: task/error/run_id text is only ever
// assigned through textContent, never HTML strings or dynamic code. The block
// is delimited by two grep-able function names: the first P4 pure helper and
// the next schedule helper after the dialog code.
const p4UiStart = source.indexOf('function schedRunValueText');
const p4UiEnd = source.indexOf('async function toggleScheduleEntry');
ok(p4UiStart > 0 && p4UiEnd > p4UiStart, 'the P4 run-history UI block is discoverable in app.js');
const p4UiSegment = p4UiStart > 0 && p4UiEnd > p4UiStart ? source.slice(p4UiStart, p4UiEnd) : '';
ok(
  !/innerHTML|insertAdjacentHTML|outerHTML/.test(p4UiSegment),
  'the P4 run-history UI block never writes raw HTML'
);
ok(
  !/\beval\s*\(|new Function\s*\(/.test(p4UiSegment),
  'the P4 run-history UI block never evaluates dynamic code'
);

// P1-3/P1-4 wiring: the save button is GET-first + snapshot-guarded, and a
// status read failure is recorded, logged and rendered on the card.
const saveConfigFn = extractFunction('saveSchedulePageConfig');
ok(/fetchScheduleConfigOnce\(\)/.test(saveConfigFn), '保存配置 GETs the latest config before POSTing');
ok(/schedConfigSnapshotChanged\(scheduleConfigBase, freshRaw\)/.test(saveConfigFn), '保存配置 compares the fresh config with the page snapshot');
ok(/schedExternalConfigChangeText\(\)/.test(saveConfigFn), '保存配置 aborts with the reload hint on an external change');
const loadConfigFn = extractFunction('loadScheduleConfig');
ok(/scheduleConfigBase = schedConfigSnapshotText\(/.test(loadConfigFn), 'loadScheduleConfig records the base config snapshot');
const refreshPageFn = extractFunction('refreshSchedulePage');
ok(/scheduleStatusError = /.test(refreshPageFn) && /schedStatusErrorText\(/.test(refreshPageFn), 'status read failures are recorded and logged');
ok(/await refreshScheduleRuns\(\)/.test(refreshPageFn), 'schedule page refresh loads the run history');
ok(/populateScheduleRunsTaskOptions\(\)/.test(refreshPageFn), 'schedule page refresh repopulates the run task filter');
ok(/schedStatusErrorText\(statusError\)/.test(extractFunction('renderScheduleStatusCard')), 'status card renders the read error');
ok(/pageSchedRunsSummary\.textContent = schedRunsStatusCardText\(/.test(extractFunction('renderScheduleStatusCard')), 'status card renders the latest-run/failure summary');

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
  .then(() => {
    console.log(
      `dom_id_audit: ${referencedIds.length} referenced ids, ${missingIds.length} dead ids` +
      (missingIds.length ? ` (${missingIds.join(', ')})` : '')
    );
    console.log(`ui_pure_test: ${checks} checks passed`);
  })
  .catch((err) => {
    console.error(err);
    process.exitCode = 1;
  });
