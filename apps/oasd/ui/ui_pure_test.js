// Pure-function regression tests for the editor UI (no server, no bundler).
//
// Run from the repository root:
//   node apps/oasd/ui/ui_pure_test.js
//
// The test extracts the pure helpers from app.js itself (so it cannot drift
// from a copy) and evaluates them in a Node vm sandbox. It also performs a few
// static contract checks on index.html / app.js for the P0-1/P0-2 fixes.
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
  candidateActionPayload,
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
  candidateActionPayload({ kind: 'ocr_line', actionSource: 'recognition', actionShape: null }),
  { action_source: 'recognition' },
  'un-edited candidate saves recognition without shape'
);
eq(
  candidateActionPayload({
    kind: 'ocr_line',
    actionSource: 'fixed',
    actionShape: { kind: 'rect', roi: [1, 2, 3, 4], invert: false },
  }),
  {
    action_shape: { kind: 'rect', roi: [1, 2, 3, 4], invert: false },
    action_source: 'fixed',
  },
  'explicitly edited candidate saves fixed + shape'
);
eq(
  candidateActionPayload({ kind: 'region', actionSource: 'recognition', actionShape: null }),
  {},
  'region keeps its roi-as-operation-area semantics (no extra action fields)'
);
eq(
  candidateActionPayload({ kind: 'template', actionSource: 'fixed', actionShape: null }),
  { action_source: 'recognition' },
  'fixed without a shape must not be emitted'
);

// --- static UI contract checks (P0-1 / dead bindings) ---
ok(/id="btn-library-refresh"/.test(html), 'index.html provides a real #btn-library-refresh');
ok(/id="task-show-all-assets"/.test(html), 'index.html provides the show-all-global switch');
ok(/id="task-assets-hint"/.test(html), 'index.html provides the unreferenced hint');
ok(!/saveSelectedCandidates/.test(source), 'dead saveSelectedCandidates is gone');
ok(!/btn-batch-save/.test(source), 'dead #btn-batch-save binding is gone');
ok(!/action_roi/.test(source), 'no stale action_roi text remains in app.js');
const saveUnified = extractFunction('saveAssetUnified');
ok(
  (saveUnified.match(/candidateActionPayload\(c\)/g) || []).length === 2,
  'both save paths (batch OCR + single) use candidateActionPayload'
);
ok(
  !/action_shape:\s*c\.actionShape\s*\|\|/.test(saveUnified),
  'save path no longer falls back to a hard-coded rect action_shape'
);
ok(
  (source.match(/await refreshGlobalAssets\(\)/g) || []).length >= 4,
  'save/delete/position/action-ROI paths refresh the global library'
);
ok(/btn-library-refresh/.test(html), 'real global refresh button exists');

// DOM id audit: report the pre-existing dead ids honestly, but fail only if an
// id touched by this fix is missing from index.html.
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
  'lines-list',
];
for (const id of fixedIds) {
  ok(!missingIds.includes(id), `fixed DOM id #${id} exists in index.html`);
}
console.log(
  `dom_id_audit: ${referencedIds.length} referenced ids, ${missingIds.length} pre-existing dead ids` +
  (missingIds.length ? ` (${missingIds.join(', ')})` : '')
);

console.log(`ui_pure_test: ${checks} checks passed`);
