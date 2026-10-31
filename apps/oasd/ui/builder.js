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
    click_point: [{ key: 'x', label: 'x', type: 'number' }, { key: 'y', label: 'y', type: 'number' }],
    dismiss_popup: [{ key: 'sel', label: '弹窗素材', type: 'asset' }, { key: 'action', label: '动作（可选）', type: 'text' }, { key: 'timeout_ms', label: '超时 ms', type: 'number' }],
    swipe: [{ key: 'from', label: '起点 [x,y]', type: 'json' }, { key: 'to', label: '终点 [x,y]', type: 'json' }, { key: 'ms', label: '时长 ms', type: 'number' }],
    finish: [{ key: 'exit', label: '出口', type: 'select', options: ['done', 'failed'] }],
    if: [{ key: 'cond', label: '条件表达式', type: 'text' }],
    loop: [{ key: 'n', label: '次数（表达式）', type: 'text' }],
    while: [{ key: 'cond', label: '条件表达式', type: 'text' }, { key: 'max_iters', label: '最大迭代（可选）', type: 'number' }],
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
  }

  function moveStepTo(fromPath, targetStepPath, key) {
    const fromArr = getContainer(fromPath);
    const idx = fromPath[fromPath.length - 1];
    const targetStep = getStep(targetStepPath);
    if (!fromArr || typeof idx !== 'number' || !targetStep || !Array.isArray(targetStep[key])) return;
    // Refuse to move a container into itself/its descendant.
    const fromKey = JSON.stringify(fromPath);
    const targetKey = JSON.stringify(targetStepPath);
    if (targetKey.startsWith(fromKey + ',') || targetKey === fromKey) return;
    const step = fromArr[idx];
    if (!step) return;
    if (fromArr === targetStep[key] && idx === targetStep[key].length - 1) return;
    fromArr.splice(idx, 1);
    targetStep[key].push(step);
    B.selected = targetStepPath.concat([key, targetStep[key].length - 1]);
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

  function gEdge(graph, from, to, label) {
    if (!from || !to || from === to) return;
    graph.edges.push({ from, to, label: label || '' });
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
        for (const ex of bodyR.exits || []) gEdge(graph, ex, node, '↺ 回边');
        for (const c of loopCtx.loop.continueNodes) gEdge(graph, c, node, 'continue');
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
    // back edge (loop/continue): curved to distinguish from forward flow
    return `M ${x1} ${y1} C ${x1 + 70} ${y1 + 34}, ${x2 - 70} ${y2 - 28}, ${x2} ${y2}`;
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

    const width = Math.max(760, ...graph.nodes.map((n) => n.cx + n.w / 2 + G.pad));
    const height = end.y + end.h + G.pad;
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

    for (const e of graph.edges) {
      const p = sEl('path', { d: edgePath(e.from, e.to), 'marker-end': 'url(#flow-arrow)' });
      p.setAttribute('class', 'flow-svg-edge');
      svg.appendChild(p);
      if (e.label) {
        const t = sEl('text', {
          x: (e.from.cx + e.to.cx) / 2,
          y: (e.from.y + e.from.h + e.to.y) / 2 - 2,
          'text-anchor': 'middle',
        });
        t.setAttribute('class', 'flow-svg-edge-label');
        t.textContent = e.label;
        svg.appendChild(t);
      }
    }
    for (const n of graph.nodes) renderSvgNode(svg, graph, n);
    root.appendChild(svg);
  }

  function assetSelect(current, kinds) {
    const sel = document.createElement('select');
    const blank = document.createElement('option');
    blank.value = '';
    blank.textContent = '（选择素材）';
    sel.appendChild(blank);
    for (const a of B.assets) {
      if (kinds && !kinds.includes(a.kind)) continue;
      const o = document.createElement('option');
      o.value = a.id;
      o.textContent = `${a.id} (${a.kind})`;
      sel.appendChild(o);
    }
    if (current && !Array.from(sel.options).some((o) => o.value === current)) {
      const o = document.createElement('option');
      o.value = current;
      o.textContent = current;
      sel.appendChild(o);
    }
    sel.value = current || '';
    return sel;
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
      for (const a of B.assets) {
        const row = document.createElement('div');
        row.textContent = `${a.id} [${a.kind}]`;
        list.appendChild(row);
      }
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
    const asset = assetSelect(loc.sel || '', null);
    assetLab.appendChild(asset);
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

  function replaceSelectedStep(next) {
    const path = B.selected;
    const arr = getContainer(path);
    if (!arr) return;
    const idx = path[path.length - 1];
    if (typeof idx !== 'number') return;
    arr.splice(idx, 1, next);
    B.selected = path;
    renderAll();
  }

  function convertSelectedStep(target) {
    const step = getStep(B.selected);
    if (!step) return;
    if (target === 'loop' && step.op === 'if') {
      const body = [...(step.then || []), ...(step.else || [])];
      replaceSelectedStep({ op: 'loop', args: { n: '1' }, body });
      status('已把 if 转换为循环；原来的是/否子动作都放进循环体');
    } else if (target === 'while' && step.op === 'if') {
      const body = [...(step.then || []), ...(step.else || [])];
      replaceSelectedStep({ op: 'while', args: { cond: 'true', max_iters: 100 }, body });
      status('已把 if 转换为当条件循环；原来的子动作放进循环体');
    } else if (target === 'if' && (step.op === 'loop' || step.op === 'while')) {
      replaceSelectedStep({ op: 'if', args: { cond: 'true' }, then: step.body || [], else: [] });
      status('已把循环转换为 if；原来的循环体放进「是」分支');
    }
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
    // Always expose the task asset/region list at the top of the inspector.
    const assetLabel = document.createElement('label');
    assetLabel.textContent = `任务素材/操作区域（${B.assets.length}）`;
    const assetPick = assetSelect(step.args.sel || '', null);
    assetPick.addEventListener('change', () => {
      if (assetPick.value) {
        step.args.sel = assetPick.value;
        status(`已选择素材：${assetPick.value}`);
        renderAll();
      }
    });
    assetLabel.appendChild(assetPick);
    root.appendChild(assetLabel);
    if (step.op === 'call_task') renderCallTaskForm(root, step);
    if (step.op === 'if' || step.op === 'while') {
      const help = document.createElement('div');
      help.className = 'muted small';
      help.textContent = '条件支持 and/or/not、&&/||/! 和括号；素材条件写成 appear(\'素材id\') / ocr(\'素材id\') == \'文字\'。';
      root.appendChild(help);
    }
    if (step.op === 'if' || step.op === 'loop' || step.op === 'while') {
      const convert = document.createElement('div');
      convert.className = 'builder-form-actions';
      if (step.op === 'if') {
        for (const [target, label] of [['loop', '转换为循环 N 次'], ['while', '转换为当条件循环']]) {
          const b = document.createElement('button');
          b.type = 'button';
          b.textContent = label;
          b.addEventListener('click', () => convertSelectedStep(target));
          convert.appendChild(b);
        }
      } else {
        const b = document.createElement('button');
        b.type = 'button';
        b.textContent = '转换为 if';
        b.addEventListener('click', () => convertSelectedStep('if'));
        convert.appendChild(b);
      }
      root.appendChild(convert);
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
        input = assetSelect(step.args[f.key], null);
        input.addEventListener('change', () => { step.args[f.key] = input.value; renderAll(); });
        lab.appendChild(input);
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
      B.selected = null;
      renderAll();
      if (st) st.textContent = '代码已应用到流程图';
      status('代码已应用，自动保存将写入 flow.json');
    } catch (e) {
      if (st) st.textContent = '代码无效：' + e.message;
    }
  }

  function renderAll() {
    renderSteps();
    renderGraph();
    renderForm();
    renderAssetPanel();
    renderCode();
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
