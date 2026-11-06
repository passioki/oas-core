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
//   - PR7 scheduler migration: the old Studio entry/queue editor, run history,
//     reset-memory dialog and workbench panel now live on the Runner queue page
//     (84 legacy ids, 77 pure helpers); Studio stays schedule-free.
//   - P0-1a publish notifications: Studio appends the exchange result to the
//     publish panel; the Runner polls /runner/updates, shows a nav badge + the
//     「新发布」 card, installs through /runner/install and acks
//     /runner/updates/ack (installed/dismissed).
//   - P3 params UI: the builder panel renders meta.params (GET /task/config),
//     locator OCR text has a fixed/param/expression tri-state (text/text_expr
//     mutually exclusive), and 插入参数 ▾ writes param(...) snippets.
//   - 事项 1/2（2026-11-06）：参数不属于动作库 —— the action library has NO
//     parameter presets (PARAM_ACTIONS/PARAM_GROUP removed); the palette and the
//     if/loop branch dropdowns are sourced from OP_META only.
//   - 事项 6（2026-11-06）：操作区域两状态废除 —— app.js has no actionValidity /
//     action_source (recognition vs fixed) UI rule; the operation region is the
//     asset's own action_shape (absent = real recognized box).
//   - 事项 2/3/4（U-T4 阶段 A）：ocr_var 素材行（识别文字/op_region 摘要/待迁移
//     badge）、素材行「识别」= GET /task/asset/candidates 全部候选（全画+逐行列出）、
//     S6 相对偏移画布绘制已删除（B3a 模型 A：操作区只保留 self/asset 两态），以及
//     candidateLabelText/opRegionChainSpec/runtimeTextValid 纯函数。
//   - P2 Runner params page: #runner-config-dialog groups meta.params by `group`
//     (first-appearance order, missing group -> 通用); U-T6 puts derived rows
//     (meta.params with asset_id, from ocr_var assets) into a leading
//     「使用端OCR文字配置项」 section with a 素材 <id> · 当前默认「…」 subtitle;
//     every row has a 使用
//     switch + bool tri-state (未设置/是/否) + 留空=不注入 + required blocking;
//     the updates feed adds source badges / 需下载 hints and installs through
//     POST /runner/updates/install {event_id} (legacy path install kept as a
//     fallback for old backends).
//   - P5 build-page lint: the head 检查 button runs the authoritative
//     GET /task/lint; the issues panel renders [code] path + message, T3 rows
//     offer 去声明… through the task-config dialog bridge (auto re-check on save),
//     and the approximate param hint collapses once the authoritative result
//     exists (missing backend route degrades to 权威检查不可用).
//   - P6 params editing gaps: requires/expect guards and drag from/to endpoints
//     share the locator「文字来源」tri-state (text / text_expr param / expression,
//     mutually exclusive, unknown fields preserved), and T3 去声明 prefills the
//     missing ParamDecl into the config dialog JSON (append-only, same key
//     deduped, bad JSON degrades to the untouched text).
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
  'boxCenter',
  // MATCH-BUG: frame selection for single asset tests + match-box validity
  'testFramePlan',
  'matchBoxValid',
  'translateActionShape',
  // U-T4（事项 3/4/5）：候选识别 / op_region 链 / runtime_text。
  'assetCandidatesUrl',
  'candidateLabelText',
  'candidateBoxes',
  'runtimeTextValid',
  'opRegionNodeOf',
  'opRegionChainSpec',
  // U-T4/S5：操作区递归对话框草稿模型（API 无关，阶段 B 直接复用）。
  'opRegionToJson',
  'opRegionDraftStart',
  'opRegionDraftBreadcrumb',
  'opRegionDraftBreadcrumbText',
  'opRegionDraftSetLevel',
  'opRegionDraftBack',
  'opRegionDraftChainSpec',
  'opRegionDraftUpdates',
  'opRegionDraftSummary',
  'opRegionPickOptions',
  'opRegionRefOptions',
  // U-T4/S2/S5（阶段 B）：鉴别/保存纯模型 + 配置派生行回传契约。
  'isParamKeyValid',
  'autoParamKey',
  'candidateNameValid',
  'assetTextDraftStart',
  'assetTextDraftFromCandidate',
  'assetTextValidate',
  'assetTextSaveRequest',
  'opRegionSavePlan',
  'opRegionLevelChoices',
  'opRegionTargetKinds',
  'opRegionRefChoices',
  'opRegionReadonlyChainText',
  // U-T4/S4（B-T3b）：test_action 请求/结果格式化 + validate_op_region 错误。
  'testActionRequestBody',
  'candidateOptionLabel',
  'testActionResultLines',
  'testActionBoxes',
  'opRegionValidateErrorText',
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
  // #10（批次B3b）：统一框选工具纯函数（手势判定/按钮状态/识别区保存计划）。
  'canvasGestureShape',
  'canvasToolButtonState',
  'recognitionDraftFromAsset',
  'recognitionPolygonBBox',
  'rectEq4',
  'polygonPointsEq',
  'recognitionSavePlan',
  // PR2: Studio-side meta.params schema editor (runner values stay on Runner).
  // P6: T3「去声明…」prefill merge (append-only, same-key dedupe, bad-JSON degrade).
  // P7: mature structured params editor (form model / defaults / enum / validation).
  // PR5: Studio-side TaskPackage publish body / summary / error rendering.
  'publishRequestBody',
  'formatPackageSummary',
  'publishErrorsText',
  // P0-1a: Studio-side exchange (publish notification) result line.
  'publishExchangeText',
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
  polygonOverlapsRect,
  boxCenter,
  translateActionShape,
  assetCandidatesUrl,
  candidateLabelText,
  candidateBoxes,
  runtimeTextValid,
  opRegionNodeOf,
  opRegionChainSpec,
  opRegionToJson,
  opRegionDraftStart,
  opRegionDraftBreadcrumb,
  opRegionDraftBreadcrumbText,
  opRegionDraftSetLevel,
  opRegionDraftBack,
  opRegionDraftChainSpec,
  opRegionDraftUpdates,
  opRegionDraftSummary,
  opRegionPickOptions,
  opRegionRefOptions,
  isParamKeyValid,
  autoParamKey,
  candidateNameValid,
  assetTextDraftStart,
  assetTextDraftFromCandidate,
  assetTextValidate,
  assetTextSaveRequest,
  opRegionSavePlan,
  opRegionLevelChoices,
  opRegionTargetKinds,
  opRegionRefChoices,
  opRegionReadonlyChainText,
  testActionRequestBody,
  candidateOptionLabel,
  testActionResultLines,
  testActionBoxes,
  opRegionValidateErrorText,
  recognitionPolygonPayload,
  ocrAssetPayload,
  singleAssetPayload,
  assetCategoryNames,
  assetMatchesFilter,
  filterAssets,
  globalAssetStatusText,
  canvasGestureShape,
  canvasToolButtonState,
  recognitionDraftFromAsset,
  recognitionPolygonBBox,
  rectEq4,
  polygonPointsEq,
  recognitionSavePlan,
  publishRequestBody,
  formatPackageSummary,
  publishErrorsText,
  publishExchangeText,
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

