// v1.2 任务构建页：动作库 + 值属性 + 实时流程图。
// 纯原生 JS（无构建步骤）；flow.json 是唯一真相，本页只编辑它。
(function () {
  const OP_META = {
    if: { label: '如果', group: '流程控制', container: 'then/else' },
    loop: { label: '循环 N 次', group: '流程控制', container: 'body' },
    while: { label: '当条件成立循环', group: '流程控制', container: 'body' },
    break: { label: '跳出循环', group: '流程控制' },
    continue: { label: '继续循环', group: '流程控制' },
    call_task: { label: '调用已有任务', group: '流程控制' },
    wait: { label: '等待', group: '识别/等待' },
    wait_until: { label: '等到条件', group: '识别/等待' },
    appear: { label: '检查出现', group: '识别/等待' },
    ocr: { label: '识别文字', group: '识别/等待' },
    click: { label: '点击素材', group: '动作' },
    click_point: { label: '点击坐标', group: '动作' },
    click_target: { label: '定位点击', group: '动作' },
    dismiss_popup: { label: '关闭弹窗', group: '动作' },
    swipe: { label: '滑动', group: '动作' },
    drag: { label: '拖动（拟人）', group: '动作' },
    set_var: { label: '设置变量', group: '数据/结束' },
    inc_var: { label: '变量自增', group: '数据/结束' },
    ocr_to_var: { label: 'OCR 结果写入变量', group: '数据/结束' },
    log: { label: '日志', group: '数据/结束' },
    finish: { label: '结束', group: '数据/结束' },
  };
  const GROUP_ORDER = ['流程控制', '识别/等待', '动作', '数据/结束'];
  const FIELDS = {
    log: [{ key: 'level', label: '级别', type: 'select', options: ['info', 'warn', 'error'] }, { key: 'text', label: '文本（表达式）', type: 'text' }],
    wait: [{ key: 'seconds', label: '秒', type: 'number' }],
    wait_until: [{ key: 'cond', label: '条件表达式', type: 'text' }, { key: 'timeout_s', label: '超时秒', type: 'number' }],
    appear: [{ key: 'sel', label: '素材', type: 'asset' }],
    ocr: [{ key: 'sel', label: 'OCR 素材', type: 'asset' }],
    click: [{ key: 'sel', label: '素材', type: 'asset' }],
    click_point: [{ key: 'x', label: 'x（表达式）', type: 'text' }, { key: 'y', label: 'y（表达式）', type: 'text' }],
    dismiss_popup: [{ key: 'sel', label: '弹窗素材', type: 'asset' }, { key: 'action', label: '动作（可选）', type: 'text' }, { key: 'timeout_ms', label: '超时 ms', type: 'number' }],
    swipe: [{ key: 'from', label: '起点 [x,y]', type: 'json' }, { key: 'to', label: '终点 [x,y]', type: 'json' }, { key: 'ms', label: '时长 ms', type: 'number' }],
    finish: [{ key: 'exit', label: '出口', type: 'select', options: ['done', 'failed'] }],
    if: [],
    loop: [{ key: 'n', label: '次数（<=0 不循环；>0 循环次数）', type: 'text' }],
    while: [],
    click_target: [],
    drag: [],
    set_var: [{ key: 'name', label: '变量名', type: 'text' }, { key: 'value', label: '值（表达式）', type: 'text' }],
    inc_var: [{ key: 'name', label: '变量名', type: 'text' }, { key: 'by', label: '增量（数字/表达式）', type: 'text' }],
    ocr_to_var: [{ key: 'sel', label: 'OCR 素材', type: 'asset' }, { key: 'var', label: '变量名', type: 'text' }],
    call_task: [],
  };
  const DEFAULTS = {
    log: { level: 'info', text: "'hello'" },
    wait: { seconds: 1 },
    wait_until: { cond: "appear('I_A')", timeout_s: 10 },
    appear: { sel: 'I_A' },
    ocr: { sel: 'O_A' },
    click: { sel: 'I_A' },
    click_point: { x: 100, y: 100 },
    dismiss_popup: { sel: 'POPUP', timeout_ms: 0 },
    swipe: { from: [1180, 260], to: [1180, 660], ms: 400 },
    finish: { exit: 'done' },
    if: { cond: 'true' },
    loop: { n: '3' },
    while: { cond: 'true' },
    click_target: { find: [{ kind: 'template', sel: 'I_A' }] },
    drag: {
      from: { kind: 'region', roi: [100, 400, 300, 200] },
      direction: 'up',
      distance: [150, 450],
      duration_ms: [300, 650],
      margin: 0.15,
    },
    set_var: { name: 'n', value: '0' },
    inc_var: { name: 'n', by: '1' },
    ocr_to_var: { sel: 'O_A', var: 'text' },
    call_task: { task: '', mode: 'run' },
    break: {},
    continue: {},
  };

  const B = {
    task: '',
    flow: null,
    assets: [],
    selected: null, // path array
    tasks: [],
    autoSaveArmed: false,
    autoSaveTimer: null,
    lastSavedFlow: '',
    saving: false,
    checked: new Set(),
    selectionHistory: [],
    selectionIndex: -1,
    navNavigating: false,
  };
  const $ = (id) => document.getElementById(id);

  function status(text, color) {
    const el = $('builder-status');
    if (!el) return;
    el.textContent = text || '—';
    el.style.color = color || '';
  }

  function esc(s) {
    return String(s == null ? '' : s);
  }

  function getRoot() {
    if (!B.flow) B.flow = { schema_version: 1, name: B.task, steps: [] };
    if (!Array.isArray(B.flow.steps)) B.flow.steps = [];
    return B.flow.steps;
  }

  // path = [arrayIndex, childKey, ...] where childKey is then/else/body.
  function getStep(path) {
    if (!path || !path.length) return null;
    let arr = getRoot();
    let obj = null;
    for (let i = 0; i < path.length; i++) {
      const seg = path[i];
      if (typeof seg === 'number') {
        obj = arr[seg];
        if (!obj) return null;
      } else {
        arr = obj[seg];
        if (!Array.isArray(arr)) return null;
      }
    }
    return obj;
  }

  function getContainer(path) {
    // Returns the array that contains the step at path.
    if (!path || !path.length) return getRoot();
    let arr = getRoot();
    let obj = null;
    for (let i = 0; i < path.length - 1; i++) {
      const seg = path[i];
      if (typeof seg === 'number') obj = arr[seg];
      else {
        arr = obj[seg];
        if (!Array.isArray(arr)) return null;
      }
    }
    const last = path[path.length - 1];
    if (typeof last === 'number') return arr;
    return obj && Array.isArray(obj[last]) ? obj[last] : null;
  }

  function childArrays(step) {
    const out = [];
    if (!step) return out;
    if (Array.isArray(step.then)) out.push(['then', step.then]);
    if (Array.isArray(step.else)) out.push(['else', step.else]);
    if (Array.isArray(step.body)) out.push(['body', step.body]);
    return out;
  }

  function stepSummary(step) {
    if (!step) return '';
    const a = step.args || {};
    switch (step.op) {
      case 'log': return esc(a.text);
      case 'wait': return `${esc(a.seconds)}s`;
      case 'wait_until': return `${esc(a.cond)} / ${esc(a.timeout_s)}s`;
      case 'appear': return esc(a.sel);
      case 'ocr': return esc(a.sel);
      case 'click': return esc(a.sel);
      case 'click_point': return `(${esc(a.x)},${esc(a.y)})`;
      case 'click_target': return JSON.stringify(a.find || []).slice(0, 60);
      case 'dismiss_popup': return esc(a.sel);
      case 'swipe': return `${JSON.stringify(a.from)}→${JSON.stringify(a.to)}`;
      case 'drag': return `${a.direction || '?'} ${a.from ? JSON.stringify(a.from).slice(0, 30) : ''}`;
      case 'finish': return esc(a.exit);
      case 'set_var': return `${esc(a.name)} = ${esc(a.value)}`;
      case 'inc_var': return `${esc(a.name)} += ${esc(a.by)}`;
      case 'ocr_to_var': return `${esc(a.sel)} → ${esc(a.var)}`;
      case 'call_task': return `${a.mode === 'inline' ? '内联' : '调用'} ${esc(a.task) || '（未选择）'}`;
      case 'if': return `if ${esc(a.cond)}`;
      case 'loop': return `×${esc(a.n)}`;
      case 'while': return `while ${esc(a.cond)}`;
      case 'break': return '';
      case 'continue': return '';
      default: return '';
    }
  }

  function renderPalette() {
    const root = $('builder-ops');
    if (!root) return;
    root.textContent = '';
    for (const g of GROUP_ORDER) {
      const h = document.createElement('div');
      h.className = 'builder-op-group';
      h.textContent = g;
      root.appendChild(h);
      for (const op of Object.keys(OP_META)) {
        if (OP_META[op].group !== g) continue;
        const b = document.createElement('button');
        b.type = 'button';
        b.textContent = OP_META[op].label + '  (' + op + ')';
        b.addEventListener('click', () => addAction(op));
        root.appendChild(b);
      }
    }
  }

  function makeStep(op) {
    const step = { op, args: JSON.parse(JSON.stringify(DEFAULTS[op] || {})) };
    if (op === 'if') {
      step.then = [];
      step.else = [];
    } else if (op === 'loop' || op === 'while') {
      step.body = [];
    }
    return step;
  }

  function addAction(op) {
    const meta = OP_META[op];
    if (!meta) return;
    const step = makeStep(op);
    // Text in the palette promises this: selected control container receives
    // the new child (if => then, loop/while => body). The inspector still lets
    // the user choose else.
    const selected = getStep(B.selected);
    const containers = childArrays(selected);
    if (containers.length) {
      const [key, arr] = containers[0];
      arr.push(step);
      B.selected = B.selected.concat([key, arr.length - 1]);
      renderAll();
      status(`已添加子动作 ${meta.label} → ${key}`);
      return;
    }
    const arr = getRoot();
    // Keep `finish` last automatically: append before the trailing finish.
    let at = arr.length;
    if (at > 0 && arr[at - 1] && arr[at - 1].op === 'finish') at -= 1;
    arr.splice(at, 0, step);
    B.selected = [at];
    renderAll();
    status(`已添加 ${meta.label}`);
  }

  function removeCheckedIn(steps, basePath) {
    let removed = 0;
    for (let i = steps.length - 1; i >= 0; i--) {
      const step = steps[i];
      const path = basePath.concat([i]);
      if (B.checked.has(JSON.stringify(path))) {
        steps.splice(i, 1);
        removed += 1;
        continue;
      }
      for (const [key, arr] of childArrays(step)) {
        removed += removeCheckedIn(arr, path.concat([key]));
      }
    }
    return removed;
  }

  function deleteCheckedSteps() {
    if (!B.checked.size) {
      status('先勾选要删除的步骤', '#ff8a80');
      return;
    }
    const n = removeCheckedIn(getRoot(), []);
    B.checked.clear();
    B.selected = null;
    renderAll();
    status(`已删除 ${n} 个勾选步骤`);
  }

  function renderSteps() {
    const root = $('builder-steps');
    if (!root) return;
    root.textContent = '';
    const steps = getRoot();
    if (!steps.length) {
      root.textContent = '（空流程；从左侧动作库添加动作）';
      return;
    }
    renderStepArray(steps, [], root, 0);
    const drop = document.createElement('div');
    drop.className = 'builder-root-drop muted small';
    drop.textContent = '拖到这里 → 移到根级';
    drop.addEventListener('dragover', (e) => {
      e.preventDefault();
      drop.classList.add('drop-target');
    });
    drop.addEventListener('dragleave', () => drop.classList.remove('drop-target'));
    drop.addEventListener('drop', (e) => {
      e.preventDefault();
      drop.classList.remove('drop-target');
      try {
        const fromPath = JSON.parse(e.dataTransfer.getData('text/plain'));
        if (Array.isArray(fromPath)) moveStepToRoot(fromPath);
      } catch (err) { /* ignore malformed drag */ }
    });
    root.appendChild(drop);
  }

  function moveStepToRoot(fromPath) {
    if (!Array.isArray(fromPath) || fromPath.length <= 1) return;
    const fromArr = getContainer(fromPath);
    const idx = fromPath[fromPath.length - 1];
    if (!fromArr || typeof idx !== 'number') return;
    const step = fromArr[idx];
    if (!step) return;
    fromArr.splice(idx, 1);
    const root = getRoot();
    let at = root.length;
    if (at > 0 && root[at - 1] && root[at - 1].op === 'finish') at -= 1;
    root.splice(at, 0, step);
    B.selected = [at];
    renderAll();
    status('已移到根级');
  }

  function stepContains(root, needle) {
    if (!root || typeof root !== 'object') return false;
    if (root === needle) return true;
    for (const [_, arr] of childArrays(root)) {
      for (const child of arr) {
        if (stepContains(child, needle)) return true;
      }
    }
    return false;
  }

  function moveStepTo(fromPath, targetStepPath, key) {
    const fromArr = getContainer(fromPath);
    const idx = fromPath[fromPath.length - 1];
    const targetStep = getStep(targetStepPath);
    if (!fromArr || typeof idx !== 'number' || !targetStep || !Array.isArray(targetStep[key])) return;
    const step = fromArr[idx];
    if (!step) return;
    // Moving a container into itself/its descendant would detach the whole
    // subtree from the flow (the invalid move used to make both levels vanish).
    if (step === targetStep || stepContains(step, targetStep)) {
      status('不能把父级移动到自己的子级', '#ff8a80');
      return;
    }
    if (fromArr === targetStep[key] && idx === targetStep[key].length - 1) return;
    const snapshot = JSON.stringify(B.flow);
    fromArr.splice(idx, 1);
    const targetArr = targetStep[key];
    if (!Array.isArray(targetArr)) {
      B.flow = JSON.parse(snapshot);
      renderAll();
      status('移动失败：目标子级不存在', '#ff8a80');
      return;
    }
    targetArr.push(step);
    B.selected = targetStepPath.concat([key, targetArr.length - 1]);
    renderAll();
    status(`已移动步骤到 ${key}`);
  }

  function renderStepArray(steps, basePath, container, depth) {
    steps.forEach((step, idx) => {
      if (!step || typeof step !== 'object') return;
      const path = basePath.concat([idx]);
      const row = document.createElement('div');
      row.className = 'builder-step';
      row.draggable = true;
      if (B.selected && JSON.stringify(B.selected) === JSON.stringify(path)) row.classList.add('selected');
      row.addEventListener('dragstart', (e) => {
        e.dataTransfer.setData('text/plain', JSON.stringify(path));
        e.dataTransfer.effectAllowed = 'move';
        row.classList.add('dragging');
      });
      row.addEventListener('dragend', () => row.classList.remove('dragging'));
      const pick = document.createElement('input');
      pick.type = 'checkbox';
      pick.className = 'builder-step-check';
      pick.checked = B.checked.has(JSON.stringify(path));
      pick.title = '勾选后可整体删除';
      pick.addEventListener('click', (e) => e.stopPropagation());
      pick.addEventListener('change', () => {
        const key = JSON.stringify(path);
        if (pick.checked) B.checked.add(key);
        else B.checked.delete(key);
      });
      const op = document.createElement('span');
      op.className = 'op';
      op.textContent = (OP_META[step.op] && OP_META[step.op].label) || step.op;
      const sum = document.createElement('span');
      sum.className = 'summary';
      sum.textContent = ' ' + stepSummary(step);
      row.append(pick, op, sum);
      const btns = document.createElement('span');
      btns.className = 'rowbtns';
      const up = document.createElement('button');
      up.type = 'button';
      up.textContent = '↑';
      up.title = '上移';
      up.addEventListener('click', (e) => { e.stopPropagation(); if (idx > 0) { [steps[idx - 1], steps[idx]] = [steps[idx], steps[idx - 1]]; B.selected = path; renderAll(); } });
      const down = document.createElement('button');
      down.type = 'button';
      down.textContent = '↓';
      down.title = '下移';
      down.addEventListener('click', (e) => { e.stopPropagation(); if (idx < steps.length - 1) { [steps[idx + 1], steps[idx]] = [steps[idx], steps[idx + 1]]; B.selected = path; renderAll(); } });
      const del = document.createElement('button');
      del.type = 'button';
      del.textContent = '删';
      del.title = '删除动作';
      del.addEventListener('click', (e) => { e.stopPropagation(); steps.splice(idx, 1); B.selected = null; renderAll(); });
      btns.append(up, down, del);
      row.append(btns);
      row.addEventListener('click', () => { B.selected = path; renderAll(); });
      container.appendChild(row);
      for (const [key, arr] of childArrays(step)) {
        const child = document.createElement('div');
        child.className = 'builder-children';
        const label = document.createElement('div');
        label.className = 'muted small';
        label.textContent = key + ':';
        child.appendChild(label);
        child.addEventListener('dragover', (e) => {
          e.preventDefault();
          child.classList.add('drop-target');
        });
        child.addEventListener('dragleave', () => child.classList.remove('drop-target'));
        child.addEventListener('drop', (e) => {
          e.preventDefault();
          child.classList.remove('drop-target');
          try {
            const fromPath = JSON.parse(e.dataTransfer.getData('text/plain'));
            if (Array.isArray(fromPath)) moveStepTo(fromPath, path, key);
          } catch (err) { /* ignore malformed drag data */ }
        });
        if (!arr.length) {
          const empty = document.createElement('div');
          empty.className = 'muted small';
          empty.textContent = '（空）';
          child.appendChild(empty);
        } else {
          renderStepArray(arr, path.concat([key]), child, depth + 1);
        }
        container.appendChild(child);
      }
    });
  }

  // v1.2.3 (R9): real control-flow graph. Build nodes/edges first, then render
  // SVG with actual edge paths (no more `↓` / `↺` text as pseudo-edges).
  const G = {
    nodeW: 210,
    nodeH: 50,
    ifW: 190,
    ifH: 76,
    junction: 22,
    gapY: 30,
    branchDX: 170,
    loopDX: 100,
    pad: 30,
  };

  function sEl(tag, attrs) {
    const el = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (const [k, v] of Object.entries(attrs || {})) el.setAttribute(k, String(v));
    return el;
  }

  function gNode(graph, opts) {
    const kind = opts.kind || 'action';
    const w = kind === 'if' ? G.ifW : kind === 'junction' ? G.junction : kind === 'terminal' ? 124 : G.nodeW;
    const h = kind === 'if' ? G.ifH : kind === 'junction' ? G.junction : kind === 'terminal' ? 38 : G.nodeH;
    const n = {
      cx: opts.cx,
      y: opts.y,
      w,
      h,
      kind,
      label: opts.label || '',
      summary: opts.summary || '',
      path: opts.path || null,
    };
    graph.nodes.push(n);
    if (kind === 'finish') graph.finishNodes.push(n);
    return n;
  }

  function gEdge(graph, from, to, label, opts) {
    if (!from || !to || from === to) return;
    graph.edges.push({ from, to, label: label || '', back: Boolean(opts && opts.back), backType: opts && opts.backType });
  }

  function connectExits(graph, exits, to, label) {
    for (const e of exits || []) gEdge(graph, e, to, label);
  }

  // Layout one sequence. `ctx.loop` collects break/continue targets for the
  // enclosing loop; nested loops replace it.
  function layoutSteps(graph, steps, basePath, cx, y, ctx) {
    let entry = null;
    let prev = [];
    let bottom = y;
    steps.forEach((step, idx) => {
      const path = basePath.concat([idx]);
      let node;
      if (step.op === 'if') {
        node = gNode(graph, {
          cx,
          y,
          kind: 'if',
          label: '如果',
          summary: stepSummary(step),
          path,
        });
        if (!entry) entry = node;
        connectExits(graph, prev, node);
        const branchY = node.y + node.h + G.gapY;
        const thenR = layoutSteps(graph, step.then || [], path.concat(['then']), cx - G.branchDX, branchY, ctx);
        const elseR = layoutSteps(graph, step.else || [], path.concat(['else']), cx + G.branchDX, branchY, ctx);
        if (thenR.entry) gEdge(graph, node, thenR.entry, '是');
        if (elseR.entry) gEdge(graph, node, elseR.entry, '否');
        const mergeY = Math.max(thenR.bottom, elseR.bottom, node.y + node.h) + G.gapY;
        const merge = gNode(graph, { cx, y: mergeY, kind: 'junction', label: '' });
        connectExits(graph, thenR.exits, merge);
        connectExits(graph, elseR.exits, merge);
        if (!thenR.exits.length && !elseR.exits.length) gEdge(graph, node, merge, '是/否');
        prev = [merge];
        bottom = Math.max(bottom, merge.y + merge.h);
        y = bottom + G.gapY;
        return;
      }
      if (step.op === 'loop' || step.op === 'while') {
        node = gNode(graph, {
          cx,
          y,
          label: step.op === 'while' ? '当条件循环' : '循环',
          summary: stepSummary(step),
          path,
        });
        if (!entry) entry = node;
        connectExits(graph, prev, node);
        const loopCtx = {
          ...ctx,
          loop: { header: node, breakNodes: [], continueNodes: [] },
        };
        const bodyR = layoutSteps(
          graph,
          step.body || [],
          path.concat(['body']),
          cx + G.loopDX,
          node.y + node.h + G.gapY,
          loopCtx
        );
        if (bodyR.entry) gEdge(graph, node, bodyR.entry, '循环');
        for (const ex of bodyR.exits || []) gEdge(graph, ex, node, '↺ 回边', { back: true, backType: 'loop' });
        for (const c of loopCtx.loop.continueNodes) gEdge(graph, c, node, 'continue', { back: true, backType: 'continue' });
        const exitY = Math.max(bodyR.bottom, node.y + node.h) + G.gapY;
        const exitJ = gNode(graph, { cx, y: exitY, kind: 'junction', label: '' });
        gEdge(graph, node, exitJ, '退出');
        for (const b of loopCtx.loop.breakNodes) gEdge(graph, b, exitJ, 'break');
        prev = [exitJ];
        bottom = Math.max(bottom, exitJ.y + exitJ.h);
        y = bottom + G.gapY;
        return;
      }
      if (step.op === 'break' || step.op === 'continue') {
        node = gNode(graph, {
          cx,
          y,
          label: step.op,
          summary: stepSummary(step),
          path,
        });
        if (!entry) entry = node;
        connectExits(graph, prev, node);
        if (ctx.loop) {
          if (step.op === 'break') ctx.loop.breakNodes.push(node);
          else ctx.loop.continueNodes.push(node);
        }
        prev = [];
        bottom = Math.max(bottom, node.y + node.h);
        y = bottom + G.gapY;
        return;
      }
      if (step.op === 'finish') {
        node = gNode(graph, {
          cx,
          y,
          kind: 'finish',
          label: '结束',
          summary: stepSummary(step),
          path,
        });
        if (!entry) entry = node;
        connectExits(graph, prev, node);
        prev = [];
        bottom = Math.max(bottom, node.y + node.h);
        y = bottom + G.gapY;
        return;
      }
      node = gNode(graph, {
        cx,
        y,
        label: (OP_META[step.op] && OP_META[step.op].label) || step.op,
        summary: stepSummary(step),
        path,
      });
      if (!entry) entry = node;
      connectExits(graph, prev, node);
      prev = [node];
      bottom = Math.max(bottom, node.y + node.h);
      y = bottom + G.gapY;
    });
    return { entry, exits: prev, bottom };
  }

  function edgePath(a, b) {
    const x1 = a.cx;
    const y1 = a.y + a.h;
    const x2 = b.cx;
    const y2 = b.y;
    if (y2 >= y1) {
      const mid = (y1 + y2) / 2;
      return `M ${x1} ${y1} L ${x1} ${mid} L ${x2} ${mid} L ${x2} ${y2}`;
    }
    return `M ${x1} ${y1} C ${x1 + 70} ${y1 + 34}, ${x2 - 70} ${y2 - 28}, ${x2} ${y2}`;
  }

  // Back edges (loop return / continue) are routed in an outer lane, never
  // through the node column. This is the usual layered-graph approach: reverse
  // the back edge conceptually, then draw it around the subgraph perimeter.
  function backLaneX(graph, idx) {
    return graph.maxRight + G.pad + 24 + idx * 18;
  }

  function backEdgePath(e, graph, idx) {
    const a = e.from;
    const b = e.to;
    const laneX = backLaneX(graph, idx);
    const endX = b.cx + b.w / 2;
    const endY = b.y + b.h / 2;
    if (e.backType === 'continue') {
      // continue starts mid-body: leave through the right side, then join the
      // same outer lane and enter the loop header from its right edge.
      return `M ${a.cx + a.w / 2} ${a.y + a.h / 2} H ${laneX} V ${endY} H ${endX}`;
    }
    const bottomY = graph.maxBottom + G.pad + 12 + idx * 10;
    return `M ${a.cx} ${a.y + a.h} V ${bottomY} H ${laneX} V ${endY} H ${endX}`;
  }

  function edgeLabelPoint(e, graph, backIdx) {
    if (e.back) {
      const laneX = backLaneX(graph, backIdx);
      return {
        x: laneX + 4,
        y: (e.from.y + e.from.h / 2 + e.to.y + e.to.h / 2) / 2,
        anchor: 'start',
      };
    }
    return {
      x: (e.from.cx + e.to.cx) / 2,
      y: (e.from.y + e.from.h + e.to.y) / 2 - 2,
      anchor: 'middle',
    };
  }

  function renderSvgNode(svg, graph, node) {
    const g = sEl('g', { class: `flow-svg-node ${node.kind}` });
    if (node.kind === 'if') {
      const p = sEl('polygon', {
        points: `${node.cx},${node.y} ${node.cx + node.w / 2},${node.y + node.h / 2} ${node.cx},${node.y + node.h} ${node.cx - node.w / 2},${node.y + node.h / 2}`,
      });
      p.setAttribute('class', 'flow-svg-shape decision');
      g.appendChild(p);
    } else if (node.kind === 'junction') {
      const c = sEl('circle', { cx: node.cx, cy: node.y + node.h / 2, r: node.w / 2 });
      c.setAttribute('class', 'flow-svg-shape junction');
      g.appendChild(c);
    } else {
      const r = sEl('rect', {
        x: node.cx - node.w / 2,
        y: node.y,
        width: node.w,
        height: node.h,
        rx: node.kind === 'terminal' || node.kind === 'finish' ? node.h / 2 : 8,
      });
      r.setAttribute('class', `flow-svg-shape ${node.kind}`);
      g.appendChild(r);
    }
    if (node.kind !== 'junction') {
      const label = sEl('text', { x: node.cx, y: node.y + 19, 'text-anchor': 'middle' });
      label.setAttribute('class', 'flow-svg-label');
      label.textContent = node.label;
      g.appendChild(label);
      if (node.summary) {
        const sum = sEl('text', { x: node.cx, y: node.y + 35, 'text-anchor': 'middle' });
        sum.setAttribute('class', 'flow-svg-summary');
        sum.textContent = node.summary.length > 28 ? node.summary.slice(0, 27) + '…' : node.summary;
        g.appendChild(sum);
      }
      const title = sEl('title');
      title.textContent = `${node.label}${node.summary ? ' · ' + node.summary : ''}`;
      g.appendChild(title);
    }
    if (node.path) {
      g.classList.add('clickable');
      if (B.selected && JSON.stringify(B.selected) === JSON.stringify(node.path)) g.classList.add('selected');
      g.addEventListener('click', () => {
        B.selected = node.path;
        renderAll();
      });
    }
    svg.appendChild(g);
  }

  function renderGraph() {
    const root = $('builder-graph');
    if (!root) return;
    root.textContent = '';
    const steps = getRoot();
    if (!steps.length) {
      root.textContent = '（空流程；添加动作后显示控制流图）';
      return;
    }
    const graph = { nodes: [], edges: [], finishNodes: [] };
    const cx = 330;
    const start = gNode(graph, { cx, y: 20, kind: 'terminal', label: '开始' });
    const layout = layoutSteps(graph, steps, [], cx, start.y + start.h + G.gapY, { loop: null });
    if (layout.entry) gEdge(graph, start, layout.entry);
    const maxBottom = graph.nodes.reduce((m, n) => Math.max(m, n.y + n.h), layout.bottom);
    const end = gNode(graph, { cx, y: maxBottom + G.gapY, kind: 'terminal', label: '流程出口' });
    for (const f of graph.finishNodes) gEdge(graph, f, end);
    for (const e of layout.exits) gEdge(graph, e, end);
    if (!layout.exits.length && !graph.finishNodes.length) gEdge(graph, start, end);

    graph.maxRight = graph.nodes.reduce((m, n) => Math.max(m, n.cx + n.w / 2), 0);
    graph.maxBottom = graph.nodes.reduce((m, n) => Math.max(m, n.y + n.h), 0);
    const backEdges = graph.edges.filter((e) => e.back).length;
    const backExtra = Math.max(0, backEdges - 1);
    const width = Math.max(
      760,
      ...graph.nodes.map((n) => n.cx + n.w / 2 + G.pad),
      graph.maxRight + G.pad + 64 + backExtra * 18
    );
    const height = Math.max(
      end.y + end.h + G.pad,
      graph.maxBottom + G.pad + 24 + backExtra * 10
    );
    const svg = sEl('svg', {
      class: 'flow-svg',
      width,
      height,
      viewBox: `0 0 ${width} ${height}`,
      role: 'img',
      'aria-label': '控制流图',
    });
    const defs = sEl('defs');
    const marker = sEl('marker', {
      id: 'flow-arrow',
      markerWidth: 8,
      markerHeight: 8,
      refX: 7,
      refY: 3,
      orient: 'auto',
      markerUnits: 'strokeWidth',
    });
    const arrow = sEl('path', { d: 'M0,0 L0,6 L7,3 z' });
    arrow.setAttribute('class', 'flow-svg-arrow');
    marker.appendChild(arrow);
    defs.appendChild(marker);
    svg.appendChild(defs);

    let backIdx = 0;
    for (const e of graph.edges) {
      const isBack = Boolean(e.back);
      const d = isBack ? backEdgePath(e, graph, backIdx) : edgePath(e.from, e.to);
      const p = sEl('path', { d, 'marker-end': 'url(#flow-arrow)' });
      p.setAttribute('class', 'flow-svg-edge' + (isBack ? ' back-edge' : ''));
      svg.appendChild(p);
      if (e.label) {
        const lp = edgeLabelPoint(e, graph, backIdx);
        const t = sEl('text', {
          x: lp.x,
          y: lp.y,
          'text-anchor': lp.anchor,
        });
        t.setAttribute('class', 'flow-svg-edge-label' + (isBack ? ' back-edge-label' : ''));
        t.textContent = e.label;
        svg.appendChild(t);
      }
      if (isBack) backIdx += 1;
    }
    for (const n of graph.nodes) renderSvgNode(svg, graph, n);
    root.appendChild(svg);
  }

  function assetSearch(current, kinds) {
    const wrap = window.OASAssetUI.createSearchPicker({
      items: B.assets,
      value: current || '',
      placeholder: '输入片段，选择匹配素材…',
      className: 'builder-asset-search',
      menuClass: 'builder-asset-menu',
      filter: (a) => !kinds || kinds.includes(a.kind),
      label: (a) => `${a.id} [${a.kind}]`,
      onChoose: (a, input) => {
        input.value = a.id;
        input.dispatchEvent(new Event('change', { bubbles: true }));
      },
    });
    return wrap;
  }

  function renderAssetPanel() {
    const box = $('builder-assets');
    if (!box) return;
    box.textContent = '';
    const head = document.createElement('div');
    head.textContent = B.task
      ? `当前任务：${B.task} · 可用素材/操作区域 ${B.assets.length} 个`
      : '当前任务：未选择';
    box.appendChild(head);
    if (B.assets.length) {
      const list = document.createElement('div');
      list.className = 'muted small builder-asset-list';
      window.OASAssetUI.renderRows(list, B.assets, {
        task: B.task,
        compact: true,
        pick: false,
        manage: false,
        actions: false,
        group: true,
        groupBy: 'category',
        onPick: (a) => status(`素材 ${a.id}（${a.kind}）：可在动作参数里引用`),
      });
      box.appendChild(list);
    } else {
      const empty = document.createElement('div');
      empty.className = 'muted small';
      empty.textContent = '（当前任务没有素材/操作区域；先到工作台识别并保存）';
      box.appendChild(empty);
    }
  }

  // v1.2.2 (N47): structured editors for the two complex actions. Raw args JSON
  // stays below as the escape hatch for multi-locator chains / advanced fields.
  function locatorFromAsset(a) {
    if (!a) return null;
    if (a.kind === 'region') return { kind: 'region', sel: a.id };
    if (a.kind === 'template') return { kind: 'template', sel: a.id };
    // A collection is stored as a template locator pointing at the collection
    // id; oas-task expands it to the member templates before runtime.
    if (a.kind === 'template_set') return { kind: 'template', sel: a.id };
    return { kind: 'ocr_text', sel: a.id, text: a.ocr_text || a.id };
  }

  function formBox(title) {
    const box = document.createElement('div');
    box.className = 'builder-subform';
    const h = document.createElement('div');
    h.className = 'muted small';
    h.textContent = title;
    box.appendChild(h);
    return box;
  }

  function numberInput(label, value, onchange) {
    const lab = document.createElement('label');
    lab.textContent = label;
    const inp = document.createElement('input');
    inp.type = 'number';
    inp.value = value == null ? '' : value;
    inp.addEventListener('change', () => onchange(Number(inp.value)));
    lab.appendChild(inp);
    return lab;
  }

  function structuredClickTarget(root, step) {
    const box = formBox('定位对象（find[0]；多定位器/内联 ROI 用下方高级 JSON）');
    const find = Array.isArray(step.args.find) ? step.args.find : [];
    const loc = Object.assign({ kind: 'template' }, find[0] || {});
    const kindLab = document.createElement('label');
    kindLab.textContent = '定位方式';
    const kind = document.createElement('select');
    for (const k of ['template', 'ocr_text', 'region']) {
      const o = document.createElement('option');
      o.value = k;
      o.textContent = k;
      kind.appendChild(o);
    }
    kind.value = loc.kind || 'template';
    kindLab.appendChild(kind);
    box.appendChild(kindLab);

    const assetLab = document.createElement('label');
    assetLab.textContent = '素材/操作区域';
    const assetSearchWrap2 = assetSearch(loc.sel || '');
    const asset = assetSearchWrap2.input;
    assetLab.appendChild(assetSearchWrap2);
    box.appendChild(assetLab);

    const textLab = document.createElement('label');
    textLab.textContent = 'OCR 文字（ocr_text 必填）';
    const text = document.createElement('input');
    text.value = loc.text || '';
    textLab.appendChild(text);
    box.appendChild(textLab);

    function sync() {
      const next = { kind: kind.value };
      if (asset.value) next.sel = asset.value;
      if (kind.value === 'ocr_text') next.text = text.value;
      step.args.find = [next].concat(find.slice(1));
      renderAll();
    }
    kind.addEventListener('change', sync);
    asset.addEventListener('change', sync);
    text.addEventListener('change', sync);
    root.appendChild(box);
  }

  function structuredDrag(root, step) {
    const box = formBox('拖动（from 区域 + 方向/距离/时长；其它 from/to 形态用下方高级 JSON）');
    const dirLab = document.createElement('label');
    dirLab.textContent = '方向（必填）';
    const dir = document.createElement('select');
    for (const d of ['up', 'down', 'left', 'right']) {
      const o = document.createElement('option');
      o.value = d;
      o.textContent = d;
      dir.appendChild(o);
    }
    dir.value = step.args.direction || 'up';
    dirLab.appendChild(dir);
    box.appendChild(dirLab);

    const from = step.args.from || {};
    if (from.kind === 'region' && Array.isArray(from.roi) && from.roi.length === 4) {
      const roi = from.roi.slice();
      const row = document.createElement('div');
      row.className = 'builder-inline';
      ['x', 'y', 'w', 'h'].forEach((k, i) =>
        row.appendChild(
          numberInput(k, roi[i], (v) => {
            roi[i] = v;
            step.args.from = { kind: 'region', roi };
            renderAll();
          })
        )
      );
      box.appendChild(row);
    } else {
      const n = document.createElement('div');
      n.className = 'muted small';
      n.textContent = 'from 不是 region（OCR 行/坐标）：请用高级 JSON 编辑';
      box.appendChild(n);
    }

    const d = Array.isArray(step.args.distance) && step.args.distance.length === 2
      ? step.args.distance
      : [150, 450];
    const dur = Array.isArray(step.args.duration_ms) && step.args.duration_ms.length === 2
      ? step.args.duration_ms
      : [300, 650];
    const dRow = document.createElement('div');
    dRow.className = 'builder-inline';
    dRow.appendChild(numberInput('最小距离', d[0], (v) => {
      step.args.distance = [v, d[1]];
      renderAll();
    }));
    dRow.appendChild(numberInput('最大距离', d[1], (v) => {
      step.args.distance = [d[0], v];
      renderAll();
    }));
    box.appendChild(dRow);
    const durRow = document.createElement('div');
    durRow.className = 'builder-inline';
    durRow.appendChild(numberInput('最小时长', dur[0], (v) => {
      step.args.duration_ms = [v, dur[1]];
      renderAll();
    }));
    durRow.appendChild(numberInput('最大时长', dur[1], (v) => {
      step.args.duration_ms = [dur[0], v];
      renderAll();
    }));
    box.appendChild(durRow);
    box.appendChild(numberInput('margin', step.args.margin == null ? 0.15 : step.args.margin, (v) => {
      step.args.margin = v;
      renderAll();
    }));
    dir.addEventListener('change', () => {
      step.args.direction = dir.value;
      renderAll();
    });
    root.appendChild(box);
  }

  function stripFinishSteps(steps) {
    return (steps || [])
      .filter((s) => s && s.op !== 'finish')
      .map((s) => {
        const out = { ...s };
        for (const k of ['then', 'else', 'body']) {
          if (Array.isArray(s[k])) out[k] = stripFinishSteps(s[k]);
        }
        return out;
      });
  }

  async function inlineCallTask(step) {
    const task = (step.args.task || '').trim();
    if (!task) {
      status('请先选择要调用的任务', '#ff8a80');
      return;
    }
    if (task === B.task) {
      status('不能调用当前任务自身（会形成递归）', '#ff8a80');
      return;
    }
    try {
      const r = await fetch('/task/flow?task=' + encodeURIComponent(task));
      const s = await r.json();
      if (!s.ok || !s.flow) throw new Error((s.errors || []).join('; ') || 'flow 读取失败');
      const steps = stripFinishSteps(s.flow.steps || []);
      if (!steps.length) {
        status(`任务「${task}」没有可内联的步骤`, '#ff8a80');
        return;
      }
      const path = (B.selected || []).slice();
      const arr = getContainer(path);
      if (!arr) {
        status('找不到当前步骤所在容器', '#ff8a80');
        return;
      }
      const idx = path[path.length - 1];
      if (typeof idx !== 'number') {
        status('当前选中不是可替换的步骤', '#ff8a80');
        return;
      }
      arr.splice(idx, 1, ...steps);
      B.selected = path.slice(0, -1).concat([idx]);
      renderAll();
      status(`已内联任务「${task}」的 ${steps.length} 个步骤（A）`);
    } catch (e) {
      status('内联失败：' + e, '#ff8a80');
    }
  }

  function renderCallTaskForm(root, step) {
    const box = document.createElement('div');
    box.className = 'builder-subform';
    const taskLabel = document.createElement('label');
    taskLabel.textContent = '被调用任务';
    const taskSel = document.createElement('select');
    taskSel.appendChild(new Option('（选择任务）', ''));
    for (const t of B.tasks) {
      if (t.name === B.task) continue;
      taskSel.appendChild(new Option(`${t.display_name || t.name} (${t.name})`, t.name));
    }
    taskSel.value = step.args.task || '';
    taskSel.addEventListener('change', () => {
      step.args.task = taskSel.value;
      renderAll();
    });
    taskLabel.appendChild(taskSel);
    box.appendChild(taskLabel);

    const modeLabel = document.createElement('label');
    modeLabel.textContent = '方式';
    const mode = document.createElement('select');
    mode.appendChild(new Option('B 运行时引用（目标改动下次运行生效）', 'run'));
    mode.appendChild(new Option('A 立即内联（复制当前步骤）', 'inline'));
    mode.value = step.args.mode === 'inline' ? 'inline' : 'run';
    mode.addEventListener('change', () => {
      step.args.mode = mode.value;
      renderAll();
    });
    modeLabel.appendChild(mode);
    box.appendChild(modeLabel);

    if (mode.value === 'inline') {
      const b = document.createElement('button');
      b.type = 'button';
      b.textContent = '立即内联到当前位置（A）';
      b.addEventListener('click', () => inlineCallTask(step));
      box.appendChild(b);
    }
    const hint = document.createElement('div');
    hint.className = 'muted small';
    hint.textContent = mode.value === 'inline'
      ? 'A：把目标任务的步骤复制进来，之后目标改动不会同步。'
      : 'B：只保存 call_task 引用；运行时由 oas-task 加载目标 flow（目标 finish 视为返回）。';
    box.appendChild(hint);
    root.appendChild(box);
  }

  function normalizeStepArgs(step) {
    if (!step || typeof step !== 'object') return;
    step.args = step.args || {};
    if (step.op === 'if') {
      delete step.args.sel;
      delete step.args.n;
      delete step.args.max_iters;
      if (step.args.cond == null || step.args.cond === '') step.args.cond = 'true';
    } else if (step.op === 'while') {
      delete step.args.sel;
      delete step.args.n;
      delete step.args.max_iters;
      if (step.args.cond == null || step.args.cond === '') step.args.cond = 'true';
    } else if (step.op === 'loop') {
      delete step.args.sel;
      delete step.args.cond;
      delete step.args.max_iters;
      if (step.args.n == null || step.args.n === '') step.args.n = '1';
    }
    for (const key of ['then', 'else', 'body']) {
      if (Array.isArray(step[key])) for (const child of step[key]) normalizeStepArgs(child);
    }
  }

  function normalizeFlow(flow) {
    if (!flow || !Array.isArray(flow.steps)) return;
    for (const step of flow.steps) normalizeStepArgs(step);
  }

  function parseConditionPreset(cond) {
    const c = String(cond || '').trim();
    let m = c.match(/^appear\(\s*['"]([^'"]+)['"]\s*\)$/);
    if (m) return { custom: false, rows: [{ kind: 'asset', neg: false, value: m[1] }], join: 'and', groupNot: false };
    m = c.match(/^(?:not\s+|!)(?:appear\(\s*['"]([^'"]+)['"]\s*\))$/);
    if (m) return { custom: false, rows: [{ kind: 'asset', neg: true, value: m[1] }], join: 'and', groupNot: false };
    m = c.match(/^text_visible\(\s*['"]([^'"]+)['"]\s*\)$/);
    if (m) return { custom: false, rows: [{ kind: 'text', neg: false, value: m[1] }], join: 'or', groupNot: false };
    m = c.match(/^(?:not\s+|!)\s*\(?\s*text_visible\(\s*['"]([^'"]+)['"]\s*\)\s*\)?$/);
    if (m) return { custom: false, rows: [{ kind: 'text', neg: true, value: m[1] }], join: 'or', groupNot: false };
    if (!c) return { custom: false, rows: [], join: 'and', groupNot: false };
    return { custom: true, rows: [], join: 'and', groupNot: false, raw: c };
  }

  function conditionRowExpr(row) {
    const v = String(row.value || '').trim();
    if (!v) return '';
    let expr;
    if (row.kind === 'asset') {
      expr = `appear('${v.replace(/'/g, '')}')`;
    } else {
      // A text row still uses the same underlying logical expression; multiple
      // texts inside one row stay OR-compatible with the previous preset.
      const values = v.split(/[,，、]|\bor\b|\|/i).map((x) => x.trim()).filter(Boolean);
      expr = values.map((x) => `text_visible('${x.replace(/'/g, '')}')`).join(' or ') || "text_visible('')";
    }
    return row.neg ? `not (${expr})` : expr;
  }

  function renderConditionEditor(root, step) {
    const box = document.createElement('div');
    box.className = 'builder-subform';
    const title = document.createElement('div');
    title.className = 'muted small';
    title.textContent = '循环/分支条件（谓词行 + and/or/not、&&/||/!、括号）';
    box.appendChild(title);

    const preset = parseConditionPreset(step.args.cond);
    const rows = (preset.rows || []).slice();
    let join = preset.join || 'and';

    const customCheckLabel = document.createElement('label');
    customCheckLabel.className = 'run-opt';
    const customCheck = document.createElement('input');
    customCheck.type = 'checkbox';
    customCheck.checked = !!preset.custom;
    customCheckLabel.append(customCheck, document.createTextNode(' 自定义表达式'));
    box.appendChild(customCheckLabel);

    const rowsBox = document.createElement('div');
    box.appendChild(rowsBox);

    const joinLabel = document.createElement('label');
    joinLabel.textContent = '行之间';
    const joinSel = document.createElement('select');
    for (const [v, label] of [['and', '且（and）'], ['or', '或（or）']]) {
      const o = document.createElement('option');
      o.value = v;
      o.textContent = label;
      joinSel.appendChild(o);
    }
    joinSel.value = join;
    joinLabel.appendChild(joinSel);
    box.appendChild(joinLabel);

    const groupLabel = document.createElement('label');
    groupLabel.className = 'run-opt';
    const groupNot = document.createElement('input');
    groupNot.type = 'checkbox';
    groupNot.checked = !!preset.groupNot;
    groupLabel.append(groupNot, document.createTextNode(' 非（整组取反）'));
    box.appendChild(groupLabel);

    const addBtn = document.createElement('button');
    addBtn.type = 'button';
    addBtn.textContent = '+ 添加条件行';
    box.appendChild(addBtn);

    const customLabel = document.createElement('label');
    customLabel.textContent = '自定义表达式（0=不循环，非 0=循环；可写 appear(...)/text_visible(...)）';
    const custom = document.createElement('input');
    custom.type = 'text';
    custom.value = preset.raw || step.args.cond || 'true';
    customLabel.appendChild(custom);
    box.appendChild(customLabel);

    const preview = document.createElement('div');
    preview.className = 'muted small';
    box.appendChild(preview);

    function renderRows() {
      rowsBox.textContent = '';
      if (!rows.length) {
        const empty = document.createElement('div');
        empty.className = 'muted small';
        empty.textContent = '（没有条件行；添加一行，或勾选自定义表达式）';
        rowsBox.appendChild(empty);
        return;
      }
      rows.forEach((row, i) => {
        const line = document.createElement('div');
        line.className = 'builder-condition-row';
        const kind = document.createElement('select');
        for (const [v, label] of [['asset', '素材'], ['text', '文字（全屏）']]) {
          const o = document.createElement('option');
          o.value = v;
          o.textContent = label;
          kind.appendChild(o);
        }
        kind.value = row.kind;
        kind.addEventListener('change', () => {
          row.kind = kind.value;
          row.value = '';
          renderRows();
          sync();
        });
        const negLabel = document.createElement('label');
        negLabel.className = 'run-opt';
        const neg = document.createElement('input');
        neg.type = 'checkbox';
        neg.checked = !!row.neg;
        neg.addEventListener('change', () => { row.neg = neg.checked; sync(); });
        negLabel.append(neg, document.createTextNode('未出现'));
        let valueEl;
        if (row.kind === 'asset') {
          const search = assetSearch(row.value || '', null);
          valueEl = search.input;
          valueEl.addEventListener('change', () => { row.value = valueEl.value.trim(); sync(); });
          line.append(kind, negLabel, search);
        } else {
          valueEl = document.createElement('input');
          valueEl.type = 'text';
          valueEl.placeholder = '文字，多个用 / 或逗号';
          valueEl.value = row.value || '';
          valueEl.addEventListener('change', () => { row.value = valueEl.value.trim(); sync(); });
          line.append(kind, negLabel, valueEl);
        }
        const del = document.createElement('button');
        del.type = 'button';
        del.textContent = '删';
        del.title = '删除该条件行';
        del.addEventListener('click', () => {
          rows.splice(i, 1);
          renderRows();
          sync();
        });
        line.appendChild(del);
        rowsBox.appendChild(line);
      });
    }

    function sync() {
      const customMode = customCheck.checked;
      rowsBox.style.display = customMode ? 'none' : 'block';
      joinLabel.style.display = customMode || rows.length < 2 ? 'none' : 'block';
      groupLabel.style.display = customMode ? 'none' : 'block';
      addBtn.style.display = customMode ? 'none' : 'inline-block';
      customLabel.style.display = customMode ? 'block' : 'none';
      step.args = step.args || {};
      delete step.args.sel;
      if (customMode) {
        step.args.cond = custom.value.trim() || 'true';
      } else {
        const exprs = rows.map(conditionRowExpr).filter(Boolean);
        let expr = exprs.length ? exprs.join(join === 'or' ? ' or ' : ' and ') : 'true';
        if (groupNot.checked && exprs.length) expr = `not (${expr})`;
        step.args.cond = expr;
      }
      preview.textContent = `当前条件：${step.args.cond}（每轮重新计算）` +
        (!customMode && groupNot.checked && rows.some((r) => r.neg)
          ? ' ⚠ 行“未出现”+ 整组“非”是双重取反，请确认'
          : '');
      joinLabel.style.display = customMode || rows.length < 2 ? 'none' : 'block';
      scheduleAutoSave();
    }

    customCheck.addEventListener('change', () => {
      if (customCheck.checked) custom.value = step.args.cond || 'true';
      sync();
    });
    joinSel.addEventListener('change', () => { join = joinSel.value; sync(); });
    groupNot.addEventListener('change', sync);
    addBtn.addEventListener('click', () => {
      rows.push({ kind: 'asset', neg: false, value: '' });
      joinSel.value = join;
      renderRows();
      sync();
    });
    custom.addEventListener('change', sync);

    renderRows();
    sync();
    root.appendChild(box);
  }

  function renderClickAlts(root, step) {
    const label = document.createElement('label');
    label.textContent = '备选素材（任一命中即可；可选多个）';
    const sel = document.createElement('select');
    sel.multiple = true;
    sel.size = Math.min(6, Math.max(2, B.assets.filter((a) => a.kind === 'template').length));
    const current = new Set(step.args.alts || []);
    for (const a of B.assets) {
      if (a.kind !== 'template') continue;
      const o = document.createElement('option');
      o.value = a.id;
      o.textContent = a.id;
      o.selected = current.has(a.id);
      sel.appendChild(o);
    }
    sel.addEventListener('change', () => {
      const picked = Array.from(sel.selectedOptions).map((o) => o.value);
      if (picked.length) step.args.alts = picked;
      else delete step.args.alts;
      scheduleAutoSave();
    });
    label.appendChild(sel);
    root.appendChild(label);
  }

  function renderForm() {
    const root = $('builder-form');
    if (!root) return;
    root.textContent = '';
    const step = getStep(B.selected);
    if (!step) {
      root.textContent = '（选中中间的步骤后编辑动作值；右侧下方始终显示可用素材/操作区域）';
      return;
    }
    const title = document.createElement('div');
    title.innerHTML = `<strong>${esc((OP_META[step.op] && OP_META[step.op].label) || step.op)}</strong>`;
    root.appendChild(title);
    step.args = step.args || {};
    // Only actions that actually consume `sel` get the generic top asset picker.
    // For if/while this picker used to write a stray `args.sel`; the condition
    // editor below owns the asset choice instead.
    const SEL_OPS = new Set(['appear', 'ocr', 'click', 'dismiss_popup', 'ocr_to_var']);
    if (SEL_OPS.has(step.op)) {
      const assetLabel = document.createElement('label');
      assetLabel.textContent = `任务素材/操作区域（${B.assets.length}）`;
      const assetSearchWrap = assetSearch(step.args.sel || '');
      const assetPick = assetSearchWrap.input;
      assetPick.addEventListener('change', () => {
        if (assetPick.value) {
          step.args.sel = assetPick.value;
          status(`已选择素材：${assetPick.value}`);
          renderAll();
        }
      });
      assetLabel.appendChild(assetSearchWrap);
      root.appendChild(assetLabel);
    }
    if (step.op === 'call_task') renderCallTaskForm(root, step);
    const exprHintOps = new Set(['log', 'wait_until', 'loop', 'set_var', 'inc_var', 'click_point']);
    if (exprHintOps.has(step.op)) {
      const hint = document.createElement('div');
      hint.className = 'muted small';
      hint.textContent = '表达式值支持 and/or/not、&&/||/!、括号、appear/ocr/param/var；数值字段按数字求值。';
      root.appendChild(hint);
    }
    if (step.op === 'if' || step.op === 'while') {
      renderConditionEditor(root, step);
    }
    for (const f of FIELDS[step.op] || []) {
      const lab = document.createElement('label');
      lab.textContent = f.label;
      let input;
      if (f.type === 'select') {
        input = document.createElement('select');
        for (const o of f.options || []) {
          const op = document.createElement('option');
          op.value = o;
          op.textContent = o;
          input.appendChild(op);
        }
        input.value = step.args[f.key] == null ? (f.options ? f.options[0] : '') : step.args[f.key];
      } else if (f.type === 'asset') {
        const search = assetSearch(step.args[f.key], null);
        const input = search.input;
        input.addEventListener('change', () => { step.args[f.key] = input.value; renderAll(); });
        lab.appendChild(search);
        root.appendChild(lab);
        continue;
      } else if (f.type === 'json') {
        input = document.createElement('textarea');
        input.value = JSON.stringify(step.args[f.key] == null ? null : step.args[f.key], null, 0);
        input.addEventListener('change', () => {
          try { step.args[f.key] = JSON.parse(input.value); renderAll(); }
          catch (e) { status('JSON 无效：' + e.message, '#ff8a80'); }
        });
        lab.appendChild(input);
        root.appendChild(lab);
        continue;
      } else {
        input = document.createElement('input');
        input.type = f.type === 'number' ? 'number' : 'text';
        input.value = step.args[f.key] == null ? '' : step.args[f.key];
        input.addEventListener('change', () => {
          if (f.type === 'number') step.args[f.key] = Number(input.value);
          else step.args[f.key] = input.value;
          renderAll();
        });
      }
      lab.appendChild(input);
      root.appendChild(lab);
    }
    if (step.op === 'click_target') structuredClickTarget(root, step);
    if (step.op === 'click') renderClickAlts(root, step);
    if (step.op === 'drag') structuredDrag(root, step);
    // raw args JSON fallback for complex actions (click_target/drag)
    const raw = document.createElement('label');
    raw.textContent = '高级：args JSON';
    const ta = document.createElement('textarea');
    ta.value = JSON.stringify(step.args, null, 2);
    ta.addEventListener('change', () => {
      try { step.args = JSON.parse(ta.value); status('args 已更新'); renderAll(); }
      catch (e) { status('args JSON 无效：' + e.message, '#ff8a80'); }
    });
    raw.appendChild(ta);
    root.appendChild(raw);

    const actions = document.createElement('div');
    actions.className = 'builder-form-actions';
    for (const [key] of childArrays(step)) {
      const row = document.createElement('div');
      row.className = 'builder-child-add';
      const branchLabel = key === 'then' ? '是' : key === 'else' ? '否' : key === 'body' ? '循环体' : key;
      const sel = document.createElement('select');
      for (const op of Object.keys(OP_META)) {
        const o = document.createElement('option');
        o.value = op;
        o.textContent = OP_META[op].label + ' (' + op + ')';
        sel.appendChild(o);
      }
      sel.value = 'log';
      const b = document.createElement('button');
      b.type = 'button';
      b.textContent = branchLabel;
      b.title = '添加子动作到 ' + key;
      b.addEventListener('click', () => {
        const arr = step[key];
        const child = makeStep(sel.value);
        arr.push(child);
        B.selected = B.selected.concat([key, arr.length - 1]);
        renderAll();
        status(`已添加 ${OP_META[sel.value] ? OP_META[sel.value].label : sel.value} → ${key}`);
      });
      row.appendChild(b);
      row.appendChild(sel);
      actions.appendChild(row);
    }
    root.appendChild(actions);
  }

  function renderCode() {
    const el = $('builder-code');
    if (!el || document.activeElement === el) return;
    el.value = B.flow ? JSON.stringify(B.flow, null, 2) : '';
  }

  function applyCode() {
    const el = $('builder-code');
    const st = $('builder-code-status');
    if (!el) return;
    try {
      const v = JSON.parse(el.value);
      if (!v || typeof v !== 'object' || !Array.isArray(v.steps)) {
        throw new Error('flow 必须包含 steps 数组');
      }
      B.flow = v;
      normalizeFlow(B.flow);
      B.selected = null;
      renderAll();
      if (st) st.textContent = '代码已应用到流程图';
      status('代码已应用，自动保存将写入 flow.json');
    } catch (e) {
      if (st) st.textContent = '代码无效：' + e.message;
    }
  }

  function recordSelection() {
    if (B.navNavigating) return;
    const key = B.selected ? JSON.stringify(B.selected) : '';
    if (B.selectionHistory[B.selectionIndex] === key) return;
    B.selectionHistory = B.selectionHistory.slice(0, B.selectionIndex + 1);
    B.selectionHistory.push(key);
    B.selectionIndex = B.selectionHistory.length - 1;
  }

  function updateHistoryButtons() {
    const back = $('builder-back');
    const fwd = $('builder-forward');
    if (back) back.disabled = B.selectionIndex <= 0;
    if (fwd) fwd.disabled = B.selectionIndex < 0 || B.selectionIndex >= B.selectionHistory.length - 1;
  }

  function navigateHistory(delta) {
    const next = B.selectionIndex + delta;
    if (next < 0 || next >= B.selectionHistory.length) return;
    B.selectionIndex = next;
    B.navNavigating = true;
    const key = B.selectionHistory[next];
    B.selected = key ? JSON.parse(key) : null;
    renderAll();
    B.navNavigating = false;
  }

  function renderAll() {
    recordSelection();
    renderSteps();
    renderGraph();
    renderForm();
    renderAssetPanel();
    renderCode();
    updateHistoryButtons();
    scheduleAutoSave();
  }

  async function refreshTaskList() {
    const sel = $('builder-task');
    if (!sel) return;
    try {
      const r = await fetch('/task/library');
      const s = await r.json();
      B.tasks = s.tasks || [];
      const keep = B.task || sel.value;
      sel.innerHTML = '<option value="">（选择任务）</option>';
      for (const t of B.tasks) {
        const o = document.createElement('option');
        o.value = t.name;
        o.textContent = `${t.display_name || t.name} (${t.name})`;
        sel.appendChild(o);
      }
      if (keep) sel.value = keep;
    } catch (e) {
      status('任务列表读取失败：' + e, '#ff8a80');
    }
  }

  async function loadTask(name) {
    B.autoSaveArmed = false;
    B.selectionHistory = [];
    B.selectionIndex = -1;
    if (!name) {
      B.task = '';
      B.flow = null;
      B.assets = [];
      B.selected = null;
      renderAll();
      return;
    }
    B.task = name;
    status('载入中…');
    try {
      const [fr, ar] = await Promise.all([
        fetch('/task/flow?task=' + encodeURIComponent(name)),
        fetch('/task/assets?task=' + encodeURIComponent(name)),
      ]);
      const fs = await fr.json();
      const as = await ar.json();
      if (!fs.ok) throw new Error((fs.errors || []).join('; ') || 'flow 读取失败');
      if (!as.ok) throw new Error((as.errors || []).join('; ') || '素材读取失败');
      B.flow = fs.flow || { schema_version: 1, name, steps: [] };
      if (!Array.isArray(B.flow.steps)) B.flow.steps = [];
      normalizeFlow(B.flow);
      B.assets = as.items || [];
      for (const c of as.collections || []) {
        B.assets.push({ id: c.id, kind: 'template_set', members: c.members || [], action_roi: c.action_roi });
      }
      B.selected = null;
      B.lastSavedFlow = JSON.stringify(B.flow);
      renderAll();
      B.autoSaveArmed = true;
      status(`已载入 ${name}：${B.flow.steps.length} 个根动作 / ${B.assets.length} 个素材`);
    } catch (e) {
      status('载入失败：' + e, '#ff8a80');
    }
  }

  function scheduleAutoSave() {
    if (!B.autoSaveArmed || !B.task || !B.flow) return;
    const json = JSON.stringify(B.flow);
    if (json === B.lastSavedFlow) return;
    if (B.autoSaveTimer) clearTimeout(B.autoSaveTimer);
    B.autoSaveTimer = setTimeout(() => { saveFlow(true); }, 800);
  }

  async function saveFlow(silent) {
    if (!B.task) {
      if (!silent) status('请先选择任务', '#ff8a80');
      return;
    }
    if (B.saving) return;
    B.saving = true;
    try {
      const r = await fetch('/task/save_flow', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ task: B.task, flow: B.flow }),
      });
      const s = await r.json();
      if (!s.ok) {
        status('保存失败：' + (s.errors || []).join('; '), '#ff8a80');
        return;
      }
      B.lastSavedFlow = JSON.stringify(B.flow);
      status(silent ? `已自动保存 ${B.task} · ${new Date().toLocaleTimeString()}` : `已保存 ${B.task} · flow.json`);
    } catch (e) {
      status('保存异常：' + e, '#ff8a80');
    } finally {
      B.saving = false;
    }
  }

  async function dryRun() {
    if (!B.task) {
      status('请先选择任务', '#ff8a80');
      return;
    }
    status('dry-run 启动中…');
    try {
      const r = await fetch('/task/run', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ task: B.task, dry_run: true }),
      });
      const s = await r.json();
      if (!s.ok) {
        status('dry-run 启动失败：' + (s.errors || []).join('; '), '#ff8a80');
        return;
      }
      for (let i = 0; i < 120; i++) {
        await new Promise((res) => setTimeout(res, 500));
        const sr = await fetch('/task/run/status');
        const st = await sr.json();
        if (!st.running) {
          status(`dry-run：${st.outcome || '—'} (${st.exit || '-'})${st.error ? ' · ' + st.error : ''}`);
          return;
        }
      }
      status('dry-run 超时（仍可能在跑，看工作台运行状态）', '#ff8a80');
    } catch (e) {
      status('dry-run 异常：' + e, '#ff8a80');
    }
  }

  async function activate(preferredTask) {
    const wanted = (preferredTask || B.task || '').trim();
    if (wanted) B.task = wanted;
    await refreshTaskList();
    if (B.task) await loadTask(B.task);
  }

  const LAYOUT_KEY = 'oas.builder.layout.v1';

  function clampLayout(v, min, max) {
    return Math.round(Math.max(min, Math.min(max, v)));
  }

  function applyBuilderLayout(left, right) {
    const main = document.querySelector('.builder-main');
    if (!main) return;
    main.style.setProperty('--builder-left', clampLayout(left, 160, 420) + 'px');
    main.style.setProperty('--builder-right', clampLayout(right, 240, 520) + 'px');
  }

  function saveBuilderLayout() {
    const main = document.querySelector('.builder-main');
    if (!main) return;
    const left = document.querySelector('.builder-palette').getBoundingClientRect().width;
    const right = document.querySelector('.builder-inspector').getBoundingClientRect().width;
    if (!(left > 40 && right > 40)) return;
    try {
      localStorage.setItem(LAYOUT_KEY, JSON.stringify({ left, right }));
    } catch (e) { /* localStorage may be disabled */ }
  }

  function initBuilderLayout() {
    const main = document.querySelector('.builder-main');
    if (!main) return;
    try {
      const saved = JSON.parse(localStorage.getItem(LAYOUT_KEY) || 'null');
      if (saved && Number.isFinite(saved.left) && Number.isFinite(saved.right)) {
        applyBuilderLayout(saved.left, saved.right);
      }
    } catch (e) { /* ignore corrupt layout */ }
    document.querySelectorAll('.builder-splitter').forEach((splitter) => {
      splitter.addEventListener('pointerdown', (ev) => {
        ev.preventDefault();
        const side = splitter.dataset.side;
        const left0 = document.querySelector('.builder-palette').getBoundingClientRect().width;
        const right0 = document.querySelector('.builder-inspector').getBoundingClientRect().width;
        const startX = ev.clientX;
        const move = (e2) => {
          const dx = e2.clientX - startX;
          if (side === 'left') applyBuilderLayout(left0 + dx, right0);
          else applyBuilderLayout(left0, right0 - dx);
        };
        const up = () => {
          window.removeEventListener('pointermove', move);
          saveBuilderLayout();
        };
        window.addEventListener('pointermove', move);
        window.addEventListener('pointerup', up, { once: true });
      });
    });
  }

  function init() {
    initBuilderLayout();
    const taskSel = $('builder-task');
    if (taskSel) taskSel.addEventListener('change', () => loadTask(taskSel.value));
    const reload = $('builder-reload');
    if (reload) reload.addEventListener('click', async () => { await refreshTaskList(); await loadTask(taskSel ? taskSel.value : ''); });
    const back = $('builder-back');
    if (back) back.addEventListener('click', () => navigateHistory(-1));
    const forward = $('builder-forward');
    if (forward) forward.addEventListener('click', () => navigateHistory(1));
    const save = $('builder-save');
    if (save) save.addEventListener('click', () => saveFlow(false));
    const dry = $('builder-dry-run');
    if (dry) dry.addEventListener('click', dryRun);
    const codeApply = $('builder-code-apply');
    if (codeApply) codeApply.addEventListener('click', applyCode);
    const delChecked = $('builder-delete-checked');
    if (delChecked) delChecked.addEventListener('click', deleteCheckedSteps);
    renderPalette();
    renderAll();
    window.OAS_BUILDER = {
      activate,
      openTask: async (name) => {
        if (taskSel) taskSel.value = name;
        await loadTask(name);
      },
    };
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
