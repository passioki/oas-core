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
    key: { label: '发送按键', group: '动作' },
    restart_app: { label: '重启 App', group: '动作' },
    set_var: { label: '设置变量', group: '数据/结束' },
    inc_var: { label: '变量自增', group: '数据/结束' },
    ocr_to_var: { label: 'OCR 结果写入变量', group: '数据/结束' },
    log: { label: '日志', group: '数据/结束' },
    finish: { label: '结束', group: '数据/结束' },
  };
  const GROUP_ORDER = ['流程控制', '识别/等待', '动作', '参数', '数据/结束'];
  // P3（参数 v2）：动作库「参数」组。These are *presets* for real ops, NOT new
  // engine ops: clicking one generates a standard click_target / wait_until whose
  // OCR text comes from a declared meta.params key (text_expr / text_visible).
  const PARAM_GROUP = '参数';
  const PARAM_ACTIONS = {
    param_click_text: { label: '参数文字点击', op: 'click_target' },
    param_wait_text: { label: '等待参数文字', op: 'wait_until' },
  };
  // Generic FIELDS text inputs that hold expressions; they get the「插入参数 ▾」
  // button. `cond` fields insert a whole text_visible(param('k')) predicate.
  const EXPR_FIELDS = {
    log: { text: 'expr' },
    wait_until: { cond: 'cond' },
    loop: { n: 'expr' },
    set_var: { value: 'expr' },
    inc_var: { by: 'expr' },
    click_point: { x: 'expr', y: 'expr' },
  };
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
    key: [{ key: 'keycode', label: 'Android keycode（BACK=4 / HOME=3）', type: 'number' }],
    restart_app: [{ key: 'package', label: '包名', type: 'text' }],
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
    key: { keycode: 4 },
    restart_app: { package: 'com.netease.onmyoji' },
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
    relations: null,
    selected: null, // path array
    tasks: [],
    // P3（参数 v2）：meta.params schema (Studio-only read; the Runner owns values).
    params: [],
    paramsLoading: false,
    paramsError: '',
    // P5：权威 lint（GET /task/lint）。lint=null 表示尚未检查/检查失败。
    lint: null,
    lintError: '',
    lintRunning: false,
    lintSeq: 0,
    lintStale: false,
    lastExprField: null,
    lastExprMode: 'expr',
    autoSaveArmed: false,
    autoSaveTimer: null,
    lastSavedFlow: '',
    saving: false,
    checked: new Set(),
    // Undo/redo of FLOW EDITS (not selection history). Every renderAll that
    // sees a changed flow stores one snapshot; selection-only changes do not.
    flowHistory: [],
    historyIndex: -1,
    restoringHistory: false,
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

  // --- P3（参数 v2）纯函数：不依赖 DOM / 全局状态，ui_pure_test 直接抽取 ------
  // 参数声明的唯一权威是 meta.params（Studio 只定义 schema）；动作库「参数」组
  // 里的条目只是标准 op 的预设，不新增引擎 op。这里只做「数据 → flow/文案」的
  // 纯转换。

  function paramSafeKey(key) {
    return String(key === undefined || key === null ? '' : key).trim().replace(/[\x27\x22\\]/g, '');
  }

  function paramTextExpr(key) {
    return "param('" + paramSafeKey(key) + "')";
  }

  function paramWaitCond(key) {
    return 'text_visible(' + paramTextExpr(key) + ')';
  }

  // A1: 「参数文字点击」→ 标准 click_target（OCR 文字来自任务参数）。
  function paramClickStep(paramKey, ocrSel) {
    const sel = String(ocrSel === undefined || ocrSel === null ? '' : ocrSel).trim() || 'O_OCR';
    return {
      op: 'click_target',
      args: {
        find: [{ kind: 'ocr_text', sel: sel, text_expr: paramTextExpr(paramKey) }],
      },
    };
  }

  // A2: 「等待参数文字出现」→ 标准 wait_until（cond = text_visible(param(...))）。
  function paramWaitStep(paramKey, timeoutSeconds) {
    const n = Number(timeoutSeconds);
    const timeout = isFinite(n) && n > 0 ? Math.round(n) : 10;
    return {
      op: 'wait_until',
      args: { cond: paramWaitCond(paramKey), timeout_s: timeout },
    };
  }

  // Locator 文字来源三态写回：固定文字写 text，任务参数/表达式写 text_expr，
  // 两者互斥（切换时删除另一个字段）。未知字段（roi/require_template/shape…）
  // 必须原样保留 —— 只 merge 已知键，禁止重建对象。
  function paramSourceApply(loc, mode, value) {
    const out = Object.assign({}, loc && typeof loc === 'object' ? loc : {});
    const text = String(value === undefined || value === null ? '' : value);
    if (mode === 'param') {
      delete out.text;
      out.text_expr = paramTextExpr(text);
    } else if (mode === 'expr') {
      delete out.text;
      out.text_expr = text;
    } else {
      delete out.text_expr;
      out.text = text;
    }
    return out;
  }

  // 读回三态：text→fixed；text_expr 恰为 param('字面量')→param；其他表达式→expr。
  function paramSourceFromLocator(loc) {
    const l = loc && typeof loc === 'object' ? loc : {};
    const hasText = Object.prototype.hasOwnProperty.call(l, 'text') &&
      l.text !== undefined && l.text !== null;
    const expr = l.text_expr === undefined || l.text_expr === null
      ? ''
      : String(l.text_expr).trim();
    if (hasText) return { mode: 'text', value: String(l.text) };
    const m = expr.match(/^param\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\)$/);
    if (m) return { mode: 'param', value: m[1] };
    if (expr) return { mode: 'expr', value: expr };
    return { mode: 'text', value: '' };
  }

  function locatorParamKey(loc) {
    const src = paramSourceFromLocator(loc);
    return src.mode === 'param' ? src.value : '';
  }

  // 互斥 + 必填其一（旧 flow 的 text 语义零变化；新 flow 用 text_expr）。
  function locatorTextSpecError(loc) {
    const l = loc && typeof loc === 'object' && !Array.isArray(loc) ? loc : {};
    const hasText = Object.prototype.hasOwnProperty.call(l, 'text');
    const hasExpr = Object.prototype.hasOwnProperty.call(l, 'text_expr');
    if (hasText && hasExpr) return 'text 与 text_expr 互斥（只能写一个）';
    if (!hasText && !hasExpr) return '缺少文字来源（text 或 text_expr 二选一）';
    return '';
  }

  // 保存前的本地校验：扫描 flow 里每个 ocr_text 定位器（find/expect/requires/
  // drag from/to 都带 kind:'ocr_text'），text 与 text_expr 必须恰好一个。
  function flowTextSpecIssues(flow) {
    const issues = [];
    function walk(node, path) {
      if (node === null || node === undefined) return;
      if (Array.isArray(node)) {
        node.forEach(function (child, i) { walk(child, path + '[' + i + ']'); });
        return;
      }
      if (typeof node !== 'object') return;
      if (node.kind === 'ocr_text') {
        const err = locatorTextSpecError(node);
        if (err) issues.push(path + '：' + err);
      }
      for (const key of Object.keys(node)) walk(node[key], path ? path + '.' + key : key);
    }
    walk({ steps: flow && flow.steps }, 'flow');
    return issues;
  }

  // 「插入参数」片段：普通表达式插 param('k')；条件字段插整段谓词。
  function paramInsertSnippet(key, mode) {
    const expr = paramTextExpr(key);
    return mode === 'cond' ? 'text_visible(' + expr + ')' : expr;
  }

  function paramDefaultText(param) {
    const p = param && typeof param === 'object' ? param : {};
    if (!Object.prototype.hasOwnProperty.call(p, 'default') ||
        p.default === undefined || p.default === null) {
      return '—';
    }
    if (p.default === '') return '（空字符串）';
    if (typeof p.default === 'string') return p.default;
    if (typeof p.default === 'number' || typeof p.default === 'boolean') return String(p.default);
    try {
      return JSON.stringify(p.default);
    } catch (e) {
      return String(p.default);
    }
  }

  // 面板行数据（分组/key/类型/label/default），非对象项被过滤。
  function paramPanelRows(params) {
    const list = Array.isArray(params) ? params : [];
    return list
      .filter(function (p) { return p && typeof p === 'object' && !Array.isArray(p); })
      .map(function (p) {
        const key = p.key === undefined || p.key === null ? '' : String(p.key).trim();
        const label = p.label === undefined || p.label === null ? '' : String(p.label).trim();
        const group = p.group === undefined || p.group === null ? '' : String(p.group).trim();
        const type = p.type === undefined || p.type === null ? '' : String(p.type).trim();
        return {
          key: key,
          label: label || key,
          type: type,
          group: group,
          defaultText: paramDefaultText(p),
        };
      });
  }

  function paramPanelSummary(params) {
    const rows = paramPanelRows(params);
    if (!rows.length) return '0 项参数';
    const groups = new Set();
    for (const row of rows) groups.add(row.group || '未分组');
    return rows.length + ' 项参数 · ' + groups.size + ' 个分组';
  }

  // flow 里所有 param('字面量') 引用（T3 同口径：只认恰好一个字符串字面量；
  // 双引号也算，因为表达式解析器两种都接受）。text_expr / cond / 任意表达式字段
  // 都会被递归扫描。
  function collectParamUses(flow) {
    const uses = [];
    const re = /param\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\)/g;
    function walk(node, path) {
      if (node === null || node === undefined) return;
      if (typeof node === 'string') {
        re.lastIndex = 0;
        let m;
        while ((m = re.exec(node)) !== null) uses.push({ key: m[1], path: path, expr: m[0] });
        return;
      }
      if (Array.isArray(node)) {
        node.forEach(function (child, i) { walk(child, path + '[' + i + ']'); });
        return;
      }
      if (typeof node !== 'object') return;
      for (const key of Object.keys(node)) walk(node[key], path ? path + '.' + key : key);
    }
    walk(flow && flow.steps, 'steps');
    return uses;
  }

  function undeclaredParamUses(params, uses) {
    const declared = new Set(paramPanelRows(params).map(function (row) { return row.key; }));
    const out = [];
    for (const use of Array.isArray(uses) ? uses : []) {
      if (!use || !use.key || declared.has(use.key)) continue;
      out.push(use);
    }
    return out;
  }

  // --- P5：权威 lint（纯函数，ui_pure_test 直接抽取；不依赖 DOM/全局状态） ---
  // 后端 GET /task/lint 返回 {ok,task,issues:[{code,path,message}],errors,count}；
  // 字段缺失时全部降级（issues 缺失=空、count 缺失=issues.length、message 缺失=''）。

  function lintIssueCode(issue) {
    const it = issue && typeof issue === 'object' && !Array.isArray(issue) ? issue : {};
    const raw = it.code !== undefined && it.code !== null ? it.code : it.rule;
    return String(raw === undefined || raw === null ? '' : raw).trim().toUpperCase();
  }

  function lintIssuePath(issue) {
    const it = issue && typeof issue === 'object' && !Array.isArray(issue) ? issue : {};
    const raw = it.path !== undefined && it.path !== null
      ? it.path
      : (it.location !== undefined && it.location !== null ? it.location : '');
    const text = String(raw === undefined || raw === null ? '' : raw).trim();
    return text || '—';
  }

  function lintIssueMessage(issue) {
    const it = issue && typeof issue === 'object' && !Array.isArray(issue) ? issue : {};
    for (const field of ['message', 'msg', 'text', 'detail']) {
      if (it[field] === undefined || it[field] === null) continue;
      const text = String(it[field]).trim();
      if (text) return text;
    }
    return '';
  }

  function lintIsT3Issue(issue) {
    return lintIssueCode(issue) === 'T3';
  }

  // One display line: 「[T3] flow.json:steps[0]：message」.
  function lintIssueLine(issue) {
    const code = lintIssueCode(issue) || '?';
    const path = lintIssuePath(issue);
    const message = lintIssueMessage(issue) || '（无说明）';
    return '[' + code + '] ' + path + '：' + message;
  }

  function lintIssuesText(issues) {
    const list = Array.isArray(issues) ? issues : [];
    return list.map(lintIssueLine).join('\n');
  }

  // Extract the undeclared param key from a T3 issue (path/message), for the
  // 「去声明…」 prefill hint. Implements the same «param(<single literal>)» rule as
  // T3 without a quoted regex (the UI test extracts these functions verbatim).
  function lintT3ParamKey(issue) {
    const text = [lintIssuePath(issue), lintIssueMessage(issue)].join(' ');
    let from = 0;
    while (from < text.length) {
      const start = text.indexOf('param(', from);
      if (start < 0) return '';
      const rest = text.slice(start + 6);
      const single = /^[ \t]*\x27([^\x27]+)\x27/.exec(rest);
      if (single) return single[1];
      const double = /^[ \t]*\x22([^\x22]+)\x22/.exec(rest);
      if (double) return double[1];
      from = start + 6;
    }
    return '';
  }

  function lintNormalizeResponse(body) {
    const valid = body && typeof body === 'object' && !Array.isArray(body);
    const b = valid ? body : {};
    const issues = Array.isArray(b.issues)
      ? b.issues.filter(function (issue) {
        return issue && typeof issue === 'object' && !Array.isArray(issue);
      })
      : [];
    const errors = Array.isArray(b.errors)
      ? b.errors
        .filter(function (line) { return line !== undefined && line !== null && String(line).trim() !== ''; })
        .map(String)
      : [];
    if (!valid && !errors.length) errors.push('响应不是 JSON 对象');
    const rawCount = Number(b.count);
    const count = isFinite(rawCount) && rawCount >= 0 ? Math.floor(rawCount) : issues.length;
    return {
      ok: !!valid && b.ok !== false,
      task: String(b.task === undefined || b.task === null ? '' : b.task),
      issues: issues,
      errors: errors,
      count: count,
    };
  }

  // Summary chip: {count,text,cls}. null result = 尚未检查.
  function lintSummary(result) {
    const r = result && typeof result === 'object' && !Array.isArray(result) ? result : null;
    if (!r) return { count: 0, text: '尚未检查', cls: 'kind' };
    const rawCount = Number(r.count);
    const issues = Array.isArray(r.issues) ? r.issues : [];
    const count = isFinite(rawCount) && rawCount >= 0 ? Math.floor(rawCount) : issues.length;
    const errors = Array.isArray(r.errors) ? r.errors : [];
    if (count > 0) return { count: count, text: count + ' 个问题', cls: 'err' };
    if (errors.length) return { count: 0, text: '检查失败', cls: 'warn' };
    return { count: 0, text: '全部通过', cls: 'ok' };
  }

  // Count badge text: '' for 0/no result (the badge is hidden then).
  function lintCountText(result) {
    const summary = lintSummary(result);
    return summary.count > 0 ? String(summary.count) : '';
  }

  function lintT3Rows(result) {
    const r = result && typeof result === 'object' && !Array.isArray(result) ? result : null;
    const issues = r && Array.isArray(r.issues) ? r.issues : [];
    return issues.filter(lintIsT3Issue);
  }

  function lintErrorText(result) {
    const r = result && typeof result === 'object' && !Array.isArray(result) ? result : null;
    const errors = r && Array.isArray(r.errors) ? r.errors : [];
    return errors.map(String).join('\n');
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
      case 'click_target': {
        const loc = (Array.isArray(a.find) && a.find[0]) || {};
        const key = locatorParamKey(loc);
        if (key) return `${esc(loc.sel)} · 文字=param('${key}')`;
        return JSON.stringify(a.find || []).slice(0, 60);
      }
      case 'dismiss_popup': return esc(a.sel);
      case 'swipe': return `${JSON.stringify(a.from)}→${JSON.stringify(a.to)}`;
      case 'key': return `keycode ${esc(a.keycode)}`;
      case 'restart_app': return esc(a.package) || '（未填包名）';
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

  // --- P3：任务参数面板（只读视图；声明入口仍是任务库「配置项…」） ----------

  // Approximate param-usage hint (front-end scan). It is only a convenience:
  // when the authoritative lint panel has a result it collapses into a low-weight
  // <details> (or shows «权威检查不可用» when the backend route is missing).
  function renderParamIssuesHint(host, keys, authoritative) {
    if (!host) return;
    host.textContent = '';
    host.className = 'builder-param-issues';
    if (!keys.length) return;
    const text = 'flow 使用了未声明参数：' + keys.join('、') +
      '；点「配置项…」声明（发布 T3 会拒绝）。';
    if (!authoritative) {
      host.className = 'builder-param-issues approx';
      host.textContent = '⚠ 近似提示（权威检查不可用）：' + text;
      return;
    }
    host.classList.add('approx');
    const details = document.createElement('details');
    details.className = 'builder-param-issues-details';
    const summary = document.createElement('summary');
    summary.textContent = '参数近似提示：' + keys.length + ' 个 key 未声明（以顶部「检查」结果为准）';
    const body = document.createElement('div');
    body.textContent = '⚠ ' + text;
    details.append(summary, body);
    host.appendChild(details);
  }

  function renderParamsPanel() {
    const list = $('builder-params-list');
    const summary = $('builder-params-summary');
    const statusEl = $('builder-params-status');
    const issuesEl = $('builder-param-issues');
    const rows = paramPanelRows(B.params);
    if (summary) {
      summary.textContent = paramPanelSummary(B.params);
      summary.className = 'builder-params-chip ' + (rows.length ? 'kind' : 'warn');
    }
    if (issuesEl) {
      const undeclared = undeclaredParamUses(B.params, collectParamUses(B.flow));
      const keys = [];
      for (const use of undeclared) {
        if (keys.indexOf(use.key) < 0) keys.push(use.key);
      }
      const authoritative = !!B.lint && !B.lintError;
      renderParamIssuesHint(issuesEl, keys, authoritative);
    }
    if (statusEl) {
      if (B.paramsError) {
        statusEl.textContent = '参数读取失败：' + B.paramsError + '（不阻塞 flow 编辑）';
        statusEl.className = 'muted small err';
      } else if (B.paramsLoading) {
        statusEl.textContent = '读取参数…';
        statusEl.className = 'muted small';
      } else if (!B.task) {
        statusEl.textContent = '先选择任务：参数来自该任务的 meta.params。';
        statusEl.className = 'muted small';
      } else if (!rows.length) {
        statusEl.textContent = '暂无任务参数：点「配置项…」在 meta.params 声明；声明后动作库「参数」组与「插入参数」可用。';
        statusEl.className = 'muted small';
      } else {
        statusEl.textContent = '参数来自 meta.params（GET /task/config）；运行值由 Runner 填写，Studio 只定义 schema。';
        statusEl.className = 'muted small ok';
      }
    }
    if (!list) return;
    list.textContent = '';
    if (B.paramsLoading) {
      list.textContent = '读取中…';
      return;
    }
    if (B.paramsError) {
      list.textContent = '参数读取失败：' + B.paramsError;
      return;
    }
    if (!B.task) {
      list.textContent = '（先选择任务）';
      return;
    }
    if (!rows.length) {
      const empty = document.createElement('div');
      empty.className = 'builder-params-empty muted small';
      empty.textContent = '暂无任务参数：点「配置项…」声明（例如 key=分组 / type=text / label=分组 / default=分组12）';
      list.appendChild(empty);
      return;
    }
    for (const row of rows) {
      const line = document.createElement('div');
      line.className = 'builder-param-row';
      const group = document.createElement('span');
      group.className = 'builder-param-group';
      group.textContent = row.group || '未分组';
      const key = document.createElement('span');
      key.className = 'builder-param-key';
      key.textContent = row.key || '（未命名）';
      const type = document.createElement('span');
      type.className = 'builder-param-type';
      type.textContent = row.type || '—';
      const label = document.createElement('span');
      label.className = 'builder-param-label';
      label.textContent = row.label || row.key;
      const def = document.createElement('span');
      def.className = 'builder-param-default';
      def.textContent = '默认 ' + row.defaultText;
      const insert = document.createElement('button');
      insert.type = 'button';
      insert.textContent = '插入参数';
      insert.title = "把 param('" + row.key + "') 插入到最近聚焦的表达式字段（或复制到剪贴板）";
      insert.disabled = !row.key;
      insert.addEventListener('click', function () { paramInsertFromPanel(row.key); });
      line.append(group, key, type, label, def, insert);
      list.appendChild(line);
    }
  }

  // --- P5：权威 lint 面板 / 请求 / T3 去声明 ----------------------------------

  function clearLintState() {
    B.lint = null;
    B.lintError = '';
    B.lintRunning = false;
    B.lintStale = false;
  }

  function lintStatusText() {
    if (B.lintRunning) return '检查中…（GET /task/lint）';
    if (B.lintError) return B.lintError;
    const summary = lintSummary(B.lint);
    if (!B.lint) return '尚未检查';
    if (summary.count > 0) return '检查完成：' + summary.count + ' 个问题 · ' + B.task;
    return '检查完成：全部通过 · ' + B.task;
  }

  function lintIssueRow(issue) {
    const code = lintIssueCode(issue) || '?';
    const row = document.createElement('div');
    row.className = 'builder-lint-row' + (lintIsT3Issue(issue) ? ' t3' : '');
    const codeEl = document.createElement('span');
    codeEl.className = 'builder-lint-code';
    codeEl.textContent = '[' + code + ']';
    const pathEl = document.createElement('span');
    pathEl.className = 'builder-lint-path';
    pathEl.textContent = lintIssuePath(issue);
    const msgEl = document.createElement('span');
    msgEl.className = 'builder-lint-message';
    msgEl.textContent = lintIssueMessage(issue) || '（无说明）';
    const actions = document.createElement('span');
    actions.className = 'builder-lint-actions';
    if (lintIsT3Issue(issue)) {
      const key = lintT3ParamKey(issue);
      const declare = document.createElement('button');
      declare.type = 'button';
      declare.textContent = '去声明…';
      declare.title = key
        ? '打开任务配置项对话框声明参数「' + key + '」；保存后自动重跑检查'
        : '打开任务配置项对话框声明缺失参数；保存后自动重跑检查';
      declare.addEventListener('click', function () { lintOpenDeclare(issue); });
      actions.appendChild(declare);
    } else if (code === 'T4') {
      const hint = document.createElement('span');
      hint.className = 'builder-lint-hint';
      hint.textContent = '素材缺失：到任务素材/全局素材库补回后重新检查';
      actions.appendChild(hint);
    }
    row.append(codeEl, pathEl, msgEl, actions);
    return row;
  }

  // Authoritative panel: hidden until a task is loaded and a check ran; the head
  // count badge stays visible after the panel is closed.
  function renderLintPanel() {
    const panel = $('builder-lint-panel');
    const countEl = $('builder-lint-count');
    const summaryEl = $('builder-lint-summary');
    const statusEl = $('builder-lint-status');
    const listEl = $('builder-lint-list');
    const errEl = $('builder-lint-error');
    const btn = $('builder-lint-btn');
    if (btn) btn.disabled = B.lintRunning === true;
    const summary = lintSummary(B.lint);
    const countText = B.lintRunning ? '' : lintCountText(B.lint);
    if (countEl) {
      countEl.textContent = countText || '0';
      countEl.className = 'builder-lint-count' +
        (summary.count > 0 ? ' err' : (B.lint && !B.lintError ? ' ok' : ''));
      if (countText) countEl.removeAttribute('hidden');
      else countEl.setAttribute('hidden', '');
    }
    if (!panel) return;
    const show = !!B.task && (B.lintRunning || !!B.lint || !!B.lintError);
    if (!show) {
      panel.setAttribute('hidden', '');
      if (summaryEl) {
        summaryEl.textContent = '尚未检查';
        summaryEl.className = 'builder-params-chip kind';
      }
      if (statusEl) {
        statusEl.textContent = B.task ? '尚未检查' : '先选择任务';
        statusEl.className = 'muted small';
      }
      if (listEl) listEl.textContent = '';
      if (errEl) errEl.textContent = '';
      return;
    }
    panel.removeAttribute('hidden');
    if (summaryEl) {
      let summaryText = summary.text;
      let summaryCls = summary.cls;
      if (B.lintError && !B.lintRunning) {
        summaryText = '权威检查不可用';
        summaryCls = 'warn';
      } else if (B.lintStale && !B.lintRunning) {
        summaryText += '（可重跑）';
      }
      summaryEl.textContent = summaryText;
      summaryEl.className = 'builder-params-chip ' + summaryCls;
    }
    if (statusEl) {
      statusEl.textContent = lintStatusText();
      statusEl.className = 'muted small' +
        (B.lintRunning ? '' : (B.lintError ? ' err' : (summary.count > 0 ? ' warn' : ' ok')));
    }
    if (errEl) {
      // The status line already names an unavailable backend; this <pre> only
      // carries response-level errors (e.g. T1/META parse failures).
      errEl.textContent = lintErrorText(B.lint);
    }
    if (!listEl) return;
    listEl.textContent = '';
    if (B.lintRunning) {
      listEl.textContent = '检查中…';
      return;
    }
    if (!B.lint) {
      listEl.textContent = '权威检查不可用（不影响 flow 编辑）；参数面板保留近似提示。';
      return;
    }
    const issues = Array.isArray(B.lint.issues) ? B.lint.issues : [];
    if (!issues.length) {
      listEl.textContent = '全部通过：没有发现问题';
      return;
    }
    for (const issue of issues) listEl.appendChild(lintIssueRow(issue));
  }

  async function lintTask(opts) {
    const options = opts && typeof opts === 'object' ? opts : {};
    const task = String(B.task || '').trim();
    if (!task) {
      clearLintState();
      renderLintPanel();
      return;
    }
    B.lintSeq += 1;
    const seq = B.lintSeq;
    B.lintRunning = true;
    B.lintStale = false;
    renderLintPanel();
    try {
      const r = await fetch('/task/lint?task=' + encodeURIComponent(task), { cache: 'no-store' });
      let body = null;
      try { body = await r.json(); } catch (e) { body = null; }
      if (seq !== B.lintSeq || B.task !== task) return; // late response guard
      if (!r.ok) {
        const detail = body && Array.isArray(body.errors) && body.errors.length
          ? body.errors.map(String).join('; ')
          : ('HTTP ' + r.status);
        B.lint = null;
        B.lintRunning = false;
        B.lintError = r.status === 404
          ? '权威检查不可用：后端未实现 GET /task/lint（或任务不存在）'
          : '权威检查不可用：' + detail;
        renderLintPanel();
        renderParamsPanel();
        if (!options.silent) status(B.lintError, '#ff8a80');
        return;
      }
      const result = lintNormalizeResponse(body);
      if (!result.ok) {
        // A 200 with ok:false / a non-JSON body is a failed check, not "全部通过".
        B.lint = null;
        B.lintRunning = false;
        B.lintError = '权威检查不可用：' + (result.errors.join('; ') || '后端返回 ok=false');
        renderLintPanel();
        renderParamsPanel();
        if (!options.silent) status(B.lintError, '#ff8a80');
        return;
      }
      B.lint = result;
      B.lintError = '';
      B.lintRunning = false;
      renderLintPanel();
      renderParamsPanel();
      if (!options.silent) {
        const summary = lintSummary(result);
        status('检查完成：' + summary.text +
          (summary.count ? '（' + result.issues.length + ' 条，见检查面板）' : ''),
        summary.count ? '#e8c46a' : '#7bd88f');
      }
    } catch (e) {
      if (seq !== B.lintSeq || B.task !== task) return;
      B.lint = null;
      B.lintRunning = false;
      B.lintError = '权威检查不可用：' + String((e && e.message) || e);
      renderLintPanel();
      renderParamsPanel();
      if (!options.silent) status(B.lintError, '#ff8a80');
    }
  }

  // T3 row「去声明…」: reuse the #task-config-dialog bridge; its onSaved handler
  // re-reads meta.params and re-runs lint, so a fixed T3 disappears here.
  function lintOpenDeclare(issue) {
    const key = lintT3ParamKey(issue);
    status('去声明参数：' + (key || '（见检查结果）') + '…');
    openTaskParamsDialog();
  }

  function closeLintPanel() {
    const panel = $('builder-lint-panel');
    if (panel) panel.setAttribute('hidden', '');
  }

  function renderPalette() {
    const root = $('builder-ops');
    if (!root) return;
    root.textContent = '';
    for (const g of GROUP_ORDER) {
      const h = document.createElement('div');
      h.className = 'builder-op-group' + (g === PARAM_GROUP ? ' builder-op-param-group' : '');
      h.textContent = g === PARAM_GROUP ? '参数（生成标准 flow，不新增 op）' : g;
      root.appendChild(h);
      if (g === PARAM_GROUP) {
        for (const key of Object.keys(PARAM_ACTIONS)) {
          const b = document.createElement('button');
          b.type = 'button';
          b.textContent = PARAM_ACTIONS[key].label + '  (' + key + ')';
          b.className = 'builder-op-param-btn';
          b.title = '生成标准 ' + PARAM_ACTIONS[key].op + '：OCR 文字来自 param(\'<key>\')';
          b.addEventListener('click', () => addParamAction(key));
          root.appendChild(b);
        }
        continue;
      }
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

  // Shared insertion: into the selected container's first child array, else the
  // root right before a trailing `finish` (the palette's finish-last promise).
  function insertStep(step, label) {
    const selected = getStep(B.selected);
    const containers = childArrays(selected);
    if (containers.length) {
      const [key, arr] = containers[0];
      arr.push(step);
      B.selected = B.selected.concat([key, arr.length - 1]);
      renderAll();
      status(`已添加子动作 ${label} → ${key}`);
      return;
    }
    const arr = getRoot();
    // Keep `finish` last automatically: append before the trailing finish.
    let at = arr.length;
    if (at > 0 && arr[at - 1] && arr[at - 1].op === 'finish') at -= 1;
    arr.splice(at, 0, step);
    B.selected = [at];
    renderAll();
    status(`已添加 ${label}`);
  }

  function addAction(op) {
    const meta = OP_META[op];
    if (!meta) return;
    insertStep(makeStep(op), meta.label);
  }

  // --- P3：动作库「参数」组 ---------------------------------------------------
  function pickParamForAction() {
    const rows = paramPanelRows(B.params);
    if (!rows.length) return null;
    for (const p of B.params) {
      if (!p || typeof p !== 'object') continue;
      const role = p.role === undefined || p.role === null ? '' : String(p.role);
      const type = p.type === undefined || p.type === null ? '' : String(p.type);
      if (role === 'ocr_text' || type === 'text' || type === 'string') return p;
    }
    return B.params[0];
  }

  function pickOcrAssetForAction() {
    for (const a of B.assets) {
      if (a && (a.kind === 'ocr_line' || a.kind === 'ocr_lines')) return a.id;
    }
    return '';
  }

  function flashParamsPanel() {
    const panel = $('builder-params');
    if (!panel) return;
    panel.classList.remove('builder-flash');
    // force a reflow so the animation can replay on repeated clicks
    void panel.offsetWidth;
    panel.classList.add('builder-flash');
    if (typeof panel.scrollIntoView === 'function') {
      try { panel.scrollIntoView({ block: 'nearest', behavior: 'smooth' }); } catch (e) { /* cosmetic */ }
    }
  }

  function addParamAction(kind) {
    const meta = PARAM_ACTIONS[kind];
    if (!meta) return;
    if (!paramPanelRows(B.params).length) {
      status('先在任务参数面板声明参数（点「配置项…」）再使用参数动作', '#ff8a80');
      flashParamsPanel();
      return;
    }
    const param = pickParamForAction() || {};
    const key = paramSafeKey(param.key);
    if (!key) {
      status('参数缺少 key，无法生成参数动作；请先在「配置项…」修正', '#ff8a80');
      return;
    }
    const step = kind === 'param_click_text'
      ? paramClickStep(key, pickOcrAssetForAction())
      : paramWaitStep(key, 10);
    insertStep(step, meta.label + '（' + meta.op + '）');
    status(`已生成 ${meta.op}：文字来源 param('${key}')（不新增 op，可在属性面板改）`);
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
      const paths = parseDragPayload(e.dataTransfer.getData('text/plain'));
      if (!paths.length) return;
      if (paths.length > 1) moveStepsGroup(paths, null, null, true);
      else moveStepToRoot(paths[0]);
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

  function pathIsAncestor(anc, path) {
    if (!Array.isArray(anc) || !Array.isArray(path) || anc.length >= path.length) return false;
    for (let i = 0; i < anc.length; i++) {
      if (anc[i] !== path[i]) return false;
    }
    return true;
  }

  function checkedPaths() {
    const out = [];
    for (const key of B.checked) {
      try {
        const p = JSON.parse(key);
        if (Array.isArray(p)) out.push(p);
      } catch (e) { /* ignore */ }
    }
    // Drop descendants when their parent is also checked (move parent once).
    return out
      .filter((p) => !out.some((q) => q !== p && pathIsAncestor(q, p)))
      .sort((a, b) => {
        if (a.length !== b.length) return a.length - b.length;
        return JSON.stringify(a).localeCompare(JSON.stringify(b));
      });
  }

  function dragPayload(fromPath) {
    const key = JSON.stringify(fromPath);
    if (B.checked.has(key) && B.checked.size > 1) {
      return { paths: checkedPaths() };
    }
    return { path: fromPath };
  }

  function moveStepsGroup(paths, targetStepPath, key, toRoot) {
    if (!Array.isArray(paths) || !paths.length) return;
    const targetStep = toRoot ? null : getStep(targetStepPath);
    if (!toRoot && (!targetStep || !Array.isArray(targetStep[key]))) return;
    const moves = [];
    for (const p of paths) {
      const arr = getContainer(p);
      const idx = p[p.length - 1];
      if (!arr || typeof idx !== 'number') continue;
      const step = arr[idx];
      if (!step) continue;
      if (!toRoot && (step === targetStep || stepContains(step, targetStep))) {
        status('不能把父级移动到自己的子级', '#ff8a80');
        return;
      }
      moves.push({ path: p, arr, idx, step });
    }
    // Remove deepest/last first so earlier array indices stay valid.
    for (const m of moves.sort((a, b) => {
      if (a.arr !== b.arr) return 0;
      return b.idx - a.idx;
    })) {
      if (m.arr[m.idx] === m.step) m.arr.splice(m.idx, 1);
    }
    const root = getRoot();
    let at = root.length;
    if (at > 0 && root[at - 1] && root[at - 1].op === 'finish') at -= 1;
    for (const m of moves) {
      if (toRoot) root.splice(at++, 0, m.step);
      else targetStep[key].push(m.step);
    }
    B.checked.clear();
    B.selected = toRoot
      ? [Math.max(0, at - moves.length)]
      : targetStepPath.concat([key, targetStep[key].length - 1]);
    renderAll();
    status(`已移动 ${moves.length} 个勾选步骤到 ${toRoot ? '根级' : key}`);
  }

  function parseDragPayload(raw) {
    try {
      const v = JSON.parse(raw || 'null');
      if (v && Array.isArray(v.paths) && v.paths.length) return v.paths;
      if (v && Array.isArray(v.path)) return [v.path];
    } catch (e) { /* ignore */ }
    return [];
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
        e.dataTransfer.setData('text/plain', JSON.stringify(dragPayload(path)));
        e.dataTransfer.effectAllowed = 'move';
        row.classList.add('dragging');
      });
      row.addEventListener('dragend', () => row.classList.remove('dragging'));
      const pick = document.createElement('input');
      pick.type = 'checkbox';
      pick.className = 'builder-step-check';
      pick.checked = B.checked.has(JSON.stringify(path));
      pick.title = '勾选后可整体删除/拖动';
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
          const paths = parseDragPayload(e.dataTransfer.getData('text/plain'));
          if (!paths.length) return;
          if (paths.length > 1) moveStepsGroup(paths, path, key, false);
          else moveStepTo(paths[0], path, key);
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

  // --- P3：属性面板「插入参数 ▾」/ 任务参数面板 -------------------------------

  // Track the last focused expression field so the params-panel row button can
  // insert into it (or copy to the clipboard when no field was touched yet).
  function trackExprField(field, mode) {
    if (!field || typeof field.addEventListener !== 'function') return;
    field.addEventListener('focus', function () {
      B.lastExprField = field;
      B.lastExprMode = mode || 'expr';
    });
  }

  function insertTextAtField(field, text) {
    if (!field) return false;
    const value = String(field.value === undefined || field.value === null ? '' : field.value);
    const start = typeof field.selectionStart === 'number' ? field.selectionStart : value.length;
    const end = typeof field.selectionEnd === 'number' ? field.selectionEnd : start;
    field.value = value.slice(0, start) + text + value.slice(end);
    const caret = start + text.length;
    if (typeof field.setSelectionRange === 'function') {
      try { field.setSelectionRange(caret, caret); } catch (e) { /* cosmetic */ }
    }
    if (typeof field.focus === 'function') field.focus();
    field.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  }

  function copyTextBestEffort(text) {
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard &&
          typeof navigator.clipboard.writeText === 'function') {
        navigator.clipboard.writeText(String(text)).catch(function () { /* clipboard blocked */ });
        return true;
      }
    } catch (e) { /* ignore */ }
    return false;
  }

  // 「插入参数 ▾」：列出已声明参数（按 group 显示），点击把 param('key')（或条件
  // 字段的 text_visible(param('key'))）插入到 getField() 返回的输入框光标处。
  function paramInsertWidget(mode, getField) {
    const wrap = document.createElement('span');
    wrap.className = 'builder-insert-param';
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.textContent = '插入参数 ▾';
    btn.title = mode === 'cond'
      ? "插入 text_visible(param('key')) 条件片段（参数需先在「任务参数」面板声明）"
      : "插入 param('key') 到当前输入框（参数需先在「任务参数」面板声明）";
    btn.setAttribute('aria-haspopup', 'true');
    btn.setAttribute('aria-expanded', 'false');
    const menu = document.createElement('div');
    menu.className = 'builder-insert-param-menu';
    menu.hidden = true;
    let open = false;

    function close() {
      if (!open) return;
      open = false;
      menu.hidden = true;
      btn.setAttribute('aria-expanded', 'false');
      document.removeEventListener('click', onDoc, true);
    }
    function onDoc(ev) {
      if (!wrap.contains(ev.target)) close();
    }
    function insert(key) {
      const snippet = paramInsertSnippet(key, mode);
      const field = typeof getField === 'function' ? getField() : null;
      if (field) {
        insertTextAtField(field, snippet);
        status('已插入 ' + snippet);
      } else if (copyTextBestEffort(snippet)) {
        status('已复制到剪贴板：' + snippet);
      } else {
        status('参数片段：' + snippet);
      }
      close();
    }
    function renderMenu() {
      menu.textContent = '';
      const head = document.createElement('div');
      head.className = 'hd';
      head.textContent = mode === 'cond'
        ? "插入参数条件（写入 text_visible(param('key'))）"
        : "插入已声明参数（写入 param('key')）";
      menu.appendChild(head);
      const rows = paramPanelRows(B.params);
      if (!rows.length) {
        const empty = document.createElement('div');
        empty.className = 'hd';
        empty.textContent = '（还没有声明参数：先在「任务参数」面板点「配置项…」声明）';
        menu.appendChild(empty);
        return;
      }
      for (const row of rows) {
        const item = document.createElement('button');
        item.type = 'button';
        item.className = 'builder-insert-param-item';
        const code = document.createElement('code');
        code.textContent = paramInsertSnippet(row.key, mode);
        item.appendChild(code);
        const grp = document.createElement('span');
        grp.className = 'grp';
        grp.textContent = (row.group || '未分组') + (row.type ? ' · ' + row.type : '');
        item.appendChild(grp);
        item.addEventListener('click', function (ev) {
          ev.stopPropagation();
          insert(row.key);
        });
        menu.appendChild(item);
      }
    }
    btn.addEventListener('click', function (ev) {
      ev.stopPropagation();
      if (open) { close(); return; }
      renderMenu();
      menu.hidden = false;
      open = true;
      btn.setAttribute('aria-expanded', 'true');
      setTimeout(function () { document.addEventListener('click', onDoc, true); }, 0);
    });
    menu.addEventListener('click', function (ev) { ev.stopPropagation(); });
    wrap.appendChild(btn);
    wrap.appendChild(menu);
    return wrap;
  }

  function paramInsertFromPanel(key) {
    const snippet = paramInsertSnippet(key, B.lastExprMode || 'expr');
    const field = B.lastExprField;
    if (field && field.isConnected && typeof field.value === 'string') {
      insertTextAtField(field, snippet);
      status('已插入 ' + snippet + '（参数来自面板）');
      return;
    }
    if (copyTextBestEffort(snippet)) {
      status('已复制到剪贴板：' + snippet + '（先点属性面板的表达式输入框可直接插入）');
    } else {
      status('参数片段：' + snippet + '（先点属性面板的表达式输入框可直接插入）');
    }
  }

  // 定位器文字来源三态（P3）：固定文字 / 任务参数 / 表达式。固定写 text，后两者
  // 写 text_expr；切换时删除另一字段。未知 locator 字段（roi / require_template /
  // shape …）必须原样保留（只 merge 已知键，禁止重建对象——旧 Studio 会丢字段）。
  function structuredClickTarget(root, step) {
    const box = formBox('定位对象（find[0]；多定位器/内联 ROI 用下方高级 JSON；expect/requires 的 OCR 文字 text_expr 也在高级 JSON 里写）');
    const find = Array.isArray(step.args.find) ? step.args.find : [];
    const base = find[0] && typeof find[0] === 'object' && !Array.isArray(find[0])
      ? find[0]
      : { kind: 'template' };
    const loc = Object.assign({ kind: 'template' }, base);
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

    const params = paramPanelRows(B.params);
    let modeSel = null;
    let fixedInput = null;
    let paramSel = null;
    let exprInput = null;

    if (kind.value === 'ocr_text') {
      const src = paramSourceFromLocator(loc);
      // A locator that already uses param(...) keeps the param source selectable
      // even when the schema failed to load / the key is no longer declared, so
      // editing another field can never silently drop text_expr.
      const keepParamSource = src.mode === 'param';
      const paramOptionsAvailable = params.length > 0 || keepParamSource;
      const currentMode = src.mode === 'param' && !paramOptionsAvailable ? 'text' : src.mode;

      const modeLab = document.createElement('label');
      modeLab.textContent = '文字来源（text / text_expr 互斥）';
      modeSel = document.createElement('select');
      for (const [value, label] of [
        ['text', '固定文字（写 text）'],
        ['param', '任务参数（写 text_expr = param(...)）'],
        ['expr', '表达式（写 text_expr）'],
      ]) {
        if (value === 'param' && !paramOptionsAvailable) continue;
        const o = document.createElement('option');
        o.value = value;
        o.textContent = label;
        modeSel.appendChild(o);
      }
      modeSel.value = currentMode;
      modeLab.appendChild(modeSel);
      box.appendChild(modeLab);
      if (!params.length && !keepParamSource) {
        const hint = document.createElement('div');
        hint.className = 'builder-source-hint';
        hint.textContent = '任务参数：先在「任务参数」面板「配置项…」声明参数，之后可选「任务参数」来源。';
        box.appendChild(hint);
      }

      if (currentMode === 'param') {
        const lab = document.createElement('label');
        lab.textContent = '任务参数 key';
        paramSel = document.createElement('select');
        const known = new Set(params.map(function (row) { return row.key; }));
        paramSel.appendChild(new Option('（选择已声明参数）', ''));
        for (const row of params) {
          paramSel.appendChild(new Option(
            (row.group || '未分组') + ' · ' + row.label + ' (' + row.key + ')',
            row.key
          ));
        }
        if (src.value && !known.has(src.value)) {
          paramSel.appendChild(new Option('未声明：' + src.value, src.value));
        }
        paramSel.value = src.value || (params[0] ? params[0].key : '');
        lab.appendChild(paramSel);
        box.appendChild(lab);
      } else if (currentMode === 'expr') {
        const lab = document.createElement('label');
        lab.textContent = '文字表达式（text_expr）';
        const exprRow = document.createElement('span');
        exprRow.className = 'builder-expr-row';
        exprInput = document.createElement('input');
        exprInput.type = 'text';
        exprInput.spellcheck = false;
        exprInput.value = src.value;
        exprRow.appendChild(exprInput);
        exprRow.appendChild(paramInsertWidget('expr', function () { return exprInput; }));
        lab.appendChild(exprRow);
        box.appendChild(lab);
        trackExprField(exprInput, 'expr');
      } else {
        const lab = document.createElement('label');
        lab.textContent = 'OCR 文字（固定字面量）';
        fixedInput = document.createElement('input');
        fixedInput.type = 'text';
        fixedInput.value = src.value;
        lab.appendChild(fixedInput);
        box.appendChild(lab);
      }

      const hint = document.createElement('div');
      hint.className = 'builder-source-hint';
      const specError = locatorTextSpecError(base);
      if (specError) {
        hint.textContent = '现有值无效：' + specError + '；改动后会按互斥规则覆盖。';
        hint.className += ' warn';
      } else if (currentMode === 'param') {
        hint.textContent = 'text_expr = ' + paramTextExpr(src.value || '') +
          ' → 运行时替换为 Runner 参数值（参数未声明时发布 T3 会报错）。';
      }
      box.appendChild(hint);
    }

    function sync() {
      const next = Object.assign({}, base, { kind: kind.value });
      if (asset.value) next.sel = asset.value;
      else delete next.sel;
      let written = next;
      if (kind.value === 'ocr_text') {
        const mode = modeSel ? modeSel.value : 'text';
        let value = '';
        if (mode === 'param') value = paramSel ? paramSel.value : '';
        else if (mode === 'expr') value = exprInput ? exprInput.value : '';
        else value = fixedInput ? fixedInput.value : '';
        written = paramSourceApply(next, mode, value);
      } else {
        // template/region locators never carry text fields; text_expr is an
        // ocr_text-only field.
        delete next.text;
        delete next.text_expr;
      }
      step.args.find = [written].concat(find.slice(1));
      renderAll();
    }
    kind.addEventListener('change', sync);
    asset.addEventListener('change', sync);
    if (modeSel) {
      modeSel.addEventListener('change', function () {
        const mode = modeSel.value;
        const value = mode === 'param'
          ? (params[0] ? params[0].key : (paramSel ? paramSel.value : ''))
          : (mode === 'expr'
            ? (exprInput ? exprInput.value : '')
            : (fixedInput ? fixedInput.value : ''));
        const next = Object.assign({}, base, { kind: 'ocr_text' });
        if (asset.value) next.sel = asset.value;
        step.args.find = [paramSourceApply(next, mode, value)].concat(find.slice(1));
        renderAll();
      });
    }
    if (paramSel) paramSel.addEventListener('change', sync);
    if (exprInput) exprInput.addEventListener('change', sync);
    if (fixedInput) fixedInput.addEventListener('change', sync);
    root.appendChild(box);
  }

  function structuredDrag(root, step) {
    const box = formBox('拖动（from 区域 + 可选 shape + 方向/距离/时长；OCR 端点的 text_expr、其它 from/to 形态用下方高级 JSON）');
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
      // O-2: keep the optional inline operation shape while editing roi. The
      // shape must never be silently deleted, but the two fields have different
      // runtime roles: a polygon/invert shape is what gets sampled (roi is the
      // old-runtime bbox fallback), while a plain non-inverted rect is sampled
      // from `roi` (the shape key is ignored by the runtime).
      const shape = from.shape;
      const row = document.createElement('div');
      row.className = 'builder-inline';
      ['x', 'y', 'w', 'h'].forEach((k, i) =>
        row.appendChild(
          numberInput(k, roi[i], (v) => {
            roi[i] = v;
            step.args.from = shape
              ? { kind: 'region', roi, shape }
              : { kind: 'region', roi };
            renderAll();
          })
        )
      );
      box.appendChild(row);
      if (shape) {
        const hint = document.createElement('div');
        hint.className = 'muted small';
        const shapeSampled = shape.kind === 'polygon' || !!shape.invert;
        hint.textContent = shapeSampled
          ? '含 polygon/反选 shape：runtime 按 shape 采样（margin 忽略）；改 roi 不会改 shape，需用高级 JSON 同步'
          : '含 rect shape（非反选）：runtime 实际只使用上面的 roi（shape 不参与采样）；改 roi 即改拖动区域';
        box.appendChild(hint);
      }
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
    let m = c.match(/^appear\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\)$/);
    if (m) return { custom: false, rows: [{ kind: 'asset', neg: false, value: m[1] }], join: 'and', groupNot: false };
    m = c.match(/^(?:not\s+|!)(?:appear\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\))$/);
    if (m) return { custom: false, rows: [{ kind: 'asset', neg: true, value: m[1] }], join: 'and', groupNot: false };
    m = c.match(/^text_visible\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\)$/);
    if (m) return { custom: false, rows: [{ kind: 'text', neg: false, value: m[1] }], join: 'or', groupNot: false };
    m = c.match(/^(?:not\s+|!)\s*\(?\s*text_visible\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\)\s*\)?$/);
    if (m) return { custom: false, rows: [{ kind: 'text', neg: true, value: m[1] }], join: 'or', groupNot: false };
    // P3: 文字（参数）行 — text_visible(param('k')) 与 not (...) 两种。
    m = c.match(/^text_visible\(\s*param\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\)\s*\)$/);
    if (m) return { custom: false, rows: [{ kind: 'param', neg: false, value: m[1] }], join: 'or', groupNot: false };
    m = c.match(/^(?:not\s+|!)\s*\(?\s*text_visible\(\s*param\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\)\s*\)\s*\)?$/);
    if (m) return { custom: false, rows: [{ kind: 'param', neg: true, value: m[1] }], join: 'or', groupNot: false };
    if (!c) return { custom: false, rows: [], join: 'and', groupNot: false };
    return { custom: true, rows: [], join: 'and', groupNot: false, raw: c };
  }

  function conditionRowExpr(row) {
    const v = String(row.value || '').trim();
    if (!v) return '';
    let expr;
    if (row.kind === 'asset') {
      expr = `appear('${v.replace(/'/g, '')}')`;
    } else if (row.kind === 'param') {
      // P3: 参数文字条件（运行时 text_visible 实参已是表达式）
      expr = paramWaitCond(v);
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
    title.textContent = '循环/分支条件（谓词行 + and/or/not、&&/||/!、括号；「文字（参数）」行 = text_visible(param(...))）';
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
    const customRow = document.createElement('span');
    customRow.className = 'builder-expr-row';
    const custom = document.createElement('input');
    custom.type = 'text';
    custom.value = preset.raw || step.args.cond || 'true';
    customRow.appendChild(custom);
    customRow.appendChild(paramInsertWidget('cond', function () { return custom; }));
    customLabel.appendChild(customRow);
    box.appendChild(customLabel);
    trackExprField(custom, 'cond');

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
        for (const [v, label] of [['asset', '素材'], ['text', '文字（全屏）'], ['param', '文字（参数）']]) {
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
        } else if (row.kind === 'param') {
          // P3: 参数行 → text_visible(param('key')); 列表来自任务参数面板。
          const declared = paramPanelRows(B.params);
          valueEl = document.createElement('select');
          valueEl.appendChild(new Option('（选择已声明参数）', ''));
          const known = new Set(declared.map((p) => p.key));
          for (const p of declared) {
            valueEl.appendChild(new Option((p.group || '未分组') + ' · ' + p.key, p.key));
          }
          if (row.value && !known.has(row.value)) {
            valueEl.appendChild(new Option('未声明：' + row.value, row.value));
          }
          valueEl.value = row.value || (declared[0] ? declared[0].key : '');
          valueEl.addEventListener('change', () => { row.value = valueEl.value; sync(); });
          line.append(kind, negLabel, valueEl);
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

  function renderBuilderRelationHint(root, step) {
    if (!B.relations || !step || !step.args) return;
    const asset = step.args.sel;
    if (!asset) return;
    const rel = B.relations;
    const frames = (rel.frames || []).filter((f) => (f.assets || []).includes(asset));
    const transitions = (rel.transitions || []).filter((t) => t.asset === asset);
    const annotations = (rel.annotations || []).filter((a) => {
      const hay = `${a.a || ''} ${a.b || ''}`;
      return hay.includes(`/${asset}`) || hay.endsWith(`:${asset}`);
    });
    if (!frames.length && !transitions.length && !annotations.length) return;
    const box = document.createElement('div');
    box.className = 'builder-relation-hint';
    const h = document.createElement('div');
    h.className = 'builder-relation-title';
    h.textContent = '关系提示（只读）';
    box.appendChild(h);
    if (frames.length) {
      const d = document.createElement('div');
      d.textContent = `该素材出现在帧：${frames.map((f) => `#${f.id} ${f.name || ''}`).join('、')}`;
      box.appendChild(d);
    }
    for (const t of transitions.slice(0, 8)) {
      const d = document.createElement('div');
      d.textContent = `录制 transition：#${t.from_frame} → #${t.to_frame}${t.task ? ' · ' + t.task : ''}`;
      box.appendChild(d);
    }
    for (const a of annotations.slice(0, 8)) {
      const d = document.createElement('div');
      d.textContent = `标注 ${a.type}：${a.a} ↔ ${a.b}${a.note ? ' · ' + a.note : ''}`;
      box.appendChild(d);
    }
    root.appendChild(box);
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
      hint.textContent = '表达式值支持 and/or/not、&&/||/!、括号、appear/ocr/param/var；数值字段按数字求值。「插入参数 ▾」把已声明参数写成 param(...)。';
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
        // P3: expression fields get「插入参数 ▾」(param()/text_visible(param())).
        const exprMode = EXPR_FIELDS[step.op] && EXPR_FIELDS[step.op][f.key];
        if (exprMode) {
          const exprRow = document.createElement('span');
          exprRow.className = 'builder-expr-row';
          exprRow.appendChild(input);
          exprRow.appendChild(paramInsertWidget(exprMode, function () { return input; }));
          trackExprField(input, exprMode);
          lab.appendChild(exprRow);
          root.appendChild(lab);
          continue;
        }
      }
      lab.appendChild(input);
      root.appendChild(lab);
    }
    if (step.op === 'click_target') structuredClickTarget(root, step);
    if (step.op === 'click') renderClickAlts(root, step);
    if (step.op === 'drag') structuredDrag(root, step);
    renderBuilderRelationHint(root, step);
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

  function cloneJson(v) {
    return v == null ? null : JSON.parse(JSON.stringify(v));
  }

  // Snapshot only when the FLOW CONTENT changed. Selecting a node or scrolling
  // the graph must not create an undo step; editing args/adding/deleting/moving
  // a step must.
  function recordFlowHistory() {
    if (!B.flow || B.restoringHistory) return;
    const key = JSON.stringify(B.flow);
    if (B.historyIndex >= 0 && B.flowHistory[B.historyIndex].key === key) return;
    B.flowHistory = B.flowHistory.slice(0, B.historyIndex + 1);
    B.flowHistory.push({
      key,
      flow: cloneJson(B.flow),
      selected: cloneJson(B.selected),
    });
    if (B.flowHistory.length > 100) {
      B.flowHistory.shift();
    }
    B.historyIndex = B.flowHistory.length - 1;
  }

  function updateHistoryButtons() {
    const undo = $('builder-back');
    const redo = $('builder-forward');
    if (undo) {
      undo.disabled = B.historyIndex <= 0;
      undo.title = '撤销上一次流程编辑（Ctrl+Z）';
    }
    if (redo) {
      redo.disabled = B.historyIndex < 0 || B.historyIndex >= B.flowHistory.length - 1;
      redo.title = '重做被撤销的流程编辑（Ctrl+Shift+Z）';
    }
  }

  function restoreHistory(index, label) {
    const item = B.flowHistory[index];
    if (!item) return;
    B.historyIndex = index;
    B.restoringHistory = true;
    B.flow = cloneJson(item.flow);
    B.selected = cloneJson(item.selected);
    renderAll();
    B.restoringHistory = false;
    updateHistoryButtons();
    status(`${label}（${B.historyIndex + 1}/${B.flowHistory.length}）`);
  }

  function undoEdit() {
    if (B.historyIndex <= 0) return;
    restoreHistory(B.historyIndex - 1, '已撤销');
  }

  function redoEdit() {
    if (B.historyIndex < 0 || B.historyIndex >= B.flowHistory.length - 1) return;
    restoreHistory(B.historyIndex + 1, '已重做');
  }

  function renderAll() {
    recordFlowHistory();
    renderSteps();
    renderGraph();
    renderForm();
    renderAssetPanel();
    renderParamsPanel();
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

  // P3: task params schema (Studio-only GET /task/config). app.js is loaded AFTER
  // builder.js, so the bridge is read at call time (window.OAS_TASK_CONFIG).
  async function fetchTaskParams(name) {
    const task = String(name === undefined || name === null ? '' : name).trim();
    if (!task) return [];
    const hook = window.OAS_TASK_CONFIG;
    if (hook && typeof hook.refresh === 'function') {
      const params = await hook.refresh(task);
      return Array.isArray(params) ? params : [];
    }
    const r = await fetch('/task/config?task=' + encodeURIComponent(task));
    const s = await r.json();
    if (!s.ok) throw new Error((s.errors || []).join('; ') || '配置项读取失败');
    return Array.isArray(s.params) ? s.params : [];
  }

  function openTaskParamsDialog(onSaved) {
    if (!B.task) {
      status('先选择任务，再编辑任务参数', '#ff8a80');
      return;
    }
    const hook = window.OAS_TASK_CONFIG;
    if (!hook || typeof hook.open !== 'function') {
      status('任务配置项对话框未就绪（app.js 未加载）', '#ff8a80');
      return;
    }
    const savedCallback = typeof onSaved === 'function' ? onSaved : null;
    hook.open(B.task, {
      // Reuse the existing #task-config-dialog; after a successful save refresh
      // the panel (meta.params changed) without reloading flow.json.
      onSaved: function (savedTask, params) {
        if (String(savedTask || '') !== B.task) return;
        B.params = Array.isArray(params) ? params : [];
        B.paramsError = '';
        renderAll();
        status('任务参数已更新：' + paramPanelSummary(B.params), '#7bd88f');
        // meta.params changed -> the authoritative T3 result is stale: re-check.
        lintTask({ silent: true });
        if (savedCallback) {
          try { savedCallback(savedTask, params); } catch (e) { /* never break the save path */ }
        }
      },
    });
  }

  async function loadTask(name) {
    B.autoSaveArmed = false;
    B.flowHistory = [];
    B.historyIndex = -1;
    clearLintState();
    if (!name) {
      B.task = '';
      B.flow = null;
      B.assets = [];
      B.relations = null;
      B.selected = null;
      B.params = [];
      B.paramsError = '';
      B.paramsLoading = false;
      renderAll();
      renderLintPanel();
      return;
    }
    B.task = name;
    B.paramsLoading = true;
    B.paramsError = '';
    status('载入中…');
    try {
      // P3: meta.params loads in parallel with flow/assets; a params failure must
      // never block flow editing (panel shows the error instead).
      const paramsPromise = fetchTaskParams(name)
        .then(function (params) { return { ok: true, params: params }; })
        .catch(function (e) { return { ok: false, error: String((e && e.message) || e) }; });
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
        B.assets.push({ id: c.id, kind: 'template_set', members: c.members || [], action_shape: c.action_shape, action_source: c.action_source });
      }
      const pr = await paramsPromise;
      B.params = pr.ok ? pr.params : [];
      B.paramsError = pr.ok ? '' : pr.error;
      B.paramsLoading = false;
      B.selected = null;
      B.lastSavedFlow = JSON.stringify(B.flow);
      try {
        const rr = await fetch('/relations/frame-assets?task=' + encodeURIComponent(name));
        const rs = await rr.json();
        B.relations = rs.ok ? rs.relations : null;
      } catch (e) {
        B.relations = null;
      }
      renderAll();
      B.autoSaveArmed = true;
      status(`已载入 ${name}：${B.flow.steps.length} 个根动作 / ${B.assets.length} 个素材 / ${paramPanelRows(B.params).length} 项参数`);
      // P5: run the authoritative lint once per load; failures stay silent and
      // never block flow editing (the panel explains 权威检查不可用).
      lintTask({ silent: true });
    } catch (e) {
      B.paramsLoading = false;
      status('载入失败：' + e, '#ff8a80');
      renderParamsPanel();
      renderLintPanel();
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
    // P3: 保存前本地校验「文字来源」互斥（结构化编辑器不会双写；高级 JSON 手改
    // 可写出 text + text_expr 同时存在，这里拦住，避免把坏 flow 写盘）。
    const specIssues = flowTextSpecIssues(B.flow);
    if (specIssues.length) {
      status('保存已阻止：' + specIssues[0] + '（text / text_expr 互斥且须有其一）', '#ff8a80');
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
      // P5: flow.json changed -> the last authoritative check is stale; the
      // user can re-run it with the head「检查」button (no auto spam on autosave).
      if (B.lint || B.lintError) {
        B.lintStale = true;
        renderLintPanel();
      }
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
    if (back) back.addEventListener('click', undoEdit);
    const forward = $('builder-forward');
    if (forward) forward.addEventListener('click', redoEdit);
    document.addEventListener('keydown', (e) => {
      if (!(e.ctrlKey || e.metaKey) || String(e.key).toLowerCase() !== 'z') return;
      const page = $('page-builder');
      if (!page || !page.classList.contains('active')) return;
      const t = e.target;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
      e.preventDefault();
      if (e.shiftKey) redoEdit();
      else undoEdit();
    });
    const save = $('builder-save');
    if (save) save.addEventListener('click', () => saveFlow(false));
    const dry = $('builder-dry-run');
    if (dry) dry.addEventListener('click', dryRun);
    const paramsDeclare = $('builder-params-declare');
    if (paramsDeclare) paramsDeclare.addEventListener('click', function () { openTaskParamsDialog(); });
    const lintBtn = $('builder-lint-btn');
    if (lintBtn) lintBtn.addEventListener('click', function () { lintTask(); });
    const lintClose = $('builder-lint-close');
    if (lintClose) lintClose.addEventListener('click', closeLintPanel);
    const codeApply = $('builder-code-apply');
    if (codeApply) codeApply.addEventListener('click', applyCode);
    const delChecked = $('builder-delete-checked');
    if (delChecked) delChecked.addEventListener('click', deleteCheckedSteps);
    renderPalette();
    renderAll();
    renderLintPanel();
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