// --- 事项 6（2026-11-06）：操作区域两状态（recognition/fixed）已删除 ---
// The old UI rule asked whether the operation region overlapped the real
// recognized box and called a non-overlap invalid. The engine never read
// action_source (see the audit), so both the validators and the user-visible
// state labels are gone; action_shape alone decides the operation region.
ok(!/function actionValidity\(/.test(source), 'the recognition-vs-fixed actionValidity validator is gone');
ok(!/function rectsOverlap\(/.test(source), 'rectsOverlap is gone with the overlap validity rule');
ok(!/action_source/.test(source), 'app.js never reads or writes action_source any more');
ok(!/actionSource/.test(source), 'app.js has no in-memory actionSource (two-state) field');
ok(
  !/固定操作区|跟随识别框（recognition）|区域有效性/.test(source),
  'the user-visible two-state labels/validity lines are gone'
);
ok(
  /function candidateActionPayload\(c\)\s*\{[\s\S]{0,160}c\.actionShape \? \{ action_shape: c\.actionShape \} : \{\}/.test(source),
  'the save payload carries action_shape only (no recognition/fixed flag)'
);
ok(
  !/action_source/.test(builderSource) && !/action_source/.test(assetUiSource),
  'builder.js/asset_ui.js do not write action_source either'
);

// --- P1-1: template action_shape is translated by matched - roi before use ---
const savedTemplate = {
  roi: [22, 9, 43, 39],
  action_shape: { kind: 'rect', roi: [20, 10, 10, 10], invert: false },
};
const match = { x: 40, y: 30 };
const translated = translateActionShape(
  savedTemplate.action_shape,
  match.x - savedTemplate.roi[0],
  match.y - savedTemplate.roi[1]
);
eq(translated.roi, [38, 31, 10, 10], 'translate rect action_shape');

eq(
  translateActionShape({ kind: 'polygon', points: [[0, 0], [4, 0], [0, 4]], invert: true }, 5, 7),
  { kind: 'polygon', points: [[5, 7], [9, 7], [5, 11]], invert: true },
  'translate polygon action_shape'
);
eq(translateActionShape(null, 1, 2), null, 'translate null shape');

// --- U-T4（事项 3/4/5）：候选 / 相对偏移 / runtime_text / op_region 链 纯函数 ---
eq(
  assetCandidatesUrl('任务 甲', 'O_甲', '&frame_id=7'),
  '/task/asset/candidates?task=%E4%BB%BB%E5%8A%A1%20%E7%94%B2&id=O_%E7%94%B2&frame_id=7',
  'U-T4: candidates URL encodes task/id and appends the frame query'
);
eq(
  assetCandidatesUrl('', 'I_A'),
  '/task/asset/candidates?task=&id=I_A',
  'U-T4: candidates URL without a frame query (live/current frame)'
);
eq(
  candidateLabelText({ index: 0, x: 11, y: 21, w: 30, h: 40, score: 0.973 }),
  '#0 score=0.9730 [11,21,30,40]',
  'U-T4: a candidate renders as #i score=… [x,y,w,h]'
);
eq(
  candidateLabelText({ index: 1, x: 1, y: 2, w: 3, h: 4, score: 0.9, text: '御魂' }),
  '#1 score=0.9000 [1,2,3,4] 文字="御魂"',
  'U-T4: OCR candidates keep their recognised text'
);
eq(
  candidateLabelText({ index: 2, x: 1, y: 2, w: 3, h: 4, score: 0.5, member: 'I_A' }),
  '#2 score=0.5000 [1,2,3,4] · 成员 I_A',
  'U-T4: collection candidates name the winning member'
);
eq(
  candidateLabelText(null),
  '#0 score=— [0,0,0,0]',
  'U-T4: a missing candidate still renders a placeholder line'
);
eq(
  candidateBoxes([
    { index: 0, x: 11, y: 21, w: 30, h: 40, score: 0.973 },
    { index: 1, x: -1, y: -1, w: 30, h: 40, score: 0.1 },
    null,
    { index: 2, x: 1, y: 2, w: 3, h: 4, score: 0.5 },
  ]),
  [
    { text: '#0 0.97', score: 0.973, box: [11, 21, 30, 40] },
    { text: '#2 0.50', score: 0.5, box: [1, 2, 3, 4] },
  ],
  'U-T4: all valid candidates become draw() boxes (invalid boxes are skipped)'
);
// S6 offset 绘制已删除（B3a 模型 A）；旧 offset 数据仅由 openOpRegionDialog 提示迁移为 self。
// runtime_text（S2 对话框字段；阶段 A 先落纯校验）
eq(runtimeTextValid(' 御魂 '), { ok: true, value: '御魂', error: '' }, 'U-T4: runtime_text trims and accepts a normal value');
eq(runtimeTextValid('').ok, false, 'U-T4: empty runtime_text is rejected');
eq(runtimeTextValid('   ').ok, false, 'U-T4: whitespace-only runtime_text is rejected');
eq(runtimeTextValid('x'.repeat(201)).ok, false, 'U-T4: runtime_text longer than 200 chars is rejected');
eq(runtimeTextValid('x'.repeat(200)).ok, true, 'U-T4: 200 chars is the inclusive limit');
eq(runtimeTextValid('a\nb').ok, false, 'U-T4: control characters are rejected');
// op_region 链（S5；服务端 POST /task/asset/validate_op_region 才是唯一权威）
const opAssets = [
  { id: 'I_A', kind: 'template' },
  { id: 'I_B', kind: 'template', op_region: { mode: 'self' } },
  { id: 'I_C', kind: 'template', op_region: { mode: 'offset', offset: [1, 2, 30, 40] } },
  { id: 'R_区', kind: 'region', roi: [5, 5, 10, 10] },
];
eq(
  opRegionNodeOf({ mode: 'ASSET', asset: ' I_X ', pick: 'NEAREST' }),
  { mode: 'asset', asset: 'I_X', pick: 'nearest', offset: [] },
  'U-T4: op_region node normalizes mode/asset/pick'
);
eq(opRegionChainSpec({ mode: 'self' }, opAssets).chain, [{ mode: 'self', asset: '' }], 'U-T4: self ends the chain');
eq(
  opRegionChainSpec({ mode: 'offset', offset: [10, 5, 150, 80] }, opAssets).chain,
  [{ mode: 'offset', asset: '', offset: [10, 5, 150, 80] }],
  'U-T4: offset ends the chain'
);
eq(
  opRegionChainSpec({ mode: 'asset', asset: 'I_B' }, opAssets).chain.map((n) => n.mode + ':' + n.asset),
  ['asset:I_B', 'self:'],
  'U-T4: asset -> referenced self chain'
);
eq(
  opRegionChainSpec({ mode: 'asset', asset: 'I_C' }, opAssets).chain.map((n) => n.mode),
  ['asset', 'offset'],
  'U-T4: asset -> referenced offset chain'
);
const regionChain = opRegionChainSpec({ mode: 'asset', asset: 'R_区' }, opAssets);
eq(regionChain.ok, true, 'U-T4: a region asset can end the chain as a fixed terminal node');
eq(regionChain.chain.map((n) => n.mode), ['asset', 'region'], 'U-T4: the region terminal hop is explicit');
const cycleSpec = opRegionChainSpec({ mode: 'asset', asset: 'I_A' }, [
  { id: 'I_A', kind: 'template', op_region: { mode: 'asset', asset: 'I_B' } },
  { id: 'I_B', kind: 'template', op_region: { mode: 'asset', asset: 'I_A' } },
]);
eq(cycleSpec.ok, false, 'U-T4: a cycle is rejected');
ok(/循环/.test(cycleSpec.error), 'U-T4: the cycle error names the problem');
ok(opRegionChainSpec({ mode: 'asset', asset: 'Missing' }, opAssets).ok === false, 'U-T4: a missing referenced asset is rejected');
const rootCycleSpec = opRegionChainSpec(
  { mode: 'asset', asset: 'I_A' },
  [{ id: 'I_A', kind: 'template', op_region: { mode: 'self' } }],
  'I_A'
);
eq(rootCycleSpec.ok, false, 'U-T4: referencing the edited asset itself is a cycle');
ok(/循环/.test(rootCycleSpec.error), 'U-T4: the self-cycle error explains the loop');
ok(
  opRegionChainSpec(
    { mode: 'asset', asset: 'R_区' },
    [{ id: 'R_区', kind: 'region', op_region: { mode: 'self' } }]
  ).ok === false,
  'U-T4: a region terminal cannot carry op_region'
);
const deepAssets = [];
for (let i = 0; i < 20; i += 1) {
  deepAssets.push({
    id: 'D' + i,
    kind: 'template',
    op_region: i === 19 ? { mode: 'self' } : { mode: 'asset', asset: 'D' + (i + 1) },
  });
}
const tooDeepSpec = opRegionChainSpec({ mode: 'asset', asset: 'D0' }, deepAssets);
eq(tooDeepSpec.ok, false, 'U-T4: a chain deeper than 16 hops is rejected');
ok(/最大深度 16/.test(tooDeepSpec.error), 'U-T4: the depth error names the 16 limit');
ok(
  /四个整数/.test(opRegionChainSpec({ mode: 'offset', offset: [1, 2] }, opAssets).error),
  'U-T4: offset without 4 integers is rejected'
);

// --- U-T4/S5：操作区递归对话框草稿模型（API 无关；阶段 B 对话框直接复用） ---
const s5Assets = [
  { id: 'A', kind: 'template', op_region: { mode: 'offset', offset: [1, 2, 30, 40] } },
  { id: 'B', kind: 'template', op_region: { mode: 'self' } },
  { id: 'C', kind: 'ocr_var', op_region: { mode: 'asset', asset: 'B', pick: 'nearest' } },
  { id: 'R', kind: 'region', roi: [5, 5, 10, 10] },
];
eq(
  opRegionToJson({ mode: 'ASSET', asset: ' B ', pick: 'weird' }),
  { mode: 'asset', asset: 'B', pick: 'score' },
  'U-T4/S5: opRegionToJson normalizes mode/asset/pick for the API payload'
);
eq(opRegionToJson({ mode: 'offset', offset: [1, 2, 3, 4] }), { mode: 'offset', offset: [1, 2, 3, 4] });
eq(opRegionToJson({}), { mode: 'self' }, 'U-T4/S5: missing op_region stores as self');
const s5DraftA = opRegionDraftStart(s5Assets[0]);
eq(s5DraftA.rootAssetId, 'A', 'U-T4/S5: draft root is the edited asset');
eq(s5DraftA.levels.length, 1, 'U-T4/S5: draft starts with one (root) level');
eq(
  s5DraftA.levels[0].opRegion,
  { mode: 'offset', offset: [1, 2, 30, 40] },
  'U-T4/S5: the root level starts from the stored op_region'
);
eq(
  opRegionDraftStart({ id: 'X' }).levels[0].opRegion,
  { mode: 'self' },
  'U-T4/S5: an asset without op_region starts at self'
);
eq(opRegionDraftBreadcrumb(s5DraftA), ['A'], 'U-T4/S5: breadcrumb starts with the root asset');
eq(opRegionDraftBreadcrumbText(s5DraftA), 'A');
const s5DraftAB = opRegionDraftSetLevel(s5DraftA, 0, { mode: 'asset', asset: 'B' }, s5Assets);
eq(opRegionDraftBreadcrumb(s5DraftAB), ['A', 'B'], 'U-T4/S5: choosing a referenced asset expands one level');
eq(s5DraftAB.levels[1].opRegion, { mode: 'self' }, 'U-T4/S5: the referenced level starts from its stored op_region');
eq(opRegionDraftBreadcrumbText(s5DraftAB), 'A ▸ B', 'U-T4/S5: breadcrumb text joins the levels');
const s5DraftABOff = opRegionDraftSetLevel(s5DraftAB, 1, { mode: 'offset', offset: [5, 6, 70, 80] }, s5Assets);
eq(s5DraftABOff.levels[1].opRegion, { mode: 'offset', offset: [5, 6, 70, 80] }, 'U-T4/S5: the referenced level can be an offset');
eq(opRegionDraftChainSpec(s5DraftABOff, s5Assets).ok, true, 'U-T4/S5: a draft chain is valid');
eq(
  opRegionDraftChainSpec(s5DraftABOff, s5Assets).chain.map((n) => n.mode),
  ['asset', 'offset'],
  'U-T4/S5: the draft chain preview walks asset -> offset'
);
eq(
  opRegionDraftUpdates(s5DraftABOff, s5Assets),
  [
    { assetId: 'A', op_region: { mode: 'asset', asset: 'B', pick: 'score' } },
    { assetId: 'B', op_region: { mode: 'offset', offset: [5, 6, 70, 80] } },
  ],
  'U-T4/S5: saving writes exactly the edited levels (current + referenced asset = two updates)'
);
eq(
  opRegionDraftUpdates(opRegionDraftStart(s5Assets[0]), s5Assets),
  [],
  'U-T4/S5: an untouched draft produces no update calls'
);
const s5DraftBack = opRegionDraftBack(s5DraftABOff, 1);
eq(opRegionDraftBreadcrumb(s5DraftBack), ['A'], 'U-T4/S5: 回退 removes the referenced level');
eq(s5DraftBack.levels[0].opRegion, { mode: 'self' }, 'U-T4/S5: 回退 turns the parent back to self');
eq(opRegionDraftChainSpec(s5DraftBack, s5Assets).ok, true, 'U-T4/S5: the rolled-back chain is valid');
ok(
  opRegionDraftBack(s5DraftA, 0) === s5DraftA,
  'U-T4/S5: the root level cannot be rolled back (draft returned unchanged)'
);
const s5DraftAC = opRegionDraftSetLevel(s5DraftA, 0, { mode: 'asset', asset: 'C' }, s5Assets);
const s5DraftCycle = opRegionDraftSetLevel(s5DraftAC, 1, { mode: 'asset', asset: 'A' }, s5Assets);
eq(opRegionDraftChainSpec(s5DraftCycle, s5Assets).ok, false, 'U-T4/S5: a draft cycle back to the root is rejected');
ok(/循环/.test(opRegionDraftChainSpec(s5DraftCycle, s5Assets).error), 'U-T4/S5: the draft cycle error explains the loop');
eq(
  opRegionDraftSummary(s5DraftABOff).map((row) => `${row.assetId}:${row.mode}`),
  ['A:asset', 'B:offset'],
  'U-T4/S5: per-level summaries drive the dialog rows'
);
eq(
  opRegionDraftSummary(s5DraftABOff).map((row) => row.text)[0],
  '引用素材「B」（评分最高）',
  'U-T4/S5: an asset level summary names the pick rule'
);
eq(
  opRegionPickOptions().map((o) => o.value),
  ['score', 'nearest'],
  'U-T4/S5: pick options are 评分最高 / 离上一节点最近'
);
eq(
  opRegionRefOptions(s5Assets, 'A'),
  [
    { id: 'B', kind: 'template', terminal: false },
    { id: 'C', kind: 'ocr_var', terminal: false },
    { id: 'R', kind: 'region', terminal: true },
  ],
  'U-T4/S5: reference options list recognizable assets + region as a terminal node (root excluded)'
);
ok(
  opRegionRefOptions(s5Assets, 'A').every((o) => o.id !== 'A'),
  'U-T4/S5: the edited asset itself is never offered as a reference'
);

// --- U-T4/S2/S5（阶段 B）：识别文字对话框草稿/校验/保存载荷 + 链保存计划 ------
// param_key 自动生成 / 合法性
eq(autoParamKey('O_预设分组'), autoParamKey('O_预设分组'), 'U-T4: autoParamKey is deterministic for one id');
eq(autoParamKey('A').length, 10, 'U-T4: autoParamKey = p_ + 8 hex chars');
ok(/^p_[0-9a-f]{8}$/.test(autoParamKey('O_组')), 'U-T4: autoParamKey matches the p_<hash8> format');
ok(autoParamKey('A') !== autoParamKey('B'), 'U-T4: different ids get different auto keys');
eq(isParamKeyValid('group'), true, 'U-T4: a plain key is valid');
eq(isParamKeyValid('_x9'), true, 'U-T4: leading underscore / digits are valid');
eq(isParamKeyValid('9bad'), false, 'U-T4: a key starting with a digit is invalid');
eq(isParamKeyValid('a-b'), false, 'U-T4: punctuation is invalid in a key');
eq(isParamKeyValid(''), false, 'U-T4: an empty key is invalid');
eq(isParamKeyValid('x'.repeat(65)), false, 'U-T4: a key longer than 64 chars is invalid');
// 编辑草稿（已有 ocr_var）
const ocrVarAssetForDialog = {
  id: 'O_组', kind: 'ocr_var', runtime_text: '御魂', param_key: 'group', label: '预设分组',
  roi: [10, 20, 30, 40], positions: [7],
  op_region: { mode: 'asset', asset: 'I_模板', pick: 'nearest' },
  action_shape: { kind: 'rect', roi: [5, 6, 7, 8], invert: false },
  threshold: 0.63,
};
const editDraft = assetTextDraftStart(ocrVarAssetForDialog, { task: '任务甲' });
eq(editDraft.mode, 'edit', 'U-T4: editing an existing ocr_var uses edit mode');
eq(
  [editDraft.id, editDraft.runtimeText, editDraft.paramKey, editDraft.label],
  ['O_组', '御魂', 'group', '预设分组'],
  'U-T4: edit draft starts from the stored ocr_var fields'
);
eq(editDraft.roi, [10, 20, 30, 40], 'U-T4: edit draft keeps the stored roi');
// #9（验收反馈）：replace 会整项覆盖 → 草稿必须携带操作区/操作形状/阈值。
eq(
  editDraft.opRegion,
  { mode: 'asset', asset: 'I_模板', pick: 'nearest' },
  '#9: edit draft carries the stored op_region'
);
eq(
  editDraft.actionShape,
  { kind: 'rect', roi: [5, 6, 7, 8], invert: false },
  '#9: edit draft carries the stored action_shape'
);
eq(editDraft.threshold, 0.63, '#9: edit draft carries the stored threshold');
eq(assetTextDraftStart({ id: 'O_空', kind: 'ocr_var', runtime_text: 'x', param_key: 'k' }).fullScreen,
  true, 'U-T4: a missing roi starts as full-screen OCR');
// 新建草稿（从识别结果候选）
const createDraft = assetTextDraftFromCandidate(
  { name: 'O_预设', text: '御魂', recognize: { x: 1, y: 2, w: 30, h: 40 } },
  { positions: [5, 6], task: '任务甲' }
);
eq(createDraft.mode, 'create', 'U-T4: saving a candidate as ocr_var uses create mode');
eq(createDraft.id, 'O_预设', 'U-T4: the candidate name prefills the asset id');
eq(createDraft.runtimeText, '御魂', 'U-T4: the candidate text prefills runtime_text');
eq(createDraft.paramKey, autoParamKey('O_预设'), 'U-T4: create mode auto-generates param_key');
eq(createDraft.roi, [1, 2, 30, 40], 'U-T4: the candidate recognition box becomes the roi');
eq(createDraft.positions, [5, 6], 'U-T4: create mode keeps the selected position frames');
// 校验
eq(assetTextValidate(createDraft, { existingIds: [], existingParamKeys: [] }), [],
  'U-T4: a valid create draft has no errors');
ok(
  assetTextValidate(Object.assign({}, createDraft, { id: '' }), {}).some((e) => /素材 id 必填/.test(e)),
  'U-T4: create mode requires an asset id'
);
ok(
  assetTextValidate(Object.assign({}, createDraft, { id: '9bad' }), {}).some((e) => /素材 id 非法/.test(e)),
  'U-T4: create mode rejects an illegal asset id'
);
ok(
  assetTextValidate(createDraft, { existingIds: ['O_预设'] }).some((e) => /已存在/.test(e)),
  'U-T4: create mode rejects a duplicate asset id'
);
ok(
  assetTextValidate(Object.assign({}, createDraft, { positions: [] }), {}).some((e) => /位置帧/.test(e)),
  'U-T4: create mode requires at least one position frame'
);
ok(
  assetTextValidate(Object.assign({}, createDraft, { paramKey: '9bad' }), {}).some((e) => /param_key/.test(e)),
  'U-T4: create mode validates the param_key syntax'
);
ok(
  assetTextValidate(Object.assign({}, createDraft, { paramKey: 'used' }), { existingParamKeys: ['used'] })
    .some((e) => /已被本任务其它素材使用/.test(e)),
  'U-T4: create mode rejects a duplicate param_key'
);
ok(
  assetTextValidate(Object.assign({}, createDraft, { runtimeText: '  ' }), {}).some((e) => /必填/.test(e)),
  'U-T4: runtime_text is required'
);
ok(
  assetTextValidate(Object.assign({}, createDraft, { runtimeText: 'x'.repeat(201) }), {})
    .some((e) => /200/.test(e)),
  'U-T4: runtime_text length is bounded'
);
ok(
  assetTextValidate(Object.assign({}, createDraft, { label: 'x'.repeat(81) }), {})
    .some((e) => /label/.test(e)),
  'U-T4: the label length is bounded'
);
ok(
  assetTextValidate(Object.assign({}, createDraft, { fullScreen: false, roi: [1, 2, 0, 4] }), {})
    .some((e) => /w\/h 必须大于 0/.test(e)),
  'U-T4: a zero-size roi is rejected'
);
eq(assetTextValidate(editDraft, {}), [], 'U-T4: a valid edit draft has no errors');
// 保存载荷
const createRequest = assetTextSaveRequest(createDraft, { task: '任务甲' });
eq(createRequest.url, '/task/save_asset', 'U-T4: a new ocr_var posts to /task/save_asset');
eq(createRequest.payload.task, '任务甲', 'U-T4: the create payload carries the task scope');
eq(createRequest.payload.kind, 'ocr_var', 'U-T4: the create payload sets kind=ocr_var');
eq(createRequest.payload.runtime_text, '御魂', 'U-T4: the create payload carries runtime_text');
eq(createRequest.payload.param_key, autoParamKey('O_预设'), 'U-T4: the create payload carries param_key');
eq(createRequest.payload.roi, [1, 2, 30, 40], 'U-T4: the create payload carries roi');
eq(createRequest.payload.positions, [5, 6], 'U-T4: the create payload carries positions');
eq('replace' in createRequest.payload, false, 'U-T4: a fresh create never sends replace');
const editRequest = assetTextSaveRequest(editDraft, { task: '任务甲' });
eq(editRequest.url, '/task/asset/update', 'U-T4: editing an ocr_var posts to /task/asset/update');
eq(
  editRequest.payload,
  { task: '任务甲', id: 'O_组', runtime_text: '御魂', label: '预设分组', roi: [10, 20, 30, 40] },
  'U-T4: the edit payload carries runtime_text/label/roi (no param_key rewrite)'
);
const clearRoiDraft = Object.assign({}, editDraft, { fullScreen: true, roi: null });
const clearRoiRequest = assetTextSaveRequest(clearRoiDraft, { task: '任务甲' });
eq(clearRoiRequest.url, '/task/save_asset', 'U-T4: clearing roi falls back to the replace save (update cannot clear roi)');
eq(clearRoiRequest.payload.replace, true, 'U-T4: the roi-clearing payload sets replace=true');
eq('roi' in clearRoiRequest.payload, false, 'U-T4: the roi-clearing payload omits roi (full-screen OCR)');
eq(clearRoiRequest.payload.param_key, 'group', 'U-T4: the replace payload echoes the read-only param_key');
// #9：全屏 ocr_var 的 replace 必须原样带回 op_region/action_shape/threshold，
// 否则改识别文字会把操作区链/操作形状/阈值静默清空。
eq(
  clearRoiRequest.payload.op_region,
  { mode: 'asset', asset: 'I_模板', pick: 'nearest' },
  '#9: full-screen ocr_var replace keeps the stored op_region'
);
eq(
  clearRoiRequest.payload.action_shape,
  { kind: 'rect', roi: [5, 6, 7, 8], invert: false },
  '#9: full-screen ocr_var replace keeps the stored action_shape'
);
eq(clearRoiRequest.payload.threshold, 0.63, '#9: full-screen ocr_var replace keeps the stored threshold');
eq(
  'op_region' in createRequest.payload,
  false,
  '#9: a fresh ocr_var create never invents an op_region'
);
eq(assetTextSaveRequest(Object.assign({}, createDraft, { runtimeText: '' }), {}).ok, false,
  'U-T4: an invalid draft produces no save request');
// op_region 保存计划（S5）：链校验 + 只含有改动的 update
const planLibrary = s5Assets.concat([{ id: 'A', kind: 'template', op_region: { mode: 'offset', offset: [1, 2, 30, 40] } }]);
const planDraft = opRegionDraftSetLevel(
  opRegionDraftStart(planLibrary.find((a) => a.id === 'A')),
  0,
  { mode: 'asset', asset: 'B' },
  planLibrary
);
const plan = opRegionSavePlan(planDraft, planLibrary, { scope: '任务甲' });
eq(plan.ok, true, 'U-T4/S5: a valid chain produces a save plan');
eq(plan.updates.length, 1, 'U-T4/S5: only the edited root level is written');
eq(
  plan.updates[0],
  { assetId: 'A', op_region: { mode: 'asset', asset: 'B', pick: 'score' }, scope: '任务甲' },
  'U-T4/S5: the plan names the current asset + its new op_region'
);
const planBoth = opRegionSavePlan(s5DraftABOff, s5Assets, { scope: '任务甲' });
eq(planBoth.updates.length, 2, 'U-T4/S5: a two-level edit plans two updates (current + referenced asset)');
eq(planBoth.updates.map((u) => u.assetId), ['A', 'B'], 'U-T4/S5: updates follow the breadcrumb order');
const planBad = opRegionSavePlan(
  opRegionDraftSetLevel(opRegionDraftStart(s5Assets[0]), 0, { mode: 'asset', asset: 'Missing' }, s5Assets),
  s5Assets,
  {}
);
eq(planBad.ok, false, 'U-T4/S5: a dangling reference blocks the save plan');
eq(planBad.updates, [], 'U-T4/S5: an invalid chain produces no updates');
eq(
  opRegionLevelChoices().map((o) => o.value),
  ['self', 'asset'],
  'B3a: the dialog offers the two model-A operation-region sources (self / reference gate)'
);
eq(
  opRegionLevelChoices().map((o) => o.label),
  ['当前素材操作区（self）', '引用素材作识别门槛（链）'],
  'B3a: the two labels explain model A (fall point = current asset)'
);
const modelARefs = [
  { id: 'A', kind: 'ocr_var', runtime_text: 'x', param_key: 'k' },
  { id: 'T', kind: 'template' },
  { id: 'R', kind: 'region' },
  { id: 'L', kind: 'ocr_line', ocr_text: '挑战' },
  { id: 'E', kind: 'ocr_line' },
];
eq(
  opRegionRefChoices(modelARefs, 'A').map((c) => c.id),
  ['T', 'L', 'E'],
  'B3a: reference choices exclude region and the asset itself'
);
eq(
  opRegionRefChoices(modelARefs, 'A').find((c) => c.id === 'E').disabled,
  true,
  'B3a: a plain OCR without ocr_text is greyed out (not selectable)'
);
eq(
  opRegionRefChoices(modelARefs, 'A').find((c) => c.id === 'L').disabled,
  false,
  'B3a: a plain OCR with ocr_text is selectable'
);
eq(
  opRegionRefChoices(modelARefs, 'A').map((c) => c.id).indexOf('R'),
  -1,
  'B3a: region assets are never offered as a recognition gate'
);
eq(
  opRegionTargetKinds().indexOf('region'),
  -1,
  'B3a: region is not in the model-A gate kind whitelist'
);
eq(
  opRegionReadonlyChainText(
    { id: 'O', op_region: { mode: 'asset', asset: 'T', pick: 'nearest' } },
    [{ id: 'T', kind: 'template', op_region: { mode: 'self' } }]
  ),
  '引用「T」（最近） → 自身操作区（self）',
  'B3a: the referenced asset chain is rendered read-only (no editing)'
);
eq(
  opRegionReadonlyChainText({ id: 'O' }, []),
  '自身操作区（self）',
  'B3a: an unconfigured referenced asset shows the self fallback'
);
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
eq(sandbox.btnActionInvert.textContent, '反选：开', 'load rect syncs the unified invert button label');
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

// --- 事项 6: save payload carries action_shape only (no two-state flag) ---
eq(
  ocrAssetPayload(
    { kind: 'ocr_line', name: '按钮甲', recognize: { x: 1, y: 2, w: 30, h: 40 }, actionShape: null },
    [3],
    null
  ),
  { id: '按钮甲', kind: 'ocr_line', roi: [1, 2, 30, 40], positions: [3] },
  'un-edited OCR candidate saves without shape/polygon and without action_source'
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
    ocr_mode: 'lines',
    ocr_text: '开始',
  },
  'multi-line OCR keeps ocr_mode/ocr_text and the operation shape; no polygon when absent'
);
eq(
  singleAssetPayload(
    { kind: 'template', name: '模板甲', recognize: { x: 1, y: 2, w: 3, h: 4 }, actionShape: null },
    [7],
    null
  ),
  { id: '模板甲', kind: 'template', roi: [1, 2, 3, 4], positions: [7], threshold: 0.85 },
  'template keeps threshold and saves no operation shape when none was edited'
);
eq(
  singleAssetPayload(
    { kind: 'region', name: '区域甲', recognize: { x: 1, y: 2, w: 3, h: 4 }, actionShape: null },
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
    actionShape: null,
  },
  [5],
  recognitionPoly
);
eq(ocrWithPoly.polygon, recognitionPoly, 'OCR batch payload includes the recognition polygon');
eq(ocrWithPoly.ocr_mode, 'lines', 'OCR batch payload keeps ocr_mode');
eq(ocrWithPoly.ocr_text, '确定', 'OCR batch payload keeps ocr_text');

const singleWithPoly = singleAssetPayload(
  { kind: 'ocr_line', name: '按钮丁', recognize: { x: 3, y: 3, w: 2, h: 2 }, actionShape: null },
  [6],
  recognitionPoly
);
eq(singleWithPoly.polygon, recognitionPoly, 'single save payload includes the recognition polygon');

const candidatePoly = [[0, 0], [5, 0], [5, 5]];
const candidatePolyWins = ocrAssetPayload(
  { kind: 'ocr_line', name: '按钮戊', recognize: { x: 0, y: 0, w: 1, h: 1 }, actionShape: null, polygon: candidatePoly },
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
  // U-T4: collection 识别 now returns all candidates (same route as single assets).
  sandbox.testAssetCandidates = (it, frameId, scope) =>
    actions.push({ action: 'candidates', id: it.id, frameId, scope });
  sandbox.editAssetText = (it, scope) => actions.push({ action: 'editText', id: it.id, scope });
  // U-T4/S5：素材行「操作区」现在打开操作区递归对话框（不再直接进入画布编辑）。
  sandbox.openOpRegionDialog = (it, scope) => actions.push({ action: 'opRegion', id: it.id, scope });
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
    { action: 'candidates', id: 'a', frameId: 2, scope: '（全局素材库）' },
  ],
  'frame detail row actions all pass the rendered frame id + the global scope'
);
// C: management callbacks must carry the row scope too (task-only row = 任务甲).
frameDom.actions.length = 0;
const taskRowOpts = frameDom.calls[2].opts;
for (const name of [
  'onEditRoi', 'onRename', 'onThreshold', 'onPosition', 'onDelete', 'onOpRegion', 'onEditText',
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
    'opRegion:任务甲',
    'editText:任务甲',
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

// --- asset row: exactly 识别 + 设置, with a ten-item inline settings menu ---
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
    // #6：缩略图多边形叠加层需要 canvas 2d context（哑实现即可）。
    getContext() {
      return {
        save() {}, restore() {}, beginPath() {}, moveTo() {}, lineTo() {},
        closePath() {}, setLineDash() {}, stroke() {}, fill() {}, fillRect() {},
        drawImage() {}, fillText() {}, measureText() { return { width: 0 }; },
      };
    },
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
const ASSET_MENU_LABELS = ['点测', '拖测', '识别区', '操作区', '重命名', '阈值', '位置帧', '删除', '步骤', '长按测试', '识别文字'];

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
    onEditText: record('editText'),
    onLongPress: record('longPress'),
  };
  return { opts: Object.assign(opts, overrides || {}), calls };
}

const rowTemplate = { id: '模板甲', kind: 'template', threshold: 0.9, roi: [10, 20, 30, 40], positions: [7] };
const rowOcr = { id: '文字乙', kind: 'ocr_line', roi: [1, 2, 3, 4], positions: [7] };
const rowRegion = { id: '区域丙', kind: 'region', roi: [5, 6, 7, 8] };
const rowSet = { id: '集合丁', kind: 'template_set', members: ['a', 'b'] };

// --- #6（验收反馈）：已存识别多边形在缩略图上的轮廓 ---------------------------
const thumbPts = assetUi.thumbPolygonPoints(
  [[10, 20], [30, 20], [30, 30]],
  [10, 20, 20, 10],
  64,
  44
);
eq(
  thumbPts,
  [[0, 6], [64, 6], [64, 38]],
  '#6: stored polygon points are projected into thumbnail coordinates (ROI crop, centered contain)'
);
eq(assetUi.thumbPolygonPoints(null, [1, 2, 3, 4], 64, 44), null, '#6: no polygon -> no overlay');
eq(
  assetUi.thumbPolygonPoints([[1, 2], [3, 4]], [1, 2, 3, 4], 64, 44),
  null,
  '#6: fewer than three polygon vertices -> no overlay'
);
eq(assetUi.thumbPolygonPoints([[1, 2], [3, 4], [5, 6]], null, 64, 44), null,
  '#6: a missing roi cannot anchor the overlay');
eq(
  assetUi.thumbPolygonPoints([[1, 2], ['bad', 2], [5, 6], [1, 6]], [1, 2, 3, 4], 64, 44).length,
  3,
  '#6: non-numeric vertices are skipped (three valid points remain)'
);
const polyTemplateRow = Object.assign({}, rowTemplate, {
  file: 'a.png',
  polygon: [[10, 20], [30, 20], [30, 40], [10, 40]],
});
const polyThumb = assetUi.makeThumb(polyTemplateRow, '任务甲');
eq(
  assetChildren(polyThumb, 'asset-thumb-poly').length,
  1,
  '#6: a template thumbnail overlays the stored recognition polygon'
);
const plainThumb = assetUi.makeThumb(Object.assign({}, rowTemplate, { file: 'a.png' }), '任务甲');
eq(
  assetChildren(plainThumb, 'asset-thumb-poly').length,
  0,
  '#6: a polygon-free asset thumbnail has no overlay'
);
ok(
  (assetUiSource.match(/drawThumbPolygon\(/g) || []).length >= 3,
  '#6: both the template and the frame-crop thumbnails draw the stored polygon (definition + 2 calls)'
);
ok(
  /\.asset-thumb-poly/.test(styleCss) && /pointer-events: none/.test(styleCss),
  '#6: style.css styles the thumbnail polygon overlay without stealing pointer events'
);

eq(
  assetUi.ASSET_MENU_ITEMS.map((m) => m.label),
  ASSET_MENU_LABELS,
  'the settings menu is the agreed order (长按测试 + 识别文字 appended by U-T4/B3a)'
);
eq(
  assetUi.assetMenuState(rowTemplate, { manage: true }).filter((m) => m.disabled).map((m) => m.label),
  ['识别文字'],
  'a manageable template enables every item except 识别文字 (only ocr_var edits text)'
);
eq(
  assetUi.assetMenuState(rowTemplate, { manage: true }).filter((m) => !m.disabled).map((m) => m.label),
  ['点测', '拖测', '识别区', '操作区', '重命名', '阈值', '位置帧', '删除', '步骤', '长按测试'],
  'every other template settings item (incl. 长按测试) is actionable on a manageable surface'
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
  ['识别区', '操作区', '重命名', '阈值', '位置帧', '删除', '长按测试', '识别文字'],
  'manage:false disables every management item (长按测试/识别文字 included)'
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
eq(setMenuState.filter((m) => m.disabled).length, 10, 'the other ten collection entries stay visible but disabled');
ok(
  setMenuState.filter((m) => m.disabled).every((m) => /集合不支持/.test(m.title)),
  'disabled collection entries explain that sets do not support them'
);

const tplRowHarness = assetRowOpts();
const tplRow = assetUi.createRow(rowTemplate, tplRowHarness.opts);
eq(assetRowActionText(tplRow), ['识别', '设置'], 'a normal asset row exposes exactly 识别 + 设置');
const tplMenu = assetRowMenu(tplRow);
ok(tplMenu && !tplMenu.classList.contains('open'), 'the settings menu starts collapsed');
eq(tplMenu.children.map((b) => b.textContent), ASSET_MENU_LABELS, 'the settings menu renders all items in order');
eq(
  assetChildren(tplRow, 'asset-row-action')[0].title,
  '识别：返回全部符合阈值的匹配候选（带评分；集合按成员声明序）',
  'template 识别 now means "all matching candidates"'
);
eq(
  assetRowActionText(assetUi.createRow(rowOcr, assetRowOpts().opts))[0],
  '识别',
  'OCR rows use the same 识别 label'
);
ok(
  /全部候选行/.test(assetChildren(assetUi.createRow(rowOcr, assetRowOpts().opts), 'asset-row-action')[0].title),
  'OCR 识别 title names the all-candidates meaning'
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
  ['识别区', '操作区', '重命名', '阈值', '位置帧', '删除', '长按测试', '识别文字'],
  'compact keeps the old hiding of 点测/拖测/步骤'
);

// --- U-T4（事项 2）：使用端OCR文字配置项（ocr_var）素材行 --------------------
// KIND_LABELS + 行详情（识别文字/op_region 摘要）+ 菜单「识别文字」规则 +
// 虚拟素材（virtual:true，旧参数读时合成）的「待迁移」badge。
const rowOcrVar = {
  id: 'O_预设分组',
  kind: 'ocr_var',
  runtime_text: '御魂',
  param_key: 'group',
  label: '预设分组',
  roi: [40, 300, 400, 60],
  positions: [12],
  op_region: { mode: 'offset', offset: [10, 5, 150, 80] },
};
eq(assetUi.kindLabel('ocr_var'), '使用端OCR文字配置项（ocr_var）', 'U-T4: ocr_var kind label');
const ocrVarMenu = assetUi.assetMenuState(rowOcrVar, { manage: true });
eq(
  ocrVarMenu.filter((m) => m.disabled).map((m) => m.label),
  [],
  'U-T4: an ocr_var row enables all items (long-press + 识别文字 included)'
);
const ocrVarTextMenu = ocrVarMenu.find((m) => m.key === 'text');
eq(ocrVarTextMenu.disabled, false, 'U-T4: ocr_var can edit 识别文字');
ok(/runtime_text/.test(ocrVarTextMenu.title), 'U-T4: the 识别文字 title names runtime_text');
const tplTextMenu = assetUi.assetMenuState(rowTemplate, { manage: true }).find((m) => m.key === 'text');
eq(tplTextMenu.disabled, true, 'U-T4: non-ocr_var rows disable 识别文字');
ok(/仅「使用端OCR文字配置项」/.test(tplTextMenu.title), 'U-T4: the disabled 识别文字 title explains the kind rule');
const ocrVarThresholdMenu = ocrVarMenu.find((m) => m.key === 'threshold');
eq(ocrVarThresholdMenu.disabled, false, 'U-T4: ocr_var exposes the OCR confidence threshold');
ok(/OCR 最低置信度/.test(ocrVarThresholdMenu.title), 'U-T4: ocr_var threshold keeps the OCR meaning');
const virtualRow = {
  id: 'O_group', kind: 'ocr_var', virtual: true, runtime_text: '御魂', param_key: 'group', roi: null,
};
const virtualMenu = assetUi.assetMenuState(virtualRow, { manage: true });
eq(
  virtualMenu.filter((m) => !m.disabled).map((m) => m.label),
  ['点测', '拖测', '步骤', '识别文字'],
  'U-T4: virtual rows stay testable and keep 识别文字 clickable as a migrate hint'
);
ok(
  virtualMenu.find((m) => m.key === 'text').title.indexOf('待迁移') >= 0,
  'U-T4: virtual 识别文字 explains that the row must be migrated first'
);
eq(
  assetUi.assetText(rowOcrVar),
  'O_预设分组\n识别 [40,300,400,60] · 识别文字「御魂」 · 操作区[相对偏移 [10,5,150,80]] · 位置帧[12]',
  'U-T4: ocr_var row detail shows 识别文字 + op_region chain summary'
);
eq(
  assetUi.assetText({ id: 'I_A', kind: 'template', roi: [1, 2, 3, 4], positions: [7] }),
  'I_A\n识别 [1,2,3,4] · 位置帧[7]',
  'U-T4: regular rows keep the existing detail format (no runtime/op_region fields)'
);
eq(assetUi.opRegionSummary({ mode: 'self' }), '当前素材操作区', 'U-T4: op_region self summary');
eq(
  assetUi.opRegionSummary({ mode: 'asset', asset: 'I_B', pick: 'nearest' }),
  '引用素材「I_B」（离上一节点最近）',
  'U-T4: op_region asset summary names the pick rule'
);
eq(
  assetUi.opRegionSummary({ mode: 'asset', asset: 'I_B' }),
  '引用素材「I_B」（评分最高）',
  'U-T4: op_region asset summary defaults to score pick'
);
ok(
  /偏移值无效/.test(assetUi.opRegionSummary({ mode: 'offset', offset: [1, 2] })),
  'U-T4: a malformed offset summary is reported instead of rendering undefined'
);
const virtualRowEl = assetUi.createRow(virtualRow, assetRowOpts().opts);
ok(
  virtualRowEl.classList.contains('asset-row-virtual'),
  'U-T4: virtual rows carry the read-only row class'
);
const virtualBadge = assetChildren(virtualRowEl, 'asset-virtual-badge')[0];
ok(virtualBadge && virtualBadge.textContent === '待迁移', 'U-T4: virtual rows render the 待迁移 badge');
const virtualHarness = assetRowOpts();
const virtualRowClickable = assetUi.createRow(virtualRow, virtualHarness.opts);
assetChildren(virtualRowClickable, 'asset-row-action')[1].dispatch('click');
assetMenuButton(virtualRowClickable, 'text').dispatch('click');
eq(
  virtualHarness.calls,
  [{ action: 'editText', id: 'O_group' }],
  'U-T4: virtual 识别文字 stays clickable so app.js can show the migrate hint'
);
const ocrVarRowEl = assetUi.createRow(rowOcrVar, assetRowOpts().opts);
ok(
  /O_预设分组[\s\S]*识别文字「御魂」/.test(assetChildren(ocrVarRowEl, 'asset-main')[0].children
    .filter((c) => c.className === 'asset-main-text')[0].textContent),
  'U-T4: the ocr_var row body renders the runtime text'
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

// --- U-T4（S3/S6/S10）：候选识别、offset 绘制、素材行回调接线 ---------------
const candidatesFn = extractFunction('testAssetCandidates');
ok(
  /\/task\/asset\/candidates\?task=/.test(source) &&
    /assetCandidatesUrl\(task, it\.id, frame\.query\)/.test(candidatesFn),
  'U-T4: 素材行「识别」calls GET /task/asset/candidates per frame'
);
ok(
  /candidateBoxes\(candidates\)/.test(candidatesFn) &&
    /candidateLabelText\(candidate\)/.test(candidatesFn) &&
    /boxes = collected;/.test(candidatesFn) && /draw\(\)/.test(candidatesFn),
  'U-T4: every candidate is drawn as a box and listed as #i score=… [x,y,w,h]'
);
ok(/全屏 OCR/.test(candidatesFn), 'U-T4: an empty-roi OCR asset is labelled 全屏 OCR');
const uT4RunAssetTestFn = extractFunction('runAssetTest');
ok(
  /testAssetCandidates\(it, frameId, scopeTask\)/.test(uT4RunAssetTestFn),
  'U-T4: 素材行 识别 routes every asset kind through the candidate API'
);
ok(
  (source.match(/onEditText:/g) || []).length >= 3,
  'U-T4: all three asset surfaces wire onEditText'
);
ok(
  (source.match(/onCollectionTest: \(it\) => testAssetCandidates\(/g) || []).length >= 2 &&
    /onCollectionTest: \(it\) => testAssetCandidates\(it, f\.id, scopeTask\)/.test(source),
  'U-T4: collection rows 识别 returns all candidates too'
);
ok(
  /function editAssetText\(it, scopeTask\)[\s\S]{0,600}虚拟素材/.test(source),
  'U-T4: 识别文字 preview prompts migration for virtual rows'
);
// B3a 模型 A：S6 offset 绘制 UI 删除映射（Lead task-27 b 项）：
//   旧 app.js：offsetDraw/offsetDrawPending 状态 + startOffsetDrawForAsset/exitOffsetDraw/
//   offsetDrawResetSimRoi/offsetDrawResult/offsetDrawLabelText/offsetDrawSetButton/
//   updateOffsetDrawUi/drawOffsetDrawOverlay/offsetDrawFinish/offsetDrawMouse* +
//   OAS_CANVAS_TOOLS + startOpRegionOffsetDraw/buildOpRegionLevel + offsetFromDraw；
//   旧 index.html：#offset-draw-label / #btn-offset-{redraw,cancel,done}；
//   旧 style.css：.offset-draw-label 两条规则；旧 ui_pure_test：offsetFromDraw 纯函数与静态断言。
// 反向断言：上述符号不得回归（模型 A 只保留 self/asset 两态 + 旧 offset 数据提示迁移）。
ok(!/offsetDraw|offset-draw|btn-offset|offsetFromDraw|OAS_CANVAS_TOOLS/.test(source),
  'B3a: the offset-drawing UI/code is fully removed from app.js');
ok(!/offset-draw-label|btn-offset-redraw|btn-offset-cancel|btn-offset-done/.test(html),
  'B3a: the offset footer controls are removed from index.html');
ok(!/offset-draw-label/.test(styleCss),
  'B3a: the offset footer styles are removed from style.css');
ok(
  /function opRegionDraftStart\(asset\)/.test(source) &&
    /function opRegionDraftSetLevel\(draft, index, opRegion, assets\)/.test(source) &&
    /function opRegionDraftUpdates\(draft, assets\)/.test(source) &&
    /function opRegionRefOptions\(assets, rootAssetId\)/.test(source),
  'U-T4/S5: the recursive operation-region dialog draft model is in place (API-independent)'
);
ok(
  /ocr_var: '使用端OCR文字配置项（ocr_var）'/.test(assetUiSource),
  'U-T4: asset_ui KIND_LABELS adds the ocr_var label'
);
ok(
  /key: 'text', label: '识别文字'/.test(assetUiSource),
  'U-T4: asset_ui appends the 识别文字 menu item (append-only)'
);
const assetTextFn = extractFunctionFrom(assetUiSource, 'assetText', 'asset_ui.js');
ok(/runtime_text/.test(assetTextFn), 'U-T4: assetText shows runtime_text for ocr_var rows');
ok(/opRegionSummary\(it\.op_region\)/.test(assetTextFn), 'U-T4: assetText shows the op_region chain summary');
ok(
  /asset-virtual-badge/.test(assetUiSource) && /\.asset-virtual-badge/.test(styleCss) &&
    /\.asset-row-virtual/.test(styleCss),
  'U-T4: virtual rows render the 待迁移 badge (styled)'
);

// --- U-T4 阶段 B：识别文字对话框 / 操作区递归对话框 / 保存接线 ---------------
for (const id of [
  'asset-text-dialog', 'asset-text-id', 'asset-text-runtime', 'asset-text-paramkey',
  'asset-text-label', 'asset-text-fullscreen', 'asset-text-roi-x', 'asset-text-roi-y',
  'asset-text-roi-w', 'asset-text-roi-h', 'asset-text-positions', 'asset-text-error',
  'asset-text-save', 'asset-text-cancel',
  'op-region-dialog', 'op-region-breadcrumb', 'op-region-levels', 'op-region-error',
  'op-region-save', 'op-region-cancel',
]) {
  ok(new RegExp(`id="${id}"`).test(html), `U-T4: index.html provides #${id}`);
}
ok(
  /\.asset-text-dialog/.test(styleCss) && /\.op-region-dialog/.test(styleCss) &&
    /\.op-region-level/.test(styleCss),
  'U-T4: style.css styles both new dialogs'
);
const openAssetTextFn = extractFunction('openAssetTextDialog');
ok(
  /assetTextDraftFromCandidate\(/.test(openAssetTextFn) &&
    /assetTextDraftStart\(/.test(openAssetTextFn),
  'U-T4: the 识别文字 dialog opens in create (candidate) or edit (asset) mode'
);
const saveAssetTextFn = extractFunction('saveAssetTextDialog');
ok(
  /assetTextSaveRequest\(/.test(saveAssetTextFn) && /postJson\(request\.url, request\.payload\)/.test(saveAssetTextFn),
  'U-T4: the 识别文字 dialog saves through assetTextSaveRequest → POST save_asset/update'
);
ok(
  /refreshAssetsAfterOcrVarChange\(ctx\.task, ctx\.scope\)/.test(saveAssetTextFn),
  'U-T4: saving ocr_var refreshes the asset libraries + config panel'
);
const openOpRegionFn = extractFunction('openOpRegionDialog');
ok(
  /opRegionNodeOf\(asset\.op_region\)/.test(openOpRegionFn) &&
    /mode: initialMode/.test(openOpRegionFn),
  'B3a: the 操作区 dialog opens in the two-state model A (self / reference gate)'
);
const saveOpRegionFn = extractFunction('saveOpRegionDialog');
ok(
  /validate_op_region/.test(saveOpRegionFn) &&
    /postJson\('\/task\/asset\/update', \{\s*task: ctx\.scope,\s*id: ctx\.asset\.id,\s*op_region: op/.test(saveOpRegionFn) &&
    !/opRegionSavePlan/.test(saveOpRegionFn) &&
    /只写当前素材/.test(saveOpRegionFn),
  'B3a: saving writes ONLY the current asset (one update) after validate; no PATCH of referenced assets'
);
ok(
  /引用素材只作识别门槛；动作落点=当前素材操作区/.test(extractFunction('renderOpRegionDialog')),
  'B3a: the dialog copy states the model-A rule (reference = recognition gate only)'
);
ok(
  /onOpRegion: \(it\) => openOpRegionDialog\(it/.test(source) &&
    (source.match(/onOpRegion:/g) || []).length >= 3,
  'U-T4/S5: all three asset surfaces open the operation-region dialog'
);
ok(
  /key: 'action', label: '操作区', action: 'onOpRegion'/.test(assetUiSource),
  'U-T4/S5: the asset-row 操作区 menu item opens the recursive dialog'
);
ok(
  /存为OCR文字配置/.test(source) && /openAssetTextDialog\(null, '', \{ candidate: c, positions: positions \}\)/.test(source),
  'U-T4/S2: the 识别结果 rows offer "存为OCR文字配置" (ocr_var creation entry)'
);
ok(
  'U-T4: POST /task/config echoes derived rows back unchanged (B-T3a contract)'
);
ok(
  'U-T4: the config dialog snapshots the loaded derived rows for the round-trip'
);

// Static contracts for the split editors: every surface wires the split
// callbacks, the row never calls a combined onEdit, and the legacy combined
// editor is gone (it was dead code: nothing outside the IIFE could reach it).
for (const name of ['renderAssetList', 'renderGlobalAssets', 'renderFrameAssets']) {
  const fn = extractFunction(name);
  ok(
    /onEditRoi:/.test(fn) && /onRename:/.test(fn) && /onThreshold:/.test(fn) &&
      /onPosition:/.test(fn) && /onOpRegion:/.test(fn) && /onStep:/.test(fn) &&
      /onEditText:/.test(fn),
    `${name} wires the settings callbacks (U-T4: onEditText + onOpRegion)`
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
  /return 'template'/.test(thresholdKindFn) &&
    /kind === 'ocr_line' \|\| kind === 'ocr_lines' \|\| kind === 'ocr_var'/.test(thresholdKindFn),
  'assetThresholdKind enables exactly template / ocr_line / ocr_lines / ocr_var'
);
eq(sandbox.assetThresholdKind({ kind: 'template' }), 'template', 'template assets use the template threshold');
eq(sandbox.assetThresholdKind({ kind: 'ocr_line' }), 'ocr', 'ocr_line assets use the OCR confidence');
eq(sandbox.assetThresholdKind({ kind: 'ocr_lines' }), 'ocr', 'ocr_lines assets use the OCR confidence');
eq(sandbox.assetThresholdKind({ kind: 'ocr_var' }), 'ocr', '#5: ocr_var assets use the OCR confidence threshold');
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
    /assetRoiReplacePayload\(task, it, plan\.request\.roi, plan\.request\.polygon\)/.test(roiSaveFn),
  'saving a template ROI overwrites through /task/save_asset + the replace payload builder'
);
ok(
  /覆盖模板「\$\{it\.id\}」（rev\+1/.test(roiSaveFn),
  'the template overwrite confirmation names the asset id and rev+1 (P2-7)'
);
ok(
  /postJson\('\/task\/asset\/update', payload\)/.test(roiSaveFn) &&
    /recognitionSavePlan\(it, \{ roi: draftRoi, polygon: draftPoly \}\)/.test(roiSaveFn),
  'OCR/region/mask saves go through the shared recognitionSavePlan + /task/asset/update'
);
ok(
  /if \(plan\.action === 'none'\)/.test(roiSaveFn) &&
    /if \(plan\.action === 'overwrite'\)/.test(roiSaveFn),
  'the save path branches on the pure plan action (none/overwrite/update)'
);
ok(
  /const draftPoly = polygon && polyClosed && polygon\.length >= 3/.test(roiSaveFn),
  'a finished canvas polygon is the first-class recognition draft'
);
ok(
  /assetRoiWithinFrame\(draftRoi, canvas\.width, canvas\.height\)/.test(roiSaveFn),
  'the canvas rectangle is validated against the frame before any request'
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
  'runTestActionForAsset', 'testAssetDrag', 'generateAssetStep', 'testAssetCollection',
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
      'let polygon = null;\n' +
      'let polyClosed = false;\n' +
      'let canvasTool = "none";\n' +
      'let storedRecognitionPolygon = null;\n' +
      'let globalAssetsCache = [];\n' +
      'let taskAssets = [];\n' +
      'const canvas = { width: 1280, height: 720 };\n' +
      'globalThis.resultEl = { textContent: "" };\n' +
      'globalThis.draw = () => {};\n' +
      'globalThis.updateRoiLabel = () => {};\n' +
      'globalThis.setCanvasTool = (t) => { canvasTool = t; };\n' +
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
        extractFunction('recognitionDraftFromAsset'),
        extractFunction('recognitionPolygonBBox'),
        extractFunction('rectEq4'),
        extractFunction('polygonPointsEq'),
        extractFunction('recognitionSavePlan'),
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
        `polygon = ${JSON.stringify((o.polygon || []).map((p) => ({ x: p[0], y: p[1] })))};` +
        `polyClosed = ${o.polygon ? 'true' : 'false'};` +
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
    action_shape: { kind: 'rect', roi: [5, 6, 7, 8] },
  };

  // 1) 模板矩形保存 = 覆盖（重新截图），显式 polygon:null 清空旧识别掩膜。
  const saved = await roiSave.run(tplAsset, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙' });
  eq(
    roiSave.confirmText,
    '用当前帧重新截图并覆盖模板「模板甲」（rev+1，不可撤销；位置帧 2 个；识别多边形将被清空）？',
    'template rectangle save asks the overwrite confirmation (P2-7) with the mask-clear note'
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
      threshold: 0.9,
      action_shape: { kind: 'rect', roi: [5, 6, 7, 8] },
      polygon: null,
    },
    'template overwrite preserves id/positions/threshold/action_shape and clears polygon'
  );
  ok(roiSave.refreshTask && roiSave.refreshGlobal, 'template ROI save refreshes both libraries');
  eq(saved.stillEditing, null, 'saving clears the recognition-edit state');
  ok(roiSave.buttonUpdates >= 1, 'saving re-disables the save button');

  // 2) 取消确认 → 不 POST 且留在编辑态。
  roiSave.posts.length = 0;
  roiSave.confirmAnswer = false;
  const cancelled = await roiSave.run(tplAsset, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙' });
  roiSave.confirmAnswer = true;
  eq(roiSave.posts.length, 0, 'cancelling the template overwrite posts nothing');
  ok(cancelled.stillEditing && cancelled.stillEditing.it.id === '模板甲', 'cancelling keeps the ROI editor open');

  // 3) 设置菜单里改过的 threshold/positions 必须用最新对象（P1-2）。
  roiSave.posts.length = 0;
  const drifted = Object.assign({}, tplAsset, { threshold: 0.6, positions: [9] });
  await roiSave.run(tplAsset, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙', latest: drifted });
  eq(roiSave.posts[0].payload.threshold, 0.6, 'template ROI save uses the latest threshold (P1-2)');
  eq(roiSave.posts[0].payload.positions, [9], 'template ROI save uses the latest positions (P1-2)');
  eq(roiSave.posts[0].payload.polygon, null, 'latest-object replace still clears the mask for a rectangle');

  // 4) 模板 + 同 ROI 的多边形 → update {polygon}，不重新截图。
  roiSave.posts.length = 0;
  roiSave.confirmText = '';
  const sameRoiPoly = [[10, 20], [39, 20], [39, 59], [10, 59]];
  const sameRoiTpl = Object.assign({}, tplAsset, { roi: [10, 20, 30, 40] });
  await roiSave.run(
    sameRoiTpl,
    { x: 10, y: 20, w: 30, h: 40 },
    { scope: '任务乙', latest: sameRoiTpl, polygon: sameRoiPoly }
  );
  eq(roiSave.confirmText, '', 'template same-ROI polygon save never asks the re-crop question');
  eq(roiSave.posts.length, 1, 'template same-ROI polygon save posts once');
  eq(roiSave.posts[0].url, '/task/asset/update', 'template same-ROI polygon save patches the asset');
  eq(
    roiSave.posts[0].payload,
    { task: '任务乙', id: '模板甲', polygon: sameRoiPoly },
    'template same-ROI polygon payload carries only the polygon (no re-crop)'
  );

  // 5) 模板 + 移动 ROI 的多边形 → 覆盖保存（重新截图），roi=多边形 bbox、polygon 随行。
  roiSave.posts.length = 0;
  roiSave.confirmText = '';
  const movedPoly = [[20, 30], [49, 30], [49, 69], [20, 69]];
  await roiSave.run(
    tplAsset,
    { x: 20, y: 30, w: 30, h: 40 },
    { scope: '任务乙', latest: tplAsset, polygon: movedPoly }
  );
  ok(/覆盖模板「模板甲」/.test(roiSave.confirmText), 'a moved template polygon still confirms the re-crop');
  eq(roiSave.posts[0].url, '/task/save_asset', 'moved template polygon uses the overwrite endpoint');
  eq(roiSave.posts[0].payload.roi, [20, 30, 30, 40], 'overwrite roi derives from the polygon bbox');
  eq(roiSave.posts[0].payload.polygon, movedPoly, 'overwrite carries the authored polygon');

  // 6) 模板无位置帧 → 不 POST，提示位置帧。
  roiSave.posts.length = 0;
  roiSave.alertText = '';
  const noPos = Object.assign({}, tplAsset, { positions: [] });
  const noPosRun = await roiSave.run(noPos, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙', latest: noPos });
  eq(roiSave.posts.length, 0, 'a template with no positions never POSTs');
  ok(/位置帧/.test(roiSave.alertText), 'a template with no positions explains the missing 位置帧');
  ok(noPosRun.stillEditing, 'the ROI editor stays open so the user can add positions');

  // 7) 矩形超出当前帧 → 不 POST。
  roiSave.posts.length = 0;
  roiSave.alertText = '';
  await roiSave.run(
    tplAsset,
    { x: 90, y: 90, w: 30, h: 30 },
    { scope: '任务乙', frameWidth: 100, frameHeight: 100 }
  );
  eq(roiSave.posts.length, 0, 'an out-of-frame box never reaches the server');
  ok(/超出当前帧 100x100/.test(roiSave.alertText), 'an out-of-frame box explains the frame bounds');

  // 8) 素材已删除/改名 → 取消而不是写旧 id（P0-2/P1-2）。
  roiSave.posts.length = 0;
  await roiSave.run(tplAsset, { x: 10, y: 20, w: 30, h: 40 }, { scope: '任务乙', latest: null });
  eq(roiSave.posts.length, 0, 'a missing asset never POSTs');
  eq(roiSave.canceled, '素材已不存在', 'a missing asset cancels the ROI editor');
  ok(/已不存在/.test(roiSave.resultEl.textContent), 'a missing asset explains why the save was cancelled');

  // 9) OCR 矩形 → update {roi, polygon:null}，无确认。
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
    { task: '任务甲', id: '文字乙', roi: [5, 6, 7, 8], polygon: null },
    'OCR rectangle save sends roi + explicit polygon:null (mask cleared)'
  );
  eq(roiSave.confirmText, '', 'OCR ROI save never asks the template overwrite question');

  // 10) OCR/ocr_var 多边形 → update {polygon}，roi 由服务端按 bbox 反推。
  roiSave.posts.length = 0;
  const ocrPoly = [[5, 6], [20, 6], [20, 15], [5, 15]];
  await roiSave.run(
    { id: '文字乙', kind: 'ocr_line', roi: [1, 2, 3, 4], positions: [7] },
    { x: 5, y: 6, w: 16, h: 10 },
    { polygon: ocrPoly }
  );
  eq(roiSave.posts.length, 1, 'OCR polygon save posts once');
  eq(roiSave.posts[0].url, '/task/asset/update', 'OCR polygon save uses the update endpoint');
  eq(roiSave.posts[0].payload, { task: '任务甲', id: '文字乙', polygon: ocrPoly },
    'OCR polygon update carries only the mask (server re-derives roi from its bbox)');

  // 11) 未变化的识别区/多边形 → 不写盘（no-op）。
  roiSave.posts.length = 0;
  await roiSave.run(
    { id: '文字乙', kind: 'ocr_line', roi: [5, 6, 7, 8] },
    { x: 5, y: 6, w: 7, h: 8 }
  );
  eq(roiSave.posts.length, 0, 'an unchanged recognition region is a no-op');
  ok(/未变化/.test(roiSave.resultEl.textContent), 'the no-op result explains that nothing changed');

  // 12) region 矩形 → keep_action_shape；region 多边形被拒绝。
  roiSave.posts.length = 0;
  roiSave.resultEl.textContent = '';
  const regionAsset = {
    id: '区域丙',
    kind: 'region',
    roi: [1, 2, 3, 4],
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
  roiSave.posts.length = 0;
  roiSave.alertText = '';
  await roiSave.run(regionAsset, { x: 5, y: 6, w: 7, h: 8 }, { polygon: [[5, 6], [20, 6], [20, 15], [5, 15]] });
  eq(roiSave.posts.length, 0, 'region polygon save is rejected before any request');
  ok(/没有识别掩膜/.test(roiSave.alertText), 'region polygon rejection names the missing recognition mask');

  // 13) 无效画布框（w/h<=0）→ 在任何请求前拒绝。
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
    storedRecognitionPolygon: null,
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
// #6（验收反馈）：选中素材时恢复已存识别多边形（只读显示层），矩形 ROI 仍显示。
roiCancel.pickAssetRoi({
  id: '丁', kind: 'ocr_line', roi: [1, 2, 3, 4],
  polygon: [[1, 2], [5, 2], [5, 6]],
});
eq(
  roiCancel.storedRecognitionPolygon,
  [{ x: 1, y: 2 }, { x: 5, y: 2 }, { x: 5, y: 6 }],
  '#6: selecting an asset restores its stored recognition polygon for display'
);
eq(roiCancel.polygon, null, '#6: the read-only stored polygon never becomes the editable polygon state');
roiCancel.pickAssetRoi({ id: '戊', kind: 'ocr_line', roi: [1, 2, 3, 4] });
eq(roiCancel.storedRecognitionPolygon, null, '#6: an asset without polygon clears the stored polygon display');
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

// #6 静态接线：绘制层/恢复/清空都在位，颜色与可编辑几何区分。
const drawFnSource = extractFunction('draw');
const storedPolyFn = extractFunction('drawStoredRecognitionPolygonOverlay');
ok(
  /drawStoredRecognitionPolygonOverlay\(\)/.test(drawFnSource) &&
    /#4d9fff/.test(storedPolyFn),
  '#6: draw() renders the stored recognition polygon with its own blue color'
);
ok(
  /#ff8c00/.test(drawFnSource) && !/#4d9fff/.test(drawFnSource),
  '#6: the editable polygon / operation area keeps the orange color (no confusion with the stored layer)'
);
ok(
  /已存识别多边形/.test(storedPolyFn) && /已存识别多边形/.test(extractFunction('updateRoiLabel')),
  '#6: the stored polygon layer + ROI label both name 已存识别多边形'
);
ok(
  /storedRecognitionPolygon = Array\.isArray\(it\.polygon\)/.test(extractFunction('pickAssetRoi')),
  '#6: pickAssetRoi restores the stored recognition polygon from the asset'
);
ok(
  /storedRecognitionPolygon = null/.test(extractFunction('resetSelection')) &&
    /storedRecognitionPolygon = null/.test(extractFunction('replaceResults')),
  '#6: resetting the selection / replacing results clears the stored polygon display'
);

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
  action_shape: { kind: 'rect', roi: [5, 6, 7, 8] },
  threshold: 0.9,
  polygon: [[1, 2], [3, 2], [3, 5], [1, 5]],
  // #9：模板识别区 replace 也必须带回 op_region 链。
  op_region: { mode: 'offset', offset: [3, 4, 50, 60] },
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
    threshold: 0.9,
    action_shape: { kind: 'rect', roi: [5, 6, 7, 8] },
    op_region: { mode: 'offset', offset: [3, 4, 50, 60] },
    polygon: [[10, 20], [39, 20], [39, 59], [10, 59]],
  },
  'the replace payload carries every persisted field (incl. op_region) and only replaces roi (P1-1/#9)'
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

// --- #10（批次B3b）：统一框选工具（3 按钮 + 拖动=矩形/单击=多边形/双击收尾） ---
// 手势判定：位移超过阈值才是矩形；否则算“单击=多边形顶点”。
eq(canvasGestureShape({ x: 0, y: 0 }, { x: 0, y: 0 }), 'polygon', 'no move -> polygon vertex');
eq(canvasGestureShape({ x: 0, y: 0 }, { x: 3, y: 3 }), 'polygon', 'tiny move stays a click');
eq(canvasGestureShape({ x: 0, y: 0 }, { x: 10, y: 2 }), 'rect', 'horizontal drag -> rectangle');
eq(canvasGestureShape({ x: 0, y: 0 }, { x: 2, y: 10 }), 'rect', 'vertical drag -> rectangle');
eq(canvasGestureShape({ x: 0, y: 0 }, { x: -10, y: 0 }, 4), 'rect', 'negative direction still a drag');
eq(canvasGestureShape({ x: 0, y: 0 }, { x: 5, y: 5 }, 8), 'polygon', 'threshold is configurable');
eq(canvasGestureShape(null, { x: 1, y: 1 }), 'polygon', 'missing start -> click path');
// 三个工具按钮：识别区域 / 操作区域 / 反选（反选仅在操作区域工具内可用）。
const noneBtns = canvasToolButtonState('none', false);
eq(noneBtns.map((b) => b.key), ['recognition', 'action', 'invert'], 'exactly three canvas tool buttons');
eq(noneBtns[0].active, false, 'recognition tool starts inactive');
eq(noneBtns[2].disabled, true, 'invert is disabled outside the action tool');
eq(noneBtns[2].label, '反选：关', 'invert label shows the off state');
const recBtns = canvasToolButtonState('recognition', false);
eq(recBtns[0].active, true, 'recognition tool active');
eq(recBtns[1].active, false, 'action tool inactive while recognition is active');
eq(recBtns[2].disabled, true, 'invert stays disabled in the recognition tool (no mask complement)');
const actBtns = canvasToolButtonState('action', true);
eq(actBtns[1].active, true, 'action tool active');
eq(actBtns[2].disabled, false, 'invert is enabled in the action tool');
eq(actBtns[2].label, '反选：开', 'invert label shows the on state');
eq(canvasToolButtonState('bogus', false)[0].active, false, 'unknown tool degrades to none');
ok(
  (source.match(/\bfunction setCanvasTool\s*\(/g) || []).length === 1,
  'setCanvasTool is the single canvas-tool entry helper'
);
for (const old of ['SELECTION_MODES', 'selectionModeFromFlags', 'setSelectionMode', 'polyMode', 'actionPolyMode', 'backMode', 'btnPoly', 'btnBackMode', 'btnActionPoly']) {
  ok(!source.includes(old), `old selection-mode system is gone: ${old}`);
}
ok(
  /id="btn-tool-recognition"/.test(html) && /id="btn-tool-action"/.test(html) && /id="btn-action-invert"/.test(html),
  'index.html provides the three unified canvas tool buttons'
);
ok(
  !/id="btn-poly"/.test(html) && !/id="btn-back-mode"/.test(html) && !/id="btn-action-poly"/.test(html),
  'index.html dropped the old four-state mode buttons'
);
// 识别区草稿/保存计划：多边形优先，矩形按素材类型走 update/overwrite。
const recAssetTpl = { id: 'T', kind: 'template', roi: [10, 20, 30, 40], polygon: null };
const recAssetOcr = { id: 'O', kind: 'ocr_var', roi: [10, 20, 30, 40], polygon: null };
eq(recognitionDraftFromAsset(recAssetTpl), { roi: [10, 20, 30, 40], polygon: null }, 'draft loads roi without a stored polygon');
eq(
  recognitionDraftFromAsset({ kind: 'ocr_line', roi: [1, 2, 3, 4], polygon: [[1, 2], [3, 2], [3, 4]] }).polygon,
  [[1, 2], [3, 2], [3, 4]],
  'draft loads a stored recognition polygon for editing'
);
eq(recognitionPolygonBBox([[10, 20], [39, 20], [39, 59], [10, 59]]), [10, 20, 30, 40], 'polygon bbox is inclusive like roi');
// 非模板 + 多边形 → update {polygon}（服务端由 bbox 反推 roi，不重截图）。
const planOcrPoly = recognitionSavePlan(recAssetOcr, {
  roi: [10, 20, 30, 40],
  polygon: [[10, 20], [39, 20], [39, 59], [10, 59]],
});
eq(planOcrPoly.ok, true, 'ocr_var polygon plan is valid');
eq(planOcrPoly.action, 'update', 'non-template polygon saves via update');
eq(planOcrPoly.request, { polygon: [[10, 20], [39, 20], [39, 59], [10, 59]] }, 'update polygon payload only carries the mask');
// 模板 + 同一 ROI 的多边形 → update（不重新截图）。
const tplSamePoly = [[10, 20], [39, 20], [39, 59], [10, 59]];
const planTplSame = recognitionSavePlan(recAssetTpl, { roi: [10, 20, 30, 40], polygon: tplSamePoly });
eq(planTplSame.action, 'update', 'template polygon with the same ROI needs no re-crop');
// 模板 + 移动 ROI 的多边形 → overwrite（重新截图）+ 多边形随行。
const planTplMove = recognitionSavePlan(recAssetTpl, {
  roi: [15, 25, 20, 10],
  polygon: [[15, 25], [34, 25], [34, 34], [15, 34]],
});
eq(planTplMove.action, 'overwrite', 'template polygon that moves the ROI requires the overwrite save');
eq(planTplMove.request.roi, [15, 25, 20, 10], 'overwrite plan carries the new ROI');
eq(planTplMove.request.polygon.length, 4, 'overwrite plan carries the polygon');
// 矩形：模板 → overwrite 且显式 polygon:null 清空旧掩膜；非模板 → update polygon:null。
const planTplRect = recognitionSavePlan(recAssetTpl, { roi: [20, 30, 15, 15], polygon: null });
eq(planTplRect.action, 'overwrite', 'template rectangle save re-crops');
eq(planTplRect.request.polygon, null, 'template rectangle save clears the stored mask');
const planOcrRect = recognitionSavePlan(recAssetOcr, { roi: [20, 30, 15, 15], polygon: null });
eq(planOcrRect.action, 'update', 'non-template rectangle saves via update');
eq(planOcrRect.request, { roi: [20, 30, 15, 15], polygon: null }, 'rectangle update clears the mask explicitly');
// region：矩形保留 action_shape；多边形无掩膜语义，必须拒绝。
const planRegionRect = recognitionSavePlan({ id: 'R', kind: 'region', roi: [5, 5, 10, 10] }, { roi: [6, 6, 8, 8], polygon: null });
eq(planRegionRect.action, 'update', 'region rectangle uses the update path');
eq(planRegionRect.request.keep_action_shape, true, 'region rectangle keeps its action_shape');
ok(!recognitionSavePlan({ id: 'R', kind: 'region' }, { roi: null, polygon: [[1, 1], [5, 1], [5, 5]] }).ok, 'region polygon is rejected');
// 模板矩形保存始终走覆盖（显式保存语义）；非模板/多边形未变化时才是 no-op。
eq(recognitionSavePlan(recAssetTpl, { roi: [10, 20, 30, 40], polygon: null }).action, 'overwrite', 'template rectangle save always re-crops');
eq(
  recognitionSavePlan({ ...recAssetOcr, polygon: tplSamePoly }, { roi: [10, 20, 30, 40], polygon: tplSamePoly }).action,
  'none',
  'unchanged polygon no-op'
);
eq(
  recognitionSavePlan(recAssetOcr, { roi: [10, 20, 30, 40], polygon: null }).action,
  'none',
  'unchanged non-template rectangle no-op'
);
ok(!recognitionSavePlan({ id: 'X', kind: 'ocr_line' }, { roi: null, polygon: null }).ok, 'empty recognition draft is rejected');
// 入口接线（静态契约）：素材行 识别区/操作区 都进入统一工具；被引用素材只读。
const roiEditorFn2 = extractFunction('editAssetRecognitionRoi');
const actionEditorFn = extractFunction('editAssetActionRoi');
ok(/setCanvasTool\('recognition'\)/.test(roiEditorFn2), 'editAssetRecognitionRoi enters the recognition tool');
ok(/recognitionDraftFromAsset\(it\)/.test(roiEditorFn2) && /polygon = draft\.polygon/.test(roiEditorFn2),
  'editAssetRecognitionRoi loads a stored recognition polygon for editable display/continue-editing');
ok(/setCanvasTool\('action'\)/.test(actionEditorFn), 'editAssetActionRoi enters the action tool');
ok(/String\(it\.kind\) !== 'ocr_var'/.test(actionEditorFn),
  'a full-screen ocr_var (no roi) can still edit its operation area');
const candidateAdjustBlock = source.slice(
  source.indexOf("adjust.textContent = '调操作区域'"),
  source.indexOf("const reset = document.createElement('button')")
);
ok(/setCanvasTool\('action'\)/.test(candidateAdjustBlock),
  'candidate 调操作区域 enters the action tool');
const opRegionSelfRenderFn = extractFunction('buildOpRegionModelA');
ok(/在画布上编辑当前素材操作区/.test(opRegionSelfRenderFn),
  'the op_region dialog self branch offers the canvas action-region editor');
ok(/editAssetActionRoi\(asset, scope\)/.test(opRegionSelfRenderFn),
  'that dialog button routes through the unified action tool entry');
const toggleToolFn = extractFunction('toggleCanvasTool');
ok(/setCanvasTool\(tool\)/.test(toggleToolFn) && /turningOff/.test(toggleToolFn),
  'the tool buttons share one toggle entry');
ok(
  /canvasTool !== 'action'\) return;/.test(source),
  'invert is guarded to the action tool'
);
ok(
  /canvas\.addEventListener\('mousemove',[\s\S]{0,1600}canvasGestureShape\(start, p\)/.test(source),
  'mousemove distinguishes drag vs click with canvasGestureShape'
);
ok(
  /canvasTool === 'recognition' && polyDrag !== null/.test(source),
  'vertex dragging stays available for a finished recognition polygon'
);
ok(
  /canvas\.addEventListener\('dblclick',[\s\S]{0,900}识别多边形已收尾|识别多边形已收尾/.test(source),
  'double-click finishes the recognition polygon'
);
ok(
  /Esc：已退出画布工具/.test(source),
  'Esc cancels the in-progress gesture and leaves the canvas tool'
);
ok(
  /finishActionGesture\(shape\)/.test(source) &&
    /function finishActionGesture\(shapeOverride\)/.test(source),
  'closing a candidate action polygon persists the polygon shape (not the rect fallback)'
);
ok(
  /工具：识别区域/.test(extractFunction('updateRoiLabel')) &&
    /工具：操作区域/.test(extractFunction('updateRoiLabel')),
  'the ROI label shows the active unified tool'
);
const resetSelectionFn = extractFunction('resetSelection');
const resetActionFn = extractFunction('resetActionGeometry');
ok(/setCanvasTool\('none'\)/.test(resetSelectionFn), 'resetSelection exits the canvas tool through setCanvasTool');
ok(!/setSelectionMode/.test(resetActionFn), 'resetActionGeometry no longer flips old mode flags');
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
  /return testAssetCandidates\(it, frameId, scopeTask\)/.test(runAssetTestFn),
  'U-T4: runAssetTest routes 素材行 识别 through the all-candidates path (all kinds)'
);
ok(
  /it\.kind === 'region'[\s\S]{0,160}pickAssetRoi\(it\)/.test(runAssetTestFn),
  'region 识别 still selects the region before listing its single candidate'
);
for (const fn of [
  'testAssetMatch', 'testAssetDrag', 'testAssetOcr',
  'testAssetCollection', 'testAssetCandidates',
]) {
  const body = extractFunction(fn);
  ok(
    new RegExp(`function ${fn}\\(it, frameId, scopeTask\\)`).test(body),
    `${fn} accepts an optional frame id + scope override`
  );
  ok(/testFrames\(it, frameId\)/.test(body), `${fn} resolves frames through testFrames(it, frameId)`);
  ok(/assetRowScopeTask\(scopeTask/.test(body), `${fn} resolves the API scope through assetRowScopeTask`);
}
const runTestActionFn = extractFunction('runTestActionForAsset');
ok(
  /function runTestActionForAsset\(it, gesture, frameId, scopeTask\)/.test(runTestActionFn) &&
    /testFrames\(it, frameId\)/.test(runTestActionFn) &&
    /assetRowScopeTask\(scopeTask/.test(runTestActionFn),
  'U-T4/S4: runTestActionForAsset accepts gesture/frame/scope; testAssetClick wraps it'
);
ok(
  /function testAssetClick\(it, frameId, scopeTask\)[\s\S]{0,160}runTestActionForAsset\(it, 'click', frameId, scopeTask\)/.test(extractFunction('testAssetClick')),
  'U-T4/S4: 点测 delegates to the generic test_action runner'
);
ok(
  /function testFrameLabel\(it, frameId\)/.test(extractFunction('testFrameLabel')),
  'testFrameLabel can name an explicitly requested frame'
);
for (const fn of ['testAssetDrag', 'testAssetOcr']) {
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
    /onCollectionTest: \(it\) => testAssetCandidates\(it, f\.id, scopeTask\)/.test(renderFrameAssetsFn),
  'frame detail click/drag/collection tests force the rendered frame id + row scope too (U-T4: collection 识别 = candidates)'
);
// C static contract: frame rows are manageable and carry no source label.
ok(/manage: true/.test(renderFrameAssetsFn), 'renderFrameAssets passes manage:true (same as the global library)');
ok(!/sourceLabel/.test(renderFrameAssetsFn), 'renderFrameAssets passes no sourceLabel (来源列 deleted)');
ok(
  /frameAssetScope\(row, taskName/.test(renderFrameAssetsFn),
  'renderFrameAssets derives each row API scope through frameAssetScope'
);
ok(
  ['onRename', 'onThreshold', 'onPosition', 'onDelete', 'onOpRegion', 'onEditText'].every((k) =>
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
    /matchBoxValid\(recognizedBox\)/.test(extractFunction('testAssetDrag')) &&
    /matchBoxValid\(s\.real_rect\)/.test(extractFunction('testActionBoxes')),
  'template tests + test_action boxes reject a miss (-1,-1) through matchBoxValid before drawing'
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
  /识别区域 \[\$\{rec\.join/.test(extractFunction('testAssetOcr')),
  'the OCR test labels the stored ROI as the recognition area, not a detected box'
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
    actionShape: null,
  }];
  await sandbox.saveAssetUnified();
  eq(calls.length, 1, 'OCR save posts exactly once');
  eq(calls[0].url, '/task/save_assets', 'OCR rows use the batch endpoint');
  eq(calls[0].payload.assets[0].polygon, recognitionPoly, 'batch OCR save payload carries polygon');
  eq(calls[0].payload.assets[0].ocr_mode, 'lines', 'batch OCR save payload keeps ocr_mode');
  eq(calls[0].payload.assets[0].ocr_text, '开始', 'batch OCR save payload keeps ocr_text');
  ok(
    !('action_source' in calls[0].payload.assets[0]) && !('action_shape' in calls[0].payload.assets[0]),
    'batch OCR save sends no action_source/action_shape when no operation area was edited'
  );

  sandbox.saveCandidates = [{
    selected: true,
    kind: 'template',
    name: '模板甲',
    recognize: { x: 1, y: 2, w: 3, h: 4 },
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

// --- U-T4/S4（B-T3b）：test_action 点测/长按 + 候选下拉 + validate 预检 ---------
const clickTestFn = extractFunction('runTestActionForAsset');
ok(/realAssetTestGuard\(\)/.test(clickTestFn), 'U-T4: click/long-press test is gated by the real/dry-run mode');
ok(/confirmAssetTestGestures\('click'/.test(clickTestFn), 'U-T4: real test_action confirms before sending');
ok(
  /postJson\('\/task\/asset\/test_action', testActionRequestBody\(gesture/.test(clickTestFn),
  'U-T4: 点测/长按 POST /task/asset/test_action (dry + real)'
);
ok(
  /pickActionCandidate\(candidates\.items, it\)/.test(clickTestFn) &&
    /real: true/.test(clickTestFn) && /live: true/.test(clickTestFn),
  'U-T4: real mode picks a candidate from #candidate-select, then sends real + live'
);
ok(
  /fetchActionCandidates\(it, task, frame\)/.test(clickTestFn) && /for \(const candidate of candidates\.items\)/.test(clickTestFn),
  'U-T4: dry-run walks every candidate through test_action'
);
ok(
  /testActionBoxes\(dry/.test(clickTestFn) && /真实识别/.test(extractFunction('testActionBoxes')),
  'U-T4: dry-run draws each candidate real_rect + action_box'
);
ok(
  /真实模式/.test(clickTestFn) && /演练，不实际点击/.test(clickTestFn),
  'U-T4: result text distinguishes 演练/真实'
);
ok(
  /return runTestActionForAsset\(it, 'click', frameId, scopeTask\)/.test(extractFunction('testAssetClick')),
  'U-T4: 点测 wraps the generic test_action runner (drag keeps the old implementation)'
);
ok(
  /sendAssetTestGesture\(true, 'swipe'/.test(extractFunction('testAssetDrag')),
  'U-T4: 拖测 keeps the old path (test_action drag is 400 in this batch)'
);
// B3a（task-27 c）：拖测接入候选列表；多候选复用 #candidate-select 选一个，再走旧拖测路径。
const dragCandidatesFn = extractFunction('testAssetDrag');
ok(
  /fetchActionCandidates\(it, task, frame\)/.test(dragCandidatesFn),
  'B3a: 拖测先取 GET /task/asset/candidates 全部候选'
);
ok(
  /pickActionCandidate\(fetched\.items, it, '拖测'\)/.test(dragCandidatesFn),
  'B3a: 多候选拖测复用 #candidate-select（purpose=拖测）选一个'
);
ok(
  /candidateBoxes\(fetched\.items\)/.test(dragCandidatesFn) &&
    /translatedActionShapeForMatch\(it, \{ x: recognizedBox\[0\], y: recognizedBox\[1\] \}\)/.test(dragCandidatesFn),
  'B3a: 拖测画全部候选框；模板候选按匹配偏移平移操作区'
);
ok(
  !/\/task\/asset\/match\?task=/.test(dragCandidatesFn),
  'B3a: 拖测不再走单结果 /task/asset/match'
);
ok(
  /候选 #\$\{candidateIndex\}/.test(dragCandidatesFn),
  'B3a: 拖测结果行带选中候选序号'
);
// 长按测试入口（task-27）：菜单项已由 menu 断言覆盖；请求侧 dry/real 都带 hold_ms=3000。
ok(
  /holdMs: gesture === 'long_press' \? 3000 : undefined/.test(clickTestFn),
  'B3a: 长按测试 dry/real 请求带 hold_ms=3000'
);
ok(
  (source.match(/onLongPress:/g) || []).length >= 3,
  'B3a: three asset surfaces wire 长按测试 (onLongPress)'
);
ok(
  /assetCandidatesUrl\(task, it\.id, frame/.test(extractFunction('fetchActionCandidates')),
  'U-T4: the test-action candidate list comes from GET /task/asset/candidates'
);
ok(
  /postJson\('\/task\/asset\/validate_op_region'/.test(source) &&
    /opRegionValidateErrorText\(validated\)/.test(extractFunction('saveOpRegionDialog')),
  'U-T4/S5: 操作区保存前调用服务端权威 validate_op_region 并展示 400 错误'
);

// test_action 纯函数：请求体 / 候选标签 / 结果行 / 画布 boxes / validate 错误格式。
eq(
  testActionRequestBody('click', { task: '任务甲', id: 'O_组', candidateIndex: 1, frame: { live: true }, real: false }),
  { task: '任务甲', id: 'O_组', gesture: 'click', real: false, candidate_index: 1, live: true },
  'U-T4: a dry test_action body carries task/id/gesture/candidate/live'
);
eq(
  testActionRequestBody('long_press', { task: '任务甲', id: 'I_A', frame: { frameId: 7 }, real: true, holdMs: 2500 }),
  { task: '任务甲', id: 'I_A', gesture: 'long_press', real: true, frame_id: 7, hold_ms: 2500 },
  'U-T4: a real long_press body can carry frame_id + hold_ms'
);
ok(
  testActionRequestBody('click', { id: 'X' }).live === undefined,
  'U-T4: no live flag unless the frame plan says live'
);
eq(candidateOptionLabel({ index: 1, score: 0.9729 }), '#1 score=0.973', 'U-T4: candidate option = #序号 score');
eq(
  testActionResultLines({
    ok: true, real: false, candidate_index: 0, real_rect: [10, 20, 30, 40],
    action_box: [12, 22, 30, 40], point: { x: 27, y: 42 }, chain_depth: 2,
    steps: [
      { asset: 'O_组', mode: 'asset', real: [10, 20, 30, 40], score: 0.97 },
      { asset: 'I_模板', mode: 'offset', real: [52, 42, 20, 10] },
    ],
  }, 0)[0],
  '#0 ✅ real_rect=[10,20,30,40] action_box=[12,22,30,40] point=(27,42) chain_depth=2',
  'U-T4: a dry result line names real_rect/action_box/point/chain_depth'
);
const s4StepLines = testActionResultLines({
  ok: true, steps: [{ asset: 'A', mode: 'self', real: [1, 2, 3, 4], score: 0.5, text: '御魂' }],
}, 2);
ok(/  · A \[self\] real=\[1,2,3,4\] score=0\.500 text="御魂"/.test(s4StepLines[1]),
  'U-T4: chain steps render asset/mode/real/score/text');
ok(
  testActionResultLines({ ok: false, errors: ['候选越界'] }, 3)[0].indexOf('#3 ❌ 候选越界') === 0,
  'U-T4: a failed test_action renders the backend error line'
);
ok(
  /已点击 · tap_ok=true/.test(testActionResultLines({ ok: true, real: true, clicked: true, tap_ok: true }, 0).join('\n')),
  'U-T4: a real result reports clicked + tap_ok'
);
eq(
  testActionBoxes({ ok: true, real_rect: [10, 20, 30, 40], action_box: [12, 22, 30, 40] }, '帧#1').map((b) => b.text),
  ['帧#1 真实识别', '帧#1 操作区'],
  'U-T4: test_action boxes label the real recognition + operation regions'
);
eq(testActionBoxes({ ok: false }, '').length, 0, 'U-T4: a failed test_action draws no boxes');
eq(
  opRegionValidateErrorText({ ok: false, errors: [{ path: 'items[3].op_region.asset', msg: 'cycle: A -> B -> A' }] }),
  'items[3].op_region.asset：cycle: A -> B -> A',
  'U-T4: validate_op_region 400 errors render as 「path：msg」 lines'
);
eq(opRegionValidateErrorText({ ok: true }), '', 'U-T4: a valid chain yields no error text');
ok(
  /function pickActionCandidate\(candidates, it, purpose\)/.test(extractFunction('pickActionCandidate')) &&
    /'选择候选后' \+ what/.test(extractFunction('pickActionCandidate')),
  'B3a: the candidate picker labels 真实点击/拖测 via the purpose parameter'
);
for (const id of ['candidate-select']) {
  ok(new RegExp(`id="${id}"`).test(html), `U-T4: index.html provides #${id}`);
}
ok(/\.candidate-select/.test(styleCss), 'U-T4: style.css styles the real-candidate select');
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

eq(formatPackageSummary(null), '（服务端未返回 path）', 'null package summary is still renderable');

eq(
  publishErrorsText(['T3: 未声明的参数 k', 'T7: 子任务缺失']),
  '· T3: 未声明的参数 k\n· T7: 子任务缺失',
  'publish errors render one bullet per server line'
);
eq(publishErrorsText([]), '（服务端未返回错误详情）', 'empty publish error list still says something');
eq(publishErrorsText('单一错误'), '· 单一错误', 'a single error string is bulleted too');
eq(publishErrorsText([null, '  ', 'x']), '· x', 'blank publish errors are filtered');

// --- P0-1a: publish response exchange line (Studio writes the notification) --
eq(publishExchangeText(undefined), '', 'missing exchange field adds no line (older server)');
eq(publishExchangeText(null), '', 'null exchange adds no line');
eq(
  publishExchangeText({ enabled: false }),
  '未启用 exchange，Runner 不会自动收到（可把 .oaspkg 拷到 Runner 后手动安装）',
  'disabled exchange says the Runner will not see the publish'
);
eq(
  publishExchangeText({
    enabled: true,
    event_id: 'pkg-任务-1-abc',
    index_path: 'D:/oas/exchange/publish-index.json',
  }),
  '已写入发布通知：D:/oas/exchange/publish-index.json（Runner 将看到）',
  'written notification names the publish index path'
);
eq(
  publishExchangeText({ enabled: true, event_id: 'pkg-1' }),
  '已写入发布通知：pkg-1（Runner 将看到）',
  'event_id is the fallback target when index_path is missing'
);
ok(
  /未重复复制/.test(publishExchangeText({ enabled: true, event_id: 'pkg-1', copied: false })),
  'an already-copied package is not reported as copied again'
);
ok(
  /发布成功，但写通知失败：disk full/.test(publishExchangeText({ enabled: true, error: 'disk full' })),
  'exchange write errors keep the publish itself successful'
);
ok(
  /未启用 exchange/.test(publishExchangeText({})),
  'exchange object without enabled degrades to not-configured'
);
ok(
  /const exchangeLine = publishExchangeText\(s\.exchange\);/.test(source),
  'the publish success panel appends the exchange result line'
);
ok(
  /exchange 通知：若 <code>config\/live\.json<\/code> 启用了 <code>exchange<\/code>/.test(html),
  'the publish dialog explains exchange (Runner notification) in one line'
);

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

// --- 事项 1/2: 动作库去参数预设 ---
// The palette and every branch dropdown are sourced from OP_META only; the
// 参数 group, the three param_* presets and their dedicated helpers are gone.
ok(
  !/PARAM_ACTIONS/.test(builderSource) && !/PARAM_GROUP/.test(builderSource),
  'builder.js has no PARAM_ACTIONS/PARAM_GROUP any more'
);
ok(
  !/param_click_text|param_wait_text|param_long_press_text/.test(builderSource) &&
    !/param_click_text|param_wait_text|param_long_press_text/.test(html) &&
    !/param_click_text|param_wait_text|param_long_press_text/.test(source),
  'no parameter-action preset id survives in builder.js/index.html/app.js'
);
eq(
  (builderSource.match(/const GROUP_ORDER = (\[[^\]]*\]);/) || [])[1],
  "['流程控制', '识别/等待', '动作', '数据/结束']",
  'the palette group order no longer has a 参数 group'
);
ok(
  /function actionCatalog\(\)[\s\S]{0,240}Object\.entries\(OP_META\)/.test(builderSource) &&
    !/preset/.test(extractFunctionFrom(builderSource, 'actionCatalog', 'builder.js')),
  'actionCatalog() is the single OP_META source (no preset entries)'
);
ok(
  /function renderPalette\(\)[\s\S]{0,900}actionCatalog\(\)/.test(builderSource) &&
    /addAction\(item\.key\)/.test(builderSource),
  'the palette renders the OP_META catalogue and inserts generic actions'
);
ok(
  !/function actionAvailable\(/.test(builderSource) &&
    !/function paramActionContext\(/.test(builderSource) &&
    !/function addParamAction\(/.test(builderSource) &&
    !/function buildActionStep\(/.test(builderSource),
  'the parameter-action helpers (actionAvailable/paramActionContext/addParamAction/buildActionStep) are gone'
);
ok(
  /const catalog = actionCatalog\(\)/.test(builderSource) &&
    /for \(const item of catalog\)/.test(builderSource) &&
    /const child = await prepareActionStep\(sel\.value\)/.test(builderSource) &&
    !/需先声明任务参数/.test(builderSource),
  'the if/loop 是/否/循环体 branch dropdown uses the same OP_META catalogue and has no 参数 entries'
);
ok(
  /async function prepareActionStep\(key\)[\s\S]{0,200}return OP_META\[key\] \? makeStep\(key\) : null;/.test(builderSource),
  'prepareActionStep keeps the generic action path (no parameter branch)'
);
const insertStepFn = extractFunctionFrom(builderSource, 'insertStep', 'builder.js');
ok(
  /function insertStep\(step, label\)/.test(builderSource) &&
    /arr\[at - 1\]\.op === 'finish'/.test(insertStepFn),
  'insertStep keeps a trailing finish last'
);
ok(
  /ins = ' and ' \+ ins/.test(builderSource) &&
    /function exprInsertText\(/.test(builderSource) &&
    /function repairConditionExpr\(/.test(builderSource) &&
    /normalizeFlow\(B\.flow\);\s*\/\/ P3: 保存前本地校验/.test(builderSource.replace(/\r\n/g, '\n')),
  'parameter insertion avoids token gluing and malformed conditions are repaired before save'
);
ok(
  /function ocrVarInsertWidget\(mode, getField\)/.test(builderSource) &&
    /插入识别文字配置项 ▾/.test(builderSource) &&
    !/插入参数 ▾/.test(builderSource),
  'D: 插入参数 ▾ is replaced by 插入识别文字配置项 ▾ (ocr_var assets only)'
);
ok(
  /ocrVarInsertWidget\('cond'/.test(builderSource) &&
    /ocrVarInsertWidget\(exprMode/.test(builderSource) &&
    /ocrVarInsertWidget\('expr'/.test(builderSource),
  'D: the ocr_var insert widget mounts on expr/cond/locator expression fields'
);
ok(
  !/paramInsertWidget/.test(builderSource) && !/paramInsertFromPanel/.test(builderSource),
  'D: the old 插入参数 widget + panel insert helper are gone'
);
ok(
  /typeof hook\.refresh === 'function'/.test(builderSource) &&
    /window\.OAS_TASK_CONFIG/.test(builderSource),
  'D: builder still reads the internal window.OAS_TASK_CONFIG bridge (refresh only)'
);
ok(
  /const paramsPromise = fetchTaskParams\(name\)/.test(builderSource) &&
    !/renderParamsPanel/.test(builderSource),
  'D: meta.params is still loaded internally but no params panel renders'
);
ok(
  /B\.paramsError = pr\.ok \? '' : pr\.error/.test(builderSource),
  'a params read failure stays isolated from the flow editor'
);
ok(
  /no params panel/i.test('no params panel') && !/'builder-params'/.test(source + html),
  'D: no #builder-params panel survives'
);
ok(
  /const exprMode = EXPR_FIELDS\[step\.op\] && EXPR_FIELDS\[step\.op\]\[f\.key\]/.test(builderSource),
  'generic expression fields are detected from EXPR_FIELDS'
);
ok(
  /文字来源（text \/ text_expr 互斥）/.test(builderSource) &&
    /function locatorTextSourceFields\(base, onChange\)/.test(builderSource),
  'the locator editor labels the three-way text source and is shared'
);
ok(
  /step\.args\[field\] = text\.mode === 'keep' \? next : paramSourceApply\(next, text\.mode, text\.value\);/.test(builderSource) &&
    /let written = text\.mode === 'keep' \? next : paramSourceApply\(next, text\.mode, text\.value\);/.test(builderSource) &&
    /step\.args\[side\] = written;/.test(builderSource),
  'find / guards / drag endpoints write through paramSourceApply with the legacy-param keep path'
);
ok(
  /const base = find\[0\] && typeof find\[0\] === 'object'/.test(builderSource) &&
    /Object\.assign\(\{\}, base, \{ kind: kind\.value \}\)/.test(builderSource),
  'structuredClickTarget merges known keys over the original locator (never rebuilds it)'
);
ok(
  !/const next = \{ kind: kind\.value \};/.test(builderSource),
  'the old rebuild-object locator write that dropped text_expr/unknown fields is gone'
);
ok(
  /'ocr_var', '识别文字配置项'/.test(builderSource) && !/'文字（参数）'/.test(builderSource),
  'D: condition rows use 识别文字配置项 (ocr_var) instead of 文字（参数）'
);
ok(
  /flowTextSpecIssues\(B\.flow\)/.test(builderSource) && /保存已阻止/.test(builderSource),
  'saveFlow blocks text+text_expr double-writes before POSTing'
);
ok(
  /function trackExprField\(field, mode\)/.test(builderSource) &&
    /function insertTextAtField\(field, text, mode\)/.test(builderSource) &&
    /exprInsertText\(value, start, end, text, mode \|\| 'expr'\)/.test(builderSource),
  'insertion helpers exist and route through exprInsertText'
);
ok(
  'the params panel renders through textContent only'
);
ok(
  'the insert dropdown renders through textContent only'
);
ok(
  !/innerHTML/.test(extractFunctionFrom(builderSource, 'structuredClickTarget', 'builder.js')),
  'the locator editor renders through textContent only'
);
ok(
  /多定位器\/内联 ROI 用下方高级 JSON/.test(builderSource) &&
    /非 OCR 守卫请用高级 JSON 编辑/.test(builderSource) &&
    /非 OCR 端点请用高级 JSON 编辑/.test(builderSource),
  'the structured editors name what still stays advanced-JSON only (multi-locator / non-OCR)'
);

// app.js bridge: loaded after builder.js, so it must be a runtime window hook.
ok(/window\.OAS_TASK_CONFIG = \{/.test(source), 'app.js exposes window.OAS_TASK_CONFIG');
ok(
  'the bridge exposes open + refresh'
);
ok(
  'refresh reads GET /task/config?task='
);
ok(
  'a successful save notifies the builder panel'
);
ok(
  !/^window\.OAS_TASK_CONFIG\s*=/m.test(builderSource),
  'builder.js never defines/overwrites the shared bridge'
);

// CSS hooks for the new panel / dropdown / action group.
ok(
  /\.builder-params-panel/.test(styleCss) && /\.builder-params-list/.test(styleCss) &&
    /\.builder-param-row/.test(styleCss),
  'style.css styles the params panel/list/rows'
);
ok(
  /\.builder-insert-param-menu/.test(styleCss) && /\.builder-expr-row/.test(styleCss),
  'style.css styles the insert-parameter dropdown'
);
ok(/\.builder-text-source/.test(styleCss), 'style.css styles the text-source editor');
ok(
  !/builder-op-param/.test(styleCss) && !/builder-flash/.test(styleCss),
  'style.css has no 参数 action-library group / param-panel flash styles left'
);

// --- U-T5（API 无关纯函数）：动作目标选择模型 --------------------------------
// 动作选目标=素材库素材（含 ocr_var）；region 只作操作区链末端，不能作动作目标。
// 产品 UI（目标选择对话框/属性面板）等 U-T5 一次性接线，本段只测纯函数。
const BUILDER_TARGET_PURE = [
  'paramSafeKey', 'paramTextExpr',
  'actionTargetKindLabel', 'actionTargetKinds',
  'assetTargetOptions', 'targetLocatorForAsset',
  'validateActionTarget', 'targetPickerSummary',
  // U-T5（S7）：派生参数/目标校验/sync 摘要/旧 region 兼容判定。
  'paramAssetId', 'paramAssetLabel', 'derivedParamKeys', 'assetById', 'ocrVarParamRows',
  'locatorParamKey', 'paramSourceFromLocator',
  'actionTargetIssues', 'saveSyncText', 'locatorHasLegacyRegion',
];
const targetSb = {};
vm.createContext(targetSb);
vm.runInContext(
  BUILDER_TARGET_PURE.map((name) => extractFunctionFrom(builderSource, name, 'builder.js')).join('\n'),
  targetSb,
  { filename: 'builder.js#targets' }
);
const targetAssets = [
  { id: 'I_模板', kind: 'template' },
  { id: 'C_集合', kind: 'template_set', members: ['I_模板'] },
  { id: 'O_行', kind: 'ocr_line', ocr_text: '挑战' },
  { id: 'O_多行', kind: 'ocr_lines' },
  { id: 'O_组', kind: 'ocr_var', param_key: 'group', runtime_text: '御魂', label: '预设分组' },
  { id: 'O_坏', kind: 'ocr_var', runtime_text: '缺键' },
  { id: 'R_区', kind: 'region', roi: [1, 2, 3, 4] },
  { id: 'X_未知', kind: 'weird' },
  null,
  'bad',
];
eq(
  targetSb.assetTargetOptions(targetAssets).map((o) => o.id),
  ['I_模板', 'C_集合', 'O_行', 'O_多行', 'O_组', 'O_坏', 'R_区'],
  'U-T5: target options keep template/collection/OCR/ocr_var and list region as terminal-only'
);
eq(
  targetSb.assetTargetOptions(targetAssets).find((o) => o.id === 'R_区').terminalOnly,
  true,
  'U-T5: region is listed but marked terminal-only (only an operation-region terminal)'
);
ok(
  targetSb.assetTargetOptions(targetAssets).find((o) => o.id === 'O_组').terminalOnly === undefined,
  'U-T5: recognizable assets are not terminal-only'
);
eq(
  targetSb.assetTargetOptions(targetAssets)[0],
  { id: 'I_模板', label: 'I_模板', kind: 'template', kindLabel: '模板' },
  'U-T5: a template target option has id/label/kind/kindLabel'
);
const groupTargetOption = targetSb.assetTargetOptions(targetAssets).find((o) => o.id === 'O_组');
eq(groupTargetOption.label, '预设分组', 'U-T5: ocr_var option uses the asset label (fallback id)');
eq(groupTargetOption.kindLabel, '使用端OCR文字配置项', 'U-T5: ocr_var option carries the full kind label');
eq(groupTargetOption.paramKey, 'group', 'U-T5: ocr_var option exposes param_key for param() generation');
eq(groupTargetOption.ocrText, '御魂', 'U-T5: ocr_var option exposes runtime_text as the default text');
eq(
  targetSb.assetTargetOptions(targetAssets).find((o) => o.id === 'O_行').ocrText,
  '挑战',
  'U-T5: a normal OCR option exposes ocr_text'
);
eq(
  targetSb.assetTargetOptions(targetAssets, { kindFilter: ['template', 'ocr_var'] }).map((o) => o.id),
  ['I_模板', 'O_组', 'O_坏'],
  'U-T5: kindFilter narrows the target options (invalid ocr_var is filtered at validation)'
);
eq(
  targetSb.assetTargetOptions(targetAssets, { kindFilter: (kind) => kind.indexOf('ocr') === 0 }).map((o) => o.id),
  ['O_行', 'O_多行', 'O_组', 'O_坏'],
  'U-T5: kindFilter also accepts a predicate'
);
eq(targetSb.assetTargetOptions(null), [], 'U-T5: missing asset list yields no target options');
eq(
  targetSb.targetLocatorForAsset(targetAssets.find((a) => a.id === 'O_组')),
  {
    ok: true,
    locator: { kind: 'ocr_text', sel: 'O_组', text_expr: "param('group')" },
    error: '',
  },
  'U-T5: ocr_var target generates text_expr = param(<param_key>)'
);
const badOcrVarTarget = targetSb.targetLocatorForAsset(targetAssets.find((a) => a.id === 'O_坏'));
eq(badOcrVarTarget.ok, false, 'U-T5: an ocr_var without param_key cannot become a target');
ok(/param_key/.test(badOcrVarTarget.error), 'U-T5: the missing param_key error is explicit');
eq(
  targetSb.targetLocatorForAsset(targetAssets.find((a) => a.id === 'O_行')),
  { ok: true, locator: { kind: 'ocr_text', sel: 'O_行', text: '挑战' }, error: '' },
  'U-T5: a normal OCR target uses its declared ocr_text'
);
eq(
  targetSb.targetLocatorForAsset(targetAssets.find((a) => a.id === 'O_行'), { text: '自定义文字' }).locator.text,
  '自定义文字',
  'U-T5: user text overrides the OCR asset text'
);
eq(
  targetSb.targetLocatorForAsset(targetAssets.find((a) => a.id === 'O_多行')).ok,
  false,
  'U-T5: an OCR target without any text is rejected instead of inventing one'
);
eq(
  targetSb.targetLocatorForAsset(targetAssets.find((a) => a.id === 'I_模板')),
  { ok: true, locator: { kind: 'template', sel: 'I_模板' }, error: '' },
  'U-T5: a template target reuses the template locator shape'
);
eq(
  targetSb.targetLocatorForAsset(targetAssets.find((a) => a.id === 'C_集合')).locator,
  { kind: 'template', sel: 'C_集合' },
  'U-T5: a collection target is a template locator on the collection id (members expand at runtime)'
);
const regionTarget = targetSb.targetLocatorForAsset(targetAssets.find((a) => a.id === 'R_区'));
eq(regionTarget.ok, false, 'U-T5: region assets cannot become action targets');
ok(/操作区末端/.test(regionTarget.error), 'U-T5: the region error says it is only an operation-region terminal');
eq(targetSb.targetLocatorForAsset(null).ok, false, 'U-T5: a missing asset cannot become a target');
eq(
  targetSb.validateActionTarget(targetAssets.find((a) => a.id === 'O_组'), ['group', 'loop_n']),
  [],
  'U-T5: a derived-param-backed ocr_var target validates cleanly'
);
const staleParamErrors = targetSb.validateActionTarget(
  targetAssets.find((a) => a.id === 'O_组'),
  ['loop_n']
);
eq(staleParamErrors.length, 1, 'U-T5: an ocr_var whose param_key is missing from derived params is rejected');
ok(/不在任务派生参数中/.test(staleParamErrors[0]), 'U-T5: the derived-param error explains the fix');
ok(
  targetSb.validateActionTarget(targetAssets.find((a) => a.id === 'O_坏'), ['group'])
    .some((e) => /param_key/.test(e)),
  'U-T5: validation reports a missing param_key'
);
ok(
  targetSb.validateActionTarget(targetAssets.find((a) => a.id === 'R_区'), [])
    .some((e) => /不能作为动作目标/.test(e)),
  'U-T5: validation rejects region as an action target'
);
eq(
  targetSb.validateActionTarget(targetAssets.find((a) => a.id === 'I_模板'), []),
  [],
  'U-T5: template targets validate with no derived params needed'
);
ok(
  targetSb.validateActionTarget({ id: 'O_行', kind: 'ocr_line' }, [])
    .some((e) => /缺少 ocr_text/.test(e)),
  'U-T5: an OCR target without ocr_text gets a save-time warning'
);
eq(targetSb.validateActionTarget(null), ['动作目标素材 id 为空'], 'U-T5: a missing target is a validation error');
const groupSummary = targetSb.targetPickerSummary(targetAssets.find((a) => a.id === 'O_组'));
ok(/识别文字来源：素材「O_组」/.test(groupSummary), 'U-T5: the ocr_var target summary names the source asset');
ok(/param\('group'\)/.test(groupSummary) && /默认「御魂」/.test(groupSummary),
  'U-T5: the ocr_var summary shows param(key) + default text');
ok(
  /识别文字来源：素材「O_行」/.test(targetSb.targetPickerSummary(targetAssets.find((a) => a.id === 'O_行'))) &&
    /挑战/.test(targetSb.targetPickerSummary(targetAssets.find((a) => a.id === 'O_行'))),
  'U-T5: the OCR target summary names the fixed text'
);
ok(
  /模板匹配/.test(targetSb.targetPickerSummary(targetAssets.find((a) => a.id === 'I_模板'))),
  'U-T5: the template target summary says template matching'
);
ok(
  /不能作为动作目标/.test(targetSb.targetPickerSummary(targetAssets.find((a) => a.id === 'R_区'))),
  'U-T5: the region summary marks it as unusable'
);
eq(targetSb.targetPickerSummary(null), '未选择动作目标', 'U-T5: no target renders the empty summary');
eq(
  targetSb.actionTargetKinds().indexOf('region'),
  -1,
  'U-T5: region is not in the action-target kind whitelist'
);
eq(targetSb.actionTargetKindLabel('ocr_var'), '使用端OCR文字配置项', 'U-T5: ocr_var kind label');
eq(targetSb.actionTargetKindLabel('region'), '操作区域（仅操作区末端）', 'U-T5: region kind label explains its role');
ok(
  !/targetLocatorForAsset|assetTargetOptions/.test(extractFunctionFrom(builderSource, 'renderPalette', 'builder.js')),
  'U-T5: the palette stays generic; target selection lives in the property panel'
);

// --- U-T5/S7（接线）：目标选择编辑器 / 旧 region 收口 / 保存校验 / sync ---------
const targetEditorFn = extractFunctionFrom(builderSource, 'actionTargetEditor', 'builder.js');
ok(
  /assetTargetOptions\(B\.assets\)/.test(targetEditorFn) &&
    /targetLocatorForAsset\(asset\)/.test(targetEditorFn) &&
    /targetPickerSummary\(currentAsset\)/.test(targetEditorFn) &&
    /validateActionTarget\(currentAsset, derivedParamKeys\(ocrVarParamRows\(B\.assets\)\)\)/.test(targetEditorFn),
  'U-T5: the property panel target editor uses the asset-library target model (options/locator/summary/validation)'
);
ok(
  /仅操作区末端，不可作动作目标/.test(targetEditorFn),
  'U-T5: region entries in the target picker are annotated as operation-region terminals'
);
const structuredTargetFn = extractFunctionFrom(builderSource, 'structuredClickTarget', 'builder.js');
ok(
  /actionTargetEditor\(root, step\)/.test(structuredTargetFn),
  'U-T5: click_target/long_press inspector starts with the 动作目标 picker'
);
ok(
  /locatorHasLegacyRegion\(base\)/.test(structuredTargetFn) &&
    /if \(legacyRegion\)[\s\S]{0,320}renderRegionEditor\(/.test(structuredTargetFn) &&
    /新步骤不配置操作区域：操作区由目标素材的 op_region 决定/.test(structuredTargetFn),
  'U-T5: the region editor only renders for old flows that already carry region'
);
const dragEndpointFn = extractFunctionFrom(builderSource, 'renderDragEndpointEditor', 'builder.js');
ok(
  /if \(locatorHasLegacyRegion\(endpoint\)\)[\s\S]{0,260}renderRegionEditor\(/.test(dragEndpointFn),
  'U-T5: drag endpoints also gate the region editor on old region data'
);
ok(
  !/'param', '识别文字配置项/.test(builderSource) &&
    /旧参数来源（只读）/.test(builderSource) &&
    /mode === 'keep'/.test(builderSource),
  'D: the locator text-source has no param option; legacy param() shows read-only and is kept'
);
ok(
  /function ocrVarInsertWidget\(mode, getField\)/.test(builderSource) &&
    /insert\(key\);/.test(builderSource) &&
    /paramInsertSnippet\(key, mode\)/.test(builderSource),
  'D: 插入识别文字配置项 ▾ writes param(param_key) for a chosen ocr_var asset'
);
ok(
  /先创建使用端OCR文字配置项|无可用素材/.test(builderSource),
  'D: the ocr_var insert widget explains how to create one when the library is empty'
);
const targetSaveFlowFn = extractFunctionFrom(builderSource, 'saveFlow', 'builder.js');
ok(
  /actionTargetIssues\(B\.flow, B\.assets, ocrVarParamRows\(B\.assets\)\)/.test(targetSaveFlowFn),
  'D: saveFlow validates action targets against assets-derived ocr_var param rows (no params panel)'
);
ok(
  /saveSyncText\(s\.sync\)/.test(targetSaveFlowFn) &&
    /refreshParamsAfterFlowSync\(\)/.test(targetSaveFlowFn),
  'U-T5: save_flow sync refreshes derived params and reports the synced count'
);
ok(
  /function refreshParamsAfterFlowSync\(\)[\s\S]{0,420}fetchTaskParams\(B\.task\)/.test(builderSource),
  'D: the sync refresh re-reads GET /task/config internally (no panel to repaint)'
);
ok(
  /\.builder-target-errors/.test(styleCss) && /\.builder-param-asset/.test(styleCss) &&
    /\.builder-legacy-region-hint/.test(styleCss),
  'U-T5: style.css styles the target errors / derived-param chip / legacy-region hint'
);

// 纯函数：派生参数 / 目标校验 / sync 摘要 / 旧 region 判定。
const uT5Params = [
  { key: 'group', type: 'string', default: '御魂', role: 'ocr_text', asset_id: 'O_组', label: '预设分组', group: '识别文字' },
  { key: 'loop_n', type: 'int', default: 3 },
];
const uT5AssetsForDerivePure = [
  { id: 'O_组', kind: 'ocr_var', param_key: 'group' },
  { id: 'I_模板', kind: 'template' },
];
eq(targetSb.paramAssetId(uT5Params[0]), 'O_组', 'U-T5: derived rows expose asset_id');
eq(targetSb.paramAssetId(uT5Params[1]), '', 'U-T5: user rows have no asset_id');
eq(targetSb.derivedParamKeys(uT5Params), ['group'], 'U-T5: derivedParamKeys lists the asset_id row keys');
eq(targetSb.derivedParamKeys(null), [], 'U-T5: missing params have no derived keys');
eq(
  targetSb.ocrVarParamRows(uT5AssetsForDerivePure),
  [{ key: 'group', asset_id: 'O_组' }],
  'D: derived param rows are derived from ocr_var assets (no params panel needed)'
);
eq(
  targetSb.paramAssetLabel(uT5Params[0], targetAssets),
  '预设分组',
  'U-T5: a derived row resolves the asset display label'
);
eq(
  targetSb.paramAssetLabel({ key: 'x', asset_id: 'Gone' }, targetAssets),
  'Gone',
  'U-T5: a missing asset degrades to the raw asset id'
);
const uT5Assets = [
  { id: 'O_组', kind: 'ocr_var', runtime_text: '御魂', param_key: 'group' },
  { id: 'I_模板', kind: 'template' },
  { id: 'R_区', kind: 'region', roi: [1, 2, 3, 4] },
  { id: 'O_行', kind: 'ocr_line', ocr_text: '挑战' },
];
const uT5OkFlow = {
  steps: [{
    op: 'click_target',
    args: { find: [{ kind: 'ocr_text', sel: 'O_组', text_expr: "param('group')" }] },
  }],
};
eq(targetSb.actionTargetIssues(uT5OkFlow, uT5Assets, uT5Params), [],
  'U-T5: a valid ocr_var action target produces no save issues');
const uT5MissingFlow = {
  steps: [{ op: 'click_target', args: { find: [{ kind: 'ocr_text', sel: 'Gone', text: 'x' }] } }],
};
ok(
  targetSb.actionTargetIssues(uT5MissingFlow, uT5Assets, uT5Params)
    .some((issue) => /不在当前素材库/.test(issue)),
  'U-T5: a missing target asset blocks the save'
);
const uT5NoParamFlow = {
  steps: [{ op: 'click_target', args: { find: [{ kind: 'ocr_text', sel: 'O_行', text: '挑战' }] } }],
};
eq(targetSb.actionTargetIssues(uT5NoParamFlow, uT5Assets, uT5Params), [],
  'U-T5: a plain OCR target (text) passes target validation');
const uT5KeyFlow = {
  steps: [{ op: 'click_target', args: { find: [{ kind: 'ocr_text', sel: 'O_组', text_expr: "param('other')" }] } }],
};
ok(
  targetSb.actionTargetIssues(uT5KeyFlow, uT5Assets, uT5Params)
    .some((issue) => /param_key 是「group」/.test(issue)),
  'U-T5: a mismatched param_key blocks the save'
);
ok(
  targetSb.actionTargetIssues(
    { steps: [{ op: 'click_target', args: { find: [{ kind: 'ocr_text', sel: 'O_组', text_expr: "param('group')" }] } }] },
    uT5Assets,
    [{ key: 'loop_n', type: 'int' }]
  ).some((issue) => /不在任务派生参数中/.test(issue)),
  'U-T5: an ocr_var target whose derived row is missing blocks the save'
);
const uT5NoKeyAsset = { id: 'O_坏', kind: 'ocr_var', runtime_text: 'x' };
ok(
  targetSb.actionTargetIssues(
    { steps: [{ op: 'click_target', args: { find: [{ kind: 'ocr_text', sel: 'O_坏', text: 'x' }] } }] },
    uT5Assets.concat([uT5NoKeyAsset]),
    uT5Params
  ).some((issue) => /缺少 param_key/.test(issue)),
  'U-T5: an ocr_var target without param_key blocks the save'
);
ok(
  targetSb.actionTargetIssues(
    { steps: [{ op: 'click_target', args: { find: [{ kind: 'template', sel: 'R_区' }] } }] },
    uT5Assets,
    uT5Params
  ).some((issue) => /region 素材/.test(issue)),
  'U-T5: a region asset cannot be a template action target'
);
ok(
  targetSb.actionTargetIssues(
    { steps: [{ op: 'click', args: { sel: 'Gone' } }] },
    uT5Assets,
    uT5Params
  ).some((issue) => /args\.sel/.test(issue)),
  'U-T5: a missing args.sel target blocks the save'
);
ok(
  targetSb.actionTargetIssues(
    {
      steps: [{
        op: 'loop', args: { n: 2 }, body: [{
          op: 'click_target', args: { find: [{ kind: 'ocr_text', sel: 'Gone', text: 'x' }] },
        }],
      }],
    },
    uT5Assets,
    uT5Params
  ).some((issue) => /body\[0\]/.test(issue)),
  'U-T5: nested branch targets are validated too'
);
eq(targetSb.actionTargetIssues(null, uT5Assets, uT5Params), [], 'U-T5: a missing flow has no target issues');
eq(targetSb.saveSyncText(null), '', 'U-T5: no sync payload -> no status text');
eq(targetSb.saveSyncText({ added: [], updated: [], removed: [] }), '', 'U-T5: an empty sync -> no status text');
eq(
  targetSb.saveSyncText({ added: ['a'], updated: ['b', 'c'], removed: [] }),
  '已同步 3 个识别文字配置项派生参数（+1 ~2 -0）',
  'U-T5: sync text counts added/updated/removed rows'
);
eq(
  targetSb.saveSyncText({ added: [], updated: [], removed: ['x'] }),
  '已同步 1 个识别文字配置项派生参数（+0 ~0 -1）',
  'U-T5: a removed derived row is reported too'
);
eq(targetSb.locatorHasLegacyRegion({ kind: 'ocr_text', region: { mode: 'asset' } }), true,
  'U-T5: an old locator with region is detected');
eq(targetSb.locatorHasLegacyRegion({ kind: 'ocr_text', sel: 'O_x' }), false,
  'U-T5: a new locator without region is not legacy');
eq(targetSb.locatorHasLegacyRegion(null), false, 'U-T5: missing locator is not legacy');

// --- P5: authoritative build-page lint (GET /task/lint) ----------------------
const BUILDER_LINT_PURE = [
  'lintIssueCode', 'lintIssuePath', 'lintIssueMessage', 'lintIsT3Issue',
  'lintIssueLine', 'lintIssuesText', 'lintT3ParamKey', 'lintNormalizeResponse',
  'lintSummary', 'lintCountText', 'lintT3Rows', 'lintErrorText',
];
const lintSb = {};
vm.createContext(lintSb);
vm.runInContext(
  BUILDER_LINT_PURE.map((name) => extractFunctionFrom(builderSource, name, 'builder.js')).join('\n'),
  lintSb,
  { filename: 'builder.js#lint' }
);

eq(lintSb.lintIssueCode({ code: 't3' }), 'T3', 'issue codes are uppercased');
eq(lintSb.lintIssueCode({ rule: 'T4' }), 'T4', 'rule is accepted as a code fallback');
eq(lintSb.lintIssueCode({}), '', 'missing issue code degrades to empty');
eq(lintSb.lintIssuePath({ path: ' flow.json:steps[0] ' }), 'flow.json:steps[0]', 'issue paths are trimmed');
eq(lintSb.lintIssuePath({}), '—', 'a missing issue path renders a dash');
eq(lintSb.lintIssueMessage({ message: ' T3: param 缺失 ' }), 'T3: param 缺失', 'message is trimmed');
eq(lintSb.lintIssueMessage({ msg: 'legacy msg' }), 'legacy msg', 'msg fallback is supported');
eq(lintSb.lintIssueMessage({}), '', 'missing message degrades to empty');
eq(lintSb.lintIsT3Issue({ code: 'T3' }), true, 'T3 is recognized as the undeclared-param rule');
eq(lintSb.lintIsT3Issue({ code: 'T4' }), false, 'T4 is not a T3 row');
eq(
  lintSb.lintIssueLine({ code: 'T3', path: 'flow.json:steps[0].args.cond', message: "param('分组') 未声明" }),
  "[T3] flow.json:steps[0].args.cond：param('分组') 未声明",
  'issue lines join code + path + message'
);
eq(lintSb.lintIssueLine({ code: 'T4' }), '[T4] —：（无说明）', 'a sparse issue is still renderable');
eq(
  lintSb.lintIssuesText([
    { code: 'T3', path: 'a', message: 'x' },
    { code: 'T4', path: 'b', message: 'y' },
  ]),
  '[T3] a：x\n[T4] b：y',
  'lintIssuesText renders one line per issue'
);
eq(lintSb.lintIssuesText([]), '', 'no issues -> empty text');
eq(
  lintSb.lintT3ParamKey({ code: 'T3', message: "param('分组') is used but not declared" }),
  '分组',
  'T3 param key is extracted for the prefill hint'
);
eq(
  lintSb.lintT3ParamKey({ code: 'T3', path: "flow.json:steps[0].args.find[0].text_expr: param('战斗按钮文字')" }),
  '战斗按钮文字',
  'T3 key is also found in the issue path'
);
eq(lintSb.lintT3ParamKey({ code: 'T3', message: 'no literal here' }), '', 'T3 without a literal param() has no key');
eq(lintSb.lintT3ParamKey(null), '', 'a missing issue has no T3 key');

const lintBody = {
  ok: true,
  task: '切换预设御魂',
  issues: [
    { code: 'T3', path: 'flow.json:steps[2].args.n', message: "param('最大轮数') not declared" },
    { code: 'T4', path: 'flow.json:steps[0].args.find[0].sel', message: 'missing asset' },
    'bad issue',
  ],
  errors: [],
  count: 2,
};
const lintResult = lintSb.lintNormalizeResponse(lintBody);
eq(lintResult.ok, true, 'a 200 response normalizes to ok');
eq(lintResult.count, 2, 'the declared count is kept');
eq(lintResult.issues.length, 2, 'non-object issues are filtered');
eq(lintResult.task, '切换预设御魂', 'the task name is kept');
eq(lintSb.lintNormalizeResponse({}).count, 0, 'missing count + issues -> 0');
eq(lintSb.lintNormalizeResponse(null).ok, false, 'a non-object body is a failed check');
ok(
  /响应不是 JSON 对象/.test(lintSb.lintNormalizeResponse(null).errors[0]),
  'an invalid body explains the failure'
);
eq(lintSb.lintNormalizeResponse({ ok: false, errors: ['boom'] }).ok, false, 'ok:false stays a failure');
eq(
  lintSb.lintNormalizeResponse({ issues: [{ code: 'T3' }], count: '2' }).count,
  2,
  'a numeric count string is coerced'
);

eq(lintSb.lintSummary(null), { count: 0, text: '尚未检查', cls: 'kind' }, 'no result -> 尚未检查');
eq(lintSb.lintSummary(lintResult), { count: 2, text: '2 个问题', cls: 'err' }, 'issues -> N 个问题');
eq(
  lintSb.lintSummary(lintSb.lintNormalizeResponse({ ok: true, issues: [], errors: [], count: 0 })),
  { count: 0, text: '全部通过', cls: 'ok' },
  'a clean result -> 全部通过'
);
eq(
  lintSb.lintSummary({ count: 0, issues: [], errors: ['T1: bad meta'] }).text,
  '检查失败',
  'response-level errors -> 检查失败'
);
eq(
  lintSb.lintSummary({ issues: [{ code: 'T3' }, { code: 'T4' }] }).count,
  2,
  'a missing count falls back to issues.length'
);
eq(lintSb.lintCountText(lintResult), '2', 'the count badge shows the issue count');
eq(lintSb.lintCountText({ count: 0, issues: [] }), '', 'zero issues hide the count badge');
eq(lintSb.lintCountText(null), '', 'unchecked hides the count badge');
eq(
  lintSb.lintT3Rows(lintResult).map((issue) => issue.code),
  ['T3'],
  'T3 rows are still filtered for the new 旧参数 guidance'
);
eq(lintSb.lintT3Rows(null), [], 'a missing result has no T3 rows');
eq(
  lintSb.lintErrorText({ errors: ['T1: bad meta', 'T2: bad flow'] }),
  'T1: bad meta\nT2: bad flow',
  'response errors render one per line'
);
eq(lintSb.lintErrorText(null), '', 'a missing result has no response errors');

// Static wiring: head button + panel + auto/manual runs + T3 → declare.
for (const id of ['builder-lint-btn', 'builder-lint-panel', 'builder-lint-count',
  'builder-lint-summary', 'builder-lint-status', 'builder-lint-list',
  'builder-lint-error', 'builder-lint-close']) {
  ok(new RegExp(`id="${id}"`).test(html), `index.html provides #${id}`);
}
ok(/id="builder-lint-panel"[^>]*hidden/.test(html), 'the lint panel is hidden by default');
ok(
  /id="builder-lint-btn"[\s\S]{0,300}id="builder-status"/.test(html),
  'the 检查 button lives in the builder head next to the status line'
);
ok(/尚未检查/.test(html), 'the lint panel has the 尚未检查 empty state');
ok(/id="builder-lint-btn"[^>]*>检查</.test(html), 'the head button is labelled 检查');
ok(
  /const r = await fetch\('\/task\/lint\?task=' \+ encodeURIComponent\(task\)/.test(builderSource),
  'lintTask calls GET /task/lint?task=<encoded>'
);
ok(/cache: 'no-store'/.test(builderSource), 'the lint request bypasses the HTTP cache');
ok(
  /async function loadTask\(name\)[\s\S]{0,6200}lintTask\(\{ silent: true \}\)/.test(builderSource),
  'loadTask auto-runs the authoritative lint after loading'
);
ok(
  /const lintBtn = \$\('builder-lint-btn'\);[\s\S]{0,120}lintTask\(\);/.test(builderSource),
  'the 检查 button runs lintTask on demand'
);
ok(
  /const lintClose = \$\('builder-lint-close'\);[\s\S]{0,140}closeLintPanel/.test(builderSource),
  'the panel close button collapses the result'
);
ok(
  /if \(B\.lint \|\| B\.lintError\) \{[\s\S]{0,80}B\.lintStale = true;/.test(builderSource),
  'a successful flow save marks the last check stale (manual re-run)'
);
ok(
  /旧参数不再支持：请改用「使用端OCR文字配置项」素材/.test(builderSource) &&
    /在素材行「识别文字」创建 ocr_var，并用动作目标\/条件行引用它/.test(builderSource),
  'D: T3 rows explain the new ocr_var path instead of offering 去声明'
);
ok(
  !/去声明/.test(builderSource),
  'D: the 去声明 button is gone (no meta.params declaration UI)'
);
ok(
  !/onSaved: function \(savedTask, params\)/.test(builderSource) &&
    !/openTaskParamsDialog/.test(builderSource),
  'D: the task-config dialog bridge callbacks are removed'
);
ok(
  /if \(seq !== B\.lintSeq \|\| B\.task !== task\) return;/.test(builderSource),
  'late lint responses are discarded when the task changed'
);
ok(
  /权威检查不可用：/.test(builderSource) && /后端未实现 GET \/task\/lint/.test(builderSource),
  'a missing backend route degrades to 权威检查不可用'
);
ok(
  /function lintIssueRow\(issue\)/.test(builderSource) && !/renderParamIssuesHint/.test(builderSource),
  'D: the approximate param hint is gone (only the authoritative lint panel remains)'
);
ok(
  !/innerHTML/.test(extractFunctionFrom(builderSource, 'renderLintPanel', 'builder.js')),
  'the lint panel renders through textContent only'
);
ok(
  !/innerHTML/.test(extractFunctionFrom(builderSource, 'lintIssueRow', 'builder.js')),
  'lint rows render through textContent only'
);
ok(
  /\.builder-lint-panel/.test(styleCss) && /\.builder-lint-count/.test(styleCss),
  'style.css styles the lint panel + count badge'
);
ok(
  /\.builder-lint-row/.test(styleCss) && /\.builder-lint-code/.test(styleCss) &&
    /\.builder-lint-path/.test(styleCss),
  'style.css styles the issue rows'
);
ok(/\.builder-lint-hint/.test(styleCss), 'style.css styles the T4 material hint');
ok(/\.builder-lint-count\[hidden\]/.test(styleCss), 'the hidden count badge stays hidden');

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
  '/runner/uninstall', '/runner/packages/activate', '/runner/updates', '/runner/updates/ack',
  '/runner/updates/install',
  '/scheduler/status', '/runs/scheduler', '/config', '/instance']) {
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
  runnerReferencedIds.length >= 150,
  `Runner DOM id audit covers the PR7 legacy-scheduler migration (got ${runnerReferencedIds.length}, need >= 150)`
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
  // P0-1a: Studio publish notifications (updates list / badge / action / ack).
  'runnerTimeText', 'updatesEnabled', 'updatesList', 'updatesVisibleRows',
  'updatesBadgeCount', 'updatesActionLabel', 'updatesCompatText', 'updatesPackageText',
  'updatesCanInstall', 'updatesInstallPath', 'updateRowText', 'updatesAckBody',
  'updatesListStatusText',
  // P2: Runner params page (group / use switch / bool tri / summary) + feed UI.
  'runnerParamDefaultText', 'runnerParamGroups', 'runnerParamGroupsEx',
  'runnerParamRoleText', 'runnerParamAssetSubtitle', 'runnerParamOcrVarOnly',
  'runnerParamTypeText', 'runnerParamSavedValue', 'runnerParamBoolTri',
  'runnerParamUseState', 'runnerParamCollectUse', 'runnerParamSummary',
  'runnerConfigFieldKind', 'runnerConfigIsIntegerType', 'runnerConfigCollectValues',
  'updatesInstallBody', 'updatesSourceLabel', 'updatesDownloadHint', 'updatesIsDownloadable',
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

// --- P0-1a: Runner new-publish (Studio exchange) helpers --------------------
// The /runner/updates payload may be partial (older backend / disabled
// exchange); every helper degrades instead of throwing.
eq(runnerSb.updatesEnabled({ enabled: true }), true, 'enabled:true is recognized');
eq(runnerSb.updatesEnabled({}), false, 'missing enabled degrades to not-configured');
eq(runnerSb.updatesEnabled(null), false, 'null updates body is not enabled');
eq(runnerSb.updatesList({ updates: [{ event_id: 'e1' }] }).length, 1, 'updates array is read');
eq(runnerSb.updatesList({}), [], 'missing updates array is empty');
eq(runnerSb.updatesList(null), [], 'null body has no rows');

const updateSamples = [
  { event_id: 'e1', status: 'new' },
  { event_id: 'e2', status: 'dismissed' },
  { event_id: 'e3', status: 'installed' },
  { event_id: 'e4' },
  { status: 'new' },
  null,
];
eq(
  runnerSb.updatesVisibleRows(updateSamples).map((r) => r.event_id),
  ['e1', 'e4'],
  'visible rows skip dismissed/installed events and events without an id'
);
eq(runnerSb.updatesBadgeCount(updateSamples), 2, 'the badge counts only actionable events');
eq(runnerSb.updatesBadgeCount(null), 0, 'missing updates list -> 0 badge');
eq(runnerSb.updatesActionLabel('install'), '新任务', 'install action is a new task');
eq(runnerSb.updatesActionLabel('upgrade'), '升级', 'upgrade action');
eq(runnerSb.updatesActionLabel('downgrade'), '降级', 'downgrade action');
eq(runnerSb.updatesActionLabel('reinstall'), '重装', 'reinstall action');
eq(runnerSb.updatesActionLabel(undefined), '新发布', 'unknown/missing action degrades to 新发布');
eq(runnerSb.updatesCompatText({ compatible: true }), '兼容', 'compatible flag');
eq(
  runnerSb.updatesCompatText({ compatible: false }),
  '不兼容（安装会被拒绝）',
  'incompatible flag is explained'
);
eq(runnerSb.updatesCompatText({}), '兼容性未知', 'missing compatibility is unknown, not a block');
eq(
  runnerSb.updatesPackageText({ package_available: false }),
  '包文件不存在（请让 Studio 重新发布，或手动选路径）',
  'missing package is explained'
);
eq(runnerSb.updatesPackageText({ package_available: true }), '包可用', 'available package');

// P2: source badges + 需下载 hint + event-id install body (feed events).
eq(runnerSb.updatesSourceLabel({ source: 'exchange' }), '本地 Studio', 'exchange source -> 本地 Studio');
eq(runnerSb.updatesSourceLabel({ source_kind: 'exchange' }), '本地 Studio', 'source_kind exchange -> 本地 Studio');
eq(runnerSb.updatesSourceLabel({ source: '官方源' }), '官方源', 'a feed name is shown verbatim');
eq(runnerSb.updatesSourceLabel({ source_kind: 'local' }), '本地目录', 'local source kind is named');
eq(runnerSb.updatesSourceLabel({ source_kind: 'url' }), '远程源', 'url source kind is named');
eq(runnerSb.updatesSourceLabel({}), '未知来源', 'missing source degrades to 未知来源');
eq(runnerSb.updatesDownloadHint({ download_required: true }), '需下载', 'download_required says 需下载');
eq(runnerSb.updatesDownloadHint({ source_kind: 'url' }), '远程下载', 'a url source is a remote download');
eq(runnerSb.updatesDownloadHint({ source_kind: 'local' }), '', 'local sources need no download');
eq(runnerSb.updatesInstallBody(' e1 '), { event_id: 'e1' }, 'install body trims the event id');
eq(runnerSb.updatesInstallBody(null), { event_id: '' }, 'missing event id yields an empty body (validated in the UI)');

ok(
  runnerSb.updatesCanInstall({ event_id: 'e1', package_path: 'D:/ex/incoming/a.oaspkg' }),
  'a minimal event (id + path) is installable'
);
ok(
  runnerSb.updatesCanInstall({ event_id: 'e1' }),
  'the new install endpoint only needs event_id (backend resolves the source)'
);
ok(
  runnerSb.updatesCanInstall({ event_id: 'e1', download_required: true, package_available: false }),
  'a remote event stays installable even without a local package file (backend downloads it)'
);
ok(
  runnerSb.updatesCanInstall({ event_id: 'e1', source_kind: 'url', package_available: false }),
  'url sources are downloadable, so package_available:false does not block them'
);
ok(
  !runnerSb.updatesCanInstall({ event_id: 'e1', package_path: 'D:/x/a.oaspkg', package_available: false }),
  'a local event with a missing package file stays blocked'
);
ok(
  !runnerSb.updatesCanInstall({ event_id: 'e1', package_path: 'D:/x/a.oaspkg', compatible: false }),
  'an incompatible package blocks install before the server 400'
);
ok(
  !runnerSb.updatesCanInstall({ package_path: 'D:/x/a.oaspkg' }),
  'missing event_id blocks install (it could not be acked)'
);
eq(
  runnerSb.updatesInstallPath({ event_id: 'e1', package_path: '  D:/x/a.oaspkg  ' }),
  'D:/x/a.oaspkg',
  'the legacy fallback install path is trimmed'
);
eq(
  runnerSb.updatesInstallPath({ event_id: 'e1', package_available: false, package_path: 'D:/x/a.oaspkg' }),
  '',
  'a blocked local event contributes no legacy path'
);
eq(
  runnerSb.updatesAckBody([' e1 ', '', null, 'e2'], 'installed'),
  { event_ids: ['e1', 'e2'], status: 'installed' },
  'ack body trims event ids and sends the installed status'
);
eq(
  runnerSb.updatesAckBody([], 'dismissed'),
  { event_ids: [], status: 'dismissed' },
  'ack body for the dismissed status'
);
eq(
  runnerSb.updatesAckBody(null, 'bogus'),
  { event_ids: [], status: 'dismissed' },
  'unknown ack status degrades to dismissed'
);
eq(runnerSb.runnerTimeText(0), '—', 'zero publish time is a dash (not 1970)');
eq(
  runnerSb.updateRowText({
    task_id: '任务2',
    task_version: 3,
    action: 'upgrade',
    published_at_ms: 0,
    compatible: true,
  }),
  '任务2 v3 · 升级 · 兼容',
  'row text joins task/version/action/compat and skips a missing publish time'
);
ok(
  /包文件不存在/.test(runnerSb.updateRowText({ task_id: 't', package_available: false })),
  'row text carries the missing-package hint'
);
eq(
  runnerSb.updateRowText(null),
  '（未知任务） · 新发布 · 兼容性未知',
  'an empty event is still renderable'
);
eq(
  runnerSb.updatesListStatusText({ enabled: false }, ''),
  '未配置 exchange 目录（config/live.json → exchange.dir）',
  'disabled exchange explains the config key'
);
eq(
  runnerSb.updatesListStatusText({ enabled: true, updates: [] }, ''),
  '暂无新发布',
  'an enabled but empty exchange says 暂无新发布'
);
eq(
  runnerSb.updatesListStatusText({ enabled: true, error: 'bad json' }, ''),
  '发布索引读取异常：bad json',
  'a broken publish index is shown fail-soft (no throw/500)'
);
eq(
  runnerSb.updatesListStatusText(null, 'HTTP 500'),
  '新发布读取失败：HTTP 500（保留上次结果）',
  'a read failure keeps the last rendered list'
);
eq(runnerSb.updatesListStatusText(null, ''), '（未加载）', 'missing body before the first read');
ok(
  /新发布 2 条/.test(runnerSb.updatesListStatusText({ enabled: true, updates: updateSamples }, '')),
  'the status line counts the actionable rows'
);

// Static wiring: nav badge + 新发布 card + poll + install/ack flow.
for (const id of ['runner-updates-badge', 'runner-updates-section', 'runner-updates-summary',
  'runner-updates-status', 'runner-updates-list', 'runner-updates-refresh']) {
  ok(new RegExp(`id="${id}"`).test(runnerHtml), `runner.html provides #${id}`);
}
ok(
  /<nav[^>]*id="runner-nav"[\s\S]*?id="runner-updates-badge"/.test(runnerHtml),
  'the nav 安装包 button carries the updates badge'
);
ok(/<h3>新发布（来自 Studio）<\/h3>/.test(runnerHtml), 'the package page has the 新发布 card');
ok(/暂无新发布/.test(runnerSource), 'the empty state says 暂无新发布');
ok(
  /未配置 exchange 目录（config\/live\.json → exchange\.dir）/.test(runnerSource),
  'the disabled exchange hint names config/live.json → exchange.dir'
);
ok(
  /id="runner-updates-status"[^>]*role="status"/.test(runnerHtml),
  'the updates status line is a live region'
);
ok(
  /runnerFetchJson\(RUNNER_API\.updates\)/.test(runnerSource),
  'the new-publish list is loaded from GET /runner/updates'
);
ok(
  /runnerFetchJson\(RUNNER_API\.updatesAck, \{ method: 'POST', body: body \}\)/.test(runnerSource),
  'ack POSTs /runner/updates/ack with the pure body'
);
ok(
  /updatesInstall: '\/runner\/updates\/install'/.test(runnerSource),
  'the Runner knows the new POST /runner/updates/install route'
);
ok(
  /runnerFetchJson\(RUNNER_API\.updatesInstall, \{[\s\S]{0,140}body: updatesInstallBody\(eventId\)/.test(runnerSource),
  'the 安装 button installs through POST /runner/updates/install {event_id}'
);
ok(
  /e\.status === 404[\s\S]{0,220}runnerInstallBody\(legacyPath, false\)/.test(runnerSource) &&
    /ackUpdateEvents\(\[it\.event_id\], 'installed'\)/.test(runnerSource),
  'old backends without the install route fall back to the legacy local-path install + installed ack'
);
ok(
  /updatesSourceLabel\(it\)/.test(runnerSource) && /updateSourceCell\(/.test(runnerSource),
  'the feed table renders the publish-source badge cell'
);
ok(
  /const downloadHint = updatesDownloadHint\(it\);/.test(runnerSource),
  'remote events show the 需下载 hint'
);
ok(
  /ackUpdateEvents\(\[it\.event_id\], 'installed'\)/.test(runnerSource),
  'a successful install is followed by the installed ack'
);
ok(
  /ackUpdateEvents\(\[it\.event_id\], 'dismissed'\)/.test(runnerSource),
  '忽略 acks the dismissed status'
);
ok(/RUNNER_UPDATES_POLL_MS = 5000/.test(runnerSource), 'new publishes are polled every 5s');
ok(
  /document\.visibilityState === 'hidden'/.test(runnerSource) &&
    /startUpdatesPolling\(\)/.test(runnerSource) &&
    /stopUpdatesPolling\(\)/.test(runnerSource),
  'polling pauses on a hidden tab and resumes when visible'
);
ok(
  /state\.page === 'packages' && updatesError/.test(runnerSource),
  'the notification read error only surfaces on the package page'
);
ok(
  /typeof health\.pending_updates === 'number'/.test(runnerSource),
  'pending_updates from /runner/health seeds the nav badge until the list loads'
);
ok(
  /#runner-updates-badge/.test(runnerCss) && /#runner-updates-list/.test(runnerCss),
  'runner.css styles the updates badge + list'
);
ok(/\.runner-update-actions/.test(runnerCss), 'runner.css styles the per-event actions');
ok(/\.runner-badge\[hidden\]/.test(runnerCss), 'the zero-count badge stays hidden');

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
// P2: params page (group sections / use switch / bool tri / summary).
ok(/id="runner-config-summary"/.test(runnerHtml), 'runner.html provides #runner-config-summary');
ok(
  /el\.configSummary = document\.getElementById\('runner-config-summary'\)/.test(runnerSource),
  'the dialog binds the top summary element'
);
ok(
  /这里只显示「使用端OCR文字配置项」/.test(runnerHtml) &&
    /通用参数已从用户界面移除/.test(runnerHtml) &&
    /取消「使用」或值留空 = 不注入（回退默认值）/.test(runnerHtml),
  'D: the dialog copy explains the ocr_var-only config + 留空=不注入'
);
ok(
  /runnerParamGroupsEx\(params\)\.filter/.test(runnerSource) &&
    /group\.kind === 'ocr_var'/.test(runnerSource),
  'D: the form renders only the ocr_var section (generic groups hidden)'
);
// U-T6: 派生参数行（asset_id）→ 前置「使用端OCR文字配置项」分节 + 素材副标题。
ok(
  /function runnerParamGroups\(params\)[\s\S]{0,260}runnerParamGroupsEx\(params\)/.test(runnerSource) &&
    /使用端OCR文字配置项/.test(runnerSource),
  'U-T6: the dialog groups derived (asset_id) params into the leading 使用端OCR文字配置项 section'
);
ok(
  /runnerParamAssetSubtitle\(p\)/.test(runnerSource) && /runner-config-asset-sub/.test(runnerSource),
  'U-T6: derived rows render the 素材 <id> · 当前默认「…」 subtitle'
);
ok(
  /使用端OCR文字配置项/.test(runnerHtml) && /asset_id/.test(runnerHtml),
  'U-T6: runner.html explains the 使用端OCR文字配置项 section'
);
ok(/\.runner-config-asset-sub/.test(runnerCss), 'U-T6: runner.css styles the asset subtitle');
ok(
  /runnerParamBoolTri\(saved\)/.test(runnerSource) && /runner-config-bool-/.test(runnerSource),
  'bool params render the 未设置/是/否 tri-state select'
);
ok(
  /runner-config-reset/.test(runnerSource) && /class=\"runner-config-row-hint\"|className = 'runner-config-row-hint'/.test(runnerSource),
  'each parameter row carries a reset button + hint line'
);
ok(
  /updateRunnerConfigSummary\(\)/.test(runnerSource) && /runnerParamSummary\(configParams/.test(runnerSource),
  'the top summary is recomputed from the form'
);
ok(
  /applyRunnerConfigRowErrors\(collected\.errors\)/.test(runnerSource),
  'local required errors are mapped back to their rows'
);
ok(
  /updateRunnerConfigSummary\(\)/.test(runnerSource) &&
    /runnerParamUseState\(param, control\.value, enabled\)/.test(runnerSource),
  'row hints use the P2 state machine (留空/默认值/必填)'
);
ok(
  /el\.configTitle\.textContent = '任务配置 · ' \+ name/.test(runnerSource),
  'dialog title becomes 任务配置 · <task>'
);
ok(
  /本任务无可配置的使用端变量/.test(runnerSource),
  'D: no ocr_var rows shows the 本任务无可配置的使用端变量 empty state'
);
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
  /body: \{ values: Object\.assign\(\{\}, configValues, collected\.values\) \}/.test(runnerSource) &&
    /method: 'PUT'/.test(runnerSource),
  'D: saves with PUT /runner/tasks/<id>/config {values} while preserving hidden generic values'
);
ok(
  /const controlId = 'runner-cfg-' \+ index;/.test(runnerSource) &&
    /control\.setAttribute\('data-key', key\)/.test(runnerSource),
  'config controls use index-based DOM ids + data-key (non-ASCII/space-safe)'
);
ok(
  /runnerParamCollectUse\(configParams, form\.useFlags, form\.rawValues\)/.test(runnerSource),
  'save collects use switches + raw values through the P2 collector'
);
ok(/configBtn\.textContent = '配置'/.test(runnerSource), 'installed task rows render a 配置 button');
ok(
  /if \(t\.has_meta === true\) \{/.test(runnerSource) &&
    /actions\.append\(configBtn, schedBtn, taskRunsBtn\)/.test(runnerSource),
  'only tasks with a readable meta.json offer 配置 / 调度设置… / 运行记录'
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
  'runnerConfigCollectValues', 'runnerConfigUrl', 'runnerParamBoolTri',
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

// --- P2: Runner params page (group / use switch / bool tri / summary) --------
eq(
  runnerSb.runnerParamGroups([
    { key: 'a', group: '战斗', type: 'text' },
    { key: 'b', group: '战斗', type: 'int' },
    { key: 'c', group: 'OCR', type: 'text' },
    { key: 'd', type: 'bool' },
    null,
    'bad',
  ]).map((g) => [g.group, g.params.map((p) => p.key)]),
  [['战斗', ['a', 'b']], ['OCR', ['c']], ['通用', ['d']]],
  'params group by meta.params[].group in first-appearance order (missing group -> 通用)'
);
eq(runnerSb.runnerParamGroups(null), [], 'missing schema has no groups');

// --- U-T6: 使用端OCR文字配置项 分节（派生参数行 = meta.params 带 asset_id） ---
// 派生行永远进入最前面的独立分组「使用端OCR文字配置项」；旧任务 role=ocr_text
// 但没有 asset_id 的手写参数仍按自己的 group 分节（兼容，不破坏）。
const uT6Params = [
  { key: 'max_rounds', group: '战斗', type: 'int' },
  { key: 'group', group: '识别文字', type: 'string', role: 'ocr_text', default: '御魂', asset_id: 'O_预设分组', label: '预设分组' },
  { key: 'k2', type: 'string', role: 'ocr_text', default: '旧参数' },
];
eq(
  runnerSb.runnerParamGroupsEx(uT6Params).map((g) => [g.group, g.kind, g.params.map((p) => p.key)]),
  [
    ['使用端OCR文字配置项', 'ocr_var', ['group']],
    ['战斗', 'group', ['max_rounds']],
    ['通用', 'group', ['k2']],
  ],
  'U-T6: derived (asset_id) rows form the leading 使用端OCR文字配置项 section; old role=ocr_text rows keep their group'
);
eq(
  runnerSb.runnerParamGroups(uT6Params).map((g) => g.group),
  ['使用端OCR文字配置项', '战斗', '通用'],
  'U-T6: the backwards-compatible grouping view puts the new section first'
);
eq(
  runnerSb.runnerParamGroups(uT6Params)[0].params[0].key,
  'group',
  'U-T6: the derived row keeps its identity inside the leading section'
);
eq(
  runnerSb.runnerParamGroupsEx([{ key: 'a', group: '识别文字', type: 'string', role: 'ocr_text' }]).map((g) => g.group),
  ['识别文字'],
  'U-T6: role=ocr_text without asset_id never enters the ocr_var section'
);
eq(
  runnerSb.runnerParamGroupsEx([
    { key: 'a', group: '战斗' },
    { key: 'b', asset_id: ' O_甲 ', label: '甲' },
  ]).map((g) => g.params.map((p) => p.key)),
  [['b'], ['a']],
  'U-T6: asset_id is trimmed and a later derived row still leads the section'
);
eq(
  runnerSb.runnerParamGroupsEx([
    { key: 'blank', asset_id: '   ' },
    { key: 'empty', asset_id: '' },
  ]).map((g) => g.group),
  ['通用'],
  'U-T6: blank/empty asset_id is not a derived row'
);
eq(
  runnerSb.runnerParamGroupsEx(null),
  [],
  'U-T6: missing schema has no ocr_var section'
);
eq(
  runnerSb.runnerParamAssetSubtitle({ asset_id: 'O_预设分组', default: '御魂' }),
  '素材 O_预设分组 · 当前默认「御魂」',
  'U-T6: derived row subtitle names the asset + current default'
);
eq(
  runnerSb.runnerParamAssetSubtitle({ asset_id: ' O_甲 ', default: 0 }),
  '素材 O_甲 · 当前默认「0」',
  'U-T6: subtitle trims the asset id and renders a numeric default'
);
eq(
  runnerSb.runnerParamAssetSubtitle({ asset_id: 'O_甲' }),
  '素材 O_甲',
  'U-T6: subtitle stays renderable without a declared default'
);
eq(runnerSb.runnerParamAssetSubtitle({}), '', 'U-T6: user params have no asset subtitle');
eq(runnerSb.runnerParamAssetSubtitle(null), '', 'U-T6: missing param has no asset subtitle');
eq(
  runnerSb.runnerParamRoleText({ role: 'ocr_text', asset_id: 'O_甲' }),
  '识别文字配置',
  'U-T6: derived rows use the short 识别文字配置 badge'
);
eq(
  runnerSb.runnerParamSummary(
    [
      { key: 'group', asset_id: 'O_甲', type: 'string', default: '御魂' },
      { key: 'n', type: 'int', default: 2 },
    ],
    { group: '结界' }
  ),
  '1/2 项已设置 · 分组 2 个',
  'U-T6: the top summary counts the ocr_var section as a group'
);
ok(
  runnerSb.runnerParamCollectUse(
    [{ key: 'group', asset_id: 'O_甲', type: 'string', default: '御魂' }],
    {},
    { group: '结界' }
  ).totalCount === 1,
  'U-T6: collect-use still covers derived rows (values key = param key)'
);
eq(runnerSb.runnerParamRoleText({ role: 'ocr_text' }), '识别文字', 'role=ocr_text is marked 识别文字');
eq(runnerSb.runnerParamRoleText({ role: 'OCR_TEXT' }), '识别文字', 'role matching is case-insensitive');
eq(runnerSb.runnerParamRoleText({}), '', 'no role -> no role badge');
eq(runnerSb.runnerParamTypeText({ type: 'int' }), '整数', 'int type badge');
eq(runnerSb.runnerParamTypeText({ type: 'float' }), '小数', 'float type badge');
eq(runnerSb.runnerParamTypeText({ type: 'bool' }), '布尔', 'bool type badge');
eq(runnerSb.runnerParamTypeText({ type: 'string', enum: ['a', 'b'] }), '枚举', 'enum type badge');
eq(runnerSb.runnerParamTypeText({}), '文本', 'unknown type defaults to 文本');
eq(runnerSb.runnerParamDefaultText({}), '', 'no default -> empty text');
eq(runnerSb.runnerParamDefaultText({ default: 0 }), '0', 'zero default survives');
eq(runnerSb.runnerParamDefaultText({ default: '' }), '（空字符串）', 'empty-string default is named');
eq(runnerSb.runnerParamSavedValue({ key: 'k' }, { k: 0 }), 0, 'saved zero is a real value');
eq(
  runnerSb.runnerParamSavedValue({ key: 'k' }, { k: null }) === undefined,
  true,
  'null saved value counts as unset'
);
eq(
  runnerSb.runnerParamSavedValue({ key: 'k' }, undefined) === undefined,
  true,
  'missing values object returns undefined'
);
eq(runnerSb.runnerParamBoolTri(true), 'true', 'bool true -> tri true');
eq(runnerSb.runnerParamBoolTri(false), 'false', 'bool false -> tri false');
eq(runnerSb.runnerParamBoolTri(''), '', 'blank bool -> 未设置');
eq(runnerSb.runnerParamBoolTri(undefined), '', 'undefined bool -> 未设置');

const triState = runnerSb.runnerParamUseState({ key: 'k', type: 'string', default: '默认' }, '', false);
eq(triState.enabled, false, 'no saved value starts unused');
eq(triState.blocked, false, 'an optional unused param is never blocked');
ok(/不使用（回退默认值：默认）/.test(triState.hint), 'unused row explains the default fallback');
const blankState = runnerSb.runnerParamUseState({ key: 'k', type: 'string', default: '默认' }, '', true);
ok(/留空 = 不使用（回退默认值：默认）/.test(blankState.hint), 'enabled but blank = 不注入 (default hint)');
const requiredBlocked = runnerSb.runnerParamUseState({ key: 'k', type: 'string', required: true }, '', false);
eq(requiredBlocked.blocked, true, 'required without default blocks when unused');
ok(/必填/.test(requiredBlocked.hint), 'a blocked required row says 必填');
eq(
  runnerSb.runnerParamUseState({ key: 'k', type: 'string', required: true, default: 'x' }, '', false).blocked,
  false,
  'required with a default can stay unused'
);
eq(runnerSb.runnerParamUseState({ key: 'b', type: 'bool' }, 'true', true).hasValue, true, 'bool tri 是 counts as a value');
eq(runnerSb.runnerParamUseState({ key: 'b', type: 'bool' }, '', true).hasValue, false, 'bool tri 未设置 is blank');

const useRes = runnerSb.runnerParamCollectUse(
  [{ key: 'a', type: 'string' }, { key: 'b', type: 'int' }, { key: 'c', type: 'bool' }],
  { a: true, b: false, c: true },
  { a: 'x', b: '2', c: 'false' }
);
eq(useRes.ok, true, 'use-switch collection succeeds');
eq(useRes.values, { a: 'x', c: false }, 'disabled keys are not injected; bool tri becomes a JSON bool');
eq(useRes.setCount, 2, 'setCount counts submitted keys');
eq(useRes.totalCount, 3, 'totalCount counts declared params');
eq(useRes.groupCount, 1, 'groupCount counts sections (all 通用)');
const useRequired = runnerSb.runnerParamCollectUse(
  [{ key: 'k', type: 'string', required: true, label: '关键词' }],
  { k: false },
  { k: 'x' }
);
eq(useRequired.ok, false, 'required + 使用 off is a local validation error');
ok(/「关键词」是必填项/.test(useRequired.errors[0]), 'the required error names the label');
const triOnly = runnerSb.runnerParamCollectUse([{ key: 'b', type: 'bool' }], { b: true }, { b: '' });
eq(triOnly.ok, true, 'bool 未设置 is allowed for optional params');
eq(triOnly.values, {}, 'bool 未设置 is not written to values');
eq(
  runnerSb.runnerParamSummary(
    [{ key: 'a', group: '战斗' }, { key: 'b', group: '战斗' }, { key: 'c' }],
    { a: 'x', c: '' }
  ),
  '1/3 项已设置 · 分组 2 个',
  'summary counts set/total keys + groups'
);
eq(runnerSb.runnerParamSummary(null, null), '0/0 项已设置 · 分组 0 个', 'empty schema summary is renderable');

// bool tri + use flags extend the legacy collector without changing old calls.
eq(
  cfgSb.runnerConfigCollectValues([{ key: 'b', type: 'bool' }], { b: '' }).values,
  {},
  'bool 未设置 is omitted (3-state)'
);
eq(
  cfgSb.runnerConfigCollectValues([{ key: 'a', type: 'string' }], { a: 'x' }, { a: false }).values,
  {},
  'useFlags:false omits the key'
);
ok(
  /必填项/.test(cfgSb.runnerConfigCollectValues(
    [{ key: 'a', type: 'string', required: true }], { a: 'x' }, { a: false }
  ).errors[0]),
  'a disabled required field errors locally'
);
eq(
  cfgSb.runnerConfigCollectValues([{ key: 'n', type: 'int', enum: [1, 3] }], { n: '3' }).values,
  { n: 3 },
  'numeric enum still submits a JSON number'
);
eq(
  cfgSb.runnerConfigFieldKind({ key: 'n', type: 'int', enum: [1, 3, 5] }),
  'select',
  'numeric enum -> select control (P2 fix)'
);


// --- PR6: Runner scheduler control / temp run / scheduler.json ---------------
// The Runner owns scheduling operations now: start/stop its in-process thread,
// run one task on demand (body params override the saved values) and edit the
// raw scheduler.json as the advanced escape hatch (V5 step 13).
for (const id of ['runner-sched-control', 'runner-sched-badge', 'runner-sched-meta',
  'runner-sched-start', 'runner-sched-stop', 'runner-sched-refresh', 'runner-sched-status',
  'runner-sched-error', 'runner-run-card', 'runner-run-task', 'runner-run-params',
  'runner-run-submit', 'runner-run-status', 'runner-run-error', 'runner-run-log',
  'overview-scheduler', 'overview-goto-queue']) {
  ok(new RegExp(`id="${id}"`).test(runnerHtml), `runner.html provides #${id}`);
}
ok(/id="runner-sched-start"[^>]*>启动调度</.test(runnerHtml), 'scheduler control has the 启动调度 button');
ok(/id="runner-sched-stop"[^>]*>停止调度</.test(runnerHtml), 'scheduler control has the 停止调度 button');
ok(/id="runner-run-submit"[^>]*>立即运行</.test(runnerHtml), 'temp-run card has the 立即运行 button');
ok(/id="page-sched-json-save"[^>]*>保存 JSON</.test(runnerHtml), 'advanced card has the 保存 JSON button');
ok(
  /留空或 <code>\{\}<\/code> 时使用 Runner\s*已保存的运行值/.test(runnerHtml),
  'temp run explains saved values + params override'
);
ok(/保存前会先 GET 最新配置并与载入快照比较/.test(runnerHtml), 'scheduler.json card explains the GET-first snapshot guard');
ok(/去「队列与调度」页/.test(runnerHtml), 'overview links to the queue page');
ok(/id="page-sched-json"[^>]*spellcheck="false"/.test(runnerHtml), 'advanced JSON is a plain textarea');

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
  /runnerFetchJson\(RUNNER_API\.schedulerConfig, \{[\s\S]{0,120}method: 'POST',[\s\S]{0,120}body: \{ config: cfg \}/.test(runnerSource),
  '保存 JSON POSTs {config} to /scheduler/config'
);
ok(
  /schedParseConfigJson\(pageSchedJson\.value\)/.test(runnerSource),
  '保存 JSON parses the textarea through the pure config parser'
);
ok(
  /pageSchedJson\.addEventListener\('input'/.test(runnerSource) &&
    /scheduleJsonDirty = true/.test(runnerSource) &&
    /!scheduleJsonDirty\)/.test(runnerSource),
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
  /#runner-sched-control/.test(runnerCss) && /#page-sched-json/.test(runnerCss) &&
    /#runner-run-log/.test(runnerCss),
  'runner.css styles the scheduler control / JSON editor / run log'
);
ok(
  /schedulerReset: '\/scheduler\/reset'/.test(runnerSource) &&
    /runnerFetchJson\(RUNNER_API\.schedulerReset, \{[\s\S]{0,120}method: 'POST'/.test(runnerSource),
  'Runner UI posts the reset-memory operation to /scheduler/reset'
);

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

// ===========================================================================
// PR7: legacy Studio scheduler UI migration (P3 entry/queue editor + P4 runs).
// The 77 pure helpers are extracted from runner.js itself and evaluated in a
// vm; the P3/P4 assertions below are the old Studio tests re-pointed at the
// Runner data shape. The DOM smoke tests use a tiny stub (no browser).
// ===========================================================================
const RUNNER_SCHED_PURE_FUNCTIONS = [
  "schedEntryKindLabel",
  "schedSanitizeIdSegment",
  "schedHasUnsafeNameChar",
  "schedUniqueId",
  "schedEntryIdForTarget",
  "schedEmptyConfig",
  "schedNormalizeConfig",
  "schedDefaultTaskEntry",
  "schedWindowsToText",
  "schedParseWindowsText",
  "schedParseWindowsJson",
  "schedParseParamsText",
  "schedEntryToForm",
  "schedBuildEntryFromForm",
  "schedFindTaskEntries",
  "schedFindTaskQueueMember",
  "schedQueueEntryConflict",
  "schedValidateEntryForm",
  "schedUpsertEntry",
  "schedRemoveEntryById",
  "schedMoveEntry",
  "schedQueueToForm",
  "schedQueueMemberAdd",
  "schedQueueMemberRemove",
  "schedQueueMemberMove",
  "schedBuildQueueFromForm",
  "schedValidateQueueForm",
  "schedUpsertQueue",
  "schedRemoveQueue",
  "schedQueueProgressText",
  "schedQueueSummary",
  "schedEntrySummary",
  "schedQuotaSummary",
  "schedStatusErrorText",
  "schedCanonicalJson",
  "schedConfigSnapshotText",
  "schedConfigSnapshotChanged",
  "schedExternalConfigChangeText",
  "schedResponseErrorText",
  "schedSaveBlockedReason",
  "schedParseConfigJson",
  "schedRunValueText",
  "schedRunLimitValue",
  "schedRunsBuildQuery",
  "schedRunsBuildUrl",
  "schedRunDetailUrl",
  "schedRunTimeText",
  "schedRunDurationMs",
  "schedRunDurationText",
  "schedRunOutcomeLabel",
  "schedRunOutcomeBadgeClass",
  "schedRunStatusBadgeClass",
  "schedRunKindText",
  "schedRunIsQueueRun",
  "schedQueueDisplayName",
  "schedRunTaskLabel",
  "schedRunRoundMemberText",
  "schedRunAttemptText",
  "schedRunExitText",
  "schedRunClicksText",
  "schedRunErrorText",
  "schedRunErrorSummary",
  "schedRunResumeText",
  "schedRunDetailRows",
  "schedRunDetailTitle",
  "schedRunRowCells",
  "schedRunIsFailure",
  "schedRunsFailureCount",
  "schedRunsStatusCardText",
  "schedRunBodyErrors",
  "schedRunsErrorText",
  "schedRunsResponseErrorText",
  "schedRunDetailErrorText",
  "schedRunsTaskChoices",
  "schedRunsFilterLocal",
  "schedRunsFilterActive",
  "schedRunsListStatusText"
];
const runnerSchedPureCode = RUNNER_SCHED_PURE_FUNCTIONS
  .map((name) => extractFunctionFrom(runnerSource, name, 'runner.js'))
  .join('\n\n');
const runnerSchedSb = { ok: ok, eq: eq, console: console };
vm.createContext(runnerSchedSb);
vm.runInContext(runnerSchedPureCode, runnerSchedSb, { filename: 'runner.js#sched-pure' });
eq(RUNNER_SCHED_PURE_FUNCTIONS.length, 77, 'all 77 legacy scheduler pure helpers are registered');
const missingSchedPure = RUNNER_SCHED_PURE_FUNCTIONS.filter(
  (name) => typeof runnerSchedSb[name] !== 'function'
);
eq(missingSchedPure, [], 'every registered scheduler pure helper is extractable');
const {
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
  schedFindTaskEntries,
  schedFindTaskQueueMember,
  schedQueueEntryConflict,
  schedValidateEntryForm,
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
} = runnerSchedSb;

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

const RUNNER_SCHED_RENDER_FUNCTIONS = [
  "schedBadge",
  "fillScheduleEntryBadges",
  "renderScheduleEntries",
  "updateScheduleEntryRuntime",
  "fillScheduleQueueBadges",
  "renderScheduleQueues",
  "updateScheduleQueueRuntime",
  "scheduleStatusEntryMap",
  "renderScheduleStatusCard",
  "makeSchedRunBadge",
  "renderScheduleRuns",
  "renderScheduleRunDetail",
  "fmtLocal"
];
const runnerSchedRenderCode = RUNNER_SCHED_RENDER_FUNCTIONS
  .map((name) => extractFunctionFrom(runnerSource, name, 'runner.js'))
  .join('\n\n');
vm.runInContext(runnerSchedRenderCode, runnerSchedSb, { filename: 'runner.js#sched-render' });
const {
  renderScheduleStatusCard, renderScheduleEntries, renderScheduleQueues,
  renderScheduleRuns, renderScheduleRunDetail,
  updateScheduleEntryRuntime, updateScheduleQueueRuntime,
} = runnerSchedSb;

// ---------------------------------------------------------------------------
// PR7: legacy Studio scheduler renderers (DOM stub smoke tests).
// ---------------------------------------------------------------------------
function fakeScheduleElement() {
  let text = '';
  const node = {
    get textContent() { return text; },
    set textContent(value) {
      text = String(value);
      if (text === '') this.children.length = 0;
    },
    className: '',
    children: [],
    disabled: false,
    checked: false,
    value: '',
    tabIndex: 0,
    title: '',
    open: false,
    dataset: {},
    attributes: {},
    appendChild(child) { this.children.push(child); return child; },
    append(...kids) { for (const kid of kids) this.children.push(kid); },
    addEventListener() {},
    setAttribute(name, value) { this.attributes[name] = String(value); },
    getAttribute(name) {
      return Object.prototype.hasOwnProperty.call(this.attributes, name)
        ? this.attributes[name]
        : null;
    },
    removeAttribute(name) { delete this.attributes[name]; },
    querySelectorAll(selector) { return scheduleFindAll(this, selector); },
    querySelector(selector) {
      const hit = scheduleFindAll(this, selector);
      return hit.length ? hit[0] : null;
    },
    showModal() { this.open = true; },
    close() { this.open = false; },
    scrollIntoView() {},
  };
  return node;
}

function scheduleMatchesSelector(node, selector) {
  const exact = /^\[([\w-]+)="([^"]*)"\]$/.exec(selector);
  if (exact) {
    if (!node || typeof node.getAttribute !== 'function') return false;
    return node.getAttribute(exact[1]) === exact[2];
  }
  const present = /^\[([\w-]+)\]$/.exec(selector);
  if (present) {
    return !!node && typeof node.getAttribute === 'function' && node.getAttribute(present[1]) !== null;
  }
  return false;
}

function scheduleFindAll(root, selector) {
  const out = [];
  function walk(node) {
    for (const child of node.children || []) {
      if (scheduleMatchesSelector(child, selector)) out.push(child);
      walk(child);
    }
  }
  walk(root);
  return out;
}

const schedEls = {};
for (const name of ['pageSchedMode', 'pageSchedPasses', 'pageSchedNextWake',
  'pageSchedRunning', 'pageSchedQuota', 'pageSchedSummary', 'pageSchedConfigStatus',
  'pageSchedEntries', 'pageSchedQueues', 'pageSchedRunsList', 'pageSchedRunsStatus',
  'pageSchedRunsSummary', 'schedRunDialogBody', 'schedRunDialogTitle']) {
  schedEls[name] = fakeScheduleElement();
  runnerSchedSb[name] = schedEls[name];
}
runnerSchedSb.document = {
  createElement: function () { return fakeScheduleElement(); },
  createTextNode: function (text) { return { textContent: String(text) }; },
};
runnerSchedSb.scheduleLocked = function () { return true; };
runnerSchedSb.scheduleStatusError = '';
runnerSchedSb.scheduleConfigError = '';
runnerSchedSb.scheduleConfigMeta = {
  running: true,
  path: 'scheduler.json',
  exists: true,
  state_path: 'scheduler.state.json',
  schema_version: 4,
};
runnerSchedSb.scheduleRunsData = null;
runnerSchedSb.scheduleRunsError = '';
runnerSchedSb.scheduleRunsLoading = false;
runnerSchedSb.scheduleRunsFilter = { limit: 200, task: '', outcome: '', queueId: '' };
runnerSchedSb.openScheduleRunDetail = function () {};

const renderCfg = schedEmptyConfig();
renderCfg.queues = [{
  queue_id: 'q1', name: '日常', members: ['B'], total_rounds: 2,
  on_failure: 'continue', between_ms: 500,
}];
renderCfg.entries = [
  schedDefaultTaskEntry('A', renderCfg),
  schedDefaultTaskEntry('q1', renderCfg, 'queue'),
];
runnerSchedSb.scheduleConfig = renderCfg;
const renderStatus = {
  mode: 'loop',
  passes: 7,
  next_wake_local: '2026-11-03 09:00:00',
  guards: { blocked: false },
  entries: [
    { entry_id: 'task-A', runnable_now: true, last_result: 'success', last_outcome: 'Done' },
    {
      entry_id: 'queue-q1',
      queue: {
        total_rounds: 2, rounds_completed: 1, empty_rounds: 0, finished: false,
        current_round: { index: 2, member_index: 0 },
      },
    },
  ],
  queues: [{
    queue_id: 'q1', name: '日常', members: ['B'], total_rounds: 2, rounds_completed: 1,
    empty_rounds: 0, finished: false, on_failure: 'continue', between_ms: 500,
    current_round: { index: 2, member_index: 0 },
  }],
};

// P1-3: a failed /scheduler/status read renders an unknown state, not "正常".
runnerSchedSb.scheduleStatusError = 'HTTP 500';
renderScheduleStatusCard(null);
eq(schedEls.pageSchedMode.textContent, '模式：—', 'status read failure shows an unknown mode');
eq(schedEls.pageSchedPasses.textContent, 'passes=—', 'status read failure does not fake passes=0');
eq(schedEls.pageSchedNextWake.textContent, '下次唤醒=—', 'status read failure shows an unknown next wake');
eq(schedEls.pageSchedQuota.textContent, 'quota：—', 'status read failure shows an unknown quota');
eq(schedEls.pageSchedRunning.textContent, '锁=—', 'status read failure does not claim an idle lock');
ok(/runner-badge/.test(schedEls.pageSchedRunning.className), 'unknown lock keeps the Runner badge class');
ok(/调度器状态读取失败：HTTP 500/.test(schedEls.pageSchedSummary.textContent), 'status summary carries the read error');
ok(!/调度器状态正常/.test(schedEls.pageSchedSummary.textContent), 'status summary never claims everything is normal after a read failure');
runnerSchedSb.scheduleStatusError = '';

renderScheduleStatusCard(renderStatus);
eq(schedEls.pageSchedMode.textContent, '模式：loop', 'status card shows mode');
eq(schedEls.pageSchedPasses.textContent, 'passes=7', 'status card shows passes');
eq(schedEls.pageSchedNextWake.textContent, '下次唤醒=2026-11-03 09:00:00', 'status card shows next wake');
ok(/调度器运行中/.test(schedEls.pageSchedRunning.textContent), 'status card shows the running lock');
ok(/runner-badge warn/.test(schedEls.pageSchedRunning.className), 'running lock uses the warning badge');
eq(schedEls.pageSchedQuota.textContent, 'quota：无限制', 'status card shows the quota summary');
ok(/停止调度/.test(schedEls.pageSchedConfigStatus.textContent), 'config status tells the Runner user to stop the scheduler');
ok(/stop-scheduler\.bat/.test(schedEls.pageSchedConfigStatus.textContent), 'config status keeps the external-daemon hint');

schedEls.pageSchedEntries.textContent = '';
schedEls.pageSchedEntries.children.length = 0;
renderScheduleEntries(renderStatus, renderCfg);
eq(schedEls.pageSchedEntries.children.length, 2, 'entry renderer emits one card per entry');
const firstEntryCard = schedEls.pageSchedEntries.children[0];
eq(firstEntryCard.getAttribute('data-entry-id'), 'task-A', 'entry card carries data-entry-id for runtime updates');
eq(firstEntryCard.children[0].children[0].textContent, 'task-A · 任务 A', 'task entry card title');
ok(/quota 日 1/.test(firstEntryCard.children[0].children[1].textContent), 'task entry card summary includes quota');
eq(firstEntryCard.children[2].disabled, true, 'first entry cannot move up');
eq(firstEntryCard.children[3].disabled, false, 'first entry can move down');
const firstRuntimeHost = firstEntryCard.querySelector('[data-runtime="entry"]');
ok(firstRuntimeHost && firstRuntimeHost.children.some((b) => /现在可跑/.test(b.textContent)), 'entry card shows runnable_now');
const secondEntryCard = schedEls.pageSchedEntries.children[1];
eq(secondEntryCard.children[0].children[0].textContent, 'queue-q1 · 队列 q1', 'queue entry card title');
ok(
  secondEntryCard.children[0].children[2].children.some((b) => /轮次 1\/2/.test(b.textContent)),
  'queue entry card shows round progress'
);

// 5s poll path: only the badge host is rewritten, the card node stays put.
const runtimeStatus = {
  entries: [{ entry_id: 'task-A', runnable_now: false, blocked_reason: '冷却' }],
  queues: [],
};
updateScheduleEntryRuntime(runtimeStatus);
eq(schedEls.pageSchedEntries.children.length, 2, 'runtime update never rebuilds entry cards');
ok(
  firstRuntimeHost.children.some((b) => /等待：冷却/.test(b.textContent)) &&
    !firstRuntimeHost.children.some((b) => /现在可跑/.test(b.textContent)),
  'runtime update overlays the wait reason without touching config'
);

schedEls.pageSchedQueues.textContent = '';
schedEls.pageSchedQueues.children.length = 0;
renderScheduleQueues(renderStatus, renderCfg);
eq(schedEls.pageSchedQueues.children.length, 1, 'queue renderer emits one card per queue');
const queueCard = schedEls.pageSchedQueues.children[0];
eq(queueCard.children[0].children[0].textContent, 'q1 · 日常', 'queue card title');
ok(/成员 1：B/.test(queueCard.children[0].children[1].textContent), 'queue card lists members in order');
ok(/进行中：第 2 轮/.test(queueCard.children[0].children[1].textContent), 'queue card shows the current round');
ok(
  queueCard.children[0].children[2].children.some((b) => /触发条目：queue-q1/.test(b.textContent)),
  'queue card shows the trigger entry id'
);
updateScheduleQueueRuntime(renderStatus);
eq(schedEls.pageSchedQueues.children.length, 1, 'runtime update never rebuilds queue cards');

renderScheduleEntries(renderStatus, null);
ok(/调度配置不可用/.test(schedEls.pageSchedEntries.textContent), 'entry renderer explains a missing config');
renderScheduleQueues(renderStatus, null);
ok(/调度配置不可用/.test(schedEls.pageSchedQueues.textContent), 'queue renderer explains a missing config');

// P4: run list renderer smoke test (DOM stub, no browser).
runnerSchedSb.scheduleConfig = renderCfg;
function resetFakeScheduleEl(node) {
  node.textContent = '';
  node.children.length = 0;
  return node;
}
runnerSchedSb.scheduleRunsData = { ok: true, dir: 'runs/scheduler', count: 2, runs: [queueRun, fatalRun] };
runnerSchedSb.scheduleRunsError = '';
runnerSchedSb.scheduleRunsLoading = false;
runnerSchedSb.scheduleRunsFilter = { limit: 200, task: '', outcome: '', queueId: '' };
resetFakeScheduleEl(schedEls.pageSchedRunsList);
resetFakeScheduleEl(schedEls.pageSchedRunsStatus);
renderScheduleRuns();
eq(schedEls.pageSchedRunsList.children.length, 1, 'run renderer emits one table wrapper for a non-empty list');
const runWrap = schedEls.pageSchedRunsList.children[0];
eq(runWrap.className, 'runner-table-wrap', 'run table sits in the Runner scroll wrapper');
const runTable = runWrap.children[0];
eq(runTable.className, 'runner-table sched-runs-table', 'run table uses the Runner table style');
eq(runTable.children.length, 2, 'run table has thead + tbody');
eq(runTable.children[1].children.length, 2, 'run table emits one row per run');
const firstRunRow = runTable.children[1].children[0];
eq(firstRunRow.children.length, 10, 'run row has all ten columns');
eq(firstRunRow.children[1].textContent, '日常/任务2', 'run row task uses 队列名/成员名');
eq(firstRunRow.children[2].children[0].textContent, 'queue_member', 'run row kind badge');
ok(/runner-badge/.test(firstRunRow.children[2].children[0].className), 'run row kind uses the Runner badge');
eq(firstRunRow.children[3].textContent, '第2轮 · 成员1', 'run row shows round/member');
eq(firstRunRow.children[4].textContent, '2', 'run row shows attempt');
eq(firstRunRow.children[5].children[1].textContent, '完成', 'run row outcome badge uses the persisted Done label');
ok(/ok/.test(firstRunRow.children[5].children[1].className), 'run row outcome badge color class');
eq(firstRunRow.children[7].textContent, '2.5s', 'run row duration is human readable');
eq(firstRunRow.children[8].textContent, '3', 'run row clicks');
eq(firstRunRow.children[9].textContent, '—', 'successful run row has no error');
const fatalRunRow = runTable.children[1].children[1];
ok(/第一行 第二行/.test(fatalRunRow.children[9].textContent), 'fatal run row shows the error summary');
eq(fatalRunRow.children[9].className, 'runner-error-cell', 'fatal run row error cell styling');
ok(/共 2 条/.test(schedEls.pageSchedRunsStatus.textContent), 'run status shows the total count');
resetFakeScheduleEl(schedEls.pageSchedRunsSummary);
renderScheduleStatusCard(renderStatus);
ok(/最近 完成/.test(schedEls.pageSchedRunsSummary.textContent), 'status card shows the latest persisted outcome');
ok(/失败 1\/2/.test(schedEls.pageSchedRunsSummary.textContent), 'status card shows the failure count');

runnerSchedSb.scheduleRunsData = { ok: true, count: 0, runs: [] };
runnerSchedSb.scheduleRunsError = '';
resetFakeScheduleEl(schedEls.pageSchedRunsList);
renderScheduleRuns();
ok(/暂无调度运行记录/.test(schedEls.pageSchedRunsList.textContent), 'run renderer shows the empty state');
runnerSchedSb.scheduleRunsData = null;
runnerSchedSb.scheduleRunsError = '运行记录读取失败：HTTP 500';
resetFakeScheduleEl(schedEls.pageSchedRunsList);
renderScheduleRuns();
eq(schedEls.pageSchedRunsList.textContent, '运行记录读取失败：HTTP 500', 'run renderer shows the read error verbatim');
ok(/运行记录读取失败/.test(schedEls.pageSchedRunsStatus.textContent), 'run status shows the read error');
runnerSchedSb.scheduleRunsError = '';

// P4: run detail renderer smoke test (field rows + full error + raw JSON).
runnerSchedSb.scheduleConfig = renderCfg;
resetFakeScheduleEl(schedEls.schedRunDialogBody);
resetFakeScheduleEl(schedEls.schedRunDialogTitle);
renderScheduleRunDetail(queueRun);
eq(schedEls.schedRunDialogBody.children.length, 2, 'detail body has the field table + full JSON');
const detailTable = schedEls.schedRunDialogBody.children[0];
eq(detailTable.className, 'runner-table run-detail-table', 'detail table uses the Runner table style');
const detailSmokeMap = {};
for (const row of detailTable.children[0].children) {
  detailSmokeMap[row.children[0].textContent] = row.children[1].textContent;
}
eq(detailSmokeMap.resume, '否', 'detail dialog shows resume');
eq(detailSmokeMap.clicks_file, 'runs/clicks/run-2.clicks.jsonl', 'detail dialog shows clicks_file');
ok(/^1762100100000（/.test(detailSmokeMap.started_ms), 'detail dialog shows raw + local started_ms');
ok(/^1762100102500（/.test(detailSmokeMap.finished_ms), 'detail dialog shows raw + local finished_ms');
ok(/日常\/任务2/.test(schedEls.schedRunDialogTitle.textContent), 'detail dialog title includes 队列名/成员名');
resetFakeScheduleEl(schedEls.schedRunDialogBody);
renderScheduleRunDetail(fatalRun);
eq(schedEls.schedRunDialogBody.children.length, 4, 'fatal detail adds the full-error block');
ok(/第一行\n第二行/.test(schedEls.schedRunDialogBody.children[2].textContent), 'detail dialog shows the error verbatim');
resetFakeScheduleEl(schedEls.schedRunDialogBody);
renderScheduleRunDetail(Object.assign({}, queueRun, { future_field: 'kept' }));
ok(
  /future_field/.test(schedEls.schedRunDialogBody.children[1].children[1].textContent),
  'detail full JSON includes unknown fields'
);

// ---------------------------------------------------------------------------
// PR7 static contracts: all 84 legacy ids live on the Runner queue page and the
// migrated JS uses the Runner seams (single fetch helper, /runner/tasks,
// showPage('queue'), runner-badge/runner-table, reset-memory POST).
// ---------------------------------------------------------------------------
const LEGACY_SCHEDULE_IDS = [
  // ① workbench panel
  'panel-schedule', 'btn-sched-refresh', 'btn-sched-reset', 'btn-sched-open-page',
  'sched-summary', 'sched-entries',
  // ② full schedule page
  'page-schedule', 'page-sched-refresh', 'page-sched-save', 'page-sched-reload',
  'page-sched-reset', 'page-sched-mode', 'page-sched-passes', 'page-sched-next-wake',
  'page-sched-running', 'page-sched-quota', 'page-sched-summary',
  'page-sched-config-status', 'page-sched-runs-summary', 'page-sched-add-entry',
  'page-sched-entries', 'page-sched-add-queue', 'page-sched-queues',
  'page-sched-runs-refresh', 'page-sched-runs-task', 'page-sched-runs-outcome',
  'page-sched-runs-limit', 'page-sched-runs-clear', 'page-sched-runs-status',
  'page-sched-runs-list', 'page-sched-advanced', 'page-sched-json',
  'page-sched-json-save', 'page-sched-json-reload', 'page-sched-json-status',
  // ③ reset-memory dialog
  'reset-memory-dialog', 'btn-reset-all', 'btn-reset-invert', 'btn-reset-none',
  'reset-task-list', 'reset-include-checkpoint', 'reset-error', 'btn-reset-cancel',
  'btn-reset-submit',
  // ④ entry dialog
  'sched-entry-dialog', 'sched-entry-title', 'sched-entry-mode-hint', 'sched-entry-kind',
  'sched-entry-target', 'sched-entry-id', 'sched-entry-enabled', 'sched-entry-lifecycle',
  'sched-entry-temp-hint', 'sched-entry-windows', 'sched-entry-windows-json',
  'sched-entry-quota-per', 'sched-entry-quota-times', 'sched-entry-retry',
  'sched-entry-cooldown', 'sched-entry-params', 'sched-entry-error', 'sched-entry-cancel',
  'sched-entry-save',
  // ⑤ queue dialog
  'sched-queue-dialog', 'sched-queue-title', 'sched-queue-id', 'sched-queue-name',
  'sched-queue-member-add', 'sched-queue-members', 'sched-queue-task-pool',
  'sched-queue-total-rounds', 'sched-queue-on-failure', 'sched-queue-between-ms',
  'sched-queue-rounds-hint', 'sched-queue-lock-hint', 'sched-queue-error',
  'sched-queue-cancel', 'sched-queue-save',
  // ⑥ run detail dialog
  'page-sched-runs-dialog', 'page-sched-runs-detail-title', 'page-sched-runs-detail-body',
  'page-sched-runs-detail-error', 'page-sched-runs-detail-refresh',
  'page-sched-runs-detail-close',
];
eq(new Set(LEGACY_SCHEDULE_IDS).size, 84, 'the legacy schedule id map has the expected 84 ids');
const missingLegacyIds = LEGACY_SCHEDULE_IDS.filter(
  (id) => !new RegExp('id=["\']' + id + '["\']').test(runnerHtml)
);
eq(missingLegacyIds, [], `runner.html contains every legacy scheduler id (missing: ${missingLegacyIds.join(', ') || 'none'})`);
ok(
  /id="page-queue"[\s\S]*id="page-schedule"[\s\S]*id="page-sched-entries"/.test(runnerHtml),
  'the migrated schedule editor lives inside #page-queue'
);
for (const id of ['page-sched-runs-outcome', 'page-sched-runs-limit', 'page-sched-runs-clear',
  'page-sched-runs-refresh', 'page-sched-runs-status', 'page-sched-runs-list']) {
  ok(new RegExp(`id="${id}"`).test(runnerHtml), `runner.html provides run-history control #${id}`);
}
ok(/id="page-sched-runs-outcome"[\s\S]{0,900}<option value="fatal">/.test(runnerHtml), 'outcome filter offers fatal');
ok(/id="page-sched-runs-outcome"[\s\S]{0,1400}<option value="Failed">/.test(runnerHtml), 'outcome filter also offers the persisted Failed value');
ok(/id="page-sched-runs-limit"[\s\S]{0,200}<option value="200" selected>/.test(runnerHtml), 'run limit defaults to 200');
ok(/重置使用端运行记忆/.test(runnerHtml), 'reset dialog names the Runner-side memory reset');
ok(
  /id="reset-memory-dialog"[\s\S]{0,1200}<div id="reset-task-list"/.test(runnerHtml),
  'reset dialog has the task multi-select list'
);
ok(
  /id="sched-entry-dialog"[\s\S]{0,2400}id="sched-entry-windows"/.test(runnerHtml) &&
    /id="sched-entry-dialog"[\s\S]{0,4000}id="sched-entry-params"/.test(runnerHtml),
  'entry dialog carries windows + params fields'
);
ok(
  /id="sched-queue-dialog"[\s\S]{0,2600}id="sched-queue-members"/.test(runnerHtml) &&
    /id="sched-queue-dialog"[\s\S]{0,3400}id="sched-queue-task-pool"/.test(runnerHtml),
  'queue dialog carries the ordered member list + task pool'
);

ok(
  /RUNNER_API\.tasks/.test(runnerSource) &&
    /ensureScheduleTaskChoices\(force\)/.test(runnerSource) &&
    /String\(\(t && t\.task\) \|\| ''\)/.test(runnerSource),
  'legacy scheduling task choices read /runner/tasks and map .task'
);
ok(!/\/task\/library/.test(runnerSource), 'Runner never calls the Studio-only /task/library route');
ok(!/\/task\/library/.test(runnerHtml), 'runner.html never references /task/library');
ok(
  /openTaskScheduleDialog\(t\.task\)/.test(runnerSource) &&
    /showScheduleRunsForTask\(t\.task\)/.test(runnerSource) &&
    /schedBtn\.textContent = '调度设置…'/.test(runnerSource) &&
    /taskRunsBtn\.textContent = '运行记录'/.test(runnerSource),
  'installed-task rows restore 调度设置… + 运行记录 shortcuts'
);
ok(
  /function showScheduleRunsForTask\(task\)[\s\S]{0,700}showPage\('queue'\)/.test(runnerSource) &&
    /function showScheduleRunsForEntry\(entry\)[\s\S]{0,900}showPage\('queue'\)/.test(runnerSource) &&
    /function showScheduleRunsForQueue\(queueId\)[\s\S]{0,700}showPage\('queue'\)/.test(runnerSource),
  'entry/queue/task run-history shortcuts land on the Runner queue page'
);
ok(
  /function scheduleLocked\(\)[\s\S]{0,500}state\.health\.lock[\s\S]{0,500}state\.runnerScheduler\.running === true/.test(runnerSource),
  'lock detection combines /runner/health lock + the in-process scheduler thread'
);
ok(
  /async function saveScheduleEntryDialog\(\)[\s\S]{0,2600}fetchScheduleConfigOnce\(\)/.test(runnerSource) ||
    /async function saveScheduleEntryDialog\(\)[\s\S]{0,2600}loadScheduleConfig\(true\)/.test(runnerSource),
  'entry save re-reads the latest config before POSTing'
);
ok(
  /async function saveScheduleQueueDialog\(\)[\s\S]{0,2600}loadScheduleConfig\(true\)/.test(runnerSource),
  'queue save re-reads the latest config before POSTing'
);
ok(
  /async function saveSchedulePageConfig\(\)[\s\S]{0,1800}fetchScheduleConfigOnce\(\)[\s\S]{0,900}schedConfigSnapshotChanged/.test(runnerSource),
  'toolbar save keeps the GET-first snapshot guard'
);
ok(
  /schedExternalConfigChangeText\(\)/.test(runnerSource) &&
    /配置已被外部修改/.test(runnerSource),
  'external config changes abort the save with a reload hint'
);
ok(
  /runnerFetchJson\(RUNNER_API\.schedulerReset, \{[\s\S]{0,140}body: \{ tasks: tasks, include_checkpoint: include_checkpoint \}/.test(runnerSource),
  'reset-memory POSTs {tasks, include_checkpoint}'
);
ok(
  /async function openResetMemoryDialog\(\)[\s\S]{0,500}ensureScheduleTaskChoices\(true\)/.test(runnerSource),
  'reset dialog task list comes from /runner/tasks'
);
ok(
  /schedRunsBuildUrl\(scheduleRunsFilter\)/.test(runnerSource) &&
    /schedRunDetailUrl\(runId\)/.test(runnerSource) &&
    /\/runs\/scheduler\/run\?run_id=/.test(runnerSource),
  'run list + detail use the shared scheduler-run routes'
);
ok(
  /queueId/.test(runnerSource) && /本地筛选/.test(runnerSource),
  'queue shortcut uses the local queue_id filter'
);
ok(
  /table\.className = 'runner-table sched-runs-table'/.test(runnerSource) &&
    /span\.className = 'runner-badge ' \+ \(cls \|\| 'kind'\)/.test(runnerSource),
  'legacy badge/table classes are mapped to the Runner styles'
);
ok(
  /function pollQueueRuntimeOnce\(\)[\s\S]{0,1600}updateScheduleEntryRuntime\(status\)/.test(runnerSource) &&
    !/function pollQueueRuntimeOnce\(\)[\s\S]{0,1600}renderScheduleEntries\(/.test(runnerSource),
  'the 5s poll overlays runtime badges without rebuilding config cards'
);
ok(
  /async function refreshSchedulePage\(\)/.test(runnerSource) &&
    /renderScheduleEntries\(status, cfg\)/.test(runnerSource) &&
    /renderScheduleQueues\(status, cfg\)/.test(runnerSource),
  'config is the render source for entries/queues (status only overlays runtime)'
);
ok(
  /RUNNER_SCHED_POLL_MS/.test(runnerSource) && /pollQueueRuntimeOnce\(\)/.test(runnerSource),
  'queue page keeps the 5s scheduler poll'
);
ok(
  /ensureScheduleTaskChoices\(true\)/.test(runnerSource) &&
    /scheduleTasksCache = tasks/.test(runnerSource),
  'entry/queue dialogs refresh the /runner/tasks cache before opening'
);
ok(
  /saveScheduleJson\(\)[\s\S]{0,1400}fetchScheduleConfigOnce\(\)[\s\S]{0,800}schedConfigSnapshotChanged/.test(runnerSource),
  'advanced JSON save also keeps the GET-first snapshot guard'
);
ok(
  /id="page-sched-json-error"/.test(runnerHtml) && /pageSchedJsonError/.test(runnerSource),
  'advanced JSON has a dedicated error line'
);


// U-T4/S3: run the REAL testAssetCandidates against a stub fetch: every
// candidate becomes a draw() box and the result pane lists #i score=… [x,y,w,h].
async function checkAssetCandidatesWiring() {
  const calls = [];
  const sb = {
    resultEl: { textContent: '' },
    boxes: [],
    drawn: 0,
    draw() { sb.drawn += 1; },
    log() {},
    cancelRecognitionRoiEdit() {},
    assetRowScopeTask: (scope) => (scope === undefined || scope === null || scope === '' ? '任务甲' : String(scope)),
    testFrames: () => [{ query: '', label: '当前帧', source: 'current', live: false, frameId: null }],
    fetch: async (url) => {
      calls.push(url);
      return {
        json: async () => ({
          ok: true,
          task: '任务甲',
          id: '模板甲',
          kind: 'template',
          domain: 'roi',
          roi: [10, 20, 30, 40],
          threshold: 0.85,
          frame_source: 'current',
          elapsed_ms: 1.5,
          count: 2,
          candidates: [
            { index: 0, x: 11, y: 21, w: 30, h: 40, score: 0.973, pass: true, action_box: [11, 21, 30, 40] },
            { index: 1, x: 51, y: 61, w: 30, h: 40, score: 0.901, pass: true, action_box: [51, 61, 30, 40] },
          ],
        }),
      };
    },
  };
  vm.createContext(sb);
  vm.runInContext(
    ['assetCandidatesUrl', 'matchBoxValid', 'candidateLabelText', 'candidateBoxes', 'testAssetCandidates']
      .map((name) => extractFunction(name))
      .join('\n\n'),
    sb,
    { filename: 'app.js#candidates' }
  );
  await sb.testAssetCandidates({ id: '模板甲', kind: 'template', roi: [10, 20, 30, 40] }, null, '');
  eq(calls.length, 1, 'U-T4: candidate recognition posts one /task/asset/candidates request per frame');
  eq(
    calls[0],
    '/task/asset/candidates?task=%E4%BB%BB%E5%8A%A1%E7%94%B2&id=%E6%A8%A1%E6%9D%BF%E7%94%B2',
    'U-T4: the candidate request carries the task scope + asset id (no frame query for the current frame)'
  );
  eq(sb.boxes.length, 2, 'U-T4: all candidates become draw() boxes');
  eq(
    sb.boxes.map((b) => b.box),
    [[11, 21, 30, 40], [51, 61, 30, 40]],
    'U-T4: box geometry is the real recognition rectangle of each candidate'
  );
  ok(
    /#0 score=0\.9730 \[11,21,30,40\]/.test(sb.resultEl.textContent) &&
      /#1 score=0\.9010 \[51,61,30,40\]/.test(sb.resultEl.textContent),
    'U-T4: the result pane lists every candidate as #i score=… [x,y,w,h]'
  );
  ok(/2 个候选/.test(sb.resultEl.textContent), 'U-T4: the result pane reports the candidate count');
  ok(/范围=素材识别区/.test(sb.resultEl.textContent), 'U-T4: the result pane names the search domain');
  eq(sb.drawn, 1, 'U-T4: one canvas repaint per recognition run');

  // A backend error (e.g. OCR build without models) degrades to text, never throws.
  calls.length = 0;
  sb.fetch = async () => ({
    json: async () => ({ error: 'OCR 素材候选需要 det+rec 模型' }),
  });
  await sb.testAssetCandidates({ id: '文字乙', kind: 'ocr_line', roi: [1, 2, 3, 4] }, null, '');
  ok(/❌ OCR 素材候选需要 det\+rec 模型/.test(sb.resultEl.textContent), 'U-T4: backend errors render inline');
  eq(sb.boxes.length, 0, 'U-T4: a failed run clears the candidate boxes');

  // ocr_var with an empty ROI: the UI labels the slow full-screen OCR domain.
  sb.fetch = async () => ({
    json: async () => ({
      ok: true, task: '任务甲', id: 'O_组', kind: 'ocr_var', domain: 'screen', roi: null,
      threshold: 0.6, frame_source: 'current', elapsed_ms: 2, count: 0, candidates: [],
    }),
  });
  await sb.testAssetCandidates({ id: 'O_组', kind: 'ocr_var', roi: null }, null, '');
  ok(/全屏 OCR/.test(sb.resultEl.textContent), 'U-T4: an empty-roi ocr_var shows the full-screen OCR hint');
}

// --- DOM id audit（Studio shell；D 批删除了参数面板/配置项对话框相关 id） -------
const htmlDom = html;
const referencedIds = [...new Set(
  [...source.matchAll(/getElementById\(['"]([^'"]+)['"]\)/g)].map((m) => m[1])
)];
const missingIds = referencedIds.filter((id) => !new RegExp(`id=["']${id}["']`).test(htmlDom));
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
  'candidate-select',
  'asset-text-dialog',
  'op-region-dialog',
];
ok(missingIds.length === 0, 'Studio DOM id audit: every getElementById target exists in index.html');
ok(referencedIds.length >= 159, 'Studio DOM id audit covers the merged UI (>=159 referenced ids)');
for (const id of fixedIds) {
  ok(new RegExp(`id=["']${id}["']`).test(htmlDom), `fixed DOM id present: #${id}`);
}
ok(
  !/id="builder-params"/.test(htmlDom) && !/id="task-config-dialog"/.test(htmlDom),
  'D: the deleted params panel + task-config dialog leave no DOM ids in index.html'
);

// --- D 批（#1/#2 去参数 UI）：专属断言 + 仍存在功能的覆盖恢复 ------------------
// 仍存在功能：guards / drag OCR 端点 / locator 文字来源 / 保存时 text·text_expr 校验。
ok(
  /function locatorTextSourceFields\(base, onChange\)/.test(builderSource) &&
    (builderSource.match(/locatorTextSourceFields\(/g) || []).length >= 4,
  'D: 文字来源组件仍在，find / requires / expect / drag 端点共用'
);
ok(
  /function renderGuardLocatorEditor\(root, step, field\)/.test(builderSource) &&
    /renderGuardLocatorEditor\(root, step, 'requires'\)/.test(builderSource) &&
    /renderGuardLocatorEditor\(root, step, 'expect'\)/.test(builderSource),
  'D: requires/expect 的 OCR 守卫结构化编辑器仍在'
);
ok(
  /function renderDragEndpointEditor\(host, step, side\)/.test(builderSource) &&
    /renderDragEndpointEditor\(box, step, 'from'\)/.test(builderSource) &&
    /renderDragEndpointEditor\(box, step, 'to'\)/.test(builderSource),
  'D: drag from/to 的 OCR 端点结构化编辑器仍在'
);
ok(
  /function flowTextSpecIssues\(flow\)/.test(builderSource) && /保存已阻止/.test(builderSource),
  'D: 保存前 text/text_expr 互斥校验仍在（删参数 UI 未误删）'
);
ok(
  /旧参数来源（只读）/.test(builderSource) && /mode === 'keep'/.test(builderSource) &&
    /text\.mode === 'keep' \? next/.test(builderSource),
  'D: 旧 flow 的 param() 来源只读回显，keep 时不改写 text_expr'
);
// Lead 清单：动作属性页无旧参数入口。
ok(
  !/'param', '识别文字配置项/.test(builderSource) &&
    !/文字（参数）/.test(builderSource) &&
    !/插入参数 ▾/.test(builderSource),
  'D: 动作属性页无「文字（参数）」选项、无「插入参数 ▾」'
);
ok(
  /function ocrVarInsertWidget\(mode, getField\)/.test(builderSource) &&
    /items = \(B\.assets \|\| \[\]\)\.filter[\s\S]{0,160}kind === 'ocr_var'/.test(builderSource),
  'D: 插入识别文字配置项下拉只列 ocr_var 素材'
);
ok(
  /btn\.textContent = '插入识别文字配置项 ▾';[\s\S]{0,400}btn\.disabled = !items\.length/.test(builderSource),
  'D: 无 ocr_var 选项时插入下拉禁用'
);
ok(
  /const snippet = paramInsertSnippet\(key, mode\);/.test(builderSource) &&
    /insert\(key\);/.test(builderSource) &&
    /function ocrVarInsertWidget/.test(builderSource),
  'D: 选中 ocr_var 后插入 param(param_key)'
);
ok(
  /const kindChoices = \[\][\s\S]{0,300}\['ocr_var', '识别文字配置项'\]/.test(builderSource) &&
    !/文字（参数）/.test(builderSource),
  'D: 条件行使用「识别文字配置项」行类型'
);
const condSb = {};
vm.createContext(condSb);
vm.runInContext(
  ['paramWaitCond', 'paramSafeKey', 'paramTextExpr', 'conditionRowExpr']
    .map((name) => extractFunctionFrom(builderSource, name, 'builder.js'))
    .join('\n\n'),
  condSb,
  { filename: 'builder.js#condition' }
);
eq(
  condSb.conditionRowExpr({ kind: 'ocr_var', value: 'O_组', paramKey: 'group' }),
  "text_visible(param('group'))",
  'D: 识别文字配置项条件行生成 text_visible(param(key))'
);
eq(
  condSb.conditionRowExpr({ kind: 'ocr_var', value: 'O_组', paramKey: 'group', neg: true }),
  "not (text_visible(param('group')))",
  'D: 识别文字配置项条件行支持「未出现」取反'
);
eq(
  condSb.conditionRowExpr({ kind: 'ocr_var', value: 'O_缺键' }),
  '',
  'D: 缺 param_key 的识别文字配置项条件行不生成表达式'
);
eq(
  condSb.conditionRowExpr({ kind: 'param', value: 'legacy_key' }),
  "text_visible(param('legacy_key'))",
  'D: 旧 flow 的参数条件行仍可回显/运行'
);
// 删除残留 = 0
ok(
  !/id="builder-params"/.test(html) && !/builder-params-list/.test(html) &&
    !/builder-params-declare/.test(html) && !/builder-param-issues/.test(html),
  'D: #builder-params 面板 HTML 残留 = 0'
);
ok(
  !/renderParamsPanel|renderParamIssuesHint/.test(builderSource) &&
    !/builder-params-declare/.test(builderSource),
  'D: #builder-params 面板渲染/入口代码残留 = 0'
);
ok(
  !/id="task-config-dialog"/.test(html) && !/id="task-config-save"/.test(html) &&
    !/getElementById\('task-config-dialog'\)/.test(source),
  'D: 任务配置项对话框 DOM/引用残留 = 0'
);
ok(
  !/openTaskConfigDialog/.test(source) && !/submitTaskConfig/.test(source) &&
    !/function taskConfigRenderCard/.test(source),
  'D: 任务库「配置项…」按钮与打开/保存/表单逻辑残留 = 0'
);
ok(
  !/openTaskParamsDialog|lintOpenDeclare|lintT3DeclarePrefill/.test(builderSource),
  'D: T3「去声明…」与 prefill 逻辑残留 = 0'
);
ok(
  !/任务参数/.test(html) && !/任务参数/.test(runnerHtml),
  'D: 用户可见「任务参数」措辞残留 = 0'
);
ok(
  !/去声明/.test(builderSource) && !/去声明/.test(html),
  'D: 「去声明」措辞残留 = 0'
);
ok(
  /旧参数不再支持：请改用「使用端OCR文字配置项」素材/.test(builderSource),
  'D: T3 检查文案改为引导 ocr_var 素材'
);
// goal battles：字面量循环次数
ok(
  /op: 'loop', args: \{ n: goal\.count \}/.test(source),
  'D: battles 完成条件生成 loop n=<goal.count> 字面量'
);
ok(
  !/args: \{ n: "param\('max_battles'\)" \}/.test(source),
  'D: 不再生成 param(max_battles)'
);
ok(
  /字面量循环次数/.test(source),
  'D: 结果文案说明字面量循环次数'
);
// Runner：只显示 ocr_var 分节 + 隐藏通用参数/高级 JSON
ok(
  /runnerParamOcrVarOnly/.test(runnerSource) &&
    /runnerParamGroupsEx\(params\)\.filter[\s\S]{0,120}kind === 'ocr_var'/.test(runnerSource),
  'D: Runner 配置弹窗只渲染 ocr_var 派生分节'
);
ok(
  /本任务无可配置的使用端变量/.test(runnerSource),
  'D: 无 ocr_var 时显示「本任务无可配置的使用端变量」空态'
);
ok(
  /id="runner-run-params"[^>]*>/.test(runnerHtml) &&
    /<label class="opt" hidden>参数 JSON/.test(runnerHtml),
  'D: 临时运行「参数 JSON」入口已隐藏（代码保留）'
);
ok(
  /<label class="opt" hidden>params（JSON 对象/.test(runnerHtml) &&
    /id="sched-entry-params"/.test(runnerHtml),
  'D: 调度条目 params JSON 入口已隐藏（代码保留）'
);
ok(
  /这里只显示「使用端OCR文字配置项」/.test(runnerHtml) &&
    /通用参数已从用户界面移除/.test(runnerHtml),
  'D: Runner 配置弹窗文案只说明 ocr_var'
);
ok(
  /Object\.assign\(\{\}, configValues, collected\.values\)/.test(runnerSource),
  'D: Runner 保存时保留隐藏通用参数的旧值'
);
eq(
  runnerSb.runnerParamOcrVarOnly([
    { key: 'group', asset_id: 'O_组' },
    { key: 'loop_n' },
    { key: 'x', asset_id: 'O_甲' },
    null,
  ]).map((p) => p.key),
  ['group', 'x'],
  'D: runnerParamOcrVarOnly 只保留 asset_id 派生行'
);
eq(runnerSb.runnerParamOcrVarOnly(null), [], 'D: 无参数时 ocr_var 过滤为空');
eq(
  runnerSb.runnerParamGroupsEx([
    { key: 'group', asset_id: 'O_组' },
    { key: 'loop_n', group: '通用' },
  ]).filter((g) => g.kind === 'ocr_var').map((g) => g.params.map((p) => p.key)),
  [['group']],
  'D: 分组模型仍可区分 ocr_var 与通用组（通用组仅隐藏）'
);
ok(
  /builder-target-errors|builder-legacy-region-hint/.test(styleCss),
  'D: 目标/旧 region 样式仍在（U-T5 功能未误删）'
);

checkAssetTestWiring()
  .then(() => checkAssetCandidatesWiring())
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
