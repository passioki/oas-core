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
    long_press: { label: '长按', group: '动作' },
    curve_swipe: { label: '曲线滑动（多段近似）', group: '动作' },
    dismiss_popup: { label: '关闭弹窗', group: '动作' },
    swipe: { label: '滑动', group: '动作' },
    drag: { label: '拖动（拟人）', group: '动作' },
    navigate: { label: '导航到页面', group: '动作' },
    key: { label: '发送按键', group: '动作' },
    restart_app: { label: '重启 App', group: '动作' },
    set_var: { label: '设置变量', group: '数据/结束' },
    inc_var: { label: '变量自增', group: '数据/结束' },
    ocr_to_var: { label: 'OCR 结果写入变量', group: '数据/结束' },
    log: { label: '日志', group: '数据/结束' },
    finish: { label: '结束', group: '数据/结束' },
  };
  const GROUP_ORDER = ['流程控制', '识别/等待', '动作', '数据/结束'];
  // Explicit palette titles for actions whose semantics are easy to misread
  // (blocking wait / long press / multi-segment curve).
  const ACTION_TITLES = {
    wait_until: '阻塞等待：条件成立后继续执行下一个 flow 步骤；超时本步失败；不会触发独立动作',
    click: '点击素材：选择素材（识别+操作区域）后点击其操作区域中心；请在画布/素材行维护素材操作区',
    click_point: '点击坐标（坐标手势，低层）：x/y 直接点击；点击元素请用「点击素材」',
    click_target: '定位点击：先选目标素材（识别门槛；动作落点=目标素材自己的操作区）',
    long_press: '长按：选择目标素材后按住其操作区域 hold_ms 毫秒（默认 3000）；可启用 hold_range 随机拟人',
    dismiss_popup: '关闭弹窗：选择弹窗素材后点击其操作区域；可选按钮文字/超时',
    swipe: '滑动（坐标手势，低层）：from/to 坐标；元素拖动请用「拖动（拟人）」素材端点',
    curve_swipe: '曲线滑动（坐标手势，低层）：points 逐段坐标；元素拖动请用「拖动（拟人）」素材端点',
    drag: '拖动（拟人）：选择起点素材/可选终点素材，方向+距离或终点素材；坐标端点已停用（旧 flow 只读）',
    navigate: '导航到页面：按全局地图规划路径并执行（目标为地图节点；dry-run 只预览不点击）',
  };

  // Single source of truth for "anything the user can add as a step".
  // The palette and the if/loop branch dropdowns (是/否/循环体) must all use
  // this catalogue. 参数不属于动作库（用户 2026-11-06 事项 1/2）：参数的引用
  // 留在 flow 表达式（text_expr / 条件）里，动作库不再提供参数预设。
  function actionCatalog() {
    return Object.entries(OP_META).map(function (entry) {
      return { key: entry[0], label: entry[1].label, group: entry[1].group };
    });
  }

  function actionMetaOf(key) {
    return OP_META[key] || null;
  }

  // Async path used by the if/loop branch insertion: every catalogue entry is a
  // real op, so it expands to makeStep(). Kept async because the upcoming
  // 动作→目标素材/操作区 batch will ask for the target here.
  async function prepareActionStep(key) {
    return OP_META[key] ? makeStep(key) : null;
  }
  // Generic FIELDS text inputs that hold expressions; they get the「插入变量 ▾」
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
    wait_until: [{ key: 'timeout_s', label: '超时秒', type: 'number' }],
    appear: [{ key: 'sel', label: '素材（识别+操作区域）', type: 'asset' }],
    ocr: [{ key: 'sel', label: 'OCR 素材', type: 'asset' }],
    click: [{ key: 'sel', label: '素材（识别+操作区域）', type: 'asset' }],
    click_point: [{ key: 'x', label: 'x（表达式）', type: 'text' }, { key: 'y', label: 'y（表达式）', type: 'text' }],
    dismiss_popup: [{ key: 'sel', label: '弹窗素材', type: 'asset' }, { key: 'action', label: '按钮文字（可选，素材库 popup 动作名）', type: 'text' }, { key: 'timeout_ms', label: '超时 ms', type: 'number' }],
    swipe: [{ key: 'from', label: '起点 [x,y]（低层坐标）', type: 'json' }, { key: 'to', label: '终点 [x,y]（低层坐标）', type: 'json' }, { key: 'ms', label: '时长 ms', type: 'number' }],
    key: [{ key: 'keycode', label: 'Android keycode（BACK=4 / HOME=3）', type: 'number' }],
    restart_app: [{ key: 'package', label: '包名', type: 'text' }],
    finish: [{ key: 'exit', label: '出口', type: 'select', options: ['done', 'failed'] }],
    if: [],
    loop: [{ key: 'n', label: '次数（<=0 不循环；>0 循环次数）', type: 'text' }],
    while: [],
    click_target: [],
    long_press: [],
    curve_swipe: [],
    drag: [],
    navigate: [{ key: 'target', label: '目标页面（全局地图节点）', type: 'node' }],
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
    long_press: { find: [{ kind: 'template', sel: 'I_A' }], hold_ms: 3000 },
    curve_swipe: {
      points: [[500, 1200], [520, 900], [560, 600], [620, 300]],
      duration_ms: [600, 1000],
    },
    drag: {
      from: { kind: 'region', roi: [100, 400, 300, 200] },
      direction: 'up',
      distance: [150, 450],
      duration_ms: [300, 650],
      margin: 0.15,
    },
    navigate: { target: '' },
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
    // P3（参数 v2）：meta.params schema (Studio-only read; the Runner owns values).
    params: [],
    paramsLoading: false,
    paramsError: '',
    // V2.0c：meta.variables 是 flow param('key') 的唯一声明来源（GET/POST /task/variables）。
    variables: [],
    variablesLoading: false,
    variablesError: '',
    variablesIssues: [],
    // seq136（D1-M3）：全局地图节点（navigate target 下拉 + 保存前校验）。
    mapNodes: [],
    mapLoaded: false,
    mapError: '',
    // P5：权威 lint（GET /task/lint）。lint=null 表示尚未检查/检查失败。
    lint: null,
    lintError: '',
    lintRunning: false,
    lintSeq: 0,
    lintStale: false,
    // P1: design resolution from /task/assets (curve_swipe bounds / validation).
    screen: [1280, 720],
    lastExprField: null,
    lastExprMode: 'expr',
    autoSaveArmed: false,
    autoSaveTimer: null,
    lastSavedFlow: '',
    saving: false,
    // seq136（C2 #8）：可编辑源码框 dirty 状态（手动编辑后构建器不覆盖）。
    codeDirty: false,
    codeLastSynced: '',
    // seq137（UI B #1）：列表拖拽内存态（统一 payload；dragover 计算插入位）。
    dragState: null,
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

  // seq130（#3）：param(key) 是否已声明为 V2.0 变量/识别文字变量（meta.variables
  // 或素材库 ocr_var.param_key）。已声明 = 正常变量来源；未声明才是「旧参数只读」。
  function declaredParamKeySet(variables, assets) {
    const set = {};
    for (const v of Array.isArray(variables) ? variables : []) {
      const key = paramSafeKey(v && v.key);
      if (key) set[key] = true;
    }
    for (const a of Array.isArray(assets) ? assets : []) {
      if (!a || String(a.kind) !== 'ocr_var') continue;
      const key = paramSafeKey(a.param_key);
      if (key) set[key] = true;
    }
    return set;
  }

  function paramKeyDeclaredIn(key, variables, assets) {
    const k = paramSafeKey(key);
    return !!k && !!declaredParamKeySet(variables, assets)[k];
  }

  // 表达式文本里的 param('key') 引用（声明状态提示用）。
  function paramRefKeysInText(text) {
    const out = [];
    const s = String(text === undefined || text === null ? '' : text);
    const re = /param\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\)/g;
    let m;
    while ((m = re.exec(s))) {
      const key = paramSafeKey(m[1]);
      if (key && out.indexOf(key) < 0) out.push(key);
    }
    return out;
  }

  function undeclaredParamKeysInText(text, variables, assets) {
    const declared = declaredParamKeySet(variables, assets);
    return paramRefKeysInText(text).filter(function (key) { return !declared[key]; });
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

  // --- P1（操作区域 / 长按 / 曲线滑动 / 等待解释）纯函数 -----------------------
  // region 可挂在 OCR locator 上（click_target.find / drag.from|to / long_press.find）：
  // {mode:"asset|ocr|nearest|offset", offset:[dx,dy,(w,h)]}；无 region = 现状。
  // 这里只做「数据 → flow/文案/校验」，不碰 DOM。

  const REGION_MODES = ['asset', 'ocr', 'nearest', 'offset'];

  function regionModeText(mode) {
    switch (String(mode === undefined || mode === null ? '' : mode).trim().toLowerCase()) {
      case '': return '默认（跟随素材配置）';
      case 'asset': return '素材操作区域';
      case 'ocr': return 'OCR 识别区域';
      case 'nearest': return '最近匹配（离识别 ROI 中心最近）';
      case 'offset': return '相对偏移';
      default: return '未知：' + String(mode);
    }
  }

  function regionHintText(mode) {
    switch (String(mode === undefined || mode === null ? '' : mode).trim().toLowerCase()) {
      case 'asset': return '使用素材操作区域（action_shape / click_back）；素材没配时回退 OCR 行 bbox';
      case 'ocr': return '强制使用本次 OCR 匹配行的 bbox（忽略素材操作区域）';
      case 'nearest': return '多行匹配时选离「OCR 识别 ROI」中心最近的一行';
      case 'offset': return '相对匹配/识别 bbox 左上角偏移：[dx,dy] 或 [dx,dy,w,h]（w/h 留空=继承 bbox 尺寸）';
      default: return '不指定 = 沿用现状：素材有操作区域用素材，否则用 OCR 行 bbox';
    }
  }

  // 操作区域素材（A 方案）：全库任意资产可作为操作区域来源；仅 asset/nearest
  // 模式使用；无匹配候选时引擎本步失败。
  function regionMatchRiskText(mode) {
    const m = String(mode === undefined || mode === null ? '' : mode).trim().toLowerCase();
    if (m === 'asset' || m === 'nearest') {
      return '操作区域素材无匹配时本步会失败；未选素材 = 用识别素材自身。';
    }
    return '';
  }

  function regionAssetKindText(kind) {
    switch (String(kind === undefined || kind === null ? '' : kind).trim().toLowerCase()) {
      case 'ocr_line': return 'OCR 行';
      case 'ocr_lines': return 'OCR 多行';
      case 'template': return '模板';
      case 'region': return '区域';
      case 'template_set': return '集合';
      default: return '其他';
    }
  }

  function regionAssetGroupOf(kind) {
    const k = String(kind === undefined || kind === null ? '' : kind).trim().toLowerCase();
    if (k === 'ocr_line' || k === 'ocr_lines') return 'ocr';
    if (k === 'template') return 'template';
    if (k === 'region') return 'region';
    if (k === 'template_set') return 'collection';
    return 'other';
  }

  // 全库「操作区域素材」下拉数据（chooser + 结构化 region 编辑器共用）：
  // 按 OCR / 模板 / 区域 / 集合 / 其他 分组；同一 id 只出现一次。
  function chooserOperationAssetOptions(assets) {
    const order = ['ocr', 'template', 'region', 'collection', 'other'];
    const labels = {
      ocr: 'OCR 识别素材',
      template: '模板素材',
      region: '区域素材',
      collection: '集合素材',
      other: '其他素材',
    };
    const buckets = { ocr: [], template: [], region: [], collection: [], other: [] };
    const seen = {};
    for (const asset of Array.isArray(assets) ? assets : []) {
      if (!asset || typeof asset !== 'object') continue;
      const id = String(asset.id === undefined || asset.id === null ? '' : asset.id).trim();
      if (!id || Object.prototype.hasOwnProperty.call(seen, id)) continue;
      seen[id] = true;
      const kind = String(asset.kind === undefined || asset.kind === null ? '' : asset.kind).trim();
      buckets[regionAssetGroupOf(kind)].push({
        id: id,
        kind: kind,
        kindText: regionAssetKindText(kind),
      });
    }
    return order
      .filter(function (group) { return buckets[group].length > 0; })
      .map(function (group) {
        return { group: group, label: labels[group], items: buckets[group] };
      });
  }

  function regionAssetFromSelector(value) {
    return String(value === undefined || value === null ? '' : value).trim();
  }

  function regionOperationAssetText(assetId, assets) {
    const id = regionAssetFromSelector(assetId);
    if (!id) return '无（用识别素材自身）';
    for (const asset of Array.isArray(assets) ? assets : []) {
      if (!asset || typeof asset !== 'object') continue;
      const assetId2 = String(asset.id === undefined || asset.id === null ? '' : asset.id).trim();
      if (assetId2 !== id) continue;
      return id + ' · ' + regionAssetKindText(asset.kind);
    }
    return '未找到：' + id;
  }

  function regionAssetIds(assets) {
    const ids = [];
    for (const asset of Array.isArray(assets) ? assets : []) {
      if (!asset || typeof asset !== 'object') continue;
      const id = String(asset.id === undefined || asset.id === null ? '' : asset.id).trim();
      if (id && ids.indexOf(id) < 0) ids.push(id);
    }
    return ids;
  }

  // 把操作区域素材并进 region（仅 asset/nearest 有 asset；ocr/offset 丢弃 asset）。
  function regionWithOperationAsset(region, asset) {
    const r = region && typeof region === 'object' && !Array.isArray(region) ? region : {};
    const mode = String(r.mode === undefined || r.mode === null ? '' : r.mode).trim().toLowerCase();
    const a = regionAssetFromSelector(asset);
    const out = {};
    if (mode === 'offset') {
      out.mode = 'offset';
      if (Array.isArray(r.offset)) out.offset = r.offset.slice();
    } else if (mode === 'asset' || mode === 'nearest') {
      out.mode = mode;
      if (a) out.asset = a;
    } else if (mode) {
      out.mode = mode;
    }
    return out;
  }

  function regionSpec(locator) {
    const loc = locator && typeof locator === 'object' && !Array.isArray(locator) ? locator : {};
    const region = loc.region;
    if (!region || typeof region !== 'object' || Array.isArray(region)) {
      return { mode: '', offset: [], asset: '' };
    }
    const mode = String(region.mode === undefined || region.mode === null ? '' : region.mode)
      .trim().toLowerCase();
    const offset = Array.isArray(region.offset) ? region.offset.slice() : [];
    const asset = typeof region.asset === 'string' ? region.asset.trim() : '';
    return { mode: mode, offset: offset, asset: asset };
  }

  // 写回 locator 的 region（保留未知字段）：mode 空 = 删除 region；非 offset 模式
  // 删除 offset；offset 模式写入 offset 数组；asset 仅 asset/nearest 模式写入。
  // region 内未知字段（未来扩展）也在切换模式时保留。
  function regionWrite(locator, mode, offset, asset) {
    const out = Object.assign({}, locator && typeof locator === 'object' ? locator : {});
    const m = String(mode === undefined || mode === null ? '' : mode).trim().toLowerCase();
    const prevRegion = out.region && typeof out.region === 'object' && !Array.isArray(out.region)
      ? out.region
      : {};
    const unknownRegion = {};
    for (const key of Object.keys(prevRegion)) {
      if (key === 'mode' || key === 'asset' || key === 'offset') continue;
      unknownRegion[key] = prevRegion[key];
    }
    if (!m) {
      delete out.region;
      return out;
    }
    const a = regionAssetFromSelector(asset);
    let region;
    if (m === 'offset') {
      region = { mode: 'offset', offset: Array.isArray(offset) ? offset.slice() : [] };
    } else if (m === 'asset' || m === 'nearest') {
      region = { mode: m };
      if (a) region.asset = a;
    } else {
      region = { mode: m };
    }
    out.region = Object.assign({}, unknownRegion, region);
    return out;
  }

  // 相对偏移四个输入 → 2/4 整数数组（w/h 同时留空=继承 bbox 尺寸）。
  function regionOffsetFromInput(dxText, dyText, wText, hText) {
    const text = function (v) { return String(v === undefined || v === null ? '' : v).trim(); };
    const intOf = function (raw) {
      if (!raw) return null;
      const n = Number(raw);
      if (!isFinite(n) || !Number.isInteger(n)) return NaN;
      return n;
    };
    const dx = intOf(text(dxText));
    const dy = intOf(text(dyText));
    if (dx === null || dy === null) {
      return { ok: false, offset: [], error: 'offset 的 dx/dy 必填（整数，可为负）' };
    }
    if (isNaN(dx) || isNaN(dy)) {
      return { ok: false, offset: [], error: 'offset 的 dx/dy 必须是整数' };
    }
    const wText2 = text(wText);
    const hText2 = text(hText);
    if (!wText2 && !hText2) return { ok: true, offset: [dx, dy], error: '' };
    if (!wText2 || !hText2) {
      return { ok: false, offset: [], error: 'offset 的 w/h 必须同时填写（继承 bbox 尺寸时都留空）' };
    }
    const w = intOf(wText2);
    const h = intOf(hText2);
    if (w === null || h === null || isNaN(w) || isNaN(h)) {
      return { ok: false, offset: [], error: 'offset 的 w/h 必须是整数' };
    }
    if (w <= 0 || h <= 0) {
      return { ok: false, offset: [], error: 'offset 的 w/h 必须大于 0' };
    }
    return { ok: true, offset: [dx, dy, w, h], error: '' };
  }

  function normalizeRegion(region) {
    const r = region && typeof region === 'object' && !Array.isArray(region) ? region : null;
    if (!r) return { ok: true, region: undefined, error: '' };
    const mode = String(r.mode === undefined || r.mode === null ? '' : r.mode).trim().toLowerCase();
    if (REGION_MODES.indexOf(mode) < 0) {
      return {
        ok: false, region: null,
        error: 'region.mode 不合法：「' + (mode || '（空）') + '」（只能是 asset/ocr/nearest/offset）',
      };
    }
    let asset = '';
    if (r.asset !== undefined && r.asset !== null) {
      if (typeof r.asset !== 'string') {
        return { ok: false, region: null, error: 'region.asset 必须是字符串（素材 id）' };
      }
      asset = r.asset.trim();
      if (!asset) {
        return { ok: false, region: null, error: 'region.asset 必须是非空字符串（不需要时省略该字段）' };
      }
      if (mode !== 'asset' && mode !== 'nearest') {
        return {
          ok: false, region: null,
          error: 'region.asset 只能与 mode=asset/nearest 组合（当前 ' + mode + '）',
        };
      }
    }
    const hasOffset = r.offset !== undefined && r.offset !== null;
    if (!hasOffset) {
      if (mode === 'offset') {
        return { ok: false, region: null, error: 'region.mode=offset 需要 offset 数组（2 或 4 个整数）' };
      }
      const normalized = { mode: mode };
      if (asset) normalized.asset = asset;
      return { ok: true, region: normalized, error: '' };
    }
    if (!Array.isArray(r.offset)) {
      return { ok: false, region: null, error: 'region.offset 必须是数组' };
    }
    if (r.offset.length !== 2 && r.offset.length !== 4) {
      return { ok: false, region: null, error: 'region.offset 必须是 2 或 4 个整数' };
    }
    for (const value of r.offset) {
      if (typeof value !== 'number' || !isFinite(value) || !Number.isInteger(value)) {
        return { ok: false, region: null, error: 'region.offset 必须是 2 或 4 个整数' };
      }
    }
    if (r.offset.length === 4 && (r.offset[2] <= 0 || r.offset[3] <= 0)) {
      return { ok: false, region: null, error: 'region.offset 的 w/h 必须大于 0' };
    }
    if (mode !== 'offset') {
      return { ok: false, region: null, error: '只有 region.mode=offset 支持 offset（当前 ' + mode + '）' };
    }
    return { ok: true, region: { mode: 'offset', offset: r.offset.slice() }, error: '' };
  }

  // 保存前扫描 flow 里所有带 region 的节点（click_target/drag/long_press locator）。
  // assetIds 可选：传入当前素材库 id 列表时，region.asset 不存在也会报错（阻止保存）。
  function regionIssues(flow, assetIds) {
    const known = Array.isArray(assetIds)
      ? assetIds.map(function (id) { return String(id); })
      : null;
    const issues = [];
    function walk(node, path) {
      if (node === null || node === undefined) return;
      if (Array.isArray(node)) {
        node.forEach(function (child, i) { walk(child, path + '[' + i + ']'); });
        return;
      }
      if (typeof node !== 'object') return;
      if (Object.prototype.hasOwnProperty.call(node, 'region')) {
        const norm = normalizeRegion(node.region);
        if (!norm.ok) {
          issues.push(path + '.region：' + norm.error);
        } else if (norm.region && norm.region.asset && known && known.indexOf(norm.region.asset) < 0) {
          issues.push(path + '.region：操作区域素材「' + norm.region.asset + '」不在当前素材库（本步会失败）');
        }
      }
      for (const key of Object.keys(node)) walk(node[key], path ? path + '.' + key : key);
    }
    walk({ steps: flow && flow.steps }, 'flow');
    return issues;
  }

  function normalizeScreenSize(value) {
    if (Array.isArray(value) && value.length >= 2) {
      const w = Number(value[0]);
      const h = Number(value[1]);
      if (isFinite(w) && isFinite(h) && w > 0 && h > 0) {
        return [Math.floor(w), Math.floor(h)];
      }
    }
    return [1280, 720];
  }

  function longPressHoldText(args) {
    const a = args && typeof args === 'object' ? args : {};
    const range = Array.isArray(a.hold_range) && a.hold_range.length === 2 ? a.hold_range : null;
    if (range) return 'hold_range ' + range[0] + '~' + range[1] + 'ms（随机拟人）';
    const hold = a.hold_ms === undefined || a.hold_ms === null ? 3000 : a.hold_ms;
    return 'hold_ms ' + hold + 'ms';
  }

  function holdRangeFromInput(minText, maxText) {
    const text = function (v) { return String(v === undefined || v === null ? '' : v).trim(); };
    const minS = text(minText);
    const maxS = text(maxText);
    if (!minS && !maxS) return { ok: true, range: null, error: '' };
    if (!minS || !maxS) {
      return { ok: false, range: null, error: 'hold_range 的最小/最大必须同时填写（不用随机时取消勾选）' };
    }
    const min = Number(minS);
    const max = Number(maxS);
    if (!isFinite(min) || !isFinite(max) || !Number.isInteger(min) || !Number.isInteger(max)) {
      return { ok: false, range: null, error: 'hold_range 必须是整数毫秒' };
    }
    if (min < 100 || max > 10000) {
      return { ok: false, range: null, error: 'hold_range 必须在 100~10000ms' };
    }
    if (min > max) return { ok: false, range: null, error: 'hold_range 最小值不能大于最大值' };
    return { ok: true, range: [min, max], error: '' };
  }

  function longPressArgsIssues(step) {
    const s = step && typeof step === 'object' ? step : {};
    const a = s.args && typeof s.args === 'object' ? s.args : {};
    const errors = [];
    if (!Array.isArray(a.find) || !a.find.length) errors.push('find 至少需要一个定位器');
    if (a.hold_ms !== undefined && a.hold_ms !== null) {
      const hold = a.hold_ms;
      if (typeof hold !== 'number' || !isFinite(hold) || !Number.isInteger(hold) ||
          hold < 100 || hold > 10000) {
        errors.push('hold_ms 必须是 100~10000 的整数（默认 3000）');
      }
    }
    if (a.hold_range !== undefined && a.hold_range !== null) {
      const range = Array.isArray(a.hold_range) ? a.hold_range : null;
      if (!range || range.length !== 2 ||
          range.some(function (v) { return typeof v !== 'number' || !Number.isInteger(v); })) {
        errors.push('hold_range 必须是 [min,max] 整数数组');
      } else {
        if (range[0] < 100 || range[1] > 10000) errors.push('hold_range 必须在 100~10000ms');
        if (range[0] > range[1]) errors.push('hold_range min 不能大于 max');
      }
    }
    return errors;
  }

  function curveSwipeStep() {
    return {
      op: 'curve_swipe',
      args: {
        points: [[500, 1200], [520, 900], [560, 600], [620, 300]],
        duration_ms: [600, 1000],
      },
    };
  }

  // points JSON 文本 → 2..=32 个屏内整数点。
  function curvePointsFromText(text, screenW, screenH) {
    const raw = String(text === undefined || text === null ? '' : text).trim();
    if (!raw) return { ok: false, points: [], error: 'points 不能为空（需要 2~32 个 [x,y]）' };
    let parsed;
    try {
      parsed = JSON.parse(raw);
    } catch (e) {
      return { ok: false, points: [], error: 'points JSON 解析失败：' + (e && e.message ? e.message : e) };
    }
    if (!Array.isArray(parsed)) {
      return { ok: false, points: [], error: 'points 必须是数组：[[x,y],[x,y],...]' };
    }
    if (parsed.length < 2 || parsed.length > 32) {
      return { ok: false, points: [], error: 'points 需要 2~32 个点（当前 ' + parsed.length + ' 个）' };
    }
    const w = Number(screenW);
    const h = Number(screenH);
    const points = [];
    for (let i = 0; i < parsed.length; i++) {
      const point = parsed[i];
      if (!Array.isArray(point) || point.length !== 2) {
        return { ok: false, points: [], error: '第 ' + (i + 1) + ' 个点必须是 [x,y]' };
      }
      const x = point[0];
      const y = point[1];
      if (typeof x !== 'number' || typeof y !== 'number' ||
          !isFinite(x) || !isFinite(y) || !Number.isInteger(x) || !Number.isInteger(y)) {
        return { ok: false, points: [], error: '第 ' + (i + 1) + ' 个点的坐标必须是整数' };
      }
      if (x < 0 || y < 0) {
        return { ok: false, points: [], error: '第 ' + (i + 1) + ' 个点坐标不能为负数' };
      }
      if (isFinite(w) && w > 0 && x > w) {
        return { ok: false, points: [], error: '第 ' + (i + 1) + ' 个点 x 超出屏幕宽度 ' + w };
      }
      if (isFinite(h) && h > 0 && y > h) {
        return { ok: false, points: [], error: '第 ' + (i + 1) + ' 个点 y 超出屏幕高度 ' + h };
      }
      points.push([x, y]);
    }
    return { ok: true, points: points, error: '' };
  }

  function curveDurationFromInput(minText, maxText) {
    const text = function (v) { return String(v === undefined || v === null ? '' : v).trim(); };
    const minS = text(minText);
    const maxS = text(maxText);
    if (!minS || !maxS) {
      return { ok: false, duration: null, error: 'duration_ms 的最小/最大都必须填写' };
    }
    const min = Number(minS);
    const max = Number(maxS);
    if (!isFinite(min) || !isFinite(max) || !Number.isInteger(min) || !Number.isInteger(max)) {
      return { ok: false, duration: null, error: 'duration_ms 必须是整数毫秒' };
    }
    if (min < 100 || max > 10000) {
      return { ok: false, duration: null, error: 'duration_ms 必须在 100~10000ms' };
    }
    if (min > max) {
      return { ok: false, duration: null, error: 'duration_ms 最小值不能大于最大值' };
    }
    return { ok: true, duration: [min, max], error: '' };
  }

  function curveSwipeArgsIssues(step, screenW, screenH) {
    const s = step && typeof step === 'object' ? step : {};
    const a = s.args && typeof s.args === 'object' ? s.args : {};
    const errors = [];
    if (!Array.isArray(a.points)) {
      errors.push('points 需要 2~32 个屏内整数点');
    } else {
      const parsed = curvePointsFromText(JSON.stringify(a.points), screenW, screenH);
      if (!parsed.ok) errors.push(parsed.error);
    }
    const duration = Array.isArray(a.duration_ms) && a.duration_ms.length === 2
      ? a.duration_ms
      : [600, 1000];
    const dur = curveDurationFromInput(String(duration[0]), String(duration[1]));
    if (!dur.ok) errors.push(dur.error);
    return errors;
  }

  // 保存前扫描 long_press / curve_swipe 的参数范围（含嵌套 then/else/body）。
  // seq136（D1-M3）：navigate target 前端校验（T11 同口径提前反馈）。
  function navigateArgsIssues(step, mapNodes, mapLoaded) {
    const s = step && typeof step === 'object' ? step : {};
    const args = s.args && typeof s.args === 'object' && !Array.isArray(s.args) ? s.args : {};
    const target = String(args.target === undefined || args.target === null ? '' : args.target).trim();
    const errors = [];
    if (!target) {
      errors.push('导航目标为空：请选择全局地图中的目标页面/节点');
      return errors;
    }
    if (mapLoaded === false) {
      errors.push('全局地图缺失：请先到「地图」页创建/保存地图（目标「' + target + '」无法校验）');
      return errors;
    }
    const ids = Array.isArray(mapNodes) ? mapNodes.map((x) => String(x)) : [];
    if (ids.indexOf(target) < 0) {
      errors.push('导航目标「' + target + '」不存在于全局地图' +
        (ids.length ? '（可选：' + ids.slice(0, 8).join('、') + (ids.length > 8 ? '…' : '') + '）' : '（地图暂无节点）'));
    }
    return errors;
  }

  function actionArgsIssues(flow, screen, mapNodes, mapLoaded) {
    const size = normalizeScreenSize(screen);
    const issues = [];
    function walk(steps, path) {
      if (!Array.isArray(steps)) return;
      steps.forEach(function (step, i) {
        if (!step || typeof step !== 'object') return;
        const at = path + '[' + i + ']';
        if (step.op === 'long_press') {
          longPressArgsIssues(step).forEach(function (error) { issues.push(at + '：' + error); });
        }
        if (step.op === 'curve_swipe') {
          curveSwipeArgsIssues(step, size[0], size[1]).forEach(function (error) {
            issues.push(at + '：' + error);
          });
        }
        if (step.op === 'navigate') {
          navigateArgsIssues(step, mapNodes, mapLoaded).forEach(function (error) {
            issues.push(at + '：' + error);
          });
        }
        for (const key of ['then', 'else', 'body']) walk(step[key], at + '.' + key);
      });
    }
    walk(flow && flow.steps, 'flow.steps');
    return issues;
  }

  // 等待类动作的统一解释（动作库 title + 属性面板 help 共用）。
  function waitBlockingHelpText() {
    return '阻塞等待：条件成立后继续执行下一个 flow 步骤（本步不会触发独立动作）；超时则本步失败（按 timeout_s 报错）。';
  }

  // 「插入变量」片段：从 meta.variables 取 key 生成 param('k')。
  function paramInsertSnippet(key, mode) {
    const expr = paramTextExpr(key);
    return mode === 'cond' ? 'text_visible(' + expr + ')' : expr;
  }

  // P7: insert a snippet at [start,end) without gluing tokens together.
  // The old implementation concatenated blindly, so inserting a parameter
  // condition into a field whose value was `true` produced
  // `truetext_visible(param('x'))` (invalid flow expression — user-reported bug).
  // - mode 'cond': when appending after a non-empty expression that does not end
  //   with an operator / open paren / comma, join with ` and `.
  // - otherwise add single spaces when either side would merge identifier tokens.
  function exprInsertText(value, start, end, snippet, mode) {
    const v = String(value === undefined || value === null ? '' : value);
    const rawSnippet = String(snippet === undefined || snippet === null ? '' : snippet);
    const s = Math.max(0, Math.min(typeof start === 'number' ? start : v.length, v.length));
    const e = Math.max(s, Math.min(typeof end === 'number' ? end : s, v.length));
    const before = v.slice(0, s);
    const after = v.slice(e);
    const trimmedBefore = before.replace(/\s+$/, '');
    const endsOpen = !trimmedBefore || /[(\[,:!&|+\-*/%=<>]$/.test(trimmedBefore) ||
      /\b(?:and|or|not)$/.test(trimmedBefore);
    let ins = rawSnippet;
    if (mode === 'cond' && s === v.length && trimmedBefore && !endsOpen &&
        !/^\s*(?:and|or)\b/.test(rawSnippet)) {
      ins = ' and ' + ins;
    } else {
      const needBefore = before !== '' && /[A-Za-z0-9_)\x27\x22]$/.test(before) &&
        /^[A-Za-z0-9_(\x27\x22]/.test(ins);
      const needAfter = after !== '' && /[A-Za-z0-9_(\x27\x22]$/.test(ins) &&
        /^[A-Za-z0-9_(\x27\x22]/.test(after);
      ins = (needBefore ? ' ' : '') + ins + (needAfter ? ' ' : '');
    }
    return { text: before + ins + after, caret: (before + ins).length };
  }

  // P7: repair the exact damage the old insertion path caused so flows already
  // saved by the buggy UI load as valid ones (`truetext_visible(...)` →
  // `true and text_visible(...)`). Only touches the unambiguous prefix form.
  function repairConditionExpr(cond) {
    const c = String(cond === undefined || cond === null ? '' : cond);
    return c.replace(
      /^(true|false)(?=(?:not\b|appear\s*\(|text_visible\s*\(|ocr\s*\(|param\s*\(|var\s*\())/,
      '$1 and '
    );
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

  // 面板行数据（分组/key/类型/label/default/assetId），非对象项被过滤。
  // assetId 存在 = 派生行（来自「使用端OCR文字配置项」素材，U-T5 起展示素材名）。
  function paramPanelRows(params) {
    const list = Array.isArray(params) ? params : [];
    return list
      .filter(function (p) { return p && typeof p === 'object' && !Array.isArray(p); })
      .map(function (p) {
        const key = p.key === undefined || p.key === null ? '' : String(p.key).trim();
        const label = p.label === undefined || p.label === null ? '' : String(p.label).trim();
        const group = p.group === undefined || p.group === null ? '' : String(p.group).trim();
        const type = p.type === undefined || p.type === null ? '' : String(p.type).trim();
        const row = {
          key: key,
          label: label || key,
          type: type,
          group: group,
          defaultText: paramDefaultText(p),
        };
        const assetId = paramAssetId(p);
        if (assetId) row.assetId = assetId;
        return row;
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
  // T3 key extraction hint. Implements the same «param(<single literal>)» rule as
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

  // P6（task-19）：requires/expect 守卫与 drag from/to 端点的只读 locator 提取，
  // 供属性面板的结构化 OCR 文字编辑器使用（纯函数，ui_pure_test 抽取执行）。
  function stepGuardLocator(step, field) {
    const s = step && typeof step === 'object' ? step : {};
    const args = s.args && typeof s.args === 'object' && !Array.isArray(s.args) ? s.args : {};
    const guard = args[field];
    return guard && typeof guard === 'object' && !Array.isArray(guard) ? guard : null;
  }

  function locatorIsOcrText(locator) {
    const loc = locator && typeof locator === 'object' && !Array.isArray(locator) ? locator : {};
    return String(loc.kind === undefined || loc.kind === null ? '' : loc.kind).trim() === 'ocr_text';
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

  // --- seq136（#10）：同层拖动排序（纯函数）+ step id 跟随 ----------------------
  let nextStepUiId = 1;

  // 非枚举 __ui_id：不写进 flow.json（JSON.stringify 忽略），但拖动/重排后可按 id
  // 重算选中 path（移动后 path 失效，id 不变）。
  function stepUiId(step) {
    if (!step || typeof step !== 'object') return '';
    if (!step.__ui_id) {
      const id = 's' + (nextStepUiId++);
      try {
        Object.defineProperty(step, '__ui_id', {
          value: id, enumerable: false, writable: true, configurable: true,
        });
      } catch (e) {
        step.__ui_id = id;
      }
    }
    return step.__ui_id;
  }

  function stepPathById(steps, id, prefix) {
    const list = Array.isArray(steps) ? steps : [];
    const pre = Array.isArray(prefix) ? prefix : [];
    for (let i = 0; i < list.length; i += 1) {
      const step = list[i];
      if (!step || typeof step !== 'object') continue;
      const path = pre.concat([i]);
      if (stepUiId(step) === id) return path;
      for (const key of ['then', 'else', 'body']) {
        const found = stepPathById(step[key], id, path.concat([key]));
        if (found) return found;
      }
    }
    return null;
  }

  // 同父级内移动：fromPath 指向的 step 插到 insertIndex（插入前的下标，0..len）。
  // 跨容器/根级以外的操作返回 ok:false，由调用方保留箭头移动。
  function moveStepWithinParent(rootSteps, fromPath, insertIndex) {
    const roots = Array.isArray(rootSteps) ? rootSteps : [];
    if (!Array.isArray(fromPath) || !fromPath.length) return { ok: false, error: '根容器不可拖动' };
    const parentPath = fromPath.slice(0, -1);
    let list = roots;
    for (let i = 0; i < parentPath.length; i += 1) {
      const seg = parentPath[i];
      if (typeof seg === 'number') list = list && list[seg];
      else list = list && list[seg];
    }
    if (!Array.isArray(list)) return { ok: false, error: '找不到拖动父容器' };
    const from = fromPath[fromPath.length - 1];
    if (!Number.isInteger(from) || from < 0 || from >= list.length) {
      return { ok: false, error: '拖动源下标无效' };
    }
    const step = list[from];
    const id = stepUiId(step);
    let to = Number(insertIndex);
    if (!Number.isFinite(to)) return { ok: false, error: '插入下标无效' };
    to = Math.max(0, Math.min(list.length, Math.round(to)));
    list.splice(from, 1);
    if (to > from) to -= 1;
    list.splice(to, 0, step);
    return {
      ok: true,
      id: id,
      moved: to !== from,
      path: parentPath.concat([to]),
    };
  }

  // 拖动指针 → 同层插入下标（上/中/下：越过节点中线即插到其后）。
  function dragInsertionIndex(siblingNodes, pointerY) {
    const nodes = Array.isArray(siblingNodes) ? siblingNodes : [];
    const y = Number(pointerY);
    if (!Number.isFinite(y) || !nodes.length) return 0;
    for (let i = 0; i < nodes.length; i += 1) {
      const n = nodes[i];
      const top = Number(n.y) || 0;
      const h = Math.max(1, Number(n.h) || 1);
      if (y < top + h / 2) return i;
    }
    return nodes.length;
  }

  // 同层节点按父 path 过滤（拖动只允许同容器内重排）。
  function sameParentGraphNodes(nodes, parentPath) {
    const key = JSON.stringify(Array.isArray(parentPath) ? parentPath : []);
    return (Array.isArray(nodes) ? nodes : [])
      .filter((n) => n && Array.isArray(n.path) && JSON.stringify(n.path.slice(0, -1)) === key)
      .sort((a, b) => a.y - b.y);
  }

  // 指示线几何：插到 idx 前 = 该节点上方；idx===len = 最后节点下方。
  function flowDropIndicatorGeometry(nodes, idx) {
    const list = Array.isArray(nodes) ? nodes : [];
    if (!list.length) return null;
    const i = Math.max(0, Math.min(list.length, Number(idx) || 0));
    const x1 = Math.min.apply(null, list.map((n) => n.cx - n.w / 2));
    const x2 = Math.max.apply(null, list.map((n) => n.cx + n.w / 2));
    if (i <= 0) return { x1: x1, x2: x2, y: list[0].y - 6 };
    if (i >= list.length) {
      const last = list[list.length - 1];
      return { x1: x1, x2: x2, y: last.y + last.h + 6 };
    }
    return { x1: x1, x2: x2, y: list[i].y - 6 };
  }

  // client 坐标 → SVG viewBox 坐标（自适应缩放后仍能命中正确节点）。
  function svgPointInViewBox(clientX, clientY, rect, viewBox) {
    const r = rect && typeof rect === 'object' ? rect : {};
    const w = Number(r.width) || 1;
    const h = Number(r.height) || 1;
    const vb = viewBox && typeof viewBox === 'object'
      ? viewBox
      : { x: 0, y: 0, width: w, height: h };
    return {
      x: vb.x + ((Number(clientX) - (Number(r.left) || 0)) / w) * vb.width,
      y: vb.y + ((Number(clientY) - (Number(r.top) || 0)) / h) * vb.height,
    };
  }

  const flowDragState = { sourcePath: null, parentPath: null };
  let flowDropIndicator = null;

  function clearFlowDropIndicator() {
    if (flowDropIndicator && flowDropIndicator.parentNode) flowDropIndicator.parentNode.removeChild(flowDropIndicator);
    flowDropIndicator = null;
  }

  function scrollSelectedStepIntoView() {
    const nodes = document.querySelectorAll('.builder-step.selected, .flow-svg-node.selected');
    for (const node of nodes) {
      if (node && typeof node.scrollIntoView === 'function') {
        try { node.scrollIntoView({ block: 'nearest', inline: 'nearest' }); } catch (e) { /* older DOM */ }
        return;
      }
    }
  }

  // 执行同层拖动落点：记录选中/被拖动 step 的 id，重排后按 id 重算 B.selected 并滚动跟随。
  function applyStepDragDrop(rootSteps, sourcePath, siblingNodes, pointerY) {
    const source = getStep(sourcePath);
    if (!source) return false;
    const draggedId = stepUiId(source);
    const selectedBefore = getStep(B.selected);
    const selectedId = selectedBefore ? stepUiId(selectedBefore) : '';
    const idx = dragInsertionIndex(siblingNodes, pointerY);
    const result = moveStepWithinParent(rootSteps, sourcePath, idx);
    if (!result.ok) return false;
    // 选中项优先跟随原选中 step 的 id（拖动的是它则自然跟到新位置）；无选中时跟随拖动项。
    if (selectedId) B.selected = stepPathById(rootSteps, selectedId) || B.selected;
    else B.selected = stepPathById(rootSteps, draggedId) || B.selected;
    renderAll();
    scheduleAutoSave();
    if (typeof requestAnimationFrame === 'function') {
      requestAnimationFrame(scrollSelectedStepIntoView);
    } else {
      setTimeout(scrollSelectedStepIntoView, 0);
    }
    return true;
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
      case 'long_press': {
        const loc = (Array.isArray(a.find) && a.find[0]) || {};
        const key = locatorParamKey(loc);
        const head = key ? `${esc(loc.sel)} · 文字=param('${key}')` : (esc(loc.sel) || JSON.stringify(a.find || []).slice(0, 40));
        return head + ' · ' + longPressHoldText(a);
      }
      case 'curve_swipe': {
        const count = Array.isArray(a.points) ? a.points.length : 0;
        const dur = Array.isArray(a.duration_ms) && a.duration_ms.length === 2 ? a.duration_ms : [600, 1000];
        return '多段近似 ' + count + ' 点 · ' + dur[0] + '~' + dur[1] + 'ms';
      }
      case 'dismiss_popup': return esc(a.sel);
      case 'swipe': return `${JSON.stringify(a.from)}→${JSON.stringify(a.to)}`;
      case 'key': return `keycode ${esc(a.keycode)}`;
      case 'restart_app': return esc(a.package) || '（未填包名）';
      case 'drag': return `${a.direction || '?'} ${a.from ? JSON.stringify(a.from).slice(0, 30) : ''}`;
      case 'navigate': return `导航到「${esc(a.target) || '（未选择节点）'}」`;
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

  // --- P5：权威 lint 面板 / 请求 / T3 旧参数指引 ----------------------------------

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
      msgEl.textContent = '旧参数引用不再支持：请在构建页「变量」面板声明变量（识别文字可用「识别文字变量」）' +
        (key ? '（flow 引用 param(\'' + key + '\')）' : '') + '——发布会被 T3 拒绝';
      const hint = document.createElement('span');
      hint.className = 'builder-lint-hint';
      hint.textContent = '在「变量」面板新建变量后再用动作目标/条件行引用它';
      actions.appendChild(hint);
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
        if (!options.silent) status(B.lintError, '#ff8a80');
        return;
      }
      B.lint = result;
      B.lintError = '';
      B.lintRunning = false;
      renderLintPanel();
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
      if (!options.silent) status(B.lintError, '#ff8a80');
    }
  }

  function closeLintPanel() {
    const panel = $('builder-lint-panel');
    if (panel) panel.setAttribute('hidden', '');
  }

  function renderPalette() {
    const root = $('builder-ops');
    if (!root) return;
    root.textContent = '';
    const catalog = actionCatalog();
    for (const g of GROUP_ORDER) {
      const entries = catalog.filter((item) => item.group === g);
      if (!entries.length) continue;
      const h = document.createElement('div');
      h.className = 'builder-op-group';
      h.textContent = g;
      root.appendChild(h);
      for (const item of entries) {
        const b = document.createElement('button');
        b.type = 'button';
        b.textContent = item.label + '  (' + item.key + ')';
        b.title = ACTION_TITLES[item.key] || (item.label + '：插入到当前选中容器/根级');
        b.addEventListener('click', () => addAction(item.key));
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

  // seq137（UI B #1）：列表视图同层拖拽插入纯函数（根/if/loop 子级、首尾）。
  // path 语义与 B.selected 一致：根 = [i]；if = [i,'then',j] / [i,'else',j]；
  // loop/while = [i,'body',j]。'inside' 只对容器步骤有效（append 到其子数组）。
  function flowChildArrayAt(flow, containerPath) {
    if (!flow || !Array.isArray(flow.steps)) return null;
    let arr = flow.steps;
    let obj = null;
    for (const seg of containerPath) {
      if (typeof seg === 'number') {
        if (!Array.isArray(arr)) return null;
        obj = arr[seg];
        if (!obj) return null;
        arr = obj;
      } else {
        if (!obj) return null;
        arr = obj[seg];
        if (!Array.isArray(arr)) return null;
      }
    }
    return Array.isArray(arr) ? arr : null;
  }

  function flowContainerChildKey(step) {
    if (!step || typeof step !== 'object') return '';
    if (step.op === 'if') return 'then';
    if (step.op === 'loop' || step.op === 'while') return 'body';
    return '';
  }

  function flowStepIsContainer(step) {
    return !!flowContainerChildKey(step);
  }

  // 行内垂直位置 → before / after / inside（中间 40% 视为容器内追加）。
  function flowDropZoneFromRect(rectTop, rectHeight, clientY) {
    const h = Math.max(1, Number(rectHeight) || 1);
    const r = (Number(clientY) - Number(rectTop || 0)) / h;
    if (r < 0.3) return 'before';
    if (r > 0.7) return 'after';
    return 'inside';
  }

  // 删除源步骤后修正 targetPath 中同一数组的下标（源在目标之前时目标前移）。
  function adjustPathAfterStepRemoval(path, fromPath) {
    const out = path.slice();
    const parent = fromPath.slice(0, -1);
    for (let i = 0; i < parent.length; i++) {
      if (path[i] !== parent[i]) return out;
    }
    const targetIdx = path[parent.length];
    const fromIdx = fromPath[fromPath.length - 1];
    if (typeof targetIdx === 'number' && targetIdx > fromIdx) {
      out[parent.length] = targetIdx - 1;
    }
    return out;
  }

  // 纯函数：把 flow 中 fromPath 步骤移动到 targetPath 的 before/after / 容器 inside。
  // 返回 {ok:true,path} 或 {ok:false,error}；before/after 仅允许同层（root 或同一子容器）。
  function moveFlowStepInsert(flow, fromPath, targetPath, position) {
    if (!flow || !Array.isArray(fromPath) || !fromPath.length ||
      !Array.isArray(targetPath) || !targetPath.length) {
      return { ok: false, error: '拖拽路径无效' };
    }
    const fromArr = flowChildArrayAt(flow, fromPath.slice(0, -1));
    const fromIdx = fromPath[fromPath.length - 1];
    const fromStep = fromArr && fromArr[fromIdx];
    if (!fromStep) return { ok: false, error: '源步骤不存在' };
    if (JSON.stringify(fromPath) === JSON.stringify(targetPath)) {
      return { ok: false, error: '拖到自身位置（无变化）' };
    }
    const targetStep = flowStepAtPath(flow, targetPath);
    if (!targetStep) return { ok: false, error: '目标步骤不存在' };
    if (fromStep === targetStep || stepContains(fromStep, targetStep)) {
      return { ok: false, error: '不能把父级移动到自己的子级' };
    }
    let pos = position === 'before' || position === 'after' || position === 'inside'
      ? position : 'after';
    if (pos === 'inside' && !flowStepIsContainer(targetStep)) pos = 'after';
    let targetArr = null;
    let insertIdx = 0;
    let parentPath = null;
    if (pos === 'inside') {
      const key = flowContainerChildKey(targetStep);
      targetArr = targetStep[key];
      if (!Array.isArray(targetArr)) return { ok: false, error: '目标容器不存在' };
      insertIdx = targetArr.length;
      parentPath = targetPath.concat([key]);
    } else {
      targetArr = flowChildArrayAt(flow, targetPath.slice(0, -1));
      if (!targetArr) return { ok: false, error: '目标容器不存在' };
      if (targetArr !== fromArr) {
        return { ok: false, error: '只能同层插入；跨层请拖到 if/loop 容器内' };
      }
      insertIdx = targetPath[targetPath.length - 1] + (pos === 'after' ? 1 : 0);
      parentPath = targetPath.slice(0, -1);
    }
    fromArr.splice(fromIdx, 1);
    if (targetArr === fromArr && insertIdx > fromIdx) insertIdx -= 1;
    const adjustedTargetPath = adjustPathAfterStepRemoval(targetPath, fromPath);
    const finalParentPath = pos === 'inside'
      ? adjustedTargetPath.concat([flowContainerChildKey(targetStep)])
      : adjustedTargetPath.slice(0, -1);
    targetArr.splice(insertIdx, 0, fromStep);
    return { ok: true, path: finalParentPath.concat([insertIdx]) };
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
      e.stopPropagation();
      drop.classList.remove('drop-target');
      const st = B.dragState;
      const paths = st && st.paths && st.paths.length
        ? st.paths.slice() : parseDragPayload(e.dataTransfer.getData('text/plain'));
      B.dragState = null;
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
      row.dataset.stepPath = JSON.stringify(path);
      if (B.selected && JSON.stringify(B.selected) === JSON.stringify(path)) row.classList.add('selected');
      row.addEventListener('dragstart', (e) => {
        const payload = dragPayload(path);
        B.dragState = { paths: payload.paths || [payload.path].filter(Boolean), sourcePath: path.slice() };
        e.dataTransfer.setData('text/plain', JSON.stringify(payload));
        e.dataTransfer.effectAllowed = 'move';
        row.classList.add('dragging');
      });
      row.addEventListener('dragend', () => {
        B.dragState = null;
        row.classList.remove('dragging');
        row.classList.remove('drop-before', 'drop-after', 'drop-inside');
        document.querySelectorAll('.builder-step.drop-before, .builder-step.drop-after, .builder-step.drop-inside')
          .forEach((n) => n.classList.remove('drop-before', 'drop-after', 'drop-inside'));
      });
      const clearRowDropClasses = () => {
        row.classList.remove('drop-before', 'drop-after', 'drop-inside');
      };
      row.addEventListener('dragover', (e) => {
        const st = B.dragState;
        // 多选拖拽沿用容器 drop（appends）；单行拖拽按上/中/下算插入位。
        if (!st || !st.paths || st.paths.length !== 1) return;
        const srcPath = st.paths[0];
        if (JSON.stringify(srcPath) === JSON.stringify(path)) return;
        const rect = row.getBoundingClientRect();
        let zone = flowDropZoneFromRect(rect.top, rect.height, e.clientY);
        if (zone === 'inside' && !flowStepIsContainer(step)) zone = 'after';
        if (zone !== 'inside' && JSON.stringify(srcPath.slice(0, -1)) !== JSON.stringify(path.slice(0, -1))) {
          clearRowDropClasses();
          return;
        }
        e.preventDefault();
        e.stopPropagation();
        if (zone === 'inside' && flowStepIsContainer(step)) {
          e.dataTransfer.dropEffect = 'move';
        } else {
          e.dataTransfer.dropEffect = 'move';
        }
        clearRowDropClasses();
        row.classList.add(zone === 'inside' ? 'drop-inside' : (zone === 'before' ? 'drop-before' : 'drop-after'));
      });
      row.addEventListener('dragleave', clearRowDropClasses);
      row.addEventListener('drop', (e) => {
        e.preventDefault();
        e.stopPropagation();
        const st = B.dragState;
        const paths = st && st.paths && st.paths.length
          ? st.paths.slice() : parseDragPayload(e.dataTransfer.getData('text/plain'));
        const rect = row.getBoundingClientRect();
        let zone = flowDropZoneFromRect(rect.top, rect.height, e.clientY);
        if (zone === 'inside' && !flowStepIsContainer(step)) zone = 'after';
        clearRowDropClasses();
        B.dragState = null;
        if (!paths.length) return;
        if (paths.length > 1) {
          if (zone === 'inside') {
            const key = flowContainerChildKey(step);
            if (key) moveStepsGroup(paths, path, key, false);
            else status('多选步骤只能拖入 if/loop 容器', '#e8c46a');
          } else {
            status('多选步骤目前只支持拖入 if/loop 容器或根级 drop 区', '#e8c46a');
          }
          return;
        }
        const srcStep = flowStepAtPath(B.flow, paths[0]);
        const movedId = srcStep ? stepUiId(srcStep) : null;
        const res = moveFlowStepInsert(B.flow, paths[0], path, zone);
        if (!res.ok) {
          status('拖拽失败：' + res.error, '#ff8a80');
          renderAll();
          return;
        }
        // seq137：用 step id 重算选中并滚动到新位置（纯 path 会因移动偏移）。
        B.selected = (movedId !== null && stepPathById(getRoot(), movedId)) || res.path;
        renderAll();
        scheduleAutoSave();
        status(zone === 'inside' ? '已移入子级容器' : (zone === 'before' ? '已插入到目标步骤之前' : '已插入到目标步骤之后'));
        const selectedRow = document.querySelector('#builder-steps .builder-step.selected');
        if (selectedRow && typeof selectedRow.scrollIntoView === 'function') {
          selectedRow.scrollIntoView({ block: 'nearest' });
        }
      });
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
      up.addEventListener('click', (e) => {
      e.stopPropagation();
      if (idx > 0) {
        const moved = steps[idx];
        const movedId = stepUiId(moved);
        [steps[idx - 1], steps[idx]] = [steps[idx], steps[idx - 1]];
        // seq136（#10）：箭头移动后同样按 step id 重算选中（不再用已失效的 path）。
        B.selected = stepPathById(getRoot(), movedId) || path;
        renderAll();
        scheduleAutoSave();
      }
    });
      const down = document.createElement('button');
      down.type = 'button';
      down.textContent = '↓';
      down.title = '下移';
      down.addEventListener('click', (e) => {
      e.stopPropagation();
      if (idx < steps.length - 1) {
        const moved = steps[idx];
        const movedId = stepUiId(moved);
        [steps[idx + 1], steps[idx]] = [steps[idx], steps[idx + 1]];
        B.selected = stepPathById(getRoot(), movedId) || path;
        renderAll();
        scheduleAutoSave();
      }
    });
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
          e.stopPropagation();
          child.classList.remove('drop-target');
          const st = B.dragState;
          const paths = st && st.paths && st.paths.length
            ? st.paths.slice() : parseDragPayload(e.dataTransfer.getData('text/plain'));
          B.dragState = null;
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
      g.setAttribute('data-step-path', JSON.stringify(node.path));
      if (B.selected && JSON.stringify(B.selected) === JSON.stringify(node.path)) g.classList.add('selected');
      g.addEventListener('click', () => {
        B.selected = node.path;
        renderAll();
      });
      // seq137（UI B #1）：图谱不再使用不可靠的 SVG HTML5 draggable（假绿）；
      // 排序统一在列表视图完成（行上/中/下插入），图谱只负责选中/可视化。
    }
    svg.appendChild(g);
  }

  // seq136（#9）：流程图容器自适应（SVG 天然矢量，无 canvas DPR 重置问题；保留
  // viewBox 节点坐标不变，按容器可用区域 fit → 居中）。纯函数便于测试。
  function fitFlowGraphSize(naturalW, naturalH, containerW, containerH, opts) {
    const nw = Math.max(1, Number(naturalW) || 1);
    const nh = Math.max(1, Number(naturalH) || 1);
    const cw = Math.max(1, Number(containerW) || 0);
    const ch = Math.max(1, Number(containerH) || 0);
    const maxScale = opts && Number(opts.maxScale) > 0 ? Number(opts.maxScale) : 1.6;
    const minScale = opts && Number(opts.minScale) > 0 ? Number(opts.minScale) : 0.1;
    const scale = Math.max(minScale, Math.min(maxScale, Math.min(cw / nw, ch / nh)));
    return {
      scale: scale,
      width: cw,
      height: ch,
      offsetX: Math.round((cw - nw * scale) / 2),
      offsetY: Math.round((ch - nh * scale) / 2),
    };
  }

  const flowGraphState = { width: 0, height: 0, fitTimer: 0 };

  function fitFlowGraph() {
    const root = $('builder-graph');
    const svg = root && root.querySelector ? root.querySelector('svg.flow-svg') : null;
    if (!root || !svg) return null;
    const rect = typeof root.getBoundingClientRect === 'function'
      ? root.getBoundingClientRect()
      : { width: 0, height: 0 };
    const availW = Math.max(0, Math.round(rect.width || 0));
    const availH = Math.max(0, Math.round(rect.height || 0)) ||
      Math.max(240, Math.round((window.innerHeight || 720) * 0.4));
    if (!availW) {
      // 容器尚未布局（隐藏页/首帧）：保留自然尺寸属性，等待 ResizeObserver。
      return null;
    }
    const fit = fitFlowGraphSize(flowGraphState.width, flowGraphState.height, availW, availH);
    svg.setAttribute('width', String(fit.width));
    svg.setAttribute('height', String(fit.height));
    svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');
    svg.style.width = '100%';
    svg.style.height = '100%';
    return fit;
  }

  function scheduleFlowGraphFit() {
    if (flowGraphState.fitTimer) return;
    const run = function () {
      flowGraphState.fitTimer = 0;
      fitFlowGraph();
    };
    flowGraphState.fitTimer = (typeof requestAnimationFrame === 'function')
      ? requestAnimationFrame(run) : setTimeout(run, 0);
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
    flowGraphState.width = width;
    flowGraphState.height = height;
    const svg = sEl('svg', {
      class: 'flow-svg',
      viewBox: `0 0 ${width} ${height}`,
      preserveAspectRatio: 'xMidYMid meet',
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
    // seq137（UI B #1）：删除 SVG draggable/dragover/drop（浏览器对 SVG <g> 不可靠，
    // 旧静态测试是假绿）；列表视图负责真实排序，图谱保留点击选中。
    // seq136（#9）：按容器可用区域 fit/居中（ResizeObserver/window.resize 再算）。
    fitFlowGraph();
  }

  // --- seq137（UI B #2）：独立全窗口流程图大页 ---------------------------------
  const flowPageState = { open: false, zoom: 1 };

  function flowPageNaturalSize() {
    const w = Number(flowGraphState.width) || 0;
    const h = Number(flowGraphState.height) || 0;
    return { width: Math.max(760, w || 760), height: Math.max(360, h || 360) };
  }

  function renderFlowPage() {
    const page = $('flow-page');
    const canvas = $('flow-page-canvas');
    if (!page || !canvas) return;
    page.hidden = !flowPageState.open;
    if (!flowPageState.open) return;
    const source = $('builder-graph');
    const svg = source && source.querySelector ? source.querySelector('svg.flow-svg') : null;
    canvas.textContent = '';
    const zoomLabel = $('flow-page-zoom-label');
    const statusEl = $('flow-page-status');
    if (!svg) {
      canvas.textContent = '（空流程；添加动作后显示控制流图）';
      if (zoomLabel) zoomLabel.textContent = '100%';
      if (statusEl) statusEl.textContent = '—';
      return;
    }
    const nat = flowPageNaturalSize();
    const clone = svg.cloneNode(true);
    clone.removeAttribute('width');
    clone.removeAttribute('height');
    clone.style.width = Math.round(nat.width * flowPageState.zoom) + 'px';
    clone.style.height = Math.round(nat.height * flowPageState.zoom) + 'px';
    clone.style.maxWidth = 'none';
    // 大页只读可视化；节点点击回写选中（列表/属性面板同步）。
    clone.querySelectorAll('[data-step-path]').forEach((g) => {
      g.style.cursor = 'pointer';
      g.addEventListener('click', () => {
        try {
          B.selected = JSON.parse(g.getAttribute('data-step-path'));
          renderAll();
          status('已选中步骤；点「返回构建」可编辑属性');
        } catch (e) { /* ignore */ }
      });
    });
    canvas.appendChild(clone);
    if (zoomLabel) zoomLabel.textContent = Math.round(flowPageState.zoom * 100) + '%';
    if (statusEl) statusEl.textContent = '步骤 ' + getRoot().length + ' 个根动作 · 滚轮滚动，按钮缩放';
  }

  function setFlowPageZoom(next) {
    flowPageState.zoom = Math.max(0.3, Math.min(3, Number(next) || 1));
    renderFlowPage();
  }

  function fitFlowPage() {
    const canvas = $('flow-page-canvas');
    const nat = flowPageNaturalSize();
    const availW = canvas && typeof canvas.getBoundingClientRect === 'function'
      ? Math.max(320, canvas.getBoundingClientRect().width - 32) : 960;
    setFlowPageZoom(availW / nat.width);
  }

  function openFlowPage() {
    const page = $('flow-page');
    if (!page) return;
    flowPageState.open = true;
    page.hidden = false;
    renderFlowPage();
    status('已打开全窗口流程图（只读；拖拽排序在列表做）');
  }

  function closeFlowPage() {
    flowPageState.open = false;
    const page = $('flow-page');
    if (page) page.hidden = true;
  }

  // --- seq137（UI B #3）：flow → Rust 只读源码投影（纯函数） -------------------
  function rustStringLiteral(value) {
    let s = String(value === undefined || value === null ? '' : value);
    s = s.split('\\').join('\\\\');
    s = s.split('"').join('\\"');
    s = s.split('\r').join('\\r');
    s = s.split('\n').join('\\n');
    return '"' + s + '"';
  }

  function rustNumLiteral(value, fallback) {
    const n = Number(value);
    return String(Number.isFinite(n) ? n : (Number.isFinite(Number(fallback)) ? Number(fallback) : 0));
  }

  // 只读投影：已知 op → ctx.* 调用（教学视图）；未知 op → 注释保留。
  // 本函数不修改 flow，也不提供 Rust→flow 编译（遗留说明见 UI）。
  function rustProjectStepLines(step, indent, lines) {
    const pad = ' '.repeat(indent);
    const s = step && typeof step === 'object' ? step : {};
    const a = s.args && typeof s.args === 'object' && !Array.isArray(s.args) ? s.args : {};
    const op = String(s.op === undefined || s.op === null ? '' : s.op);
    const emit = (text) => lines.push(pad + text);
    switch (op) {
      case 'log':
        emit('ctx.log(' + rustStringLiteral(a.text) + ');');
        return;
      case 'wait':
        emit('ctx.wait(' + rustNumLiteral(a.seconds, 1) + ');');
        return;
      case 'wait_until':
        emit('ctx.wait_until(' + rustStringLiteral(a.cond) + ', ' + rustNumLiteral(a.timeout_s, 10) + ');');
        return;
      case 'appear':
        emit('ctx.appear(' + rustStringLiteral(a.sel) + ');');
        return;
      case 'ocr':
        emit('ctx.ocr(' + rustStringLiteral(a.sel) + ');');
        return;
      case 'click':
        emit('ctx.click(' + rustStringLiteral(a.sel) + ');');
        return;
      case 'click_point':
        emit('ctx.click_point(' + rustNumLiteral(a.x, 0) + ', ' + rustNumLiteral(a.y, 0) + ');');
        return;
      case 'click_target':
        emit('ctx.click_target(' + rustStringLiteral(JSON.stringify(a.find || [])) + ');');
        return;
      case 'long_press':
        emit('ctx.long_press(' + rustStringLiteral(JSON.stringify(a.find || [])) + ', ' + rustNumLiteral(a.hold_ms, 3000) + ');');
        return;
      case 'curve_swipe':
        emit('ctx.curve_swipe(' + rustStringLiteral(JSON.stringify(a.points || [])) + ', ' + rustStringLiteral(JSON.stringify(a.duration_ms || [])) + ');');
        return;
      case 'dismiss_popup':
        emit('ctx.dismiss_popup(' + rustStringLiteral(a.sel) + ');');
        return;
      case 'swipe':
        emit('ctx.swipe(' + rustStringLiteral(JSON.stringify(a.from || [])) + ', ' + rustStringLiteral(JSON.stringify(a.to || [])) + ');');
        return;
      case 'drag':
        emit('ctx.drag(' + rustStringLiteral(JSON.stringify(a.from || {})) + ', ' + rustStringLiteral(JSON.stringify(a.to || {})) + ', ' + rustStringLiteral(a.direction) + ');');
        return;
      case 'key':
        emit('ctx.key(' + rustNumLiteral(a.keycode, 4) + ');');
        return;
      case 'restart_app':
        emit('ctx.restart_app(' + rustStringLiteral(a.package) + ');');
        return;
      case 'set_var':
        emit('ctx.set_var(' + rustStringLiteral(a.name) + ', ' + rustStringLiteral(a.value) + ');');
        return;
      case 'inc_var':
        emit('ctx.inc_var(' + rustStringLiteral(a.name) + ', ' + rustNumLiteral(a.by, 1) + ');');
        return;
      case 'ocr_to_var':
        emit('ctx.ocr_to_var(' + rustStringLiteral(a.sel) + ', ' + rustStringLiteral(a.var) + ');');
        return;
      case 'call_task':
        emit('ctx.call_task(' + rustStringLiteral(a.task) + ', ' + (a.mode === 'inline' ? 'true' : 'false') + ');');
        return;
      case 'navigate':
        emit('ctx.navigate(' + rustStringLiteral(a.target) + ');');
        return;
      case 'finish':
        emit('ctx.finish(' + rustStringLiteral(a.exit) + ');');
        return;
      case 'break':
        emit('break;');
        return;
      case 'continue':
        emit('continue;');
        return;
      case 'if': {
        emit('ctx.if_cond(' + rustStringLiteral(a.cond) + ', |ctx| {');
        (Array.isArray(s.then) ? s.then : []).forEach((child) => rustProjectStepLines(child, indent + 4, lines));
        if (Array.isArray(s.else) && s.else.length) {
          lines.push(pad + '}, |ctx| {');
          s.else.forEach((child) => rustProjectStepLines(child, indent + 4, lines));
        }
        lines.push(pad + '});');
        return;
      }
      case 'loop': {
        emit('ctx.loop(' + rustNumLiteral(a.n, 1) + ', |ctx| {');
        (Array.isArray(s.body) ? s.body : []).forEach((child) => rustProjectStepLines(child, indent + 4, lines));
        lines.push(pad + '});');
        return;
      }
      case 'while': {
        emit('ctx.while_cond(' + rustStringLiteral(a.cond) + ', |ctx| {');
        (Array.isArray(s.body) ? s.body : []).forEach((child) => rustProjectStepLines(child, indent + 4, lines));
        lines.push(pad + '});');
        return;
      }
      case '': {
        lines.push(pad + '// TODO: 空步骤（无 op），flow 中保留');
        return;
      }
      default: {
        // 已知但尚未映射到专用 ctx 生成器的 op（保留 args 便于对照）。
        if (OP_META[op]) {
          emit('ctx.step(' + rustStringLiteral(op) + ', json!(' + rustStringLiteral(JSON.stringify(a)) + ')); // 未映射专用生成器');
        } else {
          lines.push(pad + '// TODO: 未知 op ' + rustStringLiteral(op) + '（只读投影保留注释；flow 中仍保留该步骤）');
        }
      }
    }
  }

  function rustProjectFlow(flow) {
    const f = flow && typeof flow === 'object' ? flow : { schema_version: 1, name: '', steps: [] };
    const steps = Array.isArray(f.steps) ? f.steps : [];
    const lines = [];
    lines.push('// ⚠ 由 flow.json 自动生成的只读 Rust 投影（非可执行源码；不是 Rust→flow 编译器）');
    lines.push('// flow: ' + rustStringLiteral(f.name || '') + ' · schema_version: ' + rustNumLiteral(f.schema_version, 1) + ' · root_steps: ' + steps.length);
    lines.push('// 修改流程请改左侧 JSON 并「校验并应用」；本投影只读，未知 op 以注释保留。');
    lines.push('');
    lines.push('pub fn run_flow(ctx: &mut Ctx) -> Result<()> {');
    steps.forEach((step) => rustProjectStepLines(step, 4, lines));
    lines.push('    Ok(())');
    lines.push('}');
    return lines.join('\n');
  }

  function renderRustProjection() {
    const pre = $('builder-rust-code');
    if (!pre) return;
    pre.textContent = B.flow ? rustProjectFlow(B.flow) : '（无流程；选择任务后显示）';
  }

  function assetSearch(current, kinds) {
    const wrap = window.OASAssetUI.createSearchPicker({
      items: B.assets,
      value: current || '',
      placeholder: '搜索素材（识别+操作区域）…',
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

  // --- V2.0c：构建页「变量」面板 + 变量对话框（meta.variables） -----------------
  async function fetchTaskVariables(name) {
    const task = String(name === undefined || name === null ? '' : name).trim();
    if (!task) return { ok: true, variables: [], warnings: [], error: '' };
    try {
      const r = await fetch('/task/variables?task=' + encodeURIComponent(task));
      const s = await r.json();
      if (!s || s.ok === false) {
        return { ok: false, variables: [], warnings: [], error: (s && (s.errors || [s.error]).join
          ? (s.errors || [s.error]).join('; ')
          : ((s && s.error) || '变量读取失败')) };
      }
      return {
        ok: true,
        variables: Array.isArray(s.variables) ? s.variables : [],
        warnings: Array.isArray(s.lint_warnings) ? s.lint_warnings : [],
        error: '',
      };
    } catch (e) {
      return { ok: false, variables: [], warnings: [], error: String((e && e.message) || e) };
    }
  }

  async function refreshVariables() {
    if (!B.task) {
      B.variables = [];
      B.variablesError = '';
      B.variablesIssues = [];
      renderVariablesPanel();
      return;
    }
    B.variablesLoading = true;
    renderVariablesPanel();
    const res = await fetchTaskVariables(B.task);
    B.variablesLoading = false;
    B.variables = res.ok ? res.variables : [];
    B.variablesError = res.ok ? '' : res.error;
    B.variablesIssues = res.ok ? res.warnings : [];
    renderVariablesPanel();
  }

  // POST 整表（后端 /task/variables 语义：variables 为完整列表；issues 为 T3/T8
  // 校验信息，不阻断保存；成功返回后端规范化后的变量列表）。
  async function saveTaskVariables(nextVariables) {
    if (!B.task) return { ok: false, error: '请先选择任务' };
    try {
      const r = await fetch('/task/variables', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ task: B.task, variables: nextVariables }),
      });
      const s = await r.json();
      if (!s || s.ok === false) {
        return {
          ok: false,
          error: (s && Array.isArray(s.errors) ? s.errors.join('; ') : ((s && s.error) || '变量保存失败')),
        };
      }
      B.variables = Array.isArray(s.variables) ? s.variables : nextVariables;
      B.variablesIssues = Array.isArray(s.issues) ? s.issues : [];
      B.variablesError = '';
      renderVariablesPanel();
      renderAll();
      return { ok: true, issues: B.variablesIssues };
    } catch (e) {
      return { ok: false, error: String((e && e.message) || e) };
    }
  }

  function insertVariableSnippet(key) {
    const snippet = paramInsertSnippet(key, 'expr');
    const field = B.lastExprField;
    if (field && field.isConnected && typeof field.value === 'string') {
      insertTextAtField(field, snippet, 'expr');
      status('已插入 ' + snippet);
      return true;
    }
    if (copyTextBestEffort(snippet)) {
      status('已复制到剪贴板：' + snippet + '（先点表达式输入框可直接插入）');
      return true;
    }
    status('变量片段：' + snippet);
    return false;
  }

  function variableRowActionButton(text, title, onClick) {
    const b = document.createElement('button');
    b.type = 'button';
    b.textContent = text;
    b.title = title || '';
    b.addEventListener('click', onClick);
    return b;
  }

  function renderVariablesPanel() {
    const list = $('builder-vars-list');
    const summary = $('builder-vars-summary');
    const statusEl = $('builder-vars-status');
    const issuesEl = $('builder-vars-issues');
    const newBtn = $('builder-vars-new');
    if (newBtn) newBtn.disabled = !B.task;
    if (summary) {
      summary.textContent = B.variables.length + ' 个变量';
      summary.className = 'builder-params-chip ' + (B.variables.length ? 'kind' : 'warn');
    }
    if (statusEl) {
      if (B.variablesLoading) {
        statusEl.textContent = '读取变量…';
        statusEl.className = 'muted small';
      } else if (B.variablesError) {
        statusEl.textContent = '变量读取失败：' + B.variablesError;
        statusEl.className = 'muted small err';
      } else if (!B.task) {
        statusEl.textContent = '先选择任务：变量来自 meta.variables（GET/POST /task/variables）。';
        statusEl.className = 'muted small';
      } else if (!B.variables.length) {
        statusEl.textContent = '暂无变量：点「新建变量…」创建；flow 中 param(\'key\') 必须先在变量面板声明。';
        statusEl.className = 'muted small';
      } else {
        statusEl.textContent = '变量是 flow 中 param(\'key\') 的唯一声明来源；识别文字变量绑定 ocr_var 素材（T8 同步）。';
        statusEl.className = 'muted small ok';
      }
    }
    if (issuesEl) {
      issuesEl.textContent = '';
      if (Array.isArray(B.variablesIssues) && B.variablesIssues.length) {
        issuesEl.className = 'builder-vars-issues warn';
        issuesEl.textContent = '变量校验提示：' + B.variablesIssues.map(function (w) {
          return typeof w === 'string' ? w : (w.message || JSON.stringify(w));
        }).join('；');
      }
    }
    if (!list) return;
    list.textContent = '';
    if (B.variablesLoading) { list.textContent = '读取中…'; return; }
    if (!B.task) { list.textContent = '（先选择任务）'; return; }
    if (!B.variables.length) {
      const empty = document.createElement('div');
      empty.className = 'muted small';
      empty.textContent = '（还没有变量；新建后可在动作表达式里用「插入变量 ▾」引用）';
      list.appendChild(empty);
      return;
    }
    for (let i = 0; i < B.variables.length; i += 1) {
      const v = B.variables[i];
      if (!v || typeof v !== 'object') continue;
      const key = String(v.key === undefined || v.key === null ? '' : v.key);
      const row = document.createElement('div');
      row.className = 'builder-var-row';
      const keyEl = document.createElement('span');
      keyEl.className = 'builder-var-key';
      keyEl.textContent = key;
      const kind = document.createElement('span');
      kind.className = 'builder-var-kind';
      kind.textContent = variableKindText(v);
      const def = document.createElement('span');
      def.className = 'builder-var-default';
      def.textContent = '默认 ' + variableDefaultText(v);
      const label = document.createElement('span');
      label.className = 'builder-var-label';
      label.textContent = (v.label || '') + (v.group ? ' · ' + v.group : '');
      const flags = document.createElement('span');
      flags.className = 'builder-var-flags';
      const bits = [];
      if (variableScopeOf(v) === 'runtime') bits.push('运行变量');
      bits.push(v.runner_editable === false ? '使用端不可改' : '使用端可改');
      if (v.required === true) bits.push('必填');
      const uses = variableUseCount(B.flow, key);
      if (uses) bits.push('引用 ' + uses);
      flags.textContent = bits.join(' · ');
      row.append(keyEl, kind, def, label, flags);
      const actions = document.createElement('span');
      actions.className = 'builder-var-actions';
      actions.appendChild(variableRowActionButton('插入', '把 param(\'' + key + '\') 插入到最近聚焦的表达式',
        function () { insertVariableSnippet(key); }));
      actions.appendChild(variableRowActionButton('编辑', '编辑变量（识别文字变量绑定 ocr_var 素材）',
        function () { openVariableDialog(v); }));
      actions.appendChild(variableRowActionButton('删除', '删除变量（被 flow 引用时会提示）',
        function () {
          const count = variableUseCount(B.flow, key);
          if (count > 0 && !window.confirm('变量「' + key + '」被 flow 引用 ' + count +
            ' 处；删除后这些 param(\'' + key + '\') 会变成未声明（发布 T3 会报错）。仍要删除？')) {
            return;
          }
          deleteVariableAt(i);
        }));
      row.appendChild(actions);
      list.appendChild(row);
    }
  }

  async function deleteVariableAt(index) {
    const next = B.variables.slice();
    next.splice(index, 1);
    const r = await saveTaskVariables(next);
    if (!r.ok) status('删除变量失败：' + r.error, '#ff8a80');
    else status('已删除变量（' + next.length + ' 个剩余）', '#7bd88f');
  }

  // 变量对话框 DOM（index.html #variable-dialog）。
  let variableDialogCtx = null;

  function variableDialogEl(id) { return $(id); }

  function ocrVarAssetsForVariable() {
    return (B.assets || []).filter(function (a) {
      return a && a.kind === 'ocr_var' && paramSafeKey(a.param_key);
    });
  }

  function openVariableDialog(variable) {
    const dlg = variableDialogEl('variable-dialog');
    if (!dlg) return;
    variableDialogCtx = {
      editingKey: variable && variable.key ? String(variable.key) : '',
      variable: variable || null,
    };
    renderVariableDialog();
    if (typeof dlg.showModal === 'function') {
      if (!dlg.open) dlg.showModal();
    } else {
      dlg.setAttribute('open', '');
    }
  }

  function closeVariableDialog() {
    const dlg = variableDialogEl('variable-dialog');
    variableDialogCtx = null;
    if (!dlg) return;
    try { if (typeof dlg.close === 'function' && dlg.open) dlg.close(); } catch (e) { /* cosmetic */ }
    dlg.removeAttribute('open');
  }

  function renderVariableDialog() {
    const ctx = variableDialogCtx;
    if (!ctx) return;
    const f = ctx.variable ? variableToForm(ctx.variable) : {
      key: '', type: 'string', value_source: 'literal', asset_id: '', defaultText: '',
      label: '', description: '', group: '', runnerEditable: true, required: false,
      scope: 'input', min: '', max: '', enumText: '', editingKey: '',
    };
    const editing = !!ctx.variable;
    const title = variableDialogEl('variable-title');
    if (title) title.textContent = editing ? '编辑变量 · ' + f.key : '新建变量';
    const source = variableDialogEl('variable-source');
    if (source) {
      source.value = f.value_source;
      source.disabled = editing && f.value_source === 'ocr_text';
    }
    const assetSel = variableDialogEl('variable-ocr-asset');
    if (assetSel) {
      assetSel.textContent = '';
      assetSel.appendChild(new Option('（选择使用端OCR文字配置项素材）', ''));
      for (const asset of ocrVarAssetsForVariable()) {
        const runtimeText = String(asset.runtime_text === undefined || asset.runtime_text === null
          ? '' : asset.runtime_text);
        assetSel.appendChild(new Option(
          (asset.label || asset.id) + ' · ' + asset.id + ' · ' + paramTextExpr(paramSafeKey(asset.param_key)) +
            (runtimeText ? ' · 「' + runtimeText + '」' : ''),
          String(asset.id)
        ));
      }
      assetSel.value = f.asset_id || '';
    }
    const keyInput = variableDialogEl('variable-key');
    if (keyInput) {
      keyInput.value = f.key;
      keyInput.readOnly = f.value_source === 'ocr_text';
      keyInput.title = f.value_source === 'ocr_text'
        ? '识别文字变量的 key 由素材 param_key 决定（只读）'
        : 'key 语法 [A-Za-z_][A-Za-z0-9_]*（≤64）；任务内唯一';
    }
    const typeSel = variableDialogEl('variable-type');
    if (typeSel) typeSel.value = f.type;
    const defaultInput = variableDialogEl('variable-default');
    if (defaultInput) defaultInput.value = f.defaultText;
    const minInput = variableDialogEl('variable-min');
    if (minInput) minInput.value = f.min;
    const maxInput = variableDialogEl('variable-max');
    if (maxInput) maxInput.value = f.max;
    const enumInput = variableDialogEl('variable-enum');
    if (enumInput) enumInput.value = f.enumText;
    const labelInput = variableDialogEl('variable-label');
    if (labelInput) labelInput.value = f.label;
    const descInput = variableDialogEl('variable-description');
    if (descInput) descInput.value = f.description;
    const groupInput = variableDialogEl('variable-group');
    if (groupInput) groupInput.value = f.group;
    const runnerEditable = variableDialogEl('variable-runner-editable');
    if (runnerEditable) runnerEditable.checked = f.runnerEditable;
    const requiredInput = variableDialogEl('variable-required');
    if (requiredInput) requiredInput.checked = f.required;
    const scopeSel = variableDialogEl('variable-scope');
    if (scopeSel) scopeSel.value = f.scope;
    const errorEl = variableDialogEl('variable-error');
    if (errorEl) errorEl.textContent = '';
    syncVariableDialogFields();
  }

  function syncVariableDialogFields() {
    const source = variableDialogEl('variable-source');
    const isOcr = !!source && source.value === 'ocr_text';
    const scopeSel = variableDialogEl('variable-scope');
    const isRuntime = !!scopeSel && scopeSel.value === 'runtime';
    for (const id of ['variable-ocr-asset-row']) {
      const el = variableDialogEl(id);
      if (el) el.hidden = !isOcr;
    }
    for (const id of ['variable-type-row', 'variable-default-row', 'variable-min-row',
      'variable-max-row', 'variable-enum-row']) {
      const el = variableDialogEl(id);
      if (el) el.hidden = isOcr;
    }
    const runnerEditable = variableDialogEl('variable-runner-editable');
    if (runnerEditable) {
      runnerEditable.disabled = isRuntime;
      if (isRuntime) runnerEditable.checked = false;
    }
    // 识别文字变量：key 跟随所选素材 param_key，default 跟随 runtime_text。
    if (isOcr) {
      const assetSel = variableDialogEl('variable-ocr-asset');
      const asset = assetById(B.assets, assetSel ? assetSel.value : '');
      const keyInput = variableDialogEl('variable-key');
      if (keyInput && asset) keyInput.value = paramSafeKey(asset.param_key);
      const defaultInput = variableDialogEl('variable-default');
      if (defaultInput && asset) {
        defaultInput.value = String(asset.runtime_text === undefined || asset.runtime_text === null
          ? '' : asset.runtime_text);
      }
    }
  }

  function readVariableForm() {
    const val = (id) => {
      const el = variableDialogEl(id);
      return el && el.value !== undefined && el.value !== null ? String(el.value) : '';
    };
    const checked = (id) => {
      const el = variableDialogEl(id);
      return !!(el && el.checked);
    };
    const source = variableDialogEl('variable-source');
    return {
      key: val('variable-key').trim(),
      type: val('variable-type') || 'string',
      value_source: source && source.value === 'ocr_text' ? 'ocr_text' : 'literal',
      asset_id: val('variable-ocr-asset'),
      defaultText: val('variable-default'),
      min: val('variable-min'),
      max: val('variable-max'),
      enumText: val('variable-enum'),
      label: val('variable-label').trim(),
      description: val('variable-description').trim(),
      group: val('variable-group').trim(),
      runnerEditable: checked('variable-runner-editable'),
      required: checked('variable-required'),
      scope: val('variable-scope') === 'runtime' ? 'runtime' : 'input',
      editingKey: variableDialogCtx ? variableDialogCtx.editingKey : '',
    };
  }

  async function saveVariableDialog() {
    const ctx = variableDialogCtx;
    if (!ctx) return;
    const form = readVariableForm();
    const existingKeys = B.variables.map(function (v) {
      return v && v.key !== undefined && v.key !== null ? String(v.key) : '';
    }).filter(Boolean);
    const built = variableFormToBody(form, {
      existingKeys: existingKeys,
      editingKey: ctx.editingKey,
      assets: B.assets,
    });
    const errorEl = variableDialogEl('variable-error');
    if (!built.ok) {
      if (errorEl) errorEl.textContent = built.errors.join('\n');
      return;
    }
    const next = B.variables.slice();
    const idx = ctx.editingKey
      ? next.findIndex(function (v) { return v && String(v.key) === ctx.editingKey; })
      : -1;
    if (idx >= 0) next[idx] = built.variable;
    else next.push(built.variable);
    const r = await saveTaskVariables(next);
    if (!r.ok) {
      if (errorEl) errorEl.textContent = r.error;
      return;
    }
    closeVariableDialog();
    status((ctx.editingKey ? '已更新变量 ' : '已新建变量 ') + built.variable.key, '#7bd88f');
    if (r.issues && r.issues.length) {
      status('变量已保存（有校验提示：' + r.issues.length + ' 条，见变量面板）', '#e8c46a');
    }
  }

  // v1.2.2 (N47): structured editors for the two complex actions. Raw args JSON
  // stays below as the escape hatch for multi-locator chains / advanced fields.
  // U-T5：旧入口委托到新的纯函数 targetLocatorForAsset（原先无人调用，保留兼容）。
  function locatorFromAsset(a) {
    const result = targetLocatorForAsset(a);
    return result.ok ? result.locator : null;
  }

  // --- U-T5（API 无关纯函数）：动作目标选择模型 -------------------------------
  // 动作选目标=素材库素材（含 ocr_var），不再有「参数/操作区」双选；region 素材
  // 只作操作区链末端，永远不能作动作目标。纯函数，ui_pure_test 直接抽取执行；
  // 产品 UI（目标选择对话框/属性面板）等 U-T5 一次性接线，这里不接线。

  // 可作动作目标的 kind → 中文标签（region 仅作操作区末端，不在目标白名单）。
  function actionTargetKindLabel(kind) {
    switch (String(kind === undefined || kind === null ? '' : kind).trim().toLowerCase()) {
      case 'template': return '模板';
      case 'template_set': return '素材集合';
      case 'ocr_line': return 'OCR 行';
      case 'ocr_lines': return 'OCR 多行';
      case 'ocr_var': return '使用端OCR文字配置项';
      case 'region': return '操作区域（仅操作区末端）';
      default: return '其他';
    }
  }

  // 动作目标允许的 kind 白名单（模板/集合/OCR 行/OCR 多行/使用端OCR文字配置项）。
  function actionTargetKinds() {
    return ['template', 'template_set', 'ocr_line', 'ocr_lines', 'ocr_var'];
  }

  // 素材库条目 → 目标选择器候选（可搜索；region 素材列出但标注「仅操作区末端」
  // 并带 terminalOnly=true，选择时 targetLocatorForAsset 会报错拒绝）。
  // opts.kindFilter：数组或 (kind)=>boolean，用于进一步收窄（如只显示 OCR 类）。
  // 返回 [{id,label,kind,kindLabel,paramKey?,ocrText?,terminalOnly?}]。
  function assetTargetOptions(assets, opts) {
    const o = opts && typeof opts === 'object' ? opts : {};
    const filterFn = typeof o.kindFilter === 'function' ? o.kindFilter : null;
    const filterList = Array.isArray(o.kindFilter) ? o.kindFilter : null;
    const allowed = actionTargetKinds();
    const out = [];
    const seen = {};
    for (const asset of Array.isArray(assets) ? assets : []) {
      if (!asset || typeof asset !== 'object' || Array.isArray(asset)) continue;
      const id = String(asset.id === undefined || asset.id === null ? '' : asset.id).trim();
      if (!id || seen[id]) continue;
      const kind = String(asset.kind === undefined || asset.kind === null ? '' : asset.kind).trim().toLowerCase();
      const terminalOnly = kind === 'region';
      if (!terminalOnly && allowed.indexOf(kind) < 0) continue;
      if (filterList && filterList.indexOf(kind) < 0) continue;
      if (filterFn && !filterFn(kind)) continue;
      seen[id] = true;
      const rawLabel = asset.label === undefined || asset.label === null ? '' : String(asset.label).trim();
      const option = {
        id: id,
        label: rawLabel || id,
        kind: kind,
        kindLabel: actionTargetKindLabel(kind),
      };
      if (terminalOnly) {
        option.terminalOnly = true;
      } else if (kind === 'ocr_var') {
        const paramKey = String(asset.param_key === undefined || asset.param_key === null
          ? '' : asset.param_key).trim();
        if (paramKey) option.paramKey = paramKey;
        const runtimeText = String(asset.runtime_text === undefined || asset.runtime_text === null
          ? '' : asset.runtime_text).trim();
        if (runtimeText) option.ocrText = runtimeText;
      } else if (asset.ocr_text !== undefined && asset.ocr_text !== null &&
          String(asset.ocr_text).trim() !== '') {
        option.ocrText = String(asset.ocr_text).trim();
      }
      out.push(option);
    }
    return out;
  }

  // 素材 → click_target/long_press 的 find[0] locator（纯函数）。
  //   ocr_var → {kind:"ocr_text", sel, text_expr:"param('<param_key>')"}（缺键报错）
  //   普通 OCR → {kind:"ocr_text", sel, text}（opts.text 覆盖；缺文字报错，不编造）
  //   模板/集合 → {kind:"template", sel}（集合按 id 由 oas-task 展开成员）
  //   其它（region/未知）→ 错误（region 只作操作区末端）。
  function targetLocatorForAsset(asset, opts) {
    const it = asset && typeof asset === 'object' && !Array.isArray(asset) ? asset : {};
    const o = opts && typeof opts === 'object' ? opts : {};
    const id = String(it.id === undefined || it.id === null ? '' : it.id).trim();
    if (!id) return { ok: false, locator: null, error: '动作目标素材 id 为空' };
    const kind = String(it.kind === undefined || it.kind === null ? '' : it.kind).trim().toLowerCase();
    const userText = o.text === undefined || o.text === null ? '' : String(o.text).trim();
    if (kind === 'template' || kind === 'template_set') {
      return { ok: true, locator: { kind: 'template', sel: id }, error: '' };
    }
    if (kind === 'ocr_var') {
      const paramKey = paramSafeKey(it.param_key);
      if (!paramKey) {
        return {
          ok: false, locator: null,
          error: '识别文字配置项「' + id + '」缺少 param_key，无法生成 param() 文字来源',
        };
      }
      return {
        ok: true,
        locator: { kind: 'ocr_text', sel: id, text_expr: paramTextExpr(paramKey) },
        error: '',
      };
    }
    if (kind === 'ocr_line' || kind === 'ocr_lines') {
      const text = userText || String(it.ocr_text === undefined || it.ocr_text === null
        ? '' : it.ocr_text).trim();
      if (!text) {
        return {
          ok: false, locator: null,
          error: 'OCR 素材「' + id + '」缺少识别文字（text）；请填写文字，或改用「使用端OCR文字配置项」素材',
        };
      }
      return { ok: true, locator: { kind: 'ocr_text', sel: id, text: text }, error: '' };
    }
    return {
      ok: false, locator: null,
      error: '素材「' + id + '」（' + (kind || '未知类型') + '）不能作为动作目标' +
        (kind === 'region' ? '：region 只作操作区末端' : ''),
    };
  }

  // 保存前校验动作目标（errors[] 为空=通过）。targetParamKeys = 当前任务的派生
  // 参数 key 集合（meta.params 中带 asset_id 的行）；未传=跳过「是否已派生」检查。
  function validateActionTarget(asset, targetParamKeys) {
    const it = asset && typeof asset === 'object' && !Array.isArray(asset) ? asset : {};
    const id = String(it.id === undefined || it.id === null ? '' : it.id).trim();
    if (!id) return ['动作目标素材 id 为空'];
    const errors = [];
    const kind = String(it.kind === undefined || it.kind === null ? '' : it.kind).trim().toLowerCase();
    if (actionTargetKinds().indexOf(kind) < 0) {
      errors.push('素材「' + id + '」（' + (kind || '未知类型') + '）不能作为动作目标' +
        (kind === 'region' ? '：region 只作操作区末端' : ''));
      return errors;
    }
    if (kind === 'ocr_var') {
      const paramKey = paramSafeKey(it.param_key);
      if (!paramKey) {
        errors.push('识别文字配置项「' + id + '」缺少 param_key');
      } else if (Array.isArray(targetParamKeys) && targetParamKeys.indexOf(paramKey) < 0) {
        errors.push("param('" + paramKey + "') 不在任务派生参数中：请先保存素材以派生 meta.params");
      }
    } else if (kind === 'ocr_line' || kind === 'ocr_lines') {
      const text = String(it.ocr_text === undefined || it.ocr_text === null ? '' : it.ocr_text).trim();
      if (!text) errors.push('OCR 素材「' + id + '」缺少 ocr_text（也可在动作里直接填写 text）');
    }
    return errors;
  }

  // 动作属性面板的目标摘要（含「识别文字来源：素材 xxx」）。
  function targetPickerSummary(asset) {
    const it = asset && typeof asset === 'object' && !Array.isArray(asset) ? asset : null;
    if (!it) return '未选择动作目标';
    const id = String(it.id === undefined || it.id === null ? '' : it.id).trim();
    if (!id) return '未选择动作目标';
    const kind = String(it.kind === undefined || it.kind === null ? '' : it.kind).trim().toLowerCase();
    const kindLabel = actionTargetKindLabel(kind);
    if (kind === 'ocr_var') {
      const paramKey = paramSafeKey(it.param_key);
      const runtimeText = String(it.runtime_text === undefined || it.runtime_text === null
        ? '' : it.runtime_text).trim();
      let text = '目标：素材「' + id + '」（' + kindLabel + '） · 识别文字来源：素材「' + id + '」';
      text += paramKey ? ' → ' + paramTextExpr(paramKey) : '（⚠ 缺少 param_key）';
      if (runtimeText) text += ' · 默认「' + runtimeText + '」';
      return text;
    }
    if (kind === 'ocr_line' || kind === 'ocr_lines') {
      const text = String(it.ocr_text === undefined || it.ocr_text === null ? '' : it.ocr_text).trim();
      return '目标：素材「' + id + '」（' + kindLabel + '） · 识别文字来源：素材「' + id + '」' +
        (text ? '固定文字「' + text + '」' : '未固定识别文字（动作里填写 text）');
    }
    if (kind === 'template' || kind === 'template_set') {
      return '目标：素材「' + id + '」（' + kindLabel + '） · 识别来源：模板匹配';
    }
    return '素材「' + id + '」（' + kindLabel + '）不能作为动作目标' +
      (kind === 'region' ? '：region 只作操作区末端' : '');
  }

  // 派生参数行（meta.params 中带 asset_id 的行）直接给出素材 id / 展示名。
  function paramAssetId(row) {
    const r = row && typeof row === 'object' && !Array.isArray(row) ? row : {};
    const raw = r.asset_id === undefined || r.asset_id === null ? '' : String(r.asset_id).trim();
    return raw;
  }

  function paramAssetLabel(row, assets) {
    const assetId = paramAssetId(row);
    if (!assetId) return '';
    const list = Array.isArray(assets) ? assets : [];
    const asset = list.find(function (a) { return a && String(a.id) === assetId; });
    if (asset) {
      const label = String(asset.label === undefined || asset.label === null ? '' : asset.label).trim();
      return label || assetId;
    }
    return assetId;
  }

  // 当前任务的派生参数 key 集合（ocr_var 的 param_key 必须落在其中）。
  function derivedParamKeys(params) {
    const list = Array.isArray(params) ? params : [];
    const keys = [];
    for (const row of list) {
      const assetId = paramAssetId(row);
      if (!assetId) continue;
      const key = paramSafeKey(row && row.key);
      if (key && keys.indexOf(key) < 0) keys.push(key);
    }
    return keys;
  }

  // D 批（去参数 UI）：动作目标校验不再依赖 #builder-params 面板/GET task config，
  // 直接从素材库的 ocr_var 素材推导派生参数行（asset_id + param_key）。
  function ocrVarParamRows(assets) {
    const rows = [];
    for (const asset of Array.isArray(assets) ? assets : []) {
      if (!asset || asset.kind !== 'ocr_var') continue;
      const key = paramSafeKey(asset.param_key);
      if (!key) continue;
      rows.push({ key: key, asset_id: String(asset.id) });
    }
    return rows;
  }

  function assetById(assets, id) {
    const key = String(id === undefined || id === null ? '' : id).trim();
    if (!key) return null;
    const list = Array.isArray(assets) ? assets : [];
    return list.find(function (a) { return a && String(a.id) === key; }) || null;
  }

  // --- V2.0c：变量系统纯函数（meta.variables 是 flow param('key') 的唯一声明） -----
  // Variable: key/type/value_source(literal|ocr_text)/asset_id?/default/label?/
  // description?/group?/runner_editable/required/scope(input|runtime)/min?/max?/enum?
  function variableKeyValid(key) {
    return /^[A-Za-z_][A-Za-z0-9_]{0,63}$/.test(String(key === undefined || key === null ? '' : key));
  }

  function variableSourceOf(v) {
    const it = v && typeof v === 'object' ? v : {};
    return String(it.value_source === undefined || it.value_source === null
      ? 'literal' : it.value_source) === 'ocr_text' ? 'ocr_text' : 'literal';
  }

  function variableScopeOf(v) {
    const it = v && typeof v === 'object' ? v : {};
    return String(it.scope === undefined || it.scope === null ? 'input' : it.scope) === 'runtime'
      ? 'runtime' : 'input';
  }

  function variableTypeLabel(type) {
    switch (String(type === undefined || type === null ? '' : type)) {
      case 'int': return '整数';
      case 'float': return '小数';
      case 'number': return '数字';
      case 'bool': return '布尔';
      case 'string': return '文本';
      default: return String(type || '文本');
    }
  }

  // 列表「类型+来源」列：识别文字变量 / 文本 / 整数 / 布尔 / 运行变量。
  function variableKindText(v) {
    const it = v && typeof v === 'object' ? v : {};
    if (variableSourceOf(it) === 'ocr_text') return '识别文字变量';
    if (variableScopeOf(it) === 'runtime') return '运行变量';
    if (Array.isArray(it.enum) && it.enum.length) return '枚举';
    return variableTypeLabel(it.type);
  }

  // 可插入表达式的变量（scope=input；runtime 变量不暴露给使用端但 flow 内可用，
  // 这里只列 input，保持与「插入变量」语义简单一致）。
  function variablesForInsert(variables) {
    const list = Array.isArray(variables) ? variables : [];
    return list.filter(function (v) {
      return v && typeof v === 'object' && variableScopeOf(v) === 'input' &&
        variableKeyValid(v.key);
    });
  }

  function variablesParamKeys(variables) {
    const keys = [];
    for (const v of variablesForInsert(variables)) {
      const key = paramSafeKey(v.key);
      if (key && keys.indexOf(key) < 0) keys.push(key);
    }
    return keys;
  }

  // 动作目标校验用「派生参数行」形状：优先 variables，资产 fallback（D 批兼容）。
  function variablesParamRows(variables) {
    return variablesForInsert(variables).map(function (v) {
      const row = { key: paramSafeKey(v.key) };
      const assetId = paramAssetId(v);
      if (assetId) row.asset_id = assetId;
      return row;
    });
  }

  function actionTargetParamRows(variables, assets) {
    const rows = variablesParamRows(variables);
    return rows.length ? rows : ocrVarParamRows(assets);
  }

  // flow 中 param('<key>') 的引用次数（面板「引用」列 + 删除提示）。
  function variableUseCount(flow, key) {
    const target = paramSafeKey(key);
    if (!target) return 0;
    let count = 0;
    for (const use of collectParamUses(flow)) {
      if (use && use.key === target) count += 1;
    }
    return count;
  }

  function variableDefaultText(v) {
    const it = v && typeof v === 'object' ? v : {};
    if (it.default === undefined || it.default === null) return '—';
    if (it.default === '') return '（空字符串）';
    if (typeof it.default === 'string') return it.default;
    if (typeof it.default === 'number' || typeof it.default === 'boolean') return String(it.default);
    try { return JSON.stringify(it.default); } catch (e) { return String(it.default); }
  }

  function variableRowSummary(v) {
    const it = v && typeof v === 'object' ? v : {};
    const key = String(it.key === undefined || it.key === null ? '' : it.key);
    return key + ' · ' + variableKindText(it);
  }

  // 表单 → Variable body（纯函数；错误数组返回给对话框）。
  // form: {key,type,value_source,asset_id,defaultText,label,description,group,
  //        runnerEditable,required,scope,min,max,enumText}
  function variableFormToBody(form, opts) {
    const f = form && typeof form === 'object' ? form : {};
    const o = opts && typeof opts === 'object' ? opts : {};
    const errors = [];
    const key = String(f.key === undefined || f.key === null ? '' : f.key).trim();
    if (!key) errors.push('key 必填');
    else if (!variableKeyValid(key)) errors.push('key 非法（[A-Za-z_][A-Za-z0-9_]*，≤64 字符）');
    const existing = Array.isArray(o.existingKeys) ? o.existingKeys : [];
    if (key && o.editingKey !== key && existing.indexOf(key) >= 0) {
      errors.push('key「' + key + '」已被其它变量使用');
    }
    const source = f.value_source === 'ocr_text' ? 'ocr_text' : 'literal';
    const scope = f.scope === 'runtime' ? 'runtime' : 'input';
    const type = String(f.type === undefined || f.type === null ? 'string' : f.type);
    const variable = {
      key: key,
      type: type,
      value_source: source,
      runner_editable: source === 'ocr_text' ? f.runnerEditable !== false : f.runnerEditable !== false,
      required: f.required === true,
      scope: scope,
    };
    if (scope === 'runtime') variable.runner_editable = false;
    if (f.label) variable.label = String(f.label);
    if (f.description) variable.description = String(f.description);
    if (f.group) variable.group = String(f.group);
    if (source === 'ocr_text') {
      const assetId = String(f.asset_id === undefined || f.asset_id === null ? '' : f.asset_id).trim();
      if (!assetId) errors.push('识别文字变量必须绑定一个 ocr_var 素材');
      const asset = assetById(o.assets, assetId);
      if (assetId && !asset) errors.push('绑定的素材「' + assetId + '」不在素材库');
      else if (asset && String(asset.kind) !== 'ocr_var') errors.push('绑定素材必须是「使用端OCR文字配置项」（ocr_var）');
      else if (asset) {
        const assetKey = paramSafeKey(asset.param_key);
        if (!assetKey) errors.push('绑定素材缺少 param_key');
        else if (assetKey !== key) errors.push('key 必须等于素材 param_key「' + assetKey + '」');
      }
      variable.asset_id = assetId;
      if (asset) variable.default = String(asset.runtime_text === undefined || asset.runtime_text === null
        ? '' : asset.runtime_text);
    } else {
      const dtype = String(f.type === undefined || f.type === null ? 'string' : f.type);
      const parsed = literalDefaultFromText(f.defaultText, dtype, f.enumText);
      if (!parsed.ok) errors.push(parsed.error);
      else if (parsed.value !== undefined) variable.default = parsed.value;
      if (dtype === 'int') {
        if (f.min !== undefined && f.min !== null && String(f.min) !== '' && Number.isFinite(Number(f.min))) {
          variable.min = Math.round(Number(f.min));
        }
        if (f.max !== undefined && f.max !== null && String(f.max) !== '' && Number.isFinite(Number(f.max))) {
          variable.max = Math.round(Number(f.max));
        }
      } else if (dtype === 'float' || dtype === 'number') {
        if (String(f.min || '') !== '' && Number.isFinite(Number(f.min))) variable.min = Number(f.min);
        if (String(f.max || '') !== '' && Number.isFinite(Number(f.max))) variable.max = Number(f.max);
      }
      if (dtype === 'bool') {
        // bool 不支持 enum/min/max。
      } else if (String(f.enumText || '').trim()) {
        const values = String(f.enumText).split(/[,，、\n]/).map((x) => x.trim()).filter(Boolean);
        if (values.length) variable.enum = values;
      }
    }
    return { ok: errors.length === 0, errors: errors, variable: variable };
  }

  function literalDefaultFromText(text, type, enumText) {
    const raw = String(text === undefined || text === null ? '' : text).trim();
    if (type === 'bool') {
      if (raw === '') return { ok: true, value: undefined };
      if (raw === 'true' || raw === '是') return { ok: true, value: true };
      if (raw === 'false' || raw === '否') return { ok: true, value: false };
      return { ok: false, error: '布尔默认值必须是 true/false（或留空）' };
    }
    if (type === 'int') {
      if (raw === '') return { ok: true, value: undefined };
      const n = Number(raw);
      if (!Number.isInteger(n)) return { ok: false, error: '整数默认值必须是整数' };
      return { ok: true, value: n };
    }
    if (type === 'float' || type === 'number') {
      if (raw === '') return { ok: true, value: undefined };
      const n = Number(raw);
      if (!Number.isFinite(n)) return { ok: false, error: '数字默认值必须是数字' };
      return { ok: true, value: n };
    }
    const allowed = String(enumText || '').trim()
      ? String(enumText).split(/[,，、\n]/).map((x) => x.trim()).filter(Boolean)
      : null;
    if (allowed && allowed.length && allowed.indexOf(raw) < 0) {
      return { ok: false, error: '文本默认值必须是枚举成员之一' };
    }
    return { ok: true, value: raw };
  }

  function variableToForm(v) {
    const it = v && typeof v === 'object' ? v : {};
    const dtype = String(it.type === undefined || it.type === null ? 'string' : it.type);
    const source = variableSourceOf(it);
    return {
      key: String(it.key === undefined || it.key === null ? '' : it.key),
      type: dtype,
      value_source: source,
      asset_id: paramAssetId(it),
      defaultText: it.default === undefined || it.default === null
        ? '' : (typeof it.default === 'string' ? it.default : String(it.default)),
      label: String(it.label === undefined || it.label === null ? '' : it.label),
      description: String(it.description === undefined || it.description === null ? '' : it.description),
      group: String(it.group === undefined || it.group === null ? '' : it.group),
      runnerEditable: it.runner_editable !== false,
      required: it.required === true,
      scope: variableScopeOf(it),
      min: it.min === undefined || it.min === null ? '' : String(it.min),
      max: it.max === undefined || it.max === null ? '' : String(it.max),
      enumText: Array.isArray(it.enum) ? it.enum.join(', ') : '',
      editingKey: String(it.key === undefined || it.key === null ? '' : it.key),
    };
  }

  // 保存前校验动作目标（S7/U-T5）：目标素材存在；模板 locator 的 sel 必须存在且
  // 不是 region 素材；OCR locator 的 sel（ocr_var）必须有 param_key，且 text_expr
  // 里 param(key) 必须在派生参数集合中；SEL_OPS 的 args.sel 也必须存在。
  function actionTargetIssues(flow, assets, params) {
    const issues = [];
    const list = Array.isArray(assets) ? assets : [];
    const derived = derivedParamKeys(params);
    const seen = {};
    function push(path, message) {
      const key = path + '\u0000' + message;
      if (seen[key]) return;
      seen[key] = true;
      issues.push(path + '：' + message);
    }
    function checkOcrLocator(loc, path) {
      if (!loc || typeof loc !== 'object' || loc.kind !== 'ocr_text') return;
      const sel = String(loc.sel === undefined || loc.sel === null ? '' : loc.sel).trim();
      if (sel) {
        const asset = assetById(list, sel);
        if (!asset) {
          push(path, '目标素材「' + sel + '」不在当前素材库（本步会失败）');
          return;
        }
        const kind = String(asset.kind === undefined || asset.kind === null ? '' : asset.kind);
        if (kind === 'ocr_var') {
          const paramKey = paramSafeKey(asset.param_key);
          if (!paramKey) {
            push(path, '目标「' + sel + '」是识别文字配置项但缺少 param_key（先保存素材以派生 meta.params）');
            return;
          }
          const locatorKey = locatorParamKey(loc);
          if (!locatorKey) {
            push(path, '目标「' + sel + '」是识别文字配置项，文字来源应为 text_expr:"param(\'' +
              paramKey + '\')"（当前未使用 param()）');
          } else {
            if (locatorKey !== paramKey) {
              push(path, '目标「' + sel + '」的 param_key 是「' + paramKey +
                '」，但 text_expr 引用的是「' + locatorKey + '」');
            }
            if (derived.indexOf(locatorKey) < 0) {
              push(path, 'param(\'' + locatorKey + '\') 不在任务派生参数中：先保存对应识别文字配置项素材');
            }
          }
        }
      }
    }
    function walk(node, path) {
      if (node === null || node === undefined) return;
      if (Array.isArray(node)) {
        node.forEach(function (child, i) { walk(child, path + '[' + i + ']'); });
        return;
      }
      if (typeof node !== 'object') return;
      if (node.kind === 'ocr_text') checkOcrLocator(node, path);
      if (node.kind === 'template') {
        const sel = String(node.sel === undefined || node.sel === null ? '' : node.sel).trim();
        if (sel) {
          const asset = assetById(list, sel);
          if (!asset) push(path, '目标素材「' + sel + '」不在当前素材库（本步会失败）');
          else if (String(asset.kind) === 'region') {
            push(path, 'region 素材「' + sel + '」不能作为模板目标（只作操作区末端）');
          }
        }
      }
      if (node.kind === 'region') {
        const sel = String(node.sel === undefined || node.sel === null ? '' : node.sel).trim();
        if (sel) {
          const asset = assetById(list, sel);
          if (!asset) push(path, '操作区域素材「' + sel + '」不在当前素材库（本步会失败）');
          else if (String(asset.kind) !== 'region') {
            push(path, 'region 定位器引用的素材「' + sel + '」不是 region 素材');
          }
        }
      }
      for (const key of Object.keys(node)) walk(node[key], path ? path + '.' + key : key);
    }
    walk({ steps: flow && flow.steps }, 'flow');
    // SEL_OPS 的 args.sel（appear/ocr/click/dismiss_popup/ocr_to_var）。
    function walkSel(steps, basePath) {
      if (!Array.isArray(steps)) return;
      steps.forEach(function (step, i) {
        if (!step || typeof step !== 'object') return;
        const at = basePath + '[' + i + ']';
        const sel = String(step.args && step.args.sel !== undefined && step.args.sel !== null
          ? step.args.sel : '').trim();
        if (sel && sel.indexOf('lib:') !== 0 && step.op !== 'click_target' && step.op !== 'long_press') {
          if (!assetById(list, sel)) {
            push(at + '.args.sel', '目标素材「' + sel + '」不在当前素材库（本步会失败）');
          }
        }
        for (const key of ['then', 'else', 'body']) walkSel(step[key], at + '.' + key);
      });
    }
    walkSel(flow && flow.steps, 'flow.steps');
    return issues;
  }

  // /task/save_flow 成功响应里的 sync 摘要 → 状态栏文案（无变化=空串）。
  function saveSyncText(sync) {
    const s = sync && typeof sync === 'object' && !Array.isArray(sync) ? sync : null;
    if (!s) return '';
    const added = Array.isArray(s.added) ? s.added.length : 0;
    const updated = Array.isArray(s.updated) ? s.updated.length : 0;
    const removed = Array.isArray(s.removed) ? s.removed.length : 0;
    const total = added + updated + removed;
    if (!total) return '';
    return '已同步 ' + total + ' 个识别文字配置项派生参数（+' + added + ' ~' + updated + ' -' + removed + '）';
  }

  // 旧 flow 的 locator.region（操作区域）只做兼容显示：新步骤不再生成 region。
  function locatorHasLegacyRegion(locator) {
    return !!locator && typeof locator === 'object' && !Array.isArray(locator) &&
      Object.prototype.hasOwnProperty.call(locator, 'region');
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

  // --- V2.0c：属性面板「插入变量 ▾」 ------------------------------------------

  // Track the last focused expression field so the params-panel row button can
  // insert into it (or copy to the clipboard when no field was touched yet).
  function trackExprField(field, mode) {
    if (!field || typeof field.addEventListener !== 'function') return;
    field.addEventListener('focus', function () {
      B.lastExprField = field;
      B.lastExprMode = mode || 'expr';
    });
  }

  function insertTextAtField(field, text, mode) {
    if (!field) return false;
    const value = String(field.value === undefined || field.value === null ? '' : field.value);
    const start = typeof field.selectionStart === 'number' ? field.selectionStart : value.length;
    const end = typeof field.selectionEnd === 'number' ? field.selectionEnd : start;
    const ins = exprInsertText(value, start, end, text, mode || 'expr');
    field.value = ins.text;
    if (typeof field.setSelectionRange === 'function') {
      try { field.setSelectionRange(ins.caret, ins.caret); } catch (e) { /* cosmetic */ }
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

  // 「插入变量 ▾」（V2.0c）：列出 meta.variables 中 scope=input 的变量，点击把
  // param('<key>') 插入到 getField() 返回的输入框光标处；ocr_text 变量显示素材信息。
  // 无变量时禁用并提示先到变量面板创建。
  function variableInsertWidget(mode, getField) {
    const items = variablesForInsert(B.variables);
    const wrap = document.createElement('span');
    wrap.className = 'builder-insert-param';
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.textContent = '插入变量 ▾';
    btn.disabled = !items.length;
    btn.title = items.length
      ? "把 param('<key>') 插入到当前表达式输入框（变量来自构建页「变量」面板）"
      : '无可用变量：先在构建页「变量」面板点「新建变量…」创建';
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
        insertTextAtField(field, snippet, mode);
        status('已插入 ' + snippet);
      } else if (copyTextBestEffort(snippet)) {
        status('已复制到剪贴板：' + snippet);
      } else {
        status('变量片段：' + snippet);
      }
      close();
    }
    function renderMenu() {
      menu.textContent = '';
      const head = document.createElement('div');
      head.className = 'hd';
      head.textContent = '插入变量（param(key)；变量在构建页「变量」面板声明）';
      menu.appendChild(head);
      if (!items.length) {
        const empty = document.createElement('div');
        empty.className = 'hd';
        empty.textContent = '（无可用变量：先在「变量」面板新建变量）';
        menu.appendChild(empty);
        return;
      }
      for (const variable of items) {
        const key = paramSafeKey(variable.key);
        if (!key) continue;
        const item = document.createElement('button');
        item.type = 'button';
        item.className = 'builder-insert-param-item';
        const code = document.createElement('code');
        code.textContent = paramInsertSnippet(key, mode);
        item.appendChild(code);
        const grp = document.createElement('span');
        grp.className = 'grp';
        const isOcr = variableSourceOf(variable) === 'ocr_text';
        const asset = isOcr ? assetById(B.assets, paramAssetId(variable)) : null;
        grp.textContent = (variable.label || key) + ' · ' + variableKindText(variable) +
          (isOcr
            ? ' · 素材 ' + (asset ? (asset.label || asset.id) : (paramAssetId(variable) || '?')) +
              (variable.default !== undefined && variable.default !== null
                ? ' · 「' + variableDefaultText(variable) + '」' : '')
            : ' · 默认 ' + variableDefaultText(variable));
        item.appendChild(grp);
        item.addEventListener('click', function (ev) {
          ev.stopPropagation();
          insert(key);
        });
        menu.appendChild(item);
      }
    }
    btn.addEventListener('click', function (ev) {
      ev.stopPropagation();
      if (!items.length) return;
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

  // P6/D：通用「文字来源」编辑器（OCR locator）。click_target find[0]、
  // requires/expect 守卫、drag from/to 端点共用：固定文字写 text；表达式写
  // text_expr（互斥）。旧 flow 的 text_expr:"param('x')" 以「旧参数来源（只读）」
  // 回显：用户不主动改来源就原样保留（旧 flow 继续可运行）；新 flow 的识别文字
  // 配置项引用由「动作目标」选择器生成 text_expr，所以这里不再有 param 选项。
  // 控件变化时回调 onChange，由调用方用 paramSourceApply 写回（保留未知字段）。
  // 返回 {host, read(): {mode:'text'|'expr'|'keep', value}}。
  function locatorTextSourceFields(base, onChange) {
    const loc = base && typeof base === 'object' && !Array.isArray(base) ? base : {};
    const host = document.createElement('div');
    host.className = 'builder-text-source';
    const src = paramSourceFromLocator(loc);
    // seq130（#3）：param(key) 已声明为变量 → 正常表达式来源；仅未声明才是旧参数只读。
    const declaredParam = src.mode === 'param' && paramKeyDeclaredIn(src.value, B.variables, B.assets);
    const legacyParam = src.mode === 'param' && !declaredParam;
    const currentMode = legacyParam ? 'keep' : (declaredParam ? 'expr' : src.mode);
    const srcValue = declaredParam ? paramTextExpr(src.value || '') : src.value;

    const modeLab = document.createElement('label');
    modeLab.textContent = '文字来源（text / text_expr 互斥）';
    const modeSel = document.createElement('select');
    const modeOptions = [];
    if (legacyParam) modeOptions.push(['keep', '旧参数来源（只读）']);
    modeOptions.push(['text', '固定文字（写 text）'], ['expr', '表达式（写 text_expr）']);
    for (const [value, label] of modeOptions) {
      const o = document.createElement('option');
      o.value = value;
      o.textContent = label;
      modeSel.appendChild(o);
    }
    modeSel.value = currentMode;
    modeLab.appendChild(modeSel);
    host.appendChild(modeLab);

    let fixedInput = null;
    let exprInput = null;
    if (currentMode === 'expr') {
      const lab = document.createElement('label');
      lab.textContent = '文字表达式（text_expr）';
      const exprRow = document.createElement('span');
      exprRow.className = 'builder-expr-row';
      exprInput = document.createElement('input');
      exprInput.type = 'text';
      exprInput.spellcheck = false;
      exprInput.value = srcValue;
      exprRow.appendChild(exprInput);
      exprRow.appendChild(variableInsertWidget('expr', function () { return exprInput; }));
      lab.appendChild(exprRow);
      host.appendChild(lab);
      trackExprField(exprInput, 'expr');
    } else if (currentMode === 'text') {
      const lab = document.createElement('label');
      lab.textContent = 'OCR 文字（固定字面量）';
      fixedInput = document.createElement('input');
      fixedInput.type = 'text';
      fixedInput.value = srcValue;
      lab.appendChild(fixedInput);
      host.appendChild(lab);
    } else if (legacyParam) {
      const lab = document.createElement('label');
      lab.textContent = '旧参数来源（只读）';
      const readonly = document.createElement('input');
      readonly.type = 'text';
      readonly.readOnly = true;
      readonly.value = paramTextExpr(src.value || '');
      lab.appendChild(readonly);
      host.appendChild(lab);
    }

    const hint = document.createElement('div');
    hint.className = 'builder-source-hint';
    const specError = locatorTextSpecError(loc);
    if (declaredParam) {
      hint.textContent = '已声明变量来源：' + paramTextExpr(src.value || '') +
        '（构建页「变量」面板；正常引用，不再显示旧参数提示）。';
    } else if (specError) {
      hint.textContent = '现有值无效：' + specError + '；改动后会按互斥规则覆盖。';
      hint.className += ' warn';
    } else if (legacyParam) {
      hint.textContent = '旧参数来源（只读）：' + paramTextExpr(src.value || '') +
        '；不修改则原样保留（旧 flow 仍可运行）。选择其它来源会替换该表达式。';
    }
    host.appendChild(hint);

    function read() {
      const mode = modeSel.value;
      let value = '';
      if (mode === 'expr') {
        value = exprInput ? exprInput.value : '';
      } else if (mode === 'text') {
        value = fixedInput ? fixedInput.value : '';
      }
      return { mode: mode, value: value };
    }
    function emit() {
      if (typeof onChange === 'function') onChange(read());
    }
    modeSel.addEventListener('change', emit);
    if (exprInput) exprInput.addEventListener('change', emit);
    if (fixedInput) fixedInput.addEventListener('change', emit);
    return { host: host, read: read };
  }

  // P6：requires / expect 的 OCR 守卫结构化编辑器（click_target 步骤）。
  function renderGuardLocatorEditor(root, step, field) {
    const title = field === 'requires'
      ? '前置守卫 requires（可选；click 前校验）'
      : '结束守卫 expect（可选；click 后校验）';
    const box = formBox(title);
    const guard = stepGuardLocator(step, field);
    if (!guard) {
      const hint = document.createElement('div');
      hint.className = 'builder-guard-hint';
      hint.textContent = '（未设置）：在下方「高级：args JSON」里添加 ' + field +
        '（OCR 守卫形如 {"kind":"ocr_text","sel":"O_x","text":"固定"} 或 text_expr）。';
      box.appendChild(hint);
      root.appendChild(box);
      return;
    }
    if (!locatorIsOcrText(guard)) {
      const hint = document.createElement('div');
      hint.className = 'builder-guard-hint';
      hint.textContent = 'kind=' + String(guard.kind === undefined || guard.kind === null ? '?' : guard.kind) +
        '：非 OCR 守卫请用高级 JSON 编辑（这里保留原字段不改）。';
      box.appendChild(hint);
      root.appendChild(box);
      return;
    }
    const kindHint = document.createElement('div');
    kindHint.className = 'builder-guard-hint';
    kindHint.textContent = 'kind: ocr_text（text / text_expr 互斥必填其一；未知字段保留）';
    box.appendChild(kindHint);

    const assetLab = document.createElement('label');
    assetLab.textContent = 'OCR 素材';
    const searchWrap = assetSearch(guard.sel || '');
    const asset = searchWrap.input;
    assetLab.appendChild(searchWrap);
    box.appendChild(assetLab);

    // timeout_ms is part of the expect shape (and may exist on requires too).
    const hasTimeout = guard.timeout_ms !== undefined && guard.timeout_ms !== null;
    let timeoutControl = null;
    if (field === 'expect' || hasTimeout) {
      const timeoutLab = document.createElement('label');
      timeoutLab.textContent = '超时 ms';
      timeoutControl = document.createElement('input');
      timeoutControl.type = 'number';
      timeoutControl.min = '0';
      timeoutControl.step = '100';
      timeoutControl.value = hasTimeout ? String(guard.timeout_ms) : '3000';
      timeoutLab.appendChild(timeoutControl);
      box.appendChild(timeoutLab);
    }

    const textSource = locatorTextSourceFields(guard, function () { sync(); });
    box.appendChild(textSource.host);

    function sync() {
      const next = Object.assign({}, guard, { kind: 'ocr_text' });
      if (asset.value) next.sel = asset.value;
      else delete next.sel;
      if (timeoutControl) {
        const n = Number(timeoutControl.value);
        if (isFinite(n) && n >= 0) next.timeout_ms = Math.round(n);
        else delete next.timeout_ms;
      }
      const text = textSource.read();
      // D 批：旧 param 来源（keep）= 不改写 text/text_expr，只更新 kind/sel 等字段。
      step.args[field] = text.mode === 'keep' ? next : paramSourceApply(next, text.mode, text.value);
      renderAll();
    }
    asset.addEventListener('change', sync);
    if (timeoutControl) timeoutControl.addEventListener('change', sync);
    root.appendChild(box);
  }

  // P1：OCR 定位器的「操作区域」编辑器（click_target.find / drag from|to /
  // long_press.find 共用）。写回 locator.region；非 offset 模式删除 offset；
  // 默认模式删除 region。未知字段保留（调用方用 regionWrite merge）。返回
  // {host, read(): {ok, mode, offset, error}}；非法偏移不回调 onChange（保留原值）。
  function renderRegionEditor(host, locator, onChange) {
    const spec = regionSpec(locator);
    const box = document.createElement('div');
    box.className = 'builder-region-editor';
    const title = document.createElement('div');
    title.className = 'muted small';
    title.textContent = '操作区域（可选；只对 OCR 定位器生效）';
    box.appendChild(title);

    const modeLab = document.createElement('label');
    modeLab.textContent = '区域来源';
    const modeSel = document.createElement('select');
    for (const [value, label] of [
      ['', '默认（跟随素材配置）'],
      ['asset', '素材操作区域'],
      ['ocr', 'OCR 识别区域'],
      ['nearest', '最近匹配'],
      ['offset', '相对偏移 [dx,dy,w,h]'],
    ]) {
      const option = document.createElement('option');
      option.value = value;
      option.textContent = label;
      modeSel.appendChild(option);
    }
    const knownModes = ['', 'asset', 'ocr', 'nearest', 'offset'];
    if (knownModes.indexOf(spec.mode) < 0) {
      const option = document.createElement('option');
      option.value = spec.mode;
      option.textContent = regionModeText(spec.mode) + '（未知模式）';
      modeSel.appendChild(option);
    }
    modeSel.value = knownModes.indexOf(spec.mode) < 0 ? spec.mode : spec.mode;
    modeLab.appendChild(modeSel);
    box.appendChild(modeLab);

    // A 方案：操作区域素材（整个素材库；仅 asset/nearest 模式使用）。
    const opAssetRow = document.createElement('label');
    opAssetRow.className = 'builder-region-asset';
    opAssetRow.textContent = '操作区域素材（可选）';
    const opAssetSel = document.createElement('select');
    opAssetSel.appendChild(new Option('无（用识别素材自身）', ''));
    let assetListed = false;
    for (const group of chooserOperationAssetOptions(B.assets)) {
      const og = document.createElement('optgroup');
      og.label = group.label;
      for (const item of group.items) {
        const option = document.createElement('option');
        option.value = item.id;
        option.textContent = item.id + ' · ' + item.kindText;
        og.appendChild(option);
        if (item.id === spec.asset) assetListed = true;
      }
      opAssetSel.appendChild(og);
    }
    if (spec.asset && !assetListed) {
      const option = document.createElement('option');
      option.value = spec.asset;
      option.textContent = '未找到：' + spec.asset + '（未知素材）';
      opAssetSel.appendChild(option);
    }
    opAssetSel.value = spec.asset || '';
    opAssetRow.appendChild(opAssetSel);
    opAssetRow.hidden = spec.mode !== 'asset' && spec.mode !== 'nearest';
    box.appendChild(opAssetRow);

    const riskNote = document.createElement('div');
    riskNote.className = 'builder-warn-note builder-region-risk';
    riskNote.hidden = !regionMatchRiskText(spec.mode);
    riskNote.textContent = regionMatchRiskText(spec.mode);
    box.appendChild(riskNote);

    const offsetRow = document.createElement('div');
    offsetRow.className = 'builder-region-offset';
    const offsetLabels = ['dx', 'dy', 'w（可空）', 'h（可空）'];
    const offsetInputs = [];
    offsetLabels.forEach(function (labelText, index) {
      const lab = document.createElement('label');
      lab.textContent = labelText;
      const input = document.createElement('input');
      input.type = 'number';
      input.setAttribute('data-offset', String(index));
      const value = spec.offset[index];
      input.value = value === undefined || value === null ? '' : String(value);
      input.placeholder = index < 2 ? '如：0 / -10' : '留空=继承';
      lab.appendChild(input);
      offsetRow.appendChild(lab);
      offsetInputs.push(input);
    });
    offsetRow.hidden = spec.mode !== 'offset';
    box.appendChild(offsetRow);

    const hint = document.createElement('div');
    hint.className = 'builder-region-hint';
    hint.textContent = regionHintText(spec.mode);
    box.appendChild(hint);
    const err = document.createElement('div');
    err.className = 'builder-region-err';
    box.appendChild(err);

    function read() {
      const mode = modeSel.value;
      const opAsset = (mode === 'asset' || mode === 'nearest')
        ? regionAssetFromSelector(opAssetSel.value)
        : '';
      if (!mode) return { ok: true, mode: '', offset: [], asset: '', error: '' };
      if (mode !== 'offset') {
        return { ok: true, mode: mode, offset: [], asset: opAsset, error: '' };
      }
      const parsed = regionOffsetFromInput(
        offsetInputs[0].value,
        offsetInputs[1].value,
        offsetInputs[2].value,
        offsetInputs[3].value
      );
      if (!parsed.ok) {
        return { ok: false, mode: 'offset', offset: [], asset: '', error: parsed.error };
      }
      return { ok: true, mode: 'offset', offset: parsed.offset, asset: '', error: '' };
    }
    function emit() {
      const mode = modeSel.value;
      offsetRow.hidden = mode !== 'offset';
      opAssetRow.hidden = mode !== 'asset' && mode !== 'nearest';
      hint.textContent = regionHintText(mode);
      const risk = regionMatchRiskText(mode);
      riskNote.hidden = !risk;
      riskNote.textContent = risk;
      const result = read();
      err.textContent = result.error || '';
      box.classList.toggle('err', !!result.error);
      if (result.ok && typeof onChange === 'function') onChange(result);
    }
    modeSel.addEventListener('change', emit);
    opAssetSel.addEventListener('change', emit);
    for (const input of offsetInputs) input.addEventListener('change', emit);
    host.appendChild(box);
    return { host: box, read: read };
  }

  // U-T5/S7：动作目标选择（素材库搜索选择器；目标=素材库素材，含 ocr_var）。
  // 选中后按素材 kind 生成 locator（ocr_var → text_expr=param(param_key)）。region
  // 素材不在目标列表（只作操作区末端）；旧 locator 仍由下方「定位方式/文字来源」兼容编辑。
  function actionTargetEditor(root, step) {
    const find = Array.isArray(step.args.find) ? step.args.find : [];
    const currentLocator = find[0] && typeof find[0] === 'object' && !Array.isArray(find[0])
      ? find[0]
      : null;
    const currentSel = currentLocator && currentLocator.sel !== undefined && currentLocator.sel !== null
      ? String(currentLocator.sel).trim()
      : '';
    const currentAsset = assetById(B.assets, currentSel);
    const box = formBox('动作目标（素材库；含「使用端OCR文字配置项」）');
    const summary = document.createElement('div');
    summary.className = 'builder-target-summary';
    summary.textContent = currentAsset
      ? targetPickerSummary(currentAsset)
      : (currentSel ? '当前目标「' + currentSel + '」不在当前素材库' : '未选择目标素材');
    box.appendChild(summary);
    const picker = window.OASAssetUI.createSearchPicker({
      items: assetTargetOptions(B.assets),
      value: '',
      placeholder: '搜索并选择目标素材（模板/集合/OCR/使用端OCR文字配置项）…',
      className: 'builder-asset-search builder-target-search',
      menuClass: 'builder-asset-menu',
      label: function (option) {
        return option.id + ' · ' + option.kindLabel +
          (option.terminalOnly ? '（仅操作区末端，不可作动作目标）' : '') +
          (option.paramKey ? '（param(\'' + option.paramKey + '\')）' : '') +
          (option.ocrText ? ' · 文字「' + option.ocrText + '」' : '');
      },
      onChoose: function (option, input) {
        const asset = assetById(B.assets, option.id);
        if (!asset) return;
        const result = targetLocatorForAsset(asset);
        if (!result.ok) {
          status('目标选择失败：' + result.error, '#ff8a80');
          return;
        }
        const others = (Array.isArray(step.args.find) ? step.args.find : []).slice(1);
        step.args.find = [result.locator].concat(others);
        input.value = option.id;
        status('已设置动作目标：' + option.id + '（' + option.kindLabel + '）');
        renderAll();
      },
    });
    box.appendChild(picker);
    const hint = document.createElement('div');
    hint.className = 'muted small';
    hint.textContent = '目标=素材库素材：模板/集合 → template locator；普通 OCR 固定文字；' +
      '使用端OCR文字配置项 → param(param_key)。region 素材只作操作区末端，不能作动作目标。';
    box.appendChild(hint);
    if (currentAsset) {
      const errors = validateActionTarget(currentAsset, derivedParamKeys(actionTargetParamRows(B.variables, B.assets)));
      if (errors.length) {
        const err = document.createElement('div');
        err.className = 'builder-target-errors';
        err.textContent = '目标校验：' + errors.join('；');
        box.appendChild(err);
      }
    }
    root.appendChild(box);
  }

  // 定位器文字来源三态（P3，P6 起复用 locatorTextSourceFields）：固定文字 /
  // 任务参数 / 表达式。固定写 text，后两者写 text_expr；切换时删除另一字段。
  // 未知 locator 字段（roi / require_template / shape …）必须原样保留（只 merge
  // 已知键，禁止重建对象——旧 Studio 会丢字段）。
  function structuredClickTarget(root, step) {
    // U-T5：先用素材库搜索选择器选目标素材（S7），下面的定位方式/文字来源/旧 region
    // 编辑器保留给高级与旧 flow 兼容编辑。
    actionTargetEditor(root, step);
    const box = formBox('定位对象（find[0]；多定位器/内联 ROI 用下方高级 JSON）');
    const find = Array.isArray(step.args.find) ? step.args.find : [];
    const base = find[0] && typeof find[0] === 'object' && !Array.isArray(find[0])
      ? find[0]
      : { kind: 'template' };
    const loc = Object.assign({ kind: 'template' }, base);
    const legacyRegion = locatorHasLegacyRegion(base);
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

    // seq136（#3）：动作目标 picker 是唯一素材字段（旧 flow 的冗余 sel/素材字段由
    // normalizeLegacyActionTargets 自动推导/忽略）；定位方式/文字来源只读兼容旧 flow。
    let textSource = null;
    let regionEditor = null;
    if (kind.value === 'ocr_text') {
      textSource = locatorTextSourceFields(Object.assign({}, base, { kind: 'ocr_text' }), function () {
        sync();
      });
      box.appendChild(textSource.host);
      if (legacyRegion) {
        // U-T5：旧 flow 才显示 region 编辑器（高级兼容）；新步骤不再生成 region，
        // 操作区由目标素材的 op_region 决定（素材行「操作区」递归对话框）。
        regionEditor = renderRegionEditor(box, Object.assign({}, base, { kind: 'ocr_text' }), function () {
          sync();
        });
        const legacyHint = document.createElement('div');
        legacyHint.className = 'muted small builder-legacy-region-hint';
        legacyHint.textContent = '该步骤带有旧 flow 的 region（操作区域）配置：仅兼容显示/编辑；新步骤不再生成 region。';
        box.appendChild(legacyHint);
      } else {
        const note = document.createElement('div');
        note.className = 'muted small';
        note.textContent = '新步骤不配置操作区域：操作区由目标素材的 op_region 决定（到素材行「操作区」设置）。';
        box.appendChild(note);
      }
    }

    function sync() {
      // seq136（#3）：素材/操作区域由目标素材（actionTargetEditor）自动推导并保存在
      // base 里；这里只同步定位方式/文字来源/旧 region，不再维护第二个素材字段。
      const next = Object.assign({}, base, { kind: kind.value });
      let written = next;
      if (kind.value === 'ocr_text') {
        const text = textSource ? textSource.read() : { mode: 'keep', value: '' };
        written = text.mode === 'keep' ? next : paramSourceApply(next, text.mode, text.value);
        if (regionEditor) {
          const region = regionEditor.read();
          if (region.ok) written = regionWrite(written, region.mode, region.offset, region.asset);
        }
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
    root.appendChild(box);
  }

  // seq129：拖动端点 = 素材（与「点击素材」同一素材选择器/操作区域逻辑）。
  // 旧坐标/region/OCR 端点只读保留，界面不再生成/改写。
  const DRAG_ASSET_KINDS = ['template', 'template_set', 'ocr_line', 'ocr_lines', 'ocr_var', 'region'];

  function dragAssetEndpointBody(sel) {
    const id = String(sel === undefined || sel === null ? '' : sel).trim();
    if (!id) return { ok: false, endpoint: null, error: '请选择素材' };
    return { ok: true, endpoint: { kind: 'asset', sel: id }, error: '' };
  }

  function dragEndpointAssetSel(locator) {
    if (!locator || typeof locator !== 'object' || Array.isArray(locator)) return '';
    if (String(locator.kind === undefined || locator.kind === null ? '' : locator.kind) !== 'asset') return '';
    return String(locator.sel === undefined || locator.sel === null ? '' : locator.sel).trim();
  }

  function dragEndpointLegacyText(locator) {
    if (!locator || typeof locator !== 'object' || Array.isArray(locator)) return '';
    const kind = String(locator.kind === undefined || locator.kind === null ? '?' : locator.kind);
    if (kind === 'asset') return '';
    return '旧坐标/OCR 端点（kind=' + kind + '）已不再由界面生成；请改用素材端点（高级 JSON 可查看旧值，界面不会自动改写）。';
  }

  // 设置/清空拖动端点（返回新 args，不修改入参；sel 为空时删除该端点）。
  function dragArgsSetEndpoint(args, side, sel) {
    const next = Object.assign({}, args && typeof args === 'object' && !Array.isArray(args) ? args : {});
    const body = dragAssetEndpointBody(sel);
    if (!body.ok) delete next[side];
    else next[side] = body.endpoint;
    return next;
  }

  // 保存前校验拖动端点：from 必填；素材端点必须带 sel（旧 region/point/ocr_text 端点
  // 保留兼容，不阻断旧 flow；仅 UI 不再生成）。
  function dragEndpointIssues(flow) {
    const issues = [];
    function walk(steps, path) {
      if (!Array.isArray(steps)) return;
      steps.forEach(function (step, i) {
        if (!step || typeof step !== 'object') return;
        const at = path + '[' + i + ']';
        if (step.op === 'drag') {
          const args = step.args && typeof step.args === 'object' ? step.args : {};
          const from = args.from && typeof args.from === 'object' ? args.from : null;
          if (!from) issues.push(at + '.args.from：拖动必须选择起点素材');
          else if (String(from.kind === undefined || from.kind === null ? '' : from.kind) === 'asset' &&
            !String(from.sel === undefined || from.sel === null ? '' : from.sel).trim()) {
            issues.push(at + '.args.from：拖动起点未选择素材');
          }
          const to = args.to && typeof args.to === 'object' ? args.to : null;
          if (to && String(to.kind === undefined || to.kind === null ? '' : to.kind) === 'asset' &&
            !String(to.sel === undefined || to.sel === null ? '' : to.sel).trim()) {
            issues.push(at + '.args.to：拖动终点已选择素材类型但未选素材');
          }
        }
        for (const key of ['then', 'else', 'body']) walk(step[key], at + '.' + key);
      });
    }
    walk(flow && flow.steps, 'flow.steps');
    return issues;
  }

  function structuredDrag(root, step) {
    const box = formBox('拖动（拟人）：选择起点素材（必填）与可选终点素材；方向/距离/时长/margin 保留。坐标/区域端点已停用（旧 flow 只读提示），拖动落点=素材识别结果+素材自身操作区域。');
    const sides = [
      { side: 'from', title: '拖动起点素材（必填）', placeholder: '搜索并选择拖动起点素材…' },
      { side: 'to', title: '拖动终点素材（可选；不选则按方向+距离）', placeholder: '搜索并选择拖动终点素材…' },
    ];
    for (const cfg of sides) {
      const current = step.args[cfg.side] && typeof step.args[cfg.side] === 'object' ? step.args[cfg.side] : null;
      const currentSel = dragEndpointAssetSel(current);
      const currentAsset = assetById(B.assets, currentSel);
      const section = formBox(cfg.title);
      const summary = document.createElement('div');
      summary.className = 'builder-target-summary';
      summary.textContent = currentAsset
        ? targetPickerSummary(currentAsset)
        : (currentSel ? '当前素材「' + currentSel + '」不在素材库' : '未设置素材端点（旧端点值保留在高级 JSON 中）');
      section.appendChild(summary);
      const picker = window.OASAssetUI.createSearchPicker({
        items: assetTargetOptions(B.assets, { kindFilter: DRAG_ASSET_KINDS }),
        value: '',
        placeholder: cfg.placeholder,
        className: 'builder-asset-search builder-target-search',
        menuClass: 'builder-asset-menu',
        label: (item) => item.id + ' · ' + item.kindLabel + (item.terminalOnly ? '（固定区域）' : ''),
        onChoose: (item, input) => {
          step.args = dragArgsSetEndpoint(step.args, cfg.side, item.id);
          input.value = item.id;
          status('已设置拖动' + (cfg.side === 'from' ? '起点' : '终点') + '素材：' + item.id);
          renderAll();
        },
      });
      section.appendChild(picker);
      const legacy = dragEndpointLegacyText(current);
      if (legacy) {
        const warn = document.createElement('div');
        warn.className = 'muted small builder-drag-legacy-hint';
        warn.textContent = legacy;
        section.appendChild(warn);
      }
      if (cfg.side === 'to') {
        const clear = document.createElement('button');
        clear.type = 'button';
        clear.textContent = step.args.to ? '清空终点（改用方向+距离）' : '终点未设置（使用方向+距离）';
        clear.disabled = !step.args.to;
        clear.title = '删除 args.to，拖动改为按方向+距离执行';
        clear.addEventListener('click', () => {
          const next = Object.assign({}, step.args);
          delete next.to;
          step.args = next;
          status('已清空拖动终点，使用方向+距离');
          renderAll();
        });
        section.appendChild(clear);
      }
      box.appendChild(section);
    }

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

  // P1：long_press 属性面板（find 由 structuredClickTarget 复用；这里管时长）。
  function renderLongPressForm(root, step) {
    const box = formBox('长按参数');
    const hint = document.createElement('div');
    hint.className = 'muted small';
    hint.textContent = '长按：按住「定位对象」找到的元素 hold_ms 毫秒（默认 3000）；勾选随机范围后每次在 hold_range 内随机（优先于 hold_ms，拟人）。';
    box.appendChild(hint);

    const holdLab = document.createElement('label');
    holdLab.textContent = '按住时长 hold_ms（100~10000，默认 3000）';
    const hold = document.createElement('input');
    hold.type = 'number';
    hold.min = '100';
    hold.max = '10000';
    hold.step = '100';
    hold.value = String(step.args.hold_ms === undefined || step.args.hold_ms === null ? 3000 : step.args.hold_ms);
    hold.addEventListener('change', () => {
      const value = Number(hold.value);
      if (isFinite(value) && Number.isInteger(value) && value >= 100 && value <= 10000) {
        step.args.hold_ms = value;
        renderAll();
      } else {
        status('hold_ms 必须是 100~10000 的整数', '#ff8a80');
      }
    });
    holdLab.appendChild(hold);
    box.appendChild(holdLab);

    const rangeOn = Array.isArray(step.args.hold_range) && step.args.hold_range.length === 2;
    const rangeLabel = document.createElement('label');
    rangeLabel.className = 'run-opt';
    const rangeCheck = document.createElement('input');
    rangeCheck.type = 'checkbox';
    rangeCheck.checked = rangeOn;
    rangeLabel.append(rangeCheck, document.createTextNode(' 随机按住时长 hold_range（拟人）'));
    box.appendChild(rangeLabel);

    const range = rangeOn ? step.args.hold_range : [2500, 3500];
    const row = document.createElement('div');
    row.className = 'builder-inline';
    const minLab = document.createElement('label');
    minLab.textContent = '最小 ms';
    const minInput = document.createElement('input');
    minInput.type = 'number';
    minInput.min = '100';
    minInput.max = '10000';
    minInput.step = '100';
    minInput.value = String(range[0]);
    minInput.disabled = !rangeOn;
    minLab.appendChild(minInput);
    const maxLab = document.createElement('label');
    maxLab.textContent = '最大 ms';
    const maxInput = document.createElement('input');
    maxInput.type = 'number';
    maxInput.min = '100';
    maxInput.max = '10000';
    maxInput.step = '100';
    maxInput.value = String(range[1]);
    maxInput.disabled = !rangeOn;
    maxLab.appendChild(maxInput);
    row.append(minLab, maxLab);
    box.appendChild(row);

    function commitRange() {
      const parsed = holdRangeFromInput(minInput.value, maxInput.value);
      if (!parsed.ok) {
        status('hold_range 无效：' + parsed.error, '#ff8a80');
        return;
      }
      if (parsed.range) step.args.hold_range = parsed.range;
      else delete step.args.hold_range;
      renderAll();
    }
    rangeCheck.addEventListener('change', () => {
      if (rangeCheck.checked) {
        step.args.hold_range = [2500, 3500];
      } else {
        delete step.args.hold_range;
      }
      renderAll();
    });
    minInput.addEventListener('change', commitRange);
    maxInput.addEventListener('change', commitRange);
    root.appendChild(box);
  }

  // P1：curve_swipe 属性面板（points JSON + duration 范围 + 多段风险说明）。
  function renderCurveSwipeForm(root, step) {
    const box = formBox('曲线滑动（多段近似）');
    const risk = document.createElement('div');
    risk.className = 'muted small builder-warn-note';
    risk.textContent = '多段直线近似：每段是独立手势，App 可能视为多次滑动；仅在没有单次连续曲线能力时使用。';
    box.appendChild(risk);

    const pointsLabel = document.createElement('label');
    pointsLabel.textContent = '路径点 points（JSON：[[x,y],...]，2~32 个屏内整数点）';
    const pointsInput = document.createElement('textarea');
    pointsInput.className = 'builder-code';
    pointsInput.spellcheck = false;
    const defaultPoints = [[500, 1200], [520, 900], [560, 600], [620, 300]];
    const currentPoints = Array.isArray(step.args.points) ? step.args.points : defaultPoints;
    pointsInput.value = JSON.stringify(currentPoints, null, 0);
    pointsInput.placeholder = '[[500,1200],[520,900],[560,600],[620,300]]';
    pointsInput.addEventListener('change', () => {
      const parsed = curvePointsFromText(pointsInput.value, B.screen[0], B.screen[1]);
      if (!parsed.ok) {
        status('points 无效：' + parsed.error, '#ff8a80');
        return;
      }
      step.args.points = parsed.points;
      renderAll();
    });
    pointsLabel.appendChild(pointsInput);
    box.appendChild(pointsLabel);

    const dur = Array.isArray(step.args.duration_ms) && step.args.duration_ms.length === 2
      ? step.args.duration_ms
      : [600, 1000];
    const row = document.createElement('div');
    row.className = 'builder-inline';
    const minLab = document.createElement('label');
    minLab.textContent = '最小时长 ms';
    const minInput = document.createElement('input');
    minInput.type = 'number';
    minInput.min = '100';
    minInput.max = '10000';
    minInput.step = '100';
    minInput.value = String(dur[0]);
    minLab.appendChild(minInput);
    const maxLab = document.createElement('label');
    maxLab.textContent = '最大时长 ms';
    const maxInput = document.createElement('input');
    maxInput.type = 'number';
    maxInput.min = '100';
    maxInput.max = '10000';
    maxInput.step = '100';
    maxInput.value = String(dur[1]);
    maxLab.appendChild(maxInput);
    row.append(minLab, maxLab);
    box.appendChild(row);
    function commitDuration() {
      const parsed = curveDurationFromInput(minInput.value, maxInput.value);
      if (!parsed.ok) {
        status('duration_ms 无效：' + parsed.error, '#ff8a80');
        return;
      }
      step.args.duration_ms = parsed.duration;
      renderAll();
    }
    minInput.addEventListener('change', commitDuration);
    maxInput.addEventListener('change', commitDuration);

    const hint = document.createElement('div');
    hint.className = 'muted small';
    hint.textContent = '总时长在 duration_ms 范围内随机后按段均分（100~10000ms，min≤max）；当前设计分辨率 ' +
      B.screen[0] + 'x' + B.screen[1] + '。';
    box.appendChild(hint);
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

  // seq136（#3）：定位类动作（click_target/long_press）旧 flow 的冗余「素材」字段
  // 兼容：旧通用 picker 可能留下 args.sel；若 find[0] 缺 sel 则按其推导 template
  // locator，然后删除冗余 sel（不阻塞保存/加载）。
  function normalizeLegacyActionTargetArgs(args) {
    const a = args && typeof args === 'object' && !Array.isArray(args) ? args : {};
    if (!Object.prototype.hasOwnProperty.call(a, 'sel')) return a;
    const id = String(a.sel === undefined || a.sel === null ? '' : a.sel).trim();
    const find = Array.isArray(a.find) ? a.find : [];
    const first = find[0];
    const hasLocatorSel = !!(first && typeof first === 'object' && !Array.isArray(first) &&
      first.sel !== undefined && first.sel !== null && String(first.sel).trim());
    if (id && !hasLocatorSel) {
      a.find = [{ kind: 'template', sel: id }].concat(find);
    }
    delete a.sel;
    return a;
  }

  function normalizeStepArgs(step) {
    if (!step || typeof step !== 'object') return;
    step.args = step.args || {};
    if (step.op === 'click_target' || step.op === 'long_press') {
      // seq136（#3）：素材字段由目标素材 picker 唯一维护；旧冗余 sel 自动推导后删除。
      normalizeLegacyActionTargetArgs(step.args);
    }
    if (step.op === 'if') {
      delete step.args.sel;
      delete step.args.n;
      delete step.args.max_iters;
      if (step.args.cond == null || step.args.cond === '') step.args.cond = 'true';
      step.args.cond = repairConditionExpr(step.args.cond);
    } else if (step.op === 'while') {
      delete step.args.sel;
      delete step.args.n;
      delete step.args.max_iters;
      if (step.args.cond == null || step.args.cond === '') step.args.cond = 'true';
      step.args.cond = repairConditionExpr(step.args.cond);
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

  // seq136（C1 #5）：识别/等待动作清单 + 常用表单 schema（字段映射到现有 flow args；
  // 高级表达式仍由条件控件的「自定义表达式」兜底）。text_visible/on_interrupt 不是
  // 动作（text_visible 只在条件表达式里；interrupts 是 flow 级字段）。
  function recognitionWaitActionKeys() {
    return ['wait', 'wait_until', 'appear', 'ocr', 'dismiss_popup', 'ocr_to_var'];
  }

  function recognitionWaitFormSpec(op) {
    const specs = {
      wait: [
        { key: 'seconds', label: '等待秒数', type: 'number' },
      ],
      wait_until: [
        { key: 'cond', label: '等到条件', type: 'condition', required: true },
        { key: 'timeout_s', label: '超时秒', type: 'number' },
      ],
      appear: [
        { key: 'sel', label: '目标素材（识别+操作区域）', type: 'asset', required: true },
      ],
      ocr: [
        { key: 'sel', label: 'OCR 素材', type: 'asset', required: true },
      ],
      dismiss_popup: [
        { key: 'sel', label: '弹窗素材', type: 'asset', required: true },
        { key: 'action', label: '按钮文字（可选，素材库 popup 动作名）', type: 'text' },
        { key: 'timeout_ms', label: '超时 ms', type: 'number' },
      ],
      ocr_to_var: [
        { key: 'sel', label: 'OCR 素材', type: 'asset', required: true },
        { key: 'var', label: '变量名', type: 'text' },
      ],
    };
    const entries = specs[String(op === undefined || op === null ? '' : op)] || null;
    return entries ? entries.map((f) => Object.assign({}, f)) : null;
  }

  // 单层比较表达式解析：「(left op right)」/「not (...)」→ 比较行；复杂表达式返回 null
  // （保持自定义表达式模式，不擅自改写）。
  function parseCompareConditionText(cond) {
    let s = String(cond === undefined || cond === null ? '' : cond).trim();
    let neg = false;
    const negMatch = s.match(/^not\s*\(([\s\S]*)\)$/i);
    if (negMatch) {
      neg = true;
      s = negMatch[1].trim();
    }
    if (s.startsWith('(') && s.endsWith(')')) s = s.slice(1, -1).trim();
    const ops = ['>=', '<=', '==', '!=', '>', '<'];
    let depth = 0;
    let quote = '';
    let found = null;
    for (let i = 0; i < s.length; i += 1) {
      const ch = s[i];
      if (quote) {
        if (ch === '\\') i += 1;
        else if (ch === quote) quote = '';
        continue;
      }
      if (ch === "'" || ch === '"') { quote = ch; continue; }
      if (ch === '(') { depth += 1; continue; }
      if (ch === ')') { depth -= 1; continue; }
      if (depth !== 0) continue;
      for (const op of ops) {
        if (s.slice(i, i + op.length) === op) {
          if (found) return null; // 多个顶层比较 → 交给自定义表达式
          found = { index: i, op: op };
          i += op.length - 1;
          break;
        }
      }
    }
    if (!found) return null;
    const left = s.slice(0, found.index).trim();
    const right = s.slice(found.index + found.op.length).trim();
    if (!left || !right) return null;
    return { left: left, op: found.op, right: right, neg: neg };
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
    // 旧 flow：text_visible(param('k')) 条件行（只读回显）。
    m = c.match(/^text_visible\(\s*param\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\)\s*\)$/);
    if (m) return { custom: false, rows: [{ kind: 'param', neg: false, value: m[1] }], join: 'or', groupNot: false };
    m = c.match(/^(?:not\s+|!)\s*\(?\s*text_visible\(\s*param\(\s*[\x27\x22]([^\x27\x22]+)[\x27\x22]\s*\)\s*\)\s*\)?$/);
    if (m) return { custom: false, rows: [{ kind: 'param', neg: true, value: m[1] }], join: 'or', groupNot: false };
    // seq136（C1 #5）：简单比较表达式往返为「比较」条件行；复杂表达式保持自定义。
    const cmp = parseCompareConditionText(c);
    if (cmp) {
      return {
        custom: false,
        rows: [{ kind: 'compare', neg: cmp.neg, left: cmp.left, op: cmp.op, right: cmp.right }],
        join: 'and',
        groupNot: false,
      };
    }
    if (!c) return { custom: false, rows: [], join: 'and', groupNot: false };
    return { custom: true, rows: [], join: 'and', groupNot: false, raw: c };
  }

  function conditionRowExpr(row) {
    if (row.kind === 'compare') {
      // seq136（C1 #5）：比较行 → (left op right)；左右缺一不生成表达式。
      const left = String(row.left === undefined || row.left === null ? '' : row.left).trim();
      const right = String(row.right === undefined || row.right === null ? '' : row.right).trim();
      const op = ['>', '>=', '<', '<=', '==', '!='].indexOf(String(row.op || '')) >= 0 ? String(row.op) : '==';
      if (!left || !right) return '';
      const expr = '(' + left + ' ' + op + ' ' + right + ')';
      return row.neg ? `not ${expr}` : expr;
    }
    const v = String(row.value || '').trim();
    if (!v) return '';
    let expr;
    if (row.kind === 'asset') {
      expr = `appear('${v.replace(/'/g, '')}')`;
    } else if (row.kind === 'param') {
      // D 批：旧 flow 的 text_visible(param('k')) 只读回显（不再有新建入口）。
      expr = paramWaitCond(v);
    } else if (row.kind === 'ocr_var') {
      // D 批：识别文字配置项行 = ocr_var 素材的 param_key → text_visible(param(key))。
      const key = paramSafeKey(row.paramKey);
      expr = key ? paramWaitCond(key) : '';
    } else {
      // A text row still uses the same underlying logical expression; multiple
      // texts inside one row stay OR-compatible with the previous preset.
      const values = v.split(/[,，、]|\bor\b|\|/i).map((x) => x.trim()).filter(Boolean);
      expr = values.map((x) => `text_visible('${x.replace(/'/g, '')}')`).join(' or ') || "text_visible('')";
    }
    return row.neg ? `not (${expr})` : expr;
  }

  function renderConditionEditor(root, step, opts) {
    const cfg = opts && typeof opts === 'object' ? opts : {};
    const box = document.createElement('div');
    box.className = 'builder-subform';
    const title = document.createElement('div');
    title.className = 'muted small';
    title.textContent = cfg.title || '循环/分支条件（谓词行 + and/or/not、&&/||/!、括号；「识别文字配置项」行 = text_visible(param(素材 param_key))；已声明变量正常显示，仅未声明 param 只读回显）';
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
    customLabel.textContent = cfg.customLabel ||
      '自定义表达式（0=不循环，非 0=循环；可写 appear(...)/text_visible(...)）';
    const customRow = document.createElement('span');
    customRow.className = 'builder-expr-row';
    const custom = document.createElement('input');
    custom.type = 'text';
    custom.value = preset.raw || step.args.cond || 'true';
    customRow.appendChild(custom);
    customRow.appendChild(variableInsertWidget('cond', function () { return custom; }));
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
        const kindChoices = [];
        // seq130（#3）：param(key) 已声明为变量 → 正常「识别文字变量」行；仅未声明才
        // 显示「旧参数来源（只读）」。有对应 ocr_var 素材时直接转成 ocr_var 行。
        const declaredParamRow = row.kind === 'param' &&
          paramKeyDeclaredIn(row.value, B.variables, B.assets);
        if (declaredParamRow) {
          const declaredKey = paramSafeKey(row.value);
          const bound = (B.assets || []).find(function (a) {
            return a && a.kind === 'ocr_var' && paramSafeKey(a.param_key) === declaredKey;
          });
          if (bound) {
            row.kind = 'ocr_var';
            row.value = String(bound.id);
            row.paramKey = declaredKey;
          }
        }
        const stillLegacyParam = row.kind === 'param' && !declaredParamRow;
        if (stillLegacyParam) kindChoices.push(['param', '旧参数来源（只读）']);
        else if (declaredParamRow) kindChoices.push(['param', '识别文字变量（已声明）']);
        kindChoices.push(['asset', '素材'], ['text', '文字（全屏）'], ['ocr_var', '识别文字配置项']);
        // seq136（C1 #5）：比较行（左值/运算符/右值），与 if 共用同一条件控件。
        if (cfg.allowCompare !== false) kindChoices.push(['compare', '比较（值/表达式）']);
        for (const [v, label] of kindChoices) {
          const o = document.createElement('option');
          o.value = v;
          o.textContent = label;
          kind.appendChild(o);
        }
        kind.value = row.kind;
        kind.addEventListener('change', () => {
          row.kind = kind.value;
          row.value = '';
          row.paramKey = '';
          renderRows();
          sync();
        });
        const negLabel = document.createElement('label');
        negLabel.className = 'run-opt';
        const neg = document.createElement('input');
        neg.type = 'checkbox';
        neg.checked = !!row.neg;
        neg.addEventListener('change', () => { row.neg = neg.checked; sync(); });
        negLabel.append(neg, document.createTextNode(row.kind === 'compare' ? '取反' : '未出现'));
        let valueEl;
        if (row.kind === 'asset') {
          const search = assetSearch(row.value || '', null);
          valueEl = search.input;
          valueEl.addEventListener('change', () => { row.value = valueEl.value.trim(); sync(); });
          line.append(kind, negLabel, search);
        } else if (row.kind === 'ocr_var') {
          // D 批：识别文字配置项行 = 选一个 ocr_var 素材 → text_visible(param(key))。
          const ocrVars = (B.assets || []).filter(function (a) {
            return a && a.kind === 'ocr_var' && paramSafeKey(a.param_key);
          });
          valueEl = document.createElement('select');
          valueEl.appendChild(new Option('（选择使用端OCR文字配置项素材）', ''));
          const byId = {};
          for (const asset of ocrVars) {
            byId[String(asset.id)] = asset;
            const runtimeText = String(asset.runtime_text === undefined || asset.runtime_text === null
              ? '' : asset.runtime_text);
            valueEl.appendChild(new Option(
              (asset.label || asset.id) + ' · ' + asset.id + ' · ' +
                paramTextExpr(paramSafeKey(asset.param_key)) +
                (runtimeText ? ' · 「' + runtimeText + '」' : ''),
              String(asset.id)
            ));
          }
          if (row.value && !byId[row.value]) {
            valueEl.appendChild(new Option('素材不存在：' + row.value + '（旧配置）', row.value));
          }
          valueEl.value = row.value || '';
          const applyAsset = () => {
            const asset = byId[valueEl.value];
            row.value = valueEl.value;
            row.paramKey = asset ? paramSafeKey(asset.param_key) : '';
          };
          applyAsset();
          valueEl.addEventListener('change', () => { applyAsset(); sync(); });
          line.append(kind, negLabel, valueEl);
        } else if (row.kind === 'compare') {
          // seq136（C1 #5）：比较行 = 左值/运算符/右值 → (left op right)；值支持表达式。
          const leftEl = document.createElement('input');
          leftEl.type = 'text';
          leftEl.className = 'builder-condition-compare-left';
          leftEl.placeholder = '左值/表达式（如 ocr_result）';
          leftEl.value = row.left || '';
          leftEl.addEventListener('change', () => { row.left = leftEl.value.trim(); sync(); });
          const opEl = document.createElement('select');
          opEl.className = 'builder-condition-compare-op';
          for (const op of ['>', '>=', '<', '<=', '==', '!=']) {
            opEl.appendChild(new Option(op, op));
          }
          opEl.value = ['>', '>=', '<', '<=', '==', '!='].indexOf(String(row.op)) >= 0 ? String(row.op) : '==';
          opEl.addEventListener('change', () => { row.op = opEl.value; sync(); });
          const rightEl = document.createElement('input');
          rightEl.type = 'text';
          rightEl.className = 'builder-condition-compare-right';
          rightEl.placeholder = '右值/表达式（如 3 或 param(\'x\')）';
          rightEl.value = row.right || '';
          rightEl.addEventListener('change', () => { row.right = rightEl.value.trim(); sync(); });
          line.append(kind, negLabel, leftEl, opEl, rightEl);
        } else if (row.kind === 'param') {
          // seq130（#3）：已声明的 param(key) 是正常变量来源；未声明才旧参数只读。
          const legacy = document.createElement('span');
          legacy.className = 'muted small';
          legacy.textContent = declaredParamRow
            ? '变量（已声明）：' + paramTextExpr(String(row.value || ''))
            : '旧参数来源（只读）：' + paramTextExpr(String(row.value || ''));
          line.append(kind, negLabel, legacy);
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
    // seq129：动作属性面板统一展示动作标题提示（与动作库 title 同一来源）。
    const opTitle = ACTION_TITLES[step.op];
    if (opTitle) {
      const opHint = document.createElement('div');
      opHint.className = 'muted small builder-op-hint';
      opHint.textContent = opTitle;
      root.appendChild(opHint);
    }
    if (step.op === 'swipe' || step.op === 'curve_swipe' || step.op === 'click_point') {
      const coordHint = document.createElement('div');
      coordHint.className = 'muted small builder-coord-hint';
      coordHint.textContent = '坐标手势（低层）：直接使用绝对坐标；元素拖动请用「拖动（拟人）」素材端点，点击元素请用「点击素材」。';
      root.appendChild(coordHint);
    }
    step.args = step.args || {};
    // seq136（#3）：删除顶部重复的「素材（识别+操作区域）」picker。素材字段只在
    // FIELDS（appear/ocr/click/dismiss_popup/ocr_to_var）或动作目标 picker
    // （click_target/long_press）出现一次；locator/操作区域由目标素材 kind 推导。
    if (step.op === 'call_task') renderCallTaskForm(root, step);
    // P1: wait/wait_until are BLOCKING waits, not actions.
    if (step.op === 'wait' || step.op === 'wait_until') {
      const waitHelp = document.createElement('div');
      waitHelp.className = 'builder-wait-help muted small';
      waitHelp.textContent = waitBlockingHelpText();
      root.appendChild(waitHelp);
    }
    const exprHintOps = new Set(['log', 'wait_until', 'loop', 'set_var', 'inc_var', 'click_point']);
    if (exprHintOps.has(step.op)) {
      const hint = document.createElement('div');
      hint.className = 'muted small';
      hint.textContent = '表达式值支持 and/or/not、&&/||/!、括号、appear/ocr/param/var；数值字段按数字求值。「插入变量 ▾」把变量 key 写成 param(...)。';
      root.appendChild(hint);
    }
    if (step.op === 'if' || step.op === 'while') {
      renderConditionEditor(root, step);
    }
    // seq136（C1 #5）：识别/等待动作的常用表单复用 if 的条件行控件（素材/文字/识别
    // 文字变量/比较行 + 与或非 + 自定义表达式兜底），避免只能写裸表达式。
    if (step.op === 'wait_until') {
      renderConditionEditor(root, step, {
        title: '等到条件（与 if 相同的条件行：素材/文字/识别文字配置项/比较值 + 与或非；条件成立后继续，超时本步失败）',
        customLabel: '自定义表达式（条件成立后继续；可写 appear(...)/text_visible(...)/比较；非 0/true 成立）',
      });
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
      } else if (f.type === 'node') {
        // seq136（D1-M3）：navigate target 下拉来自全局地图（无地图时提示先建地图）。
        const sel = document.createElement('select');
        sel.appendChild(new Option('（选择地图节点）', ''));
        for (const id of B.mapNodes) sel.appendChild(new Option(id, id));
        const current = step.args[f.key] === undefined || step.args[f.key] === null
          ? '' : String(step.args[f.key]);
        if (current && B.mapNodes.indexOf(current) < 0) {
          sel.appendChild(new Option('不存在于地图：' + current + '（旧 flow）', current));
        }
        sel.value = current;
        sel.addEventListener('change', () => {
          step.args[f.key] = sel.value;
          renderAll();
        });
        lab.appendChild(sel);
        root.appendChild(lab);
        const nodeHint = document.createElement('div');
        nodeHint.className = 'muted small';
        nodeHint.textContent = B.mapLoaded
          ? `全局地图节点 ${B.mapNodes.length} 个；目标不存在时保存会阻断（与服务端 T11 一致）。`
          : '全局地图未加载：请先到「地图」页创建/保存地图，再选择目标节点（保存会被阻断）。';
        root.appendChild(nodeHint);
        continue;
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
        // V2.0c: expression fields get「插入变量 ▾」(meta.variables keys).
        const exprMode = EXPR_FIELDS[step.op] && EXPR_FIELDS[step.op][f.key];
        if (exprMode) {
          const exprRow = document.createElement('span');
          exprRow.className = 'builder-expr-row';
          exprRow.appendChild(input);
          exprRow.appendChild(variableInsertWidget(exprMode, function () { return input; }));
          trackExprField(input, exprMode);
          lab.appendChild(exprRow);
          root.appendChild(lab);
          // seq130（#3）：表达式字段里的 param(key) 声明状态提示。
          const refKeys = paramRefKeysInText(step.args[f.key]);
          if (refKeys.length) {
            const undeclared = undeclaredParamKeysInText(step.args[f.key], B.variables, B.assets);
            const refHint = document.createElement('div');
            refHint.className = 'muted small builder-param-ref-hint' + (undeclared.length ? ' warn' : '');
            refHint.textContent = undeclared.length
              ? '表达式引用了未声明 param：' + undeclared.join('、') + '（发布 T3 报错；先在「变量」面板声明）'
              : '表达式引用的变量均已声明：' + refKeys.join('、');
            root.appendChild(refHint);
          }
          continue;
        }
      }
      lab.appendChild(input);
      root.appendChild(lab);
    }
    if (step.op === 'click_target' || step.op === 'long_press') {
      // P1: long_press reuses the click_target locator editor (kind/sel/文字来源/操作区域).
      structuredClickTarget(root, step);
      if (step.op === 'click_target') {
        // P6: requires/expect OCR 守卫也支持结构化「文字来源」三态。
        renderGuardLocatorEditor(root, step, 'requires');
        renderGuardLocatorEditor(root, step, 'expect');
      }
      if (step.op === 'long_press') renderLongPressForm(root, step);
    }
    if (step.op === 'click') renderClickAlts(root, step);
    if (step.op === 'drag') structuredDrag(root, step);
    if (step.op === 'curve_swipe') renderCurveSwipeForm(root, step);
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
      // Unified action catalogue: the branch dropdown offers exactly the same
      // actions as the left palette (both are sourced from OP_META only).
      const catalog = actionCatalog();
      for (const item of catalog) {
        const o = document.createElement('option');
        o.value = item.key;
        o.textContent = item.label + ' (' + item.key + ')';
        sel.appendChild(o);
      }
      const defaultKey = catalog.find((item) => item.key === 'log') ? 'log' : (catalog[0] || {}).key;
      if (defaultKey) sel.value = defaultKey;
      const b = document.createElement('button');
      b.type = 'button';
      b.textContent = branchLabel;
      b.title = '添加子动作到「' + branchLabel + '」分支（与左侧动作库同一来源）';
      b.addEventListener('click', async () => {
        const child = await prepareActionStep(sel.value);
        if (!child) return;
        const arr = step[key];
        arr.push(child);
        B.selected = B.selected.concat([key, arr.length - 1]);
        renderAll();
        const meta = actionMetaOf(sel.value);
        status(`已添加 ${meta ? meta.label : sel.value} → ${key}`);
      });
      row.appendChild(b);
      row.appendChild(sel);
      actions.appendChild(row);
    }
    root.appendChild(actions);
  }

  // --- seq136（C2 #8）：可编辑源码框纯函数 -----------------------------------
  // 最终 flow JSON（canonical=JSON.stringify(flow,null,2)）；已知 op 清单来自
  // OP_META（固定数组，避免把运行期校验依赖到 DOM/网络）。
  function flowSourceKnownOps() {
    return ['if', 'loop', 'while', 'break', 'continue', 'call_task',
      'wait', 'wait_until', 'appear', 'ocr',
      'click', 'click_point', 'click_target', 'long_press', 'curve_swipe',
      'dismiss_popup', 'swipe', 'drag', 'navigate', 'key', 'restart_app',
      'set_var', 'inc_var', 'ocr_to_var', 'log', 'finish'];
  }

  function flowSourceKnownOp(op) {
    return flowSourceKnownOps().indexOf(String(op === undefined || op === null ? '' : op)) >= 0;
  }

  function flowSourceText(flow) {
    return flow ? JSON.stringify(flow, null, 2) : '';
  }

  function flowStepCount(flow) {
    let n = 0;
    function walk(steps) {
      if (!Array.isArray(steps)) return;
      for (const step of steps) {
        if (!step || typeof step !== 'object') continue;
        n += 1;
        for (const key of ['then', 'else', 'body']) walk(step[key]);
      }
    }
    walk(flow && flow.steps);
    return n;
  }

  // JSON.parse 错误位置 → 源码行/列（不依赖浏览器）。
  function flowSourceParseErrorLine(text, err) {
    const raw = String(text === undefined || text === null ? '' : text);
    const msg = String(err && err.message ? err.message : err || '');
    const m = msg.match(/position\s+(\d+)/i);
    if (!m) return null;
    const pos = Math.max(0, Math.min(Number(m[1]), raw.length));
    const before = raw.slice(0, pos);
    const line = before.split('\n').length;
    const col = pos - (before.lastIndexOf('\n') + 1) + 1;
    return { line: line, column: col };
  }

  function validateFlowSourceStep(step, path, errors) {
    if (!step || typeof step !== 'object' || Array.isArray(step)) {
      errors.push({ path: path, message: '步骤必须是对象' });
      return;
    }
    const op = String(step.op === undefined || step.op === null ? '' : step.op).trim();
    if (!op) errors.push({ path: path + '.op', message: '缺少 op' });
    else if (!flowSourceKnownOp(op)) errors.push({ path: path + '.op', message: '未知 op「' + op + '」' });
    if (step.args !== undefined && step.args !== null &&
      (typeof step.args !== 'object' || Array.isArray(step.args))) {
      errors.push({ path: path + '.args', message: 'args 必须是对象' });
    }
    for (const key of ['then', 'else', 'body']) {
      const children = step[key];
      if (children === undefined || children === null) continue;
      if (!Array.isArray(children)) {
        errors.push({ path: path + '.' + key, message: '必须是步骤数组' });
        continue;
      }
      children.forEach((child, i) => validateFlowSourceStep(child, path + '.' + key + '[' + i + ']', errors));
    }
  }

  // 解析 + 结构/op/args/children 校验；错误带 path/行/列，失败不产出 flow。
  function parseFlowSource(text) {
    const raw = String(text === undefined || text === null ? '' : text);
    if (!raw.trim()) {
      return { ok: false, flow: null, errors: [{ path: '(root)', message: '源码为空' }] };
    }
    let v;
    try {
      v = JSON.parse(raw);
    } catch (e) {
      const loc = flowSourceParseErrorLine(raw, e);
      return {
        ok: false,
        flow: null,
        errors: [{
          path: '(JSON)',
          message: String(e && e.message ? e.message : e),
          line: loc ? loc.line : null,
          column: loc ? loc.column : null,
        }],
      };
    }
    const errors = [];
    if (!v || typeof v !== 'object' || Array.isArray(v)) {
      errors.push({ path: '(root)', message: 'flow 必须是 JSON 对象' });
    } else if (!Array.isArray(v.steps)) {
      errors.push({ path: 'steps', message: 'flow 必须包含 steps 数组' });
    } else {
      v.steps.forEach((step, i) => validateFlowSourceStep(step, 'steps[' + i + ']', errors));
    }
    if (errors.length) return { ok: false, flow: null, errors: errors };
    return { ok: true, flow: v, errors: [] };
  }

  function flowSourceErrorText(errors) {
    const list = Array.isArray(errors) ? errors : [];
    return list.map((e) => {
      let loc = String(e && e.path ? e.path : '(root)');
      if (e && e.line !== undefined && e.line !== null) {
        loc = '第 ' + e.line + ' 行' + (e.column !== undefined && e.column !== null ? ' 第 ' + e.column + ' 列' : '') + ' · ' + loc;
      }
      return loc + '：' + String(e && e.message ? e.message : '无效');
    }).join('\n');
  }

  // 纯函数：按 path 取步骤（不依赖 B）。
  function flowStepAtPath(flow, path) {
    if (!Array.isArray(path) || !path.length) return null;
    let arr = flow && Array.isArray(flow.steps) ? flow.steps : null;
    let obj = null;
    for (const seg of path) {
      if (typeof seg === 'number') {
        if (!Array.isArray(arr)) return null;
        obj = arr[seg];
        if (!obj) return null;
      } else {
        if (!obj) return null;
        arr = obj[seg];
        if (!Array.isArray(arr)) return null;
      }
    }
    return obj;
  }

  // 源码覆盖后尽量保留选中：同 path 且 op 相同才保留，否则清空。
  function selectedPathAfterSourceReplace(oldSelected, oldFlow, newFlow) {
    if (!Array.isArray(oldSelected) || !oldSelected.length) return null;
    const oldStep = flowStepAtPath(oldFlow, oldSelected);
    if (!oldStep) return null;
    const newStep = flowStepAtPath(newFlow, oldSelected);
    if (!newStep) return null;
    if (String(oldStep.op) !== String(newStep.op)) return null;
    return oldSelected.slice();
  }

  function flowSourceIsDirty(currentSource, lastSyncedSource) {
    return String(currentSource === undefined || currentSource === null ? '' : currentSource) !==
      String(lastSyncedSource === undefined || lastSyncedSource === null ? '' : lastSyncedSource);
  }

  function updateCodeStatus(text, color) {
    const st = $('builder-code-status');
    if (st) {
      st.textContent = text || '—';
      st.style.color = color || '';
    }
  }

  function showCodeErrors(errors) {
    const pre = $('builder-code-error');
    if (!pre) return;
    const text = flowSourceErrorText(errors);
    pre.textContent = text;
    pre.hidden = !text;
  }

  function renderCode() {
    const el = $('builder-code');
    if (!el) return;
    // dirty：用户手动编辑过，构建器编辑不得覆盖；等待「放弃修改/校验并应用」。
    if (B.codeDirty) {
      updateCodeStatus('源码有未应用修改（dirty）', '#e8c46a');
      return;
    }
    if (document.activeElement === el) return;
    const src = flowSourceText(B.flow);
    el.value = src;
    B.codeLastSynced = src;
    updateCodeStatus('已同步当前流程', '');
  }

  function discardCodeEdits() {
    const el = $('builder-code');
    B.codeDirty = false;
    const src = flowSourceText(B.flow);
    if (el) el.value = src;
    B.codeLastSynced = src;
    showCodeErrors([]);
    updateCodeStatus('已放弃源码修改（恢复当前流程）', '#7bd88f');
    if (el) el.blur();
  }

  async function applyCode() {
    const el = $('builder-code');
    const st = $('builder-code-status');
    if (!el) return;
    const parsed = parseFlowSource(el.value);
    if (!parsed.ok) {
      // 校验失败：显示精确 path/行/字段，不覆盖当前流程。
      B.codeDirty = flowSourceIsDirty(el.value, B.codeLastSynced);
      showCodeErrors(parsed.errors);
      updateCodeStatus('源码校验失败（未覆盖流程）', '#ff8a80');
      if (st) st.textContent = '源码校验失败（未覆盖流程，见下方错误）';
      return;
    }
    const count = flowStepCount(parsed.flow);
    if (!window.confirm(`将用源码覆盖当前流程图（${count} 个动作）？\n\n当前流程会被源码替换；确认后仍走现有保存流程。`)) {
      updateCodeStatus('已取消应用源码', '#e8c46a');
      return;
    }
    const oldFlow = B.flow;
    const oldSelected = B.selected;
    B.flow = parsed.flow;
    normalizeFlow(B.flow);
    B.selected = selectedPathAfterSourceReplace(oldSelected, oldFlow, B.flow);
    B.codeDirty = false;
    const src = flowSourceText(B.flow);
    B.codeLastSynced = src;
    el.value = src;
    showCodeErrors([]);
    renderAll();
    updateCodeStatus(`源码已应用（${count} 个动作），正在保存…`, '#e8c46a');
    await saveFlow(false);
    const savedOk = B.lastSavedFlow === JSON.stringify(B.flow);
    updateCodeStatus(
      savedOk ? '源码已应用并保存' : '源码已应用，但保存未完成（见状态栏/服务端校验）',
      savedOk ? '#7bd88f' : '#ff8a80'
    );
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
    renderVariablesPanel();
    renderCode();
    renderRustProjection();
    if (flowPageState.open) renderFlowPage();
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

  // U-T5/B-T3a：save_flow 的 sync 改了派生参数行 -> 重新读取 meta.params 并刷新面板。
  // V2.0c：同步重读 variables（T8 可能同步 ocr_text 变量 default）。
  async function refreshParamsAfterFlowSync() {
    if (!B.task) return;
    try {
      const params = await fetchTaskParams(B.task);
      B.params = Array.isArray(params) ? params : [];
      B.paramsError = '';
    } catch (e) {
      B.paramsError = String((e && e.message) || e);
    }
    try {
      const vres = await fetchTaskVariables(B.task);
      if (vres.ok) {
        B.variables = vres.variables;
        B.variablesIssues = vres.warnings;
        renderVariablesPanel();
      }
    } catch (e) { /* keep the last good list */ }
  }

  async function loadTask(name) {
    B.autoSaveArmed = false;
    B.flowHistory = [];
    B.historyIndex = -1;
    // seq136（C2 #8）：切换任务时放弃旧任务源码的 dirty/错误状态。
    B.codeDirty = false;
    B.codeLastSynced = '';
    showCodeErrors([]);
    clearLintState();
    if (!name) {
      B.task = '';
      B.flow = null;
      B.assets = [];
      B.selected = null;
      B.params = [];
      B.paramsError = '';
      B.paramsLoading = false;
      B.variables = [];
      B.variablesError = '';
      B.variablesIssues = [];
      B.mapNodes = [];
      B.mapLoaded = false;
      B.mapError = '';
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
      const variablesPromise = fetchTaskVariables(name);
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
      // P1: screen bounds come from the assets response (design_resolution);
      // curve_swipe points must stay inside it. Older/absent -> 1280x720.
      B.screen = normalizeScreenSize(as.design_resolution);
      for (const c of as.collections || []) {
        B.assets.push({ id: c.id, kind: 'template_set', members: c.members || [], action_shape: c.action_shape });
      }
      const pr = await paramsPromise;
      B.params = pr.ok ? pr.params : [];
      B.paramsError = pr.ok ? '' : pr.error;
      B.paramsLoading = false;
      const vres = await variablesPromise;
      B.variables = vres.ok ? vres.variables : [];
      B.variablesError = vres.ok ? '' : vres.error;
      B.variablesIssues = vres.ok ? vres.warnings : [];
      // seq136（D1-M3）：加载全局地图节点（GET /map；失败不阻断构建，只提示）。
      try {
        const mr = await fetch('/map');
        const ms = await mr.json();
        if (ms && ms.ok) {
          B.mapLoaded = !!ms.exists;
          const nodes = ms.map && Array.isArray(ms.map.nodes) ? ms.map.nodes : [];
          B.mapNodes = nodes.map((n) => String(n.id));
          B.mapError = '';
        } else {
          B.mapLoaded = false;
          B.mapNodes = [];
          B.mapError = ((ms && ms.errors) || []).join('; ') || '地图读取失败';
        }
      } catch (e) {
        B.mapLoaded = false;
        B.mapNodes = [];
        B.mapError = String(e);
      }
      B.selected = null;
      B.lastSavedFlow = JSON.stringify(B.flow);
      renderAll();
      B.autoSaveArmed = true;
      status(`已载入 ${name}：${B.flow.steps.length} 个根动作 / ${B.assets.length} 个素材 / ${B.variables.length} 个变量`);
      // P5: run the authoritative lint once per load; failures stay silent and
      // never block flow editing (the panel explains 权威检查不可用).
      lintTask({ silent: true });
    } catch (e) {
      B.paramsLoading = false;
      status('载入失败：' + e, '#ff8a80');
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
    // P7: strip stale non-flow args (e.g. an old `args.sel` left on if/while by
    // the legacy generic asset picker) and repair the old malformed condition
    // concatenation before validating/writing.
    normalizeFlow(B.flow);
    // P3: 保存前本地校验「文字来源」互斥（结构化编辑器不会双写；高级 JSON 手改
    // 可写出 text + text_expr 同时存在，这里拦住，避免把坏 flow 写盘）。
    // P1: 同时校验 region 形状与 long_press/curve_swipe 参数范围。
    // U-T5：动作目标校验（素材存在 / ocr_var 有 param_key / param key 已派生）。
    const saveIssues = flowTextSpecIssues(B.flow)
      .concat(regionIssues(B.flow, regionAssetIds(B.assets)))
      .concat(actionArgsIssues(B.flow, B.screen, B.mapNodes, B.mapLoaded))
      .concat(dragEndpointIssues(B.flow))
      .concat(actionTargetIssues(B.flow, B.assets, actionTargetParamRows(B.variables, B.assets)));
    if (saveIssues.length) {
      status('保存已阻止：' + saveIssues[0] +
        (saveIssues.length > 1 ? '（另有 ' + (saveIssues.length - 1) + ' 项问题，见高级 JSON/属性面板）' : ''),
      '#ff8a80');
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
      // seq136（C2 #8）：保存成功后源码框同步（dirty 手动编辑不被覆盖）。
      if (!B.codeDirty) {
        B.codeLastSynced = flowSourceText(B.flow);
        renderCode();
      }
      // U-T5/B-T3a：save_flow 成功后后端会同步派生参数（响应 sync）；提示并刷新面板。
      const syncText = saveSyncText(s.sync);
      const syncWarnings = Array.isArray(s.sync_warnings) ? s.sync_warnings : [];
      if (syncText) {
        status((silent ? `已自动保存 ${B.task}` : `已保存 ${B.task} · flow.json`) + ' · ' + syncText);
        log(syncText, 'INFO', s.sync);
        await refreshParamsAfterFlowSync();
      } else if (syncWarnings.length) {
        status('保存已写入，但派生参数同步有警告：' + syncWarnings.join('; '), '#e8c46a');
      } else {
        status(silent ? `已自动保存 ${B.task} · ${new Date().toLocaleTimeString()}` : `已保存 ${B.task} · flow.json`);
      }
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
    // seq136（#9）：流程图自适应——容器尺寸变化/窗口 resize 重新 fit（不改变节点坐标）。
    const graphRoot = $('builder-graph');
    if (graphRoot && typeof ResizeObserver === 'function') {
      new ResizeObserver(function () { scheduleFlowGraphFit(); }).observe(graphRoot);
    }
    window.addEventListener('resize', scheduleFlowGraphFit);
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
    const lintBtn = $('builder-lint-btn');
    if (lintBtn) lintBtn.addEventListener('click', function () { lintTask(); });
    const lintClose = $('builder-lint-close');
    if (lintClose) lintClose.addEventListener('click', closeLintPanel);
    const codeApply = $('builder-code-apply');
    if (codeApply) codeApply.addEventListener('click', applyCode);
    // seq136（C2 #8）：源码手动编辑进入 dirty（构建器编辑不覆盖），放弃/应用后恢复。
    const codeEl = $('builder-code');
    if (codeEl) {
      codeEl.addEventListener('input', () => {
        if (!B.codeDirty && flowSourceIsDirty(codeEl.value, B.codeLastSynced)) {
          B.codeDirty = true;
          updateCodeStatus('源码有未应用修改（dirty）', '#e8c46a');
        }
      });
    }
    const codeDiscard = $('builder-code-discard');
    if (codeDiscard) codeDiscard.addEventListener('click', discardCodeEdits);
    // seq137（UI B #2）：全窗口流程图大页（打开/返回/缩放/适应窗口）。
    const openFlowBtn = $('builder-open-flow-page');
    if (openFlowBtn) openFlowBtn.addEventListener('click', openFlowPage);
    const flowBack = $('flow-page-back');
    if (flowBack) flowBack.addEventListener('click', closeFlowPage);
    const flowZoomIn = $('flow-page-zoom-in');
    if (flowZoomIn) flowZoomIn.addEventListener('click', () => setFlowPageZoom(flowPageState.zoom * 1.2));
    const flowZoomOut = $('flow-page-zoom-out');
    if (flowZoomOut) flowZoomOut.addEventListener('click', () => setFlowPageZoom(flowPageState.zoom / 1.2));
    const flowFit = $('flow-page-fit');
    if (flowFit) flowFit.addEventListener('click', fitFlowPage);
    const flowCanvas = $('flow-page-canvas');
    if (flowCanvas) {
      flowCanvas.addEventListener('wheel', (ev) => {
        if (!flowPageState.open || !ev.ctrlKey) return;
        ev.preventDefault();
        setFlowPageZoom(flowPageState.zoom * (ev.deltaY > 0 ? 0.9 : 1.1));
      }, { passive: false });
    }
    document.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape' && flowPageState.open) closeFlowPage();
    });
    const delChecked = $('builder-delete-checked');
    if (delChecked) delChecked.addEventListener('click', deleteCheckedSteps);
    // V2.0c：变量面板 + 变量对话框。
    const varsNew = $('builder-vars-new');
    if (varsNew) varsNew.addEventListener('click', function () { openVariableDialog(null); });
    const varsRefresh = $('builder-vars-refresh');
    if (varsRefresh) varsRefresh.addEventListener('click', function () { refreshVariables(); });
    const varSource = $('variable-source');
    if (varSource) varSource.addEventListener('change', syncVariableDialogFields);
    const varAsset = $('variable-ocr-asset');
    if (varAsset) varAsset.addEventListener('change', syncVariableDialogFields);
    const varScope = $('variable-scope');
    if (varScope) varScope.addEventListener('change', syncVariableDialogFields);
    const varSave = $('variable-save');
    if (varSave) varSave.addEventListener('click', function () { saveVariableDialog(); });
    const varCancel = $('variable-cancel');
    if (varCancel) varCancel.addEventListener('click', closeVariableDialog);
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
