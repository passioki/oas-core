// OAS Runner（使用端）只读 UI。
// 独立于 Studio 的编辑器脚本：不加载构建/流程/素材模块，也不调用创作或构建接口。
// 所有请求都是只读 GET，统一经过 runnerFetchJson（超时、HTTP 错误、JSON 错误归一化）。
'use strict';

(function () {
  const RUNNER_FETCH_TIMEOUT_MS = 8000;
  const RUNNER_RUNS_LIMIT = 100;
  const RUNNER_RECENT_LIMIT = 5;
  const RUNNER_PAGES = ['overview', 'tasks', 'queue', 'runs', 'settings'];
  const RUNNER_PAGE_LABELS = {
    overview: '概览',
    tasks: '已安装任务',
    queue: '队列与调度',
    runs: '运行记录',
    settings: '设置',
  };
  const RUNNER_API = {
    health: '/runner/health',
    tasks: '/runner/tasks',
    schedulerStatus: '/scheduler/status',
    schedulerRuns: '/runs/scheduler',
    config: '/config',
    instance: '/instance',
  };

  const state = {
    page: 'overview',
    busy: false,
    queued: false,
    health: null,
    tasks: null,
    recentRuns: null,
    recentRunsError: '',
    scheduler: null,
    runs: null,
    config: null,
    instance: null,
  };

  const el = {};

  // --- 纯格式化辅助（无 DOM 依赖；ui_pure_test 会单独抽取验证） ---------------

  function runnerValueText(value) {
    if (value === undefined || value === null || value === '') return '—';
    if (typeof value === 'string') return value;
    if (typeof value === 'number' || typeof value === 'boolean') return String(value);
    try {
      const text = JSON.stringify(value);
      return text == null ? '—' : text;
    } catch (e) {
      return String(value);
    }
  }

  function runnerTimeText(ms) {
    if (ms === undefined || ms === null || ms === '') return '—';
    const n = Number(ms);
    if (!isFinite(n) || n <= 0) return '—';
    try {
      return new Date(n).toLocaleString('zh-CN', { hour12: false });
    } catch (e) {
      return String(ms);
    }
  }

  function runnerDurationText(ms) {
    if (ms === undefined || ms === null || ms === '') return '—';
    const n = Number(ms);
    if (!isFinite(n) || n < 0) return '—';
    if (n < 1000) return Math.round(n) + 'ms';
    if (n < 60000) return (n / 1000).toFixed(1) + 's';
    const total = Math.round(n / 1000);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    if (h > 0) return h + 'h' + m + 'm';
    return m + 'm' + s + 's';
  }

  function runnerRunDurationMs(run) {
    const r = run || {};
    const direct = Number(r.duration_ms);
    if (r.duration_ms !== undefined && r.duration_ms !== null && r.duration_ms !== '' &&
        isFinite(direct) && direct >= 0) {
      return direct;
    }
    const start = Number(r.started_ms);
    const end = Number(r.finished_ms);
    if (r.started_ms !== undefined && r.started_ms !== null && r.started_ms !== '' &&
        r.finished_ms !== undefined && r.finished_ms !== null && r.finished_ms !== '' &&
        isFinite(start) && isFinite(end) && end >= start) {
      return end - start;
    }
    return null;
  }

  function runnerErrorText(error) {
    if (error === undefined || error === null) return '';
    if (typeof error === 'string') return error.trim();
    if (typeof error === 'number' || typeof error === 'boolean') return String(error);
    if (typeof error === 'object' && error.message) return String(error.message);
    try {
      const text = JSON.stringify(error);
      return text == null ? '' : text;
    } catch (e) {
      return String(error);
    }
  }

  function runnerErrorSummary(error, maxLen) {
    const text = runnerErrorText(error).replace(/\s+/g, ' ').trim();
    if (!text) return '';
    const limit = Number(maxLen) > 0 ? Math.floor(Number(maxLen)) : 80;
    return text.length > limit ? text.slice(0, Math.max(0, limit - 1)) + '…' : text;
  }

  function runnerRunsErrorText(reason) {
    const text = String(reason === undefined || reason === null ? '' : reason).trim();
    return '运行记录读取失败：' + (text || '未知错误');
  }

  function runnerParamKeys(params) {
    if (params === undefined || params === null) return [];
    if (Array.isArray(params)) {
      return params.map(function (p) {
        if (p && typeof p === 'object') return String(p.key === undefined || p.key === null ? '' : p.key).trim();
        return String(p === undefined || p === null ? '' : p).trim();
      }).filter(Boolean);
    }
    if (typeof params === 'object') return Object.keys(params);
    return [];
  }

  function runnerFlagText(value) {
    if (value === true) return '有';
    if (value === false) return '无';
    return '—';
  }

  function runnerTaskStatusText(task) {
    const t = task || {};
    return t.unmanaged === false ? '已安装' : '未托管/本机目录';
  }

  function runnerOutcomeClass(outcome) {
    const key = String(outcome === undefined || outcome === null ? '' : outcome).trim().toLowerCase();
    if (key === 'done' || key === 'success') return 'ok';
    if (key === 'failed' || key === 'fatal' || key === 'error') return 'err';
    if (key === 'exhausted' || key === 'resource' || key === 'blocked' || key === 'retryable') return 'warn';
    return 'kind';
  }

  function runnerLockText(lock) {
    if (lock === undefined || lock === null) return '调度锁 空闲';
    if (typeof lock === 'boolean') return '调度锁 ' + (lock ? '占用' : '空闲');
    if (typeof lock === 'number') return '调度锁 占用（pid ' + lock + '）';
    if (typeof lock === 'string') {
      const text = lock.trim();
      return text ? '调度锁 ' + text : '调度锁 空闲';
    }
    if (typeof lock === 'object') {
      const held = lock.held !== undefined ? lock.held
        : (lock.running !== undefined ? lock.running : undefined);
      const pid = lock.pid !== undefined ? lock.pid : lock.lock_pid;
      if (held === false) return '调度锁 空闲';
      if (pid !== undefined && pid !== null && pid !== '') return '调度锁 占用（pid ' + pid + '）';
      if (held === true) return '调度锁 占用';
      return '调度锁 空闲';
    }
    return '调度锁 —';
  }

  function runnerLockClass(lock) {
    const text = runnerLockText(lock);
    if (text.indexOf('占用') >= 0) return 'warn';
    if (text.indexOf('空闲') >= 0) return 'ok';
    return 'kind';
  }

  function runnerRunningText(running) {
    if (running === undefined || running === null || running === '') return '空闲';
    if (typeof running === 'string') return running;
    if (typeof running !== 'object') return String(running);
    const bits = [];
    if (running.entry_id) bits.push(String(running.entry_id));
    if (running.task) bits.push(String(running.task));
    if (running.pass !== undefined && running.pass !== null && running.pass !== '') {
      bits.push('pass ' + running.pass);
    }
    if (running.attempt !== undefined && running.attempt !== null && running.attempt !== '') {
      bits.push('attempt ' + running.attempt);
    }
    if (running.since_local) bits.push('自 ' + running.since_local);
    return bits.length ? bits.join(' · ') : '执行中';
  }

  function runnerGuardText(guards) {
    if (!guards || typeof guards !== 'object') return '—';
    if (guards.blocked) {
      return '已阻断' + (guards.blackout_reason ? '（' + guards.blackout_reason + '）' : '');
    }
    const active = Array.isArray(guards.active_hours) ? guards.active_hours.length : 0;
    const blackouts = Array.isArray(guards.blackouts) ? guards.blackouts.length : 0;
    return '正常（时段 ' + active + ' · 屏蔽 ' + blackouts + '）';
  }

  function runnerEntryStateText(entry) {
    const e = entry || {};
    let text = e.enabled === false ? '停用' : (e.enabled === true ? '启用' : '—');
    if (e.blocked_reason) text += ' · 等待 ' + e.blocked_reason;
    else if (e.runnable_now === true) text += ' · 可运行';
    return text;
  }

  function runnerQueueRoundsText(queue) {
    const q = queue || {};
    const total = Number(q.total_rounds);
    const done = Number(q.rounds_completed);
    const completed = isFinite(done) && done >= 0 ? Math.floor(done) : 0;
    if (!isFinite(total) || total <= 0) return completed + '/∞';
    return completed + '/' + Math.floor(total);
  }

  function runnerClicksText(value) {
    if (value === undefined || value === null || value === '') return '—';
    if (typeof value === 'number' || typeof value === 'string') return String(value);
    return runnerValueText(value);
  }

  function runnerExitText(run) {
    const r = run || {};
    if (r.exit_code !== undefined && r.exit_code !== null && r.exit_code !== '') {
      return runnerValueText(r.exit_code);
    }
    if (r.exit !== undefined && r.exit !== null && r.exit !== '') return runnerValueText(r.exit);
    return '—';
  }

  function runnerMemberText(value) {
    if (value === undefined || value === null || value === '') return '—';
    const n = Number(value);
    if (isFinite(n) && n >= 0) return '成员 ' + (Math.floor(n) + 1);
    return String(value);
  }

  // --- HTTP：所有请求的唯一入口 ------------------------------------------------

  function runnerBodyErrors(body) {
    if (!body || typeof body !== 'object') return '';
    if (Array.isArray(body.errors) && body.errors.length) {
      return body.errors
        .filter(function (e) { return e !== undefined && e !== null && e !== ''; })
        .map(String)
        .join('；');
    }
    if (body.error !== undefined && body.error !== null && body.error !== '') return String(body.error);
    if (body.message !== undefined && body.message !== null && body.message !== '') {
      return String(body.message);
    }
    return '';
  }

  async function runnerFetchJson(url) {
    const controller = typeof AbortController === 'function' ? new AbortController() : null;
    const timer = controller ? setTimeout(function () { controller.abort(); }, RUNNER_FETCH_TIMEOUT_MS) : null;
    try {
      const res = await fetch(url, {
        method: 'GET',
        headers: { Accept: 'application/json' },
        cache: 'no-store',
        signal: controller ? controller.signal : undefined,
      });
      const text = await res.text();
      let body = null;
      try {
        body = text ? JSON.parse(text) : null;
      } catch (e) {
        body = null;
      }
      const detail = runnerBodyErrors(body);
      if (!res.ok) {
        throw new Error('HTTP ' + res.status + (detail ? '：' + detail : ''));
      }
      if (!body || typeof body !== 'object') throw new Error('响应不是 JSON 对象');
      if (body.ok === false) throw new Error(detail || '服务端返回 ok=false');
      return body;
    } catch (e) {
      if (e && e.name === 'AbortError') {
        throw new Error('请求超时（' + Math.round(RUNNER_FETCH_TIMEOUT_MS / 1000) + ' 秒）');
      }
      throw e instanceof Error ? e : new Error(runnerErrorText(e) || '请求失败');
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  // --- DOM 辅助 ----------------------------------------------------------------

  function clearNode(node) {
    if (!node) return;
    while (node.firstChild) node.removeChild(node.firstChild);
  }

  function makeBadge(text, cls) {
    const span = document.createElement('span');
    span.className = 'runner-badge ' + (cls || 'kind');
    span.textContent = String(text === undefined || text === null ? '—' : text);
    return span;
  }

  function renderKv(host, rows) {
    if (!host) return;
    clearNode(host);
    if (!rows || !rows.length) {
      host.textContent = '—';
      return;
    }
    const dl = document.createElement('dl');
    dl.className = 'runner-kv';
    for (const pair of rows) {
      const dt = document.createElement('dt');
      dt.textContent = String(pair[0]);
      const dd = document.createElement('dd');
      dd.textContent = pair[1] === undefined || pair[1] === null ? '—' : String(pair[1]);
      dl.appendChild(dt);
      dl.appendChild(dd);
    }
    host.appendChild(dl);
  }

  function appendTableCell(tr, value) {
    const td = document.createElement('td');
    if (value && value.nodeType === 1) {
      td.appendChild(value);
    } else if (value && typeof value === 'object' && value.text !== undefined) {
      td.textContent = String(value.text);
      if (value.cls) td.className = value.cls;
      if (value.title) td.title = value.title;
    } else {
      td.textContent = value === undefined || value === null || value === '' ? '—' : String(value);
    }
    tr.appendChild(td);
  }

  function renderTable(host, headers, rows, emptyText) {
    if (!host) return;
    clearNode(host);
    if (!rows || !rows.length) {
      host.textContent = emptyText || '（无数据）';
      return;
    }
    const wrap = document.createElement('div');
    wrap.className = 'runner-table-wrap';
    const table = document.createElement('table');
    table.className = 'runner-table';
    const thead = document.createElement('thead');
    const headRow = document.createElement('tr');
    for (const label of headers) {
      const th = document.createElement('th');
      th.textContent = label;
      headRow.appendChild(th);
    }
    thead.appendChild(headRow);
    const tbody = document.createElement('tbody');
    for (const row of rows) {
      const tr = document.createElement('tr');
      for (const cell of row) appendTableCell(tr, cell);
      tbody.appendChild(tr);
    }
    table.appendChild(thead);
    table.appendChild(tbody);
    wrap.appendChild(table);
    host.appendChild(wrap);
  }

  function setBadge(node, text, cls) {
    if (!node) return;
    node.textContent = text;
    node.className = 'runner-badge ' + (cls || 'kind');
  }

  function setPageStatus(text, kind) {
    if (!el.pageStatus) return;
    el.pageStatus.textContent = text;
    el.pageStatus.className = 'muted small' + (kind ? ' ' + kind : '');
  }

  function updateLastRefresh() {
    if (!el.lastRefresh) return;
    const now = new Date();
    const pad = function (n) { return String(n).padStart(2, '0'); };
    el.lastRefresh.textContent = '上次刷新 ' +
      pad(now.getHours()) + ':' + pad(now.getMinutes()) + ':' + pad(now.getSeconds());
  }

  function setFetching(busy) {
    state.busy = !!busy;
    const buttons = [
      el.refresh, el.healthRefresh, el.tasksRefresh, el.queueRefresh,
      el.runsRefresh, el.settingsRefresh,
    ];
    for (const button of buttons) {
      if (button) button.disabled = !!busy;
    }
  }

  function bindElements() {
    el.pageStatus = document.getElementById('runner-page-status');
    el.refresh = document.getElementById('runner-refresh');
    el.connBadge = document.getElementById('runner-conn-badge');
    el.versionBadge = document.getElementById('runner-version-badge');
    el.portBadge = document.getElementById('runner-port-badge');
    el.lockBadge = document.getElementById('runner-lock-badge');
    el.lastRefresh = document.getElementById('runner-last-refresh');
    el.nav = document.getElementById('runner-nav');
    el.navButtons = el.nav
      ? Array.prototype.slice.call(el.nav.querySelectorAll('.app-nav-btn'))
      : [];
    el.pages = Array.prototype.slice.call(document.querySelectorAll('.app-page'));

    el.healthRefresh = document.getElementById('runner-health-refresh');
    el.overviewHealth = document.getElementById('overview-health');
    el.recentRuns = document.getElementById('overview-recent-runs');
    el.recentStatus = document.getElementById('overview-recent-status');

    el.tasksRefresh = document.getElementById('runner-tasks-refresh');
    el.tasksCount = document.getElementById('runner-tasks-count');
    el.tasksStatus = document.getElementById('runner-tasks-status');
    el.tasksList = document.getElementById('runner-tasks-list');

    el.queueRefresh = document.getElementById('runner-queue-refresh');
    el.queueStatus = document.getElementById('runner-queue-status');
    el.queueSummary = document.getElementById('queue-summary');
    el.queueEntries = document.getElementById('queue-entries');
    el.queueQueues = document.getElementById('queue-queues');

    el.runsRefresh = document.getElementById('runner-runs-refresh');
    el.runsStatus = document.getElementById('runner-runs-status');
    el.runsList = document.getElementById('runner-runs-list');

    el.settingsRefresh = document.getElementById('runner-settings-refresh');
    el.settingsStatus = document.getElementById('runner-settings-status');
    el.settingsDevice = document.getElementById('settings-device');
    el.settingsPaths = document.getElementById('settings-paths');
    el.settingsServe = document.getElementById('settings-serve');
    el.settingsRunner = document.getElementById('settings-runner');
    el.settingsScheduler = document.getElementById('settings-scheduler');
    el.settingsInstance = document.getElementById('settings-instance');
    el.settingsConfigJson = document.getElementById('settings-config-json');
  }

  // --- 运行面健康 / 概览 --------------------------------------------------------

  function applyHealth(health) {
    const h = health || {};
    setBadge(el.connBadge, 'API 已连接', 'ok');
    if (el.connBadge) el.connBadge.removeAttribute('title');
    setBadge(el.versionBadge, '版本 ' + runnerValueText(h.version), 'kind');
    setBadge(el.portBadge, '端口 ' + runnerValueText(h.port), 'kind');
    setBadge(el.lockBadge, runnerLockText(h.lock), runnerLockClass(h.lock));
  }

  function applyHealthError(error) {
    setBadge(el.connBadge, 'API 连接失败', 'err');
    setBadge(el.versionBadge, '版本 —', 'kind');
    setBadge(el.portBadge, '端口 —', 'kind');
    setBadge(el.lockBadge, '调度锁 —', 'kind');
    if (el.connBadge) el.connBadge.title = runnerErrorText(error);
  }

  async function refreshHealth() {
    try {
      const health = await runnerFetchJson(RUNNER_API.health);
      state.health = health;
      applyHealth(health);
      return '';
    } catch (e) {
      state.health = null;
      applyHealthError(e);
      return '健康检查读取失败：' + runnerErrorText(e);
    }
  }

  function renderOverviewHealth() {
    if (!state.health) {
      if (el.overviewHealth) el.overviewHealth.textContent = '—';
      return;
    }
    const h = state.health;
    const tasks = state.tasks && Array.isArray(state.tasks.tasks) ? state.tasks.tasks : null;
    renderKv(el.overviewHealth, [
      ['运行面', runnerValueText(h.surface)],
      ['版本', runnerValueText(h.version)],
      ['端口', runnerValueText(h.port)],
      ['调度模式', runnerValueText(h.scheduler_mode)],
      ['调度锁', runnerLockText(h.lock)],
      ['任务目录', runnerValueText(h.tasks_dir)],
      ['运行目录', runnerValueText(h.runs_dir)],
      ['状态文件', runnerValueText(h.state_path)],
      ['已安装任务数', tasks ? String(tasks.length) : '—'],
    ]);
  }

  function renderRecentRuns() {
    if (!el.recentRuns) return;
    if (state.recentRunsError) {
      el.recentRuns.textContent = '—';
      if (el.recentStatus) el.recentStatus.textContent = '最近运行读取失败：' + state.recentRunsError;
      return;
    }
    if (!state.recentRuns) {
      el.recentRuns.textContent = '（未加载）';
      if (el.recentStatus) el.recentStatus.textContent = '—';
      return;
    }
    const runs = Array.isArray(state.recentRuns.runs) ? state.recentRuns.runs : [];
    if (el.recentStatus) el.recentStatus.textContent = '最近 ' + runs.length + ' 条';
    if (!runs.length) {
      el.recentRuns.textContent = '暂无运行记录';
      return;
    }
    const rows = runs.map(function (run) {
      const r = run || {};
      return [
        runnerTimeText(r.started_ms),
        runnerValueText(r.task),
        runnerValueText(r.kind),
        makeBadge(runnerValueText(r.outcome), runnerOutcomeClass(r.outcome)),
        runnerExitText(r),
        runnerDurationText(runnerRunDurationMs(r)),
      ];
    });
    renderTable(el.recentRuns, ['时间', '任务', 'kind', 'outcome', 'exit_code', 'duration'], rows, '暂无运行记录');
  }

  function renderOverview() {
    renderOverviewHealth();
    renderRecentRuns();
  }

  async function loadOverview() {
    const errors = [];
    try {
      state.tasks = await runnerFetchJson(RUNNER_API.tasks);
    } catch (e) {
      state.tasks = null;
      errors.push('任务列表读取失败：' + runnerErrorText(e));
    }
    try {
      state.recentRuns = await runnerFetchJson(
        RUNNER_API.schedulerRuns + '?limit=' + RUNNER_RECENT_LIMIT
      );
      state.recentRunsError = '';
    } catch (e) {
      state.recentRuns = null;
      state.recentRunsError = runnerErrorText(e);
      errors.push('最近运行读取失败：' + state.recentRunsError);
    }
    renderOverview();
    return errors;
  }

  // --- 已安装任务 --------------------------------------------------------------

  function renderTasks() {
    if (!el.tasksList) return;
    const body = state.tasks || {};
    const tasks = Array.isArray(body.tasks) ? body.tasks : [];
    if (el.tasksCount) el.tasksCount.textContent = tasks.length + ' 个任务';
    const metaErrors = tasks.filter(function (t) {
      return t && t.meta_error;
    }).length;
    if (el.tasksStatus) {
      el.tasksStatus.textContent = '目录：' + runnerValueText(body.dir) +
        (metaErrors ? ' · 元数据异常 ' + metaErrors + ' 个' : '');
    }
    if (!tasks.length) {
      el.tasksList.textContent = '还没有安装任务包（安装功能将在后续 PR 提供）';
      return;
    }
    const rows = tasks.map(function (task) {
      const t = task || {};
      const title = t.meta_error
        ? 'meta.json 读取失败：' + t.meta_error
        : (t.version !== undefined && t.version !== null ? '版本 ' + t.version : '');
      return [
        { text: runnerValueText(t.task), title: title },
        runnerFlagText(t.has_flow),
        runnerFlagText(t.has_meta),
        runnerFlagText(t.has_assets),
        runnerParamKeys(t.params).join(', ') || '—',
        runnerTaskStatusText(t),
      ];
    });
    renderTable(el.tasksList, ['任务', 'flow', 'meta', 'assets', '参数 schema', '状态'], rows, '（没有任务）');
  }

  async function loadTasksPage() {
    try {
      state.tasks = await runnerFetchJson(RUNNER_API.tasks);
      renderTasks();
      return [];
    } catch (e) {
      state.tasks = null;
      const message = '任务列表读取失败：' + runnerErrorText(e);
      if (el.tasksCount) el.tasksCount.textContent = '—';
      if (el.tasksStatus) el.tasksStatus.textContent = message;
      if (el.tasksList) el.tasksList.textContent = message;
      return [message];
    }
  }

  // --- 队列与调度 --------------------------------------------------------------

  function renderQueue() {
    const s = state.scheduler || {};
    renderKv(el.queueSummary, [
      ['模式', runnerValueText(s.mode)],
      ['passes', runnerValueText(s.passes)],
      ['下次唤醒', runnerValueText(s.next_wake_local || runnerTimeText(s.next_wake_ms))],
      ['正在执行', runnerRunningText(s.running)],
      ['调度锁', state.health ? runnerLockText(state.health.lock) : '调度锁 —'],
      ['护栏', runnerGuardText(s.guards)],
      ['条目数', Array.isArray(s.entries) ? String(s.entries.length) : '—'],
      ['生成时间', runnerValueText(s.generated_local || runnerTimeText(s.generated_ms))],
    ]);
    if (el.queueStatus) {
      el.queueStatus.textContent = '配置：' + runnerValueText(s.config_path) +
        ' · 状态：' + runnerValueText(s.state_path);
    }

    const entries = Array.isArray(s.entries) ? s.entries : [];
    const entryRows = entries.map(function (entry) {
      const e = entry || {};
      return [
        runnerValueText(e.entry_id),
        runnerValueText(e.task),
        runnerValueText(e.kind),
        runnerValueText(e.lifecycle),
        runnerEntryStateText(e),
        runnerValueText(e.last_result || e.last_outcome),
        runnerValueText(e.next_due_local || runnerTimeText(e.next_due_ms)),
      ];
    });
    renderTable(
      el.queueEntries,
      ['条目', '任务', 'kind', 'lifecycle', '状态', '上次结果', '下次到期'],
      entryRows,
      '（没有调度条目）'
    );

    const queues = Array.isArray(s.queues) ? s.queues : [];
    const queueRows = queues.map(function (queue) {
      const q = queue || {};
      return [
        runnerValueText(q.name || q.queue_id),
        Array.isArray(q.members) ? q.members.join(', ') : '—',
        runnerQueueRoundsText(q),
        q.finished === true ? '已完成' : '进行中',
        runnerValueText(q.next_due_local || runnerTimeText(q.next_due_ms)),
      ];
    });
    renderTable(
      el.queueQueues,
      ['队列', '成员', '轮次', '状态', '下次到期'],
      queueRows,
      '（没有队列）'
    );
  }

  async function loadQueuePage() {
    try {
      state.scheduler = await runnerFetchJson(RUNNER_API.schedulerStatus);
      renderQueue();
      return [];
    } catch (e) {
      state.scheduler = null;
      const message = '调度器状态读取失败：' + runnerErrorText(e);
      if (el.queueStatus) el.queueStatus.textContent = message;
      if (el.queueSummary) el.queueSummary.textContent = '—';
      if (el.queueEntries) el.queueEntries.textContent = '—';
      if (el.queueQueues) el.queueQueues.textContent = '—';
      return [message];
    }
  }

  // --- 运行记录 ----------------------------------------------------------------

  function renderRuns() {
    if (!el.runsList) return;
    const body = state.runs || {};
    const runs = Array.isArray(body.runs) ? body.runs : [];
    if (el.runsStatus) {
      el.runsStatus.textContent = '共 ' + (body.count !== undefined ? body.count : runs.length) +
        ' 条 · 目录：' + runnerValueText(body.dir);
    }
    if (!runs.length) {
      el.runsList.textContent = '暂无运行记录';
      return;
    }
    const rows = runs.map(function (run) {
      const r = run || {};
      const fullError = runnerErrorText(r.error);
      return [
        runnerTimeText(r.started_ms),
        runnerValueText(r.task),
        runnerValueText(r.kind),
        runnerValueText(r.round),
        runnerMemberText(r.member_index),
        makeBadge(runnerValueText(r.outcome), runnerOutcomeClass(r.outcome)),
        runnerExitText(r),
        runnerDurationText(runnerRunDurationMs(r)),
        runnerClicksText(r.clicks),
        {
          text: runnerErrorSummary(r.error, 120) || '—',
          cls: fullError ? 'runner-error-cell' : '',
          title: fullError,
        },
      ];
    });
    renderTable(
      el.runsList,
      ['时间', '任务', 'kind', 'round', 'member', 'outcome', 'exit_code', 'duration', 'clicks', 'error'],
      rows,
      '暂无运行记录'
    );
  }

  async function loadRunsPage() {
    try {
      state.runs = await runnerFetchJson(RUNNER_API.schedulerRuns + '?limit=' + RUNNER_RUNS_LIMIT);
      renderRuns();
      return [];
    } catch (e) {
      state.runs = null;
      const message = runnerRunsErrorText(runnerErrorText(e));
      if (el.runsStatus) el.runsStatus.textContent = message;
      if (el.runsList) el.runsList.textContent = message;
      return [message];
    }
  }

  // --- 设置 --------------------------------------------------------------------

  function renderSettings() {
    const configBody = state.config;
    const cfg = configBody && configBody.config && typeof configBody.config === 'object'
      ? configBody.config
      : null;
    const health = state.health || {};

    if (!cfg) {
      renderKv(el.settingsDevice, []);
      renderKv(el.settingsPaths, []);
      renderKv(el.settingsServe, []);
      renderKv(el.settingsRunner, []);
      renderKv(el.settingsScheduler, []);
      if (el.settingsConfigJson) el.settingsConfigJson.textContent = '—';
    } else {
      const device = cfg.device || {};
      renderKv(el.settingsDevice, [
        ['serial', runnerValueText(device.serial)],
        ['adb_path', runnerValueText(device.adb_path)],
      ]);

      const paths = cfg.paths || {};
      renderKv(el.settingsPaths, [
        ['tasks_dir', runnerValueText(paths.tasks_dir || health.tasks_dir)],
        ['runs_dir', runnerValueText(paths.runs_dir || health.runs_dir)],
        ['library_dir', runnerValueText(paths.library_dir)],
      ]);

      const serve = cfg.serve || {};
      renderKv(el.settingsServe, [
        ['port', runnerValueText(serve.port)],
        ['bind', runnerValueText(serve.bind)],
        ['ui', runnerValueText(serve.ui)],
      ]);

      const runner = cfg.runner || null;
      renderKv(el.settingsRunner, [
        ['port', runner ? runnerValueText(runner.port) : runnerValueText(health.port)],
        ['bind', runner ? runnerValueText(runner.bind) : '—'],
        ['来源', runner ? 'config/live.json 的 runner 段' : '配置未声明（使用运行时端口）'],
      ]);

      const scheduler = cfg.scheduler || {};
      renderKv(el.settingsScheduler, [
        ['plan', runnerValueText(scheduler.plan)],
        ['state', runnerValueText(scheduler.state)],
        ['report', runnerValueText(scheduler.report)],
      ]);

      if (el.settingsConfigJson) {
        try {
          el.settingsConfigJson.textContent = JSON.stringify(cfg, null, 2);
        } catch (e) {
          el.settingsConfigJson.textContent = '—';
        }
      }
    }

    const instanceBody = state.instance;
    if (instanceBody) {
      const inst = instanceBody || {};
      const instState = inst.state || {};
      renderKv(el.settingsInstance, [
        ['active', runnerValueText(inst.active)],
        ['name', runnerValueText(inst.name)],
        ['device_serial', runnerValueText(inst.device_serial)],
        ['adb_path', runnerValueText(inst.adb_path)],
        ['serve_port', runnerValueText(inst.serve_port)],
        ['bind', runnerValueText(inst.bind)],
        ['device', runnerValueText(instState.device)],
        ['error', runnerValueText(instState.error)],
      ]);
    } else {
      renderKv(el.settingsInstance, []);
    }

    if (el.settingsStatus) {
      el.settingsStatus.textContent = '只读读取自 /config 与 /instance';
    }
  }

  async function loadSettingsPage() {
    const errors = [];
    try {
      state.config = await runnerFetchJson(RUNNER_API.config);
    } catch (e) {
      state.config = null;
      errors.push('配置读取失败：' + runnerErrorText(e));
    }
    try {
      state.instance = await runnerFetchJson(RUNNER_API.instance);
    } catch (e) {
      state.instance = null;
      errors.push('实例读取失败：' + runnerErrorText(e));
    }
    renderSettings();
    if (errors.length && el.settingsStatus) el.settingsStatus.textContent = errors.join('；');
    return errors;
  }

  // --- 页面切换 / 刷新 ----------------------------------------------------------

  function showPage(name) {
    const page = RUNNER_PAGES.indexOf(name) >= 0 ? name : 'overview';
    state.page = page;
    for (const pane of el.pages) {
      pane.classList.toggle('active', pane.id === 'page-' + page);
    }
    for (const button of el.navButtons) {
      button.classList.toggle('active', button.getAttribute('data-page') === page);
    }
    document.title = 'OAS Runner（使用端）· ' + (RUNNER_PAGE_LABELS[page] || '概览');
    // Deep-link friendly for a future Android WebView; history may be
    // unavailable on file/embedded contexts, so this must stay best-effort.
    try {
      if (window.history && window.history.replaceState) {
        window.history.replaceState(null, '', '#' + page);
      }
    } catch (e) {
      /* ignore: hash sync is cosmetic */
    }
  }

  function pageFromHash() {
    try {
      const raw = String(window.location.hash || '').replace(/^#/, '').trim();
      return RUNNER_PAGES.indexOf(raw) >= 0 ? raw : '';
    } catch (e) {
      return '';
    }
  }

  async function refreshCurrentPage() {
    if (state.busy) {
      state.queued = true;
      return;
    }
    setFetching(true);
    setPageStatus('读取中…');
    const errors = [];
    try {
      const healthError = await refreshHealth();
      if (healthError) errors.push(healthError);
      let pageErrors = [];
      if (state.page === 'overview') pageErrors = await loadOverview();
      else if (state.page === 'tasks') pageErrors = await loadTasksPage();
      else if (state.page === 'queue') pageErrors = await loadQueuePage();
      else if (state.page === 'runs') pageErrors = await loadRunsPage();
      else if (state.page === 'settings') pageErrors = await loadSettingsPage();
      for (const message of pageErrors) errors.push(message);
      updateLastRefresh();
      if (errors.length) setPageStatus(errors.join('；'), 'err');
      else setPageStatus('已刷新', 'ok');
    } catch (e) {
      setPageStatus('刷新失败：' + runnerErrorText(e), 'err');
    } finally {
      setFetching(false);
      if (state.queued) {
        state.queued = false;
        refreshCurrentPage();
      }
    }
  }

  function bindEvents() {
    if (el.refresh) el.refresh.addEventListener('click', function () { refreshCurrentPage(); });
    if (el.healthRefresh) el.healthRefresh.addEventListener('click', function () { refreshCurrentPage(); });
    if (el.tasksRefresh) el.tasksRefresh.addEventListener('click', function () { refreshCurrentPage(); });
    if (el.queueRefresh) el.queueRefresh.addEventListener('click', function () { refreshCurrentPage(); });
    if (el.runsRefresh) el.runsRefresh.addEventListener('click', function () { refreshCurrentPage(); });
    if (el.settingsRefresh) el.settingsRefresh.addEventListener('click', function () { refreshCurrentPage(); });
    for (const button of el.navButtons) {
      button.addEventListener('click', function () {
        const page = button.getAttribute('data-page');
        if (page === state.page) {
          refreshCurrentPage();
          return;
        }
        showPage(page);
        refreshCurrentPage();
      });
    }
    window.addEventListener('hashchange', function () {
      const page = pageFromHash();
      if (!page || page === state.page) return;
      showPage(page);
      refreshCurrentPage();
    });
  }

  function init() {
    bindElements();
    bindEvents();
    showPage(pageFromHash() || 'overview');
    setPageStatus('连接中…');
    refreshCurrentPage();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
