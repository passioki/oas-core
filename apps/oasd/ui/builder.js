// v1.2 任务构建页：动作库 + 值属性 + 实时流程图。
// 纯原生 JS（无构建步骤）；flow.json 是唯一真相，本页只编辑它。
(function () {
  const OP_META = {
    if: { label: '如果', group: '流程控制', container: 'then/else' },
    loop: { label: '循环 N 次', group: '流程控制', container: 'body' },
    while: { label: '当条件成立循环', group: '流程控制', container: 'body' },
    break: { label: '跳出循环', group: '流程控制' },
    continue: { label: '继续循环', group: '流程控制' },
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
    break: {},
    continue: {},
  };

  const B = {
    task: '',
    flow: null,
    assets: [],
    selected: null, // path array
    tasks: [],
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

  function addAction(op) {
    const meta = OP_META[op];
    if (!meta) return;
    const step = { op, args: JSON.parse(JSON.stringify(DEFAULTS[op] || {})) };
    if (op === 'if') {
      step.then = [];
      step.else = [];
    } else if (op === 'loop' || op === 'while') {
      step.body = [];
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

  function renderStepArray(steps, basePath, container, depth) {
    steps.forEach((step, idx) => {
      if (!step || typeof step !== 'object') return;
      const path = basePath.concat([idx]);
      const row = document.createElement('div');
      row.className = 'builder-step';
      if (B.selected && JSON.stringify(B.selected) === JSON.stringify(path)) row.classList.add('selected');
      const op = document.createElement('span');
      op.className = 'op';
      op.textContent = (OP_META[step.op] && OP_META[step.op].label) || step.op;
      const sum = document.createElement('span');
      sum.className = 'summary';
      sum.textContent = ' ' + stepSummary(step);
      row.append(op, sum);
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

  function renderGraph() {
    const root = $('builder-graph');
    if (!root) return;
    root.textContent = '';
    const steps = getRoot();
    if (!steps.length) {
      root.textContent = '（空流程）';
      return;
    }
    const wrap = document.createElement('div');
    renderGraphArray(steps, wrap);
    root.appendChild(wrap);
  }

  function renderGraphArray(steps, container) {
    steps.forEach((step, idx) => {
      if (idx > 0) {
        const a = document.createElement('div');
        a.className = 'graph-arrow';
        a.textContent = '↓';
        container.appendChild(a);
      }
      const meta = OP_META[step.op] || { label: step.op };
      const node = document.createElement('div');
      node.className = 'graph-node' + (childArrays(step).length ? ' control' : '');
      node.textContent = `${meta.label}${stepSummary(step) ? '：' + stepSummary(step) : ''}`;
      container.appendChild(node);
      for (const [key, arr] of childArrays(step)) {
        const branch = document.createElement('div');
        branch.style.marginLeft = '14px';
        const lb = document.createElement('div');
        lb.className = 'muted small';
        lb.textContent = key;
        branch.appendChild(lb);
        if (arr.length) renderGraphArray(arr, branch);
        container.appendChild(branch);
      }
    });
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

  function renderForm() {
    const root = $('builder-form');
    if (!root) return;
    root.textContent = '';
    const step = getStep(B.selected);
    if (!step) {
      root.textContent = '（选中一个动作）';
      return;
    }
    const title = document.createElement('div');
    title.innerHTML = `<strong>${esc((OP_META[step.op] && OP_META[step.op].label) || step.op)}</strong>`;
    root.appendChild(title);
    step.args = step.args || {};
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
      const b = document.createElement('button');
      b.type = 'button';
      b.textContent = '添加子动作到 ' + key;
      b.addEventListener('click', () => {
        const arr = step[key];
        const child = { op: 'log', args: { level: 'info', text: "'子动作'" } };
        arr.push(child);
        B.selected = B.selected.concat([key, arr.length - 1]);
        renderAll();
      });
      actions.appendChild(b);
    }
    root.appendChild(actions);

    const assets = $('builder-assets');
    if (assets) {
      assets.textContent = B.assets.length
        ? `可用素材/区域（${B.assets.length}）：\n` + B.assets.map((a) => `${a.id} [${a.kind}]${a.action_roi ? ' 操作区' + JSON.stringify(a.action_roi) : ''}`).join('\n')
        : '（该任务暂无素材/区域）';
    }
  }

  function renderAll() {
    renderSteps();
    renderGraph();
    renderForm();
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
      B.selected = null;
      renderAll();
      status(`已载入 ${name}：${B.flow.steps.length} 个根动作 / ${B.assets.length} 个素材`);
    } catch (e) {
      status('载入失败：' + e, '#ff8a80');
    }
  }

  async function saveFlow() {
    if (!B.task) {
      status('请先选择任务', '#ff8a80');
      return;
    }
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
      status(`已保存 ${B.task} · flow.json`);
    } catch (e) {
      status('保存异常：' + e, '#ff8a80');
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

  async function activate() {
    await refreshTaskList();
    if (B.task) await loadTask(B.task);
  }

  function init() {
    const taskSel = $('builder-task');
    if (taskSel) taskSel.addEventListener('change', () => loadTask(taskSel.value));
    const reload = $('builder-reload');
    if (reload) reload.addEventListener('click', async () => { await refreshTaskList(); await loadTask(taskSel ? taskSel.value : ''); });
    const save = $('builder-save');
    if (save) save.addEventListener('click', saveFlow);
    const dry = $('builder-dry-run');
    if (dry) dry.addEventListener('click', dryRun);
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
