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
//   - the mutually-exclusive selection mode dispatch.
'use strict';

const fs = require('fs');
const path = require('path');
const vm = require('vm');
const assert = require('assert');

const appPath = path.join(__dirname, 'app.js');
const htmlPath = path.join(__dirname, 'index.html');
const source = fs.readFileSync(appPath, 'utf8');
const html = fs.readFileSync(htmlPath, 'utf8');

function extractFunction(name) {
  const re = new RegExp(`(?:^|\\n)(?:async\\s+)?function\\s+${name}\\s*\\(`);
  const m = re.exec(source);
  if (!m) throw new Error(`function ${name} not found in app.js`);
  const start = m.index + (m[0][0] === '\n' ? 1 : 0);
  let i = source.indexOf('{', m.index);
  let depth = 0;
  let quote = null;
  let esc = false;
  let lineComment = false;
  let blockComment = false;
  for (; i < source.length; i++) {
    const ch = source[i];
    const next = source[i + 1];
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
      if (depth === 0) return source.slice(start, i + 1);
    }
  }
  throw new Error(`unterminated function ${name}`);
}

const PURE_FUNCTIONS = [
  'actionShapeBBox',
  'actionShapeContains',
  'actionShapeEffectiveContains',
  'segmentIntersects',
  'polygonOverlapsRect',
  'rectsOverlap',
  'actionValidity',
  'boxCenter',
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
  'renderGlobalAssets',
  'selectionModeFromFlags',
];

const sandbox = {};
vm.createContext(sandbox);
vm.runInContext(PURE_FUNCTIONS.map(extractFunction).join('\n\n'), sandbox, {
  filename: 'app.js#pure',
});

const {
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
  sandbox.window = {
    OASAssetUI: {
      renderRows(container, rows) {
        rendered.ids = rows.map((r) => r.id);
        container.textContent = 'rendered';
      },
    },
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
ok(!/saveSelectedCandidates/.test(source), 'dead saveSelectedCandidates is gone');
ok(!/btn-batch-save/.test(source), 'dead #btn-batch-save binding is gone');
ok(!/action_roi/.test(source), 'no stale action_roi text remains in app.js');

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

checkSavePathWiring(saveUnified)
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
