// OAS Runner（使用端）UI。
// 独立于 Studio 的编辑器脚本：不加载构建/流程/素材模块，也不调用创作或构建接口。
// 除「任务配置值」的 GET/PUT 外，其余请求都是只读 GET；所有请求统一经过
// runnerFetchJson（超时、HTTP 错误、JSON 错误归一化）。
'use strict';

(function () {
  const RUNNER_FETCH_TIMEOUT_MS = 8000;
  const RUNNER_RUNS_LIMIT = 100;
  const RUNNER_RECENT_LIMIT = 5;
  // PR6: queue-page scheduler status poll + temp-run result poll.
  const RUNNER_SCHED_POLL_MS = 5000;
  const RUNNER_RUN_POLL_MS = 500;
  const RUNNER_RUN_POLL_MAX_MS = 60000;
  const RUNNER_RUN_LOG_TAIL = 20;
  // POST /runner/scheduler/stop answers within 3 s (`timeout:true` while a long
  // attempt finishes). The UI then polls the scheduler status itself: 1 s tick,
  // 30 s budget, so a long attempt is shown as "停止中" instead of "停止失败".
  const RUNNER_STOP_POLL_MS = 1000;
  const RUNNER_STOP_POLL_MAX_MS = 30000;
  const RUNNER_PAGES = ['overview', 'tasks', 'packages', 'queue', 'runs', 'settings'];
  const RUNNER_PAGE_LABELS = {
    overview: '概览',
    tasks: '已安装任务',
    packages: '安装包',
    queue: '队列与调度',
    runs: '运行记录',
    settings: '设置',
  };
  const RUNNER_API = {
    health: '/runner/health',
    tasks: '/runner/tasks',
    packages: '/runner/packages',
    install: '/runner/install',
    uninstall: '/runner/uninstall',
    packagesActivate: '/runner/packages/activate',
    schedulerStatus: '/scheduler/status',
    schedulerConfig: '/scheduler/config',
    schedulerStart: '/runner/scheduler/start',
    schedulerStop: '/runner/scheduler/stop',
    schedulerThreadStatus: '/runner/scheduler/status',
    run: '/runner/run',
    runStatus: '/runner/run/status',
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
    packages: null,
    recentRuns: null,
    recentRunsError: '',
    scheduler: null,
    runnerScheduler: null,
    schedConfig: null,
    runs: null,
    config: null,
    instance: null,
  };

  const el = {};

  // PR2: 当前打开的任务配置对话框状态（运行值保存在 Runner 本机）。
  let configTask = '';
  let configParams = [];
  let configLoaded = false;
  let configBusy = false;

  // PR5: 安装/卸载/回滚是串行操作；期间禁用安装按钮并拒绝重复提交。
  let packagesBusy = false;

  // PR6: scheduler start/stop, temp run and scheduler.json saves are serial
  // operations; the timers are the only long-lived state.
  let schedBusy = false;
  let schedStopping = false;
  let schedJsonBusy = false;
  let schedJsonDirty = false;
  let runBusy = false;
  let runPollTimer = null;
  let runPollStartedMs = 0;
  let queuePollTimer = null;
  let stopPollTimer = null;
  let stopPollStartedMs = 0;

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

  // --- PR2: 任务配置值（纯函数，ui_pure_test 单独抽取执行） -------------------
  // schema 来自发布包的 meta.params（Studio 定义）；这里只负责生成表单、
  // 收集/转换本机运行值并拼 API URL。不依赖 DOM。

  function runnerConfigFieldKind(param) {
    const p = param && typeof param === 'object' ? param : {};
    const type = String(p.type === undefined || p.type === null ? '' : p.type).trim().toLowerCase();
    if (type === 'bool' || type === 'boolean') return 'bool';
    if (type === 'int' || type === 'integer' || type === 'float' || type === 'number') {
      return 'number';
    }
    if (Array.isArray(p.enum) && p.enum.length) return 'select';
    return 'text';
  }

  function runnerConfigIsIntegerType(param) {
    const p = param && typeof param === 'object' ? param : {};
    const type = String(p.type === undefined || p.type === null ? '' : p.type).trim().toLowerCase();
    return type === 'int' || type === 'integer';
  }

  function runnerConfigInitialValue(param, values) {
    const p = param && typeof param === 'object' ? param : {};
    const key = p.key === undefined || p.key === null ? '' : String(p.key).trim();
    const src = values && typeof values === 'object' ? values : {};
    if (key && Object.prototype.hasOwnProperty.call(src, key)) {
      const current = src[key];
      if (current !== undefined && current !== null) return current;
    }
    if (p.default !== undefined && p.default !== null) return p.default;
    return runnerConfigFieldKind(p) === 'bool' ? false : '';
  }

  function runnerConfigCollectValues(params, fieldValues) {
    const list = Array.isArray(params) ? params : [];
    const fields = fieldValues && typeof fieldValues === 'object' ? fieldValues : {};
    const values = {};
    const errors = [];
    for (const param of list) {
      if (!param || typeof param !== 'object' || Array.isArray(param)) continue;
      const key = param.key === undefined || param.key === null ? '' : String(param.key).trim();
      if (!key) continue;
      const rawLabel = param.label === undefined || param.label === null ? '' : String(param.label).trim();
      const label = rawLabel || key;
      const kind = runnerConfigFieldKind(param);
      const raw = Object.prototype.hasOwnProperty.call(fields, key) ? fields[key] : undefined;
      let present = true;
      let value;
      if (kind === 'bool') {
        value = raw === true || raw === 'true' || raw === 1 || raw === '1';
      } else if (kind === 'number') {
        const text = raw === undefined || raw === null ? '' : String(raw).trim();
        if (!text) {
          present = false;
        } else {
          const num = Number(text);
          if (!isFinite(num)) {
            errors.push('「' + label + '」必须是数字（当前：' + text + '）');
            continue;
          }
          if (runnerConfigIsIntegerType(param) && !Number.isInteger(num)) {
            errors.push('「' + label + '」必须是整数');
            continue;
          }
          // min/max/enum are authoritative on the server: send the value and
          // let a 400 name the key + range/options (the dialog renders every
          // server error line verbatim). Local checks stay limited to
          // conversion so the UI never hides the real API contract.
          value = num;
        }
      } else {
        const text = raw === undefined || raw === null ? '' : String(raw);
        if (!text) {
          present = false;
        } else {
          const options = Array.isArray(param.enum) ? param.enum.map(String) : [];
          if (options.length && options.indexOf(text) < 0) {
            errors.push('「' + label + '」必须是以下之一：' + options.join(' / '));
            continue;
          }
          value = text;
        }
      }
      if (!present) {
        const hasDefault = param.default !== undefined && param.default !== null;
        if (param.required === true && !hasDefault) {
          errors.push('「' + label + '」是必填项');
        }
        continue;
      }
      values[key] = value;
    }
    return { ok: errors.length === 0, values: values, errors: errors };
  }

  function runnerConfigUrl(task) {
    const name = String(task === undefined || task === null ? '' : task).trim();
    return '/runner/tasks/' + encodeURIComponent(name) + '/config';
  }

  // --- PR5: 安装包（纯函数，ui_pure_test 单独抽取执行） -----------------------
  // 安装、卸载与回滚都走 Runner-only 路由；这里只拼请求体、格式化和把服务端
  // 错误码翻成可执行的一句话。不依赖 DOM。

  // POST /runner/install body. The path is shown/trimmed as typed: the Runner
  // is a local surface, so no drive-letter rewriting or platform guessing.
  function runnerInstallBody(path, force) {
    const text = String(path === undefined || path === null ? '' : path).trim();
    return { path: text, force: force === true };
  }

  // Normalized display row for one installed package. Missing fields render
  // '—' instead of "undefined"; sha is shortened only for display (title keeps
  // the full value).
  function formatPackageRow(pkg) {
    const p = pkg && typeof pkg === 'object' ? pkg : {};
    const text = (v) => (v === undefined || v === null || v === '' ? '' : String(v));
    const packageId = text(p.package_id);
    const sha = text(p.sha256);
    return {
      task_id: text(p.task_id) || '—',
      version: text(p.version) || '—',
      package_id: packageId || '—',
      package_id_short: packageId && packageId.length > 28
        ? packageId.slice(0, 28) + '…'
        : (packageId || '—'),
      sha256: sha || '—',
      sha_short: sha ? (sha.length > 12 ? sha.slice(0, 12) + '…' : sha) : '—',
      installed_at_ms: p.installed_at_ms === undefined || p.installed_at_ms === null
        ? null
        : p.installed_at_ms,
      backup: text(p.backup) || '—',
    };
  }

  // 「已安装任务」行里的包信息徽标：已安装 vN / 未纳管。When the backend does
  // not return installed/version/package_id/unmanaged at all, stay neutral.
  function runnerTaskPackageBadge(task) {
    const t = task && typeof task === 'object' ? task : {};
    const version = t.version === undefined || t.version === null || t.version === ''
      ? ''
      : String(t.version);
    const packageId = t.package_id === undefined || t.package_id === null
      ? ''
      : String(t.package_id).trim();
    if (t.unmanaged === true) {
      return {
        text: '未纳管',
        cls: 'warn',
        title: 'tasks/ 裸目录：未经安装包管理；可在「安装包」页查看',
      };
    }
    if (t.unmanaged === false || t.installed === true || packageId) {
      return {
        text: version ? '已安装 v' + version : '已安装',
        cls: 'ok',
        title: packageId ? 'package_id：' + packageId : '已由安装包管理',
      };
    }
    return { text: '—', cls: 'kind', title: '任务列表未返回包信息' };
  }

  // Map the Runner install/uninstall errors to one actionable Chinese line
  // (details stay server-authoritative; this only explains the common codes).
  function runnerPackagesErrorText(err) {
    let raw;
    if (err === undefined || err === null) raw = '';
    else if (typeof err === 'string') raw = err;
    else if (typeof err === 'number' || typeof err === 'boolean') raw = String(err);
    else if (err.message !== undefined && err.message !== null) raw = String(err.message);
    else {
      try {
        raw = JSON.stringify(err);
      } catch (e) {
        raw = String(err);
      }
    }
    const text = String(raw === undefined || raw === null ? '' : raw).replace(/\s+/g, ' ').trim();
    if (!text) return '未知错误';
    const lower = text.toLowerCase();
    if (lower.indexOf('dirty_task') >= 0 || text.indexOf('被修改') >= 0) {
      return '任务目录在本机被修改过（dirty_task）：确认要覆盖时勾选 force 后重试。';
    }
    if (lower.indexOf('flow_ops') >= 0) {
      return '安装包需要更新的 Runner：manifest.engine.flow_ops 超出当前 Runner 支持范围。';
    }
    if (lower.indexOf('checksum') >= 0 || lower.indexOf('sha256') >= 0 || text.indexOf('校验') >= 0) {
      return '安装包校验失败（checksum）：文件不完整或被修改，请重新发布后再安装。';
    }
    if (lower.indexOf('not found') >= 0 || lower.indexOf('enoent') >= 0 ||
        text.indexOf('找不到') >= 0) {
      return '找不到安装包文件：' + text;
    }
    if (lower.indexOf('exists') >= 0 || text.indexOf('已存在') >= 0) {
      return '任务已存在：' + text;
    }
    if (lower.indexOf('version') >= 0 || text.indexOf('版本') >= 0) {
      return '版本不满足：' + text;
    }
    if (lower.indexOf('depend') >= 0 || text.indexOf('依赖') >= 0) {
      return '依赖缺失：' + text;
    }
    return text;
  }

  // --- PR6: 调度启停 / 临时运行 / scheduler.json（纯函数，ui_pure_test 单独抽取）
  // 只做「请求体 + 文案」的纯转换：不碰 DOM，也不直接发请求。

  // POST /runner/run body. The task is required; the params textarea is
  // optional. Empty text and `{}` mean "use the Runner's saved values"
  // (config/values.json); any key overrides a saved value. Anything that is not
  // a JSON object is rejected locally, before a request is sent.
  function runnerRunBody(task, paramsText) {
    const name = String(task === undefined || task === null ? '' : task).trim();
    if (!name) return { ok: false, body: null, error: '请选择要运行的任务' };
    const raw = String(paramsText === undefined || paramsText === null ? '' : paramsText).trim();
    if (!raw) return { ok: true, body: { task: name }, error: '' };
    let parsed;
    try {
      parsed = JSON.parse(raw);
    } catch (e) {
      return {
        ok: false,
        body: null,
        error: '参数不是合法 JSON：' + (e && e.message ? e.message : String(e)),
      };
    }
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return {
        ok: false,
        body: null,
        error: '参数必须是 JSON 对象（如 {"关键词":"突破"}）',
      };
    }
    const body = { task: name };
    if (Object.keys(parsed).length) body.params = parsed;
    return { ok: true, body: body, error: '' };
  }

  // One sentence for GET /runner/run/status (running / finished / failed).
  function runnerRunStatusText(status) {
    const s = status && typeof status === 'object' ? status : {};
    const task = s.task === undefined || s.task === null || s.task === '' ? '' : String(s.task);
    if (s.running === true) return '运行中：' + (task || '—');
    if (s.outcome !== undefined && s.outcome !== null && s.outcome !== '') {
      return '已结束：' + (task || '—') + ' · outcome=' + String(s.outcome);
    }
    if (s.error !== undefined && s.error !== null && s.error !== '') {
      return '失败：' + (task || '—') + ' · ' + runnerErrorSummary(s.error, 160);
    }
    if (s.finished_ms !== undefined && s.finished_ms !== null && s.finished_ms !== '') {
      return '已结束：' + (task || '—');
    }
    return '—';
  }

  // GET /runner/scheduler/status -> badge text (运行中 / 停止中 / 已停止).
  function runnerSchedStatusText(status) {
    const s = status && typeof status === 'object' ? status : null;
    if (!s) return '—';
    if (s.running === true) return s.stop_requested === true ? '停止中' : '运行中';
    if (s.stop_requested === true && s.thread_alive === true) return '停止中';
    return '已停止';
  }

  // The scheduling lock is a cross-process file lock; its pid is the one piece
  // of info that explains "who is running" (Runner thread vs `oasd scheduler`).
  function runnerSchedLockPidText(status) {
    const s = status && typeof status === 'object' ? status : null;
    if (!s) return '—';
    const pid = s.lock_pid;
    if (pid === undefined || pid === null || pid === '') return '—';
    return String(pid);
  }

  function runnerSchedThreadText(status) {
    const s = status && typeof status === 'object' ? status : null;
    if (!s || s.thread_alive === undefined || s.thread_alive === null) return '—';
    return s.thread_alive === true ? '存活' : '已退出';
  }

  // POST /scheduler/config body. The textarea holds the raw scheduler.json
  // document; the server shape is {config: <object>} (same as the old Studio
  // page contract). The Runner never rewrites the document locally.
  function runnerSchedJsonBody(text) {
    const raw = String(text === undefined || text === null ? '' : text).trim();
    if (!raw) return { ok: false, body: null, error: 'JSON 为空：请先载入或粘贴 scheduler.json' };
    let parsed;
    try {
      parsed = JSON.parse(raw);
    } catch (e) {
      return {
        ok: false,
        body: null,
        error: 'JSON 解析失败：' + (e && e.message ? e.message : String(e)),
      };
    }
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return { ok: false, body: null, error: '必须是 JSON 对象（scheduler.json 的顶层对象）' };
    }
    return { ok: true, body: { config: parsed }, error: '' };
  }

  // One actionable line per server error (the 409 busy/lease errors arrive in
  // body.errors; they are shown verbatim, one per line).
  function runnerErrorLines(error) {
    if (error && Array.isArray(error.details) && error.details.length) {
      return error.details.map(function (line) { return '· ' + String(line); });
    }
    return ['· ' + (runnerErrorText(error) || '未知错误')];
  }

  // --- HTTP：所有请求的唯一入口 ------------------------------------------------

  // Human label for the machine-readable 409 codes. The backend sends
  // `{ok:false,error:"device_busy"|"scheduler_locked"|"run_busy",errors:[...],
  // message:"..."}`; the summary must keep the code meaning instead of
  // replacing it with the human `errors` line (P1-1).
  function runnerErrorCodeLabel(code) {
    switch (String(code === undefined || code === null ? '' : code)) {
      case 'device_busy': return '设备被占用';
      case 'scheduler_locked': return '调度锁被占用';
      case 'run_busy': return '已有运行在进行';
      default: return '';
    }
  }

  function runnerBodyErrors(body) {
    if (!body || typeof body !== 'object') return '';
    const code = body.error === undefined || body.error === null ? '' : String(body.error);
    const message = body.message !== undefined && body.message !== null
      ? String(body.message).trim()
      : '';
    const label = runnerErrorCodeLabel(code);
    if (label) return message ? label + '：' + message : label;
    if (message) return message;
    if (code) return code;
    if (Array.isArray(body.errors) && body.errors.length) {
      return body.errors
        .filter(function (e) { return e !== undefined && e !== null && e !== ''; })
        .map(String)
        .join('；');
    }
    return '';
  }

  // POST /runner/scheduler/stop body -> user-facing text (P1-2). A timeout is a
  // "still stopping" state, never a failure: the UI polls until running=false.
  function runnerStopText(body) {
    const b = body && typeof body === 'object' ? body : null;
    if (!b) return '已请求停止（等待调度线程退出）';
    if (b.already_stopped === true) return '调度本来就没在运行';
    if (b.timeout === true || b.stopping === true) return '停止中（当前任务跑完后退出）';
    if (b.stopped === true || b.running === false) return '调度已停止';
    return '已请求停止（等待调度线程退出）';
  }

  function runnerStopPending(body) {
    const b = body && typeof body === 'object' ? body : null;
    return !!b && (b.timeout === true || b.stopping === true);
  }

  // Scheduler history rows shown only when present (`last_error` / `last_stop`
  // explain an auto-start failure or the previous stop reason; P2 UI).
  function runnerSchedHistoryRows(status) {
    const s = status && typeof status === 'object' ? status : null;
    const rows = [];
    if (!s) return rows;
    if (s.last_stop !== undefined && s.last_stop !== null &&
        String(s.last_stop).trim() !== '') {
      rows.push(['最后停止', String(s.last_stop)]);
    }
    if (s.last_error !== undefined && s.last_error !== null &&
        String(s.last_error).trim() !== '') {
      rows.push(['最后错误', String(s.last_error)]);
    }
    return rows;
  }

  async function runnerFetchJson(url, opts) {
    const options = opts && typeof opts === 'object' ? opts : {};
    const method = options.method ? String(options.method).toUpperCase() : 'GET';
    const headers = { Accept: 'application/json' };
    let payload;
    if (options.body !== undefined && options.body !== null) {
      headers['Content-Type'] = 'application/json';
      payload = JSON.stringify(options.body);
    }
    const controller = typeof AbortController === 'function' ? new AbortController() : null;
    const timer = controller ? setTimeout(function () { controller.abort(); }, RUNNER_FETCH_TIMEOUT_MS) : null;
    try {
      const res = await fetch(url, {
        method: method,
        headers: headers,
        cache: 'no-store',
        body: payload,
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
        const err = new Error('HTTP ' + res.status + (detail ? '：' + detail : ''));
        err.status = res.status;
        // Keep the machine code (`device_busy` / `scheduler_locked` /
        // `run_busy`) so callers can branch on it instead of regexing the text.
        if (body && typeof body === 'object' &&
            body.error !== undefined && body.error !== null && body.error !== '') {
          err.code = String(body.error);
        }
        if (body && Array.isArray(body.errors) && body.errors.length) {
          err.details = body.errors.map(String);
        }
        throw err;
      }
      if (!body || typeof body !== 'object') throw new Error('响应不是 JSON 对象');
      if (body.ok === false) {
        const err = new Error(detail || '服务端返回 ok=false');
        if (body.error !== undefined && body.error !== null && body.error !== '') {
          err.code = String(body.error);
        }
        if (Array.isArray(body.errors) && body.errors.length) {
          err.details = body.errors.map(String);
        }
        throw err;
      }
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
      el.refresh, el.healthRefresh, el.tasksRefresh, el.packagesRefresh,
      el.queueRefresh, el.runsRefresh, el.settingsRefresh, el.schedRefresh,
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

    el.packagesRefresh = document.getElementById('runner-packages-refresh');
    el.packagesSummary = document.getElementById('runner-packages-summary');
    el.packagesStatus = document.getElementById('runner-packages-status');
    el.packagesCount = document.getElementById('runner-packages-count');
    el.packagesList = document.getElementById('runner-packages-list');
    el.packagesUnmanagedStatus = document.getElementById('runner-packages-unmanaged-status');
    el.packagesUnmanaged = document.getElementById('runner-packages-unmanaged');
    el.installPath = document.getElementById('runner-install-path');
    el.installForce = document.getElementById('runner-install-force');
    el.installSubmit = document.getElementById('runner-install-submit');
    el.installStatus = document.getElementById('runner-install-status');
    el.installResult = document.getElementById('runner-install-result');
    el.installWarnings = document.getElementById('runner-install-warnings');
    el.installError = document.getElementById('runner-install-error');

    el.configDialog = document.getElementById('runner-config-dialog');
    el.configTitle = document.getElementById('runner-config-title');
    el.configForm = document.getElementById('runner-config-form');
    el.configFields = document.getElementById('runner-config-fields');
    el.configError = document.getElementById('runner-config-error');
    el.configStatus = document.getElementById('runner-config-status');
    el.configSave = document.getElementById('runner-config-save');
    el.configClose = document.getElementById('runner-config-close');

    el.queueRefresh = document.getElementById('runner-queue-refresh');
    el.queueStatus = document.getElementById('runner-queue-status');
    el.queueSummary = document.getElementById('queue-summary');
    el.queueEntries = document.getElementById('queue-entries');
    el.queueQueues = document.getElementById('queue-queues');
    // PR6: scheduler control / temp run / advanced scheduler.json editor.
    el.schedControl = document.getElementById('runner-sched-control');
    el.schedBadge = document.getElementById('runner-sched-badge');
    el.schedMeta = document.getElementById('runner-sched-meta');
    el.schedStart = document.getElementById('runner-sched-start');
    el.schedStop = document.getElementById('runner-sched-stop');
    el.schedRefresh = document.getElementById('runner-sched-refresh');
    el.schedStatus = document.getElementById('runner-sched-status');
    el.schedError = document.getElementById('runner-sched-error');
    el.runTask = document.getElementById('runner-run-task');
    el.runParams = document.getElementById('runner-run-params');
    el.runSubmit = document.getElementById('runner-run-submit');
    el.runStatus = document.getElementById('runner-run-status');
    el.runError = document.getElementById('runner-run-error');
    el.runLog = document.getElementById('runner-run-log');
    el.schedJson = document.getElementById('runner-sched-json');
    el.schedJsonSave = document.getElementById('runner-sched-json-save');
    el.schedJsonStatus = document.getElementById('runner-sched-json-status');
    el.schedJsonError = document.getElementById('runner-sched-json-error');
    el.overviewScheduler = document.getElementById('overview-scheduler');
    el.overviewGotoQueue = document.getElementById('overview-goto-queue');

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
    const sched = h.scheduler && typeof h.scheduler === 'object' ? h.scheduler : null;
    renderKv(el.overviewHealth, [
      ['运行面', runnerValueText(h.surface)],
      ['版本', runnerValueText(h.version)],
      ['端口', runnerValueText(h.port)],
      ['调度模式', runnerValueText(h.scheduler_mode)],
      ['调度锁', runnerLockText(h.lock)],
      ['调度状态', sched ? runnerSchedStatusText(sched) : '—'],
      ['锁 pid', runnerSchedLockPidText(sched)],
      ['调度线程', runnerSchedThreadText(sched)],
      ['任务目录', runnerValueText(h.tasks_dir)],
      ['运行目录', runnerValueText(h.runs_dir)],
      ['状态文件', runnerValueText(h.state_path)],
      ['已安装任务数', tasks ? String(tasks.length) : '—'],
    ]);
  }

  function renderOverviewScheduler() {
    if (!el.overviewScheduler) return;
    const sched = state.runnerScheduler ||
      (state.health && state.health.scheduler && typeof state.health.scheduler === 'object'
        ? state.health.scheduler
        : null);
    if (!sched) {
      el.overviewScheduler.textContent = '—';
      return;
    }
    const passes = sched.passes === undefined || sched.passes === null ? '—' : String(sched.passes);
    renderKv(el.overviewScheduler, [
      ['调度状态', runnerSchedStatusText(sched)],
      ['调度线程', runnerSchedThreadText(sched)],
      ['锁 pid', runnerSchedLockPidText(sched)],
      ['停止请求', sched.stop_requested === true ? '已请求（当前任务跑完后停止）' : '无'],
      ['passes', passes],
      ['上次 pass', runnerTimeText(sched.last_pass_ms)],
    ].concat(runnerSchedHistoryRows(sched)));
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
    renderOverviewScheduler();
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
      el.tasksList.textContent =
        '还没有任务：请到「安装包」页安装 .oaspkg，或把任务目录放到 tasks/ 下（未纳管）。';
      return;
    }
    const rows = tasks.map(function (task) {
      const t = task || {};
      const title = t.meta_error
        ? 'meta.json 读取失败：' + t.meta_error
        : (t.version !== undefined && t.version !== null ? '版本 ' + t.version : '');
      const configBtn = document.createElement('button');
      configBtn.type = 'button';
      configBtn.className = 'runner-config-open';
      configBtn.textContent = '配置';
      configBtn.title = '编辑该任务的运行值（保存在本机 config/values.json）';
      configBtn.addEventListener('click', function () { openRunnerConfigDialog(t.task); });
      // PR5: package provenance badge (installed vN / unmanaged bare dir).
      const packageBadge = runnerTaskPackageBadge(t);
      const badgeEl = makeBadge(packageBadge.text, packageBadge.cls);
      badgeEl.title = packageBadge.title;
      return [
        { text: runnerValueText(t.task), title: title },
        runnerFlagText(t.has_flow),
        runnerFlagText(t.has_meta),
        runnerFlagText(t.has_assets),
        runnerParamKeys(t.params).join(', ') || '—',
        runnerTaskStatusText(t),
        badgeEl,
        t.has_meta === true ? configBtn : { text: '—' },
      ];
    });
    renderTable(
      el.tasksList,
      ['任务', 'flow', 'meta', 'assets', '参数 key', '状态', '包', '操作'],
      rows,
      '（没有任务）'
    );
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

  // --- 安装包（安装 / 列表 / 卸载 / 回滚） -------------------------------------

  function setPackagesBusy(busy) {
    if (el.installSubmit) el.installSubmit.disabled = !!busy;
    if (el.packagesRefresh) el.packagesRefresh.disabled = !!busy;
  }

  function installErrorLines(error) {
    if (error && Array.isArray(error.details) && error.details.length) {
      return error.details.map(function (line) {
        return '· ' + runnerPackagesErrorText(line);
      });
    }
    return ['· ' + runnerPackagesErrorText(error)];
  }

  function renderInstallResult(body) {
    const b = body && typeof body === 'object' ? body : {};
    renderKv(el.installResult, [
      ['package_id', runnerValueText(b.package_id)],
      ['task', runnerValueText(b.task_id !== undefined ? b.task_id : b.task)],
      ['version', runnerValueText(b.version)],
      ['replaced', b.replaced === true ? '是（旧版本已备份）' : (b.replaced === false ? '否' : '—')],
      ['backup', runnerValueText(b.backup)],
    ]);
    const warnings = Array.isArray(b.warnings) ? b.warnings : [];
    if (el.installWarnings) {
      el.installWarnings.textContent = warnings.length
        ? 'warnings：\n' + warnings.map(function (w) { return '· ' + String(w); }).join('\n')
        : '';
    }
  }

  function packageRowActions(row) {
    const wrap = document.createElement('span');
    wrap.className = 'runner-package-actions';
    const task = row && row.task_id && row.task_id !== '—' ? row.task_id : '';
    const uninstall = document.createElement('button');
    uninstall.type = 'button';
    uninstall.textContent = '卸载';
    uninstall.title = '卸载该任务包（任务目录先备份到 packages/backup/）';
    uninstall.className = 'runner-package-uninstall';
    uninstall.disabled = !task;
    uninstall.addEventListener('click', function () { uninstallRunnerPackage(task); });
    const activate = document.createElement('button');
    activate.type = 'button';
    activate.textContent = '回滚';
    activate.title = '回滚到该任务最近一次备份（Phase 1）';
    activate.className = 'runner-package-activate';
    activate.disabled = !task;
    activate.addEventListener('click', function () { activateRunnerPackage(task); });
    wrap.appendChild(uninstall);
    wrap.appendChild(activate);
    return wrap;
  }

  function renderPackages(body) {
    const b = body && typeof body === 'object' ? body : {};
    const packages = Array.isArray(b.packages) ? b.packages
      : (Array.isArray(b.installed) ? b.installed : []);
    if (el.packagesCount) el.packagesCount.textContent = packages.length + ' 个包';
    if (el.packagesSummary) {
      el.packagesSummary.textContent = 'packages/installed.json · 共 ' + packages.length + ' 个包';
    }
    const rows = packages.map(function (pkg) {
      const row = formatPackageRow(pkg);
      return [
        row.task_id,
        row.version,
        { text: row.package_id_short, title: row.package_id === '—' ? '' : row.package_id },
        { text: row.sha_short, title: row.sha256 === '—' ? '' : row.sha256 },
        runnerTimeText(row.installed_at_ms),
        { text: row.backup, title: row.backup === '—' ? '' : row.backup },
        packageRowActions(row),
      ];
    });
    renderTable(
      el.packagesList,
      ['任务', '版本', 'package_id', 'sha256', '安装时间', '备份', '操作'],
      rows,
      '尚未安装任务包（可用上方「安装」导入 .oaspkg）'
    );
  }

  function renderUnmanaged(items) {
    const list = Array.isArray(items) ? items : [];
    if (el.packagesUnmanagedStatus) {
      el.packagesUnmanagedStatus.textContent = list.length
        ? list.length + ' 个未纳管任务'
        : '没有未纳管任务';
    }
    if (!el.packagesUnmanaged) return;
    clearNode(el.packagesUnmanaged);
    if (!list.length) {
      el.packagesUnmanaged.textContent = '没有未纳管任务';
      return;
    }
    for (const item of list) {
      const t = item && typeof item === 'object' ? item : { task: String(item) };
      const name = runnerValueText(t.task !== undefined ? t.task : (t.task_id || t.name));
      const row = document.createElement('div');
      row.className = 'runner-unmanaged-row';
      const strong = document.createElement('strong');
      strong.textContent = name;
      const note = document.createElement('span');
      note.className = 'muted small';
      note.textContent = ' 未纳管（裸目录）· tasks/' + name;
      row.appendChild(strong);
      row.appendChild(note);
      el.packagesUnmanaged.appendChild(row);
    }
  }

  async function loadPackagesPage() {
    const errors = [];
    try {
      const body = await runnerFetchJson(RUNNER_API.packages);
      state.packages = body;
      renderPackages(body);
      if (el.packagesStatus) {
        el.packagesStatus.textContent = '已安装清单读取自 /runner/packages';
        el.packagesStatus.className = 'muted small';
      }
    } catch (e) {
      state.packages = null;
      const message = '安装包列表读取失败：' + runnerErrorText(e);
      if (el.packagesSummary) el.packagesSummary.textContent = '—';
      if (el.packagesCount) el.packagesCount.textContent = '—';
      if (el.packagesList) el.packagesList.textContent = message;
      if (el.packagesStatus) {
        el.packagesStatus.textContent = message;
        el.packagesStatus.className = 'muted small err';
      }
      return [message];
    }
    let unmanaged = null;
    const pkgBody = state.packages || {};
    if (Array.isArray(pkgBody.unmanaged)) unmanaged = pkgBody.unmanaged;
    if (!unmanaged) {
      try {
        const tasksBody = await runnerFetchJson(RUNNER_API.tasks);
        state.tasks = tasksBody;
        const tasks = Array.isArray(tasksBody.tasks) ? tasksBody.tasks : [];
        unmanaged = tasks.filter(function (t) { return t && t.unmanaged === true; });
      } catch (e) {
        const message = '未纳管任务读取失败：' + runnerErrorText(e);
        unmanaged = [];
        errors.push(message);
      }
    }
    renderUnmanaged(unmanaged);
    return errors;
  }

  async function installRunnerPackage() {
    if (packagesBusy) return;
    const body = runnerInstallBody(
      el.installPath ? el.installPath.value : '',
      el.installForce ? el.installForce.checked : false
    );
    if (!body.path) {
      if (el.installError) el.installError.textContent = '· 请填写本机 .oaspkg 路径';
      if (el.installStatus) {
        el.installStatus.textContent = '未安装';
        el.installStatus.className = 'muted small err';
      }
      return;
    }
    packagesBusy = true;
    setPackagesBusy(true);
    if (el.installError) el.installError.textContent = '';
    if (el.installWarnings) el.installWarnings.textContent = '';
    if (el.installResult) el.installResult.textContent = '安装中…';
    if (el.installStatus) {
      el.installStatus.textContent = '安装中…（sha256 / 解包 / 复检）';
      el.installStatus.className = 'muted small';
    }
    try {
      const result = await runnerFetchJson(RUNNER_API.install, { method: 'POST', body: body });
      renderInstallResult(result);
      if (el.installStatus) {
        el.installStatus.textContent = '安装成功';
        el.installStatus.className = 'muted small ok';
      }
      setPageStatus('已安装任务包：' + runnerValueText(result.task_id || result.task), 'ok');
      await loadPackagesPage();
    } catch (e) {
      if (el.installError) el.installError.textContent = installErrorLines(e).join('\n');
      if (el.installResult) el.installResult.textContent = '—';
      if (el.installStatus) {
        el.installStatus.textContent = '安装失败';
        el.installStatus.className = 'muted small err';
      }
    } finally {
      packagesBusy = false;
      setPackagesBusy(false);
    }
  }

  async function uninstallRunnerPackage(task) {
    if (packagesBusy || !task) return;
    if (typeof window.confirm === 'function' &&
        !window.confirm('卸载任务包「' + task + '」？\n\n任务目录会先备份到 packages/backup/，再从已安装清单移除。')) {
      return;
    }
    packagesBusy = true;
    setPackagesBusy(true);
    if (el.packagesStatus) {
      el.packagesStatus.textContent = '卸载中…';
      el.packagesStatus.className = 'muted small';
    }
    try {
      await runnerFetchJson(RUNNER_API.uninstall, { method: 'POST', body: { task_id: task } });
      setPageStatus('已卸载任务包：' + task, 'ok');
      await loadPackagesPage();
    } catch (e) {
      if (el.packagesStatus) {
        el.packagesStatus.textContent = '卸载失败：' + installErrorLines(e).join('；');
        el.packagesStatus.className = 'muted small err';
      }
    } finally {
      packagesBusy = false;
      setPackagesBusy(false);
    }
  }

  async function activateRunnerPackage(task) {
    if (packagesBusy || !task) return;
    if (typeof window.confirm === 'function' &&
        !window.confirm('回滚任务「' + task + '」到最近备份？\n\n当前任务目录会先备份。')) {
      return;
    }
    packagesBusy = true;
    setPackagesBusy(true);
    if (el.packagesStatus) {
      el.packagesStatus.textContent = '回滚中…';
      el.packagesStatus.className = 'muted small';
    }
    try {
      await runnerFetchJson(RUNNER_API.packagesActivate, { method: 'POST', body: { task_id: task } });
      setPageStatus('已回滚任务包：' + task, 'ok');
      await loadPackagesPage();
    } catch (e) {
      if (el.packagesStatus) {
        el.packagesStatus.textContent = '回滚失败：' + installErrorLines(e).join('；');
        el.packagesStatus.className = 'muted small err';
      }
    } finally {
      packagesBusy = false;
      setPackagesBusy(false);
    }
  }

  // --- 任务配置值（Runner 本机 config/values.json） ---------------------------

  function runnerConfigDefaultText(value) {
    if (value === '') return '（空字符串）';
    return runnerValueText(value);
  }

  function runnerConfigHintText(param, kind) {
    const p = param || {};
    const bits = [];
    if (kind === 'number') {
      const hasMin = p.min !== undefined && p.min !== null && p.min !== '' && isFinite(Number(p.min));
      const hasMax = p.max !== undefined && p.max !== null && p.max !== '' && isFinite(Number(p.max));
      if (hasMin && hasMax) bits.push('范围 ' + Number(p.min) + ' ~ ' + Number(p.max));
      else if (hasMin) bits.push('最小 ' + Number(p.min));
      else if (hasMax) bits.push('最大 ' + Number(p.max));
      if (runnerConfigIsIntegerType(p)) bits.push('整数');
    }
    if ((kind === 'select' || kind === 'number') && Array.isArray(p.enum) && p.enum.length) {
      bits.push('可选：' + p.enum.map(String).join(' / '));
    }
    if (p.default !== undefined && p.default !== null) {
      bits.push('默认：' + runnerConfigDefaultText(p.default));
    }
    return bits.join(' · ');
  }

  function renderRunnerConfigFields(params, values) {
    if (!el.configFields) return;
    clearNode(el.configFields);
    el.configFields.className = '';
    if (!params.length) {
      el.configFields.textContent = '该任务没有配置项';
      el.configFields.className = 'runner-config-empty';
      return;
    }
    for (let i = 0; i < params.length; i++) {
      const param = params[i] && typeof params[i] === 'object' ? params[i] : {};
      const key = param.key === undefined || param.key === null ? '' : String(param.key).trim();
      if (!key) continue;
      const kind = runnerConfigFieldKind(param);
      const initial = runnerConfigInitialValue(param, values);
      const rawLabel = param.label === undefined || param.label === null ? '' : String(param.label).trim();
      const labelText = rawLabel || key;
      const controlId = 'runner-cfg-' + i;

      const field = document.createElement('div');
      field.className = 'runner-config-field';

      const label = document.createElement('label');
      label.setAttribute('for', controlId);
      label.textContent = labelText;
      if (param.required === true) {
        const required = document.createElement('span');
        required.className = 'runner-config-required';
        required.textContent = '＊必填';
        label.appendChild(required);
      }
      field.appendChild(label);

      if (param.description) {
        const desc = document.createElement('div');
        desc.className = 'runner-config-desc';
        desc.textContent = String(param.description);
        field.appendChild(desc);
      }

      let control;
      if (kind === 'bool') {
        control = document.createElement('input');
        control.type = 'checkbox';
        control.checked = initial === true;
      } else if (kind === 'number') {
        control = document.createElement('input');
        control.type = 'number';
        control.inputMode = 'decimal';
        control.step = runnerConfigIsIntegerType(param) ? '1' : 'any';
        if (param.min !== undefined && param.min !== null && param.min !== '' &&
            isFinite(Number(param.min))) {
          control.min = String(Number(param.min));
        }
        if (param.max !== undefined && param.max !== null && param.max !== '' &&
            isFinite(Number(param.max))) {
          control.max = String(Number(param.max));
        }
        control.value = initial === undefined || initial === null || initial === ''
          ? '' : String(initial);
      } else if (kind === 'select') {
        control = document.createElement('select');
        const options = param.enum.map(String);
        const current = initial === undefined || initial === null ? '' : String(initial);
        const emptyOption = document.createElement('option');
        emptyOption.value = '';
        emptyOption.textContent = param.default !== undefined && param.default !== null
          ? '（默认：' + runnerConfigDefaultText(param.default) + '）'
          : '（未设置）';
        control.appendChild(emptyOption);
        for (const optionText of options) {
          const option = document.createElement('option');
          option.value = optionText;
          option.textContent = optionText;
          control.appendChild(option);
        }
        control.value = options.indexOf(current) >= 0 ? current : '';
      } else {
        control = document.createElement('input');
        control.type = 'text';
        control.value = initial === undefined || initial === null ? '' : String(initial);
      }
      control.id = controlId;
      control.name = controlId;
      control.setAttribute('data-key', key);
      if (param.required === true) control.setAttribute('aria-required', 'true');
      field.appendChild(control);

      const hintText = runnerConfigHintText(param, kind);
      if (hintText) {
        const hint = document.createElement('div');
        hint.className = 'runner-config-hint';
        hint.textContent = hintText;
        field.appendChild(hint);
      }
      el.configFields.appendChild(field);
    }
  }

  function closeRunnerConfigDialog() {
    if (!el.configDialog) return;
    if (typeof el.configDialog.close === 'function') {
      if (el.configDialog.open) el.configDialog.close();
    } else {
      el.configDialog.removeAttribute('open');
    }
  }

  async function loadRunnerConfig(task) {
    configLoaded = false;
    try {
      const body = await runnerFetchJson(runnerConfigUrl(task));
      if (configTask !== task) return; // 已关闭/切换到别的任务，丢弃过期响应
      const params = Array.isArray(body.params) ? body.params : [];
      const values = body.values && typeof body.values === 'object' ? body.values : {};
      configParams = params;
      configLoaded = true;
      renderRunnerConfigFields(params, values);
      if (el.configError) el.configError.textContent = '';
      if (el.configStatus) {
        let statusText = '已加载 ' + params.length + ' 项配置';
        if (body.updated_ms !== undefined && body.updated_ms !== null) {
          statusText += ' · 上次保存 ' + runnerTimeText(body.updated_ms);
        }
        el.configStatus.textContent = statusText;
        el.configStatus.className = 'muted small';
      }
      if (el.configSave) el.configSave.disabled = params.length === 0;
    } catch (e) {
      if (configTask !== task) return;
      const message = '配置读取失败：' + runnerErrorText(e);
      if (el.configError) el.configError.textContent = message;
      if (el.configStatus) {
        el.configStatus.textContent = '读取失败';
        el.configStatus.className = 'muted small err';
      }
      if (el.configFields) {
        clearNode(el.configFields);
        el.configFields.textContent = '（读取失败）';
        el.configFields.className = 'runner-config-empty';
      }
      if (el.configSave) el.configSave.disabled = true;
    }
  }

  function openRunnerConfigDialog(task) {
    const name = String(task === undefined || task === null ? '' : task).trim();
    if (!el.configDialog || !name) return;
    configTask = name;
    configParams = [];
    configLoaded = false;
    configBusy = false;
    if (el.configTitle) el.configTitle.textContent = '任务配置 · ' + name;
    if (el.configStatus) {
      el.configStatus.textContent = '读取中…';
      el.configStatus.className = 'muted small';
    }
    if (el.configError) el.configError.textContent = '';
    if (el.configFields) {
      clearNode(el.configFields);
      el.configFields.textContent = '（读取中…）';
      el.configFields.className = 'runner-config-empty';
    }
    if (el.configSave) el.configSave.disabled = true;
    if (typeof el.configDialog.showModal === 'function') {
      if (!el.configDialog.open) el.configDialog.showModal();
    } else {
      el.configDialog.setAttribute('open', '');
    }
    loadRunnerConfig(name);
  }

  async function saveRunnerConfig() {
    if (configBusy || !configLoaded || !configTask) return;
    const task = configTask;
    const fieldValues = {};
    if (el.configFields) {
      const controls = Array.prototype.slice.call(
        el.configFields.querySelectorAll('input[data-key], select[data-key]')
      );
      for (const control of controls) {
        const key = control.getAttribute('data-key');
        if (!key) continue;
        fieldValues[key] = control.type === 'checkbox' ? control.checked : control.value;
      }
    }
    const collected = runnerConfigCollectValues(configParams, fieldValues);
    if (!collected.ok) {
      if (el.configError) {
        el.configError.textContent = collected.errors.map(function (line) {
          return '· ' + line;
        }).join('\n');
      }
      if (el.configStatus) {
        el.configStatus.textContent = '未保存：' + collected.errors.length + ' 处校验错误';
        el.configStatus.className = 'muted small err';
      }
      return;
    }
    configBusy = true;
    if (el.configSave) el.configSave.disabled = true;
    if (el.configError) el.configError.textContent = '';
    if (el.configStatus) {
      el.configStatus.textContent = '保存中…';
      el.configStatus.className = 'muted small';
    }
    try {
      const body = await runnerFetchJson(runnerConfigUrl(task), {
        method: 'PUT',
        body: { values: collected.values },
      });
      const saved = body.values && typeof body.values === 'object' ? body.values : collected.values;
      const savedCount = Object.keys(saved).length;
      setPageStatus('已保存任务配置：' + task + '（' + savedCount + ' 项）', 'ok');
      closeRunnerConfigDialog();
      // 刷新列表（值本身不显示在列表里，但保持页面数据新鲜）。
      await loadTasksPage();
    } catch (e) {
      let message;
      if (e && e.status === 404) {
        message = '任务已不存在：' + task + '（可能已被卸载）';
      } else if (e && Array.isArray(e.details) && e.details.length) {
        message = '配置保存失败：\n' + e.details.map(function (line) {
          return '· ' + line;
        }).join('\n');
      } else {
        message = '配置保存失败：' + runnerErrorText(e);
      }
      if (el.configError) el.configError.textContent = message;
      if (el.configStatus) {
        el.configStatus.textContent = '保存失败';
        el.configStatus.className = 'muted small err';
      }
    } finally {
      configBusy = false;
      if (el.configSave && configLoaded && configTask === task) {
        el.configSave.disabled = configParams.length === 0;
      }
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

  function renderQueueSchedulerControl() {
    const s = state.runnerScheduler;
    const text = s ? runnerSchedStatusText(s) : '—';
    const cls = text === '运行中' ? 'ok' : (text === '停止中' ? 'warn' : 'kind');
    setBadge(el.schedBadge, text, cls);
    if (!s) {
      if (el.schedMeta) el.schedMeta.textContent = '（调度线程状态未加载）';
      return;
    }
    const tail = Array.isArray(s.log_tail) ? s.log_tail : [];
    const lastLine = tail.length ? String(tail[tail.length - 1]) : '—';
    renderKv(el.schedMeta, [
      ['调度线程', runnerSchedThreadText(s)],
      ['锁 pid', runnerSchedLockPidText(s)],
      ['停止请求', s.stop_requested === true ? '已请求（跑完当前任务后停止）' : '无'],
      ['passes', s.passes === undefined || s.passes === null ? '—' : String(s.passes)],
      ['上次 pass', runnerTimeText(s.last_pass_ms)],
      ['日志尾部', lastLine],
    ].concat(runnerSchedHistoryRows(s)));
  }

  function setSchedBusy(busy) {
    schedBusy = !!busy;
    if (el.schedStart) el.schedStart.disabled = schedBusy || schedStopping;
    if (el.schedStop) el.schedStop.disabled = schedBusy;
    if (el.schedRefresh) el.schedRefresh.disabled = schedBusy;
  }

  // While a stop is pending (backend `timeout:true`, long attempt finishing),
  // keep "启动调度" disabled until the status poll observes running=false; the
  // stop button stays clickable (a repeated stop is a harmless no-op).
  function setSchedStopping(stopping) {
    schedStopping = !!stopping;
    if (el.schedStart) el.schedStart.disabled = schedBusy || schedStopping;
  }

  function setSchedJsonBusy(busy) {
    schedJsonBusy = !!busy;
    if (el.schedJsonSave) el.schedJsonSave.disabled = !!busy;
  }

  function setRunBusy(busy) {
    runBusy = !!busy;
    if (el.runSubmit) el.runSubmit.disabled = !!busy;
  }

  function renderRunTaskOptions() {
    if (!el.runTask) return;
    const body = state.tasks || {};
    const tasks = Array.isArray(body.tasks) ? body.tasks : [];
    const current = String(el.runTask.value || '');
    const names = [];
    clearNode(el.runTask);
    for (const task of tasks) {
      const t = task || {};
      const name = t.task === undefined || t.task === null ? '' : String(t.task).trim();
      if (!name) continue;
      names.push(name);
      const option = document.createElement('option');
      option.value = name;
      option.textContent = name;
      el.runTask.appendChild(option);
    }
    if (!names.length) {
      const option = document.createElement('option');
      option.value = '';
      option.textContent = '（没有可运行的任务）';
      el.runTask.appendChild(option);
      el.runTask.disabled = true;
      return;
    }
    el.runTask.disabled = false;
    el.runTask.value = names.indexOf(current) >= 0 ? current : names[0];
  }

  async function loadRunnerSchedulerStatus() {
    try {
      state.runnerScheduler = await runnerFetchJson(RUNNER_API.schedulerThreadStatus);
      renderQueueSchedulerControl();
      renderOverviewScheduler();
      return '';
    } catch (e) {
      state.runnerScheduler = null;
      renderQueueSchedulerControl();
      renderOverviewScheduler();
      const message = '调度线程状态读取失败：' + runnerErrorText(e);
      if (el.queueStatus) el.queueStatus.textContent = message;
      return message;
    }
  }

  async function loadRunnerSchedJson() {
    if (!el.schedJson) return '';
    try {
      const body = await runnerFetchJson(RUNNER_API.schedulerConfig);
      const cfg = body.config && typeof body.config === 'object' ? body.config : null;
      state.schedConfig = cfg;
      el.schedJson.value = cfg ? JSON.stringify(cfg, null, 2) : '';
      schedJsonDirty = false;
      if (el.schedJsonError) el.schedJsonError.textContent = '';
      if (el.schedJsonStatus) {
        el.schedJsonStatus.textContent = cfg
          ? '已从磁盘载入（' + runnerValueText(body.path) + '）'
          : '配置文件不存在：保存后会创建';
        el.schedJsonStatus.className = 'muted small';
      }
      return '';
    } catch (e) {
      state.schedConfig = null;
      el.schedJson.value = '';
      const message = 'scheduler.json 读取失败：' + runnerErrorText(e);
      if (el.schedJsonError) el.schedJsonError.textContent = message;
      if (el.schedJsonStatus) {
        el.schedJsonStatus.textContent = '读取失败';
        el.schedJsonStatus.className = 'muted small err';
      }
      return message;
    }
  }

  async function loadQueuePage() {
    const errors = [];
    try {
      state.scheduler = await runnerFetchJson(RUNNER_API.schedulerStatus);
      renderQueue();
    } catch (e) {
      state.scheduler = null;
      const message = '调度器状态读取失败：' + runnerErrorText(e);
      if (el.queueStatus) el.queueStatus.textContent = message;
      if (el.queueSummary) el.queueSummary.textContent = '—';
      if (el.queueEntries) el.queueEntries.textContent = '—';
      if (el.queueQueues) el.queueQueues.textContent = '—';
      errors.push(message);
    }
    const schedError = await loadRunnerSchedulerStatus();
    if (schedError) errors.push(schedError);
    // Always reload on queue refresh: install/uninstall/activate on the package
    // page can add/remove tasks, and a stale dropdown would 404 at run time.
    try {
      state.tasks = await runnerFetchJson(RUNNER_API.tasks);
    } catch (e) {
      errors.push('任务列表读取失败：' + runnerErrorText(e));
    }
    renderRunTaskOptions();
    if (!schedJsonDirty) {
      const jsonError = await loadRunnerSchedJson();
      if (jsonError) errors.push(jsonError);
    }
    return errors;
  }

  // Refresh every scheduling surface after start/stop/save: the header badge,
  // the overview card, GET /runner/scheduler/status and GET /scheduler/status.
  async function refreshSchedulerSurfaces() {
    await refreshHealth();
    await loadRunnerSchedulerStatus();
    await loadQueuePage();
  }

  async function startRunnerScheduler() {
    if (schedBusy) return;
    setSchedBusy(true);
    if (el.schedError) el.schedError.textContent = '';
    if (el.schedStatus) {
      el.schedStatus.textContent = '启动中…';
      el.schedStatus.className = 'muted small';
    }
    try {
      const body = await runnerFetchJson(RUNNER_API.schedulerStart, { method: 'POST' });
      const already = body.already_running === true;
      if (el.schedStatus) {
        el.schedStatus.textContent = already ? '调度本来就在运行' : '调度已启动';
        el.schedStatus.className = 'muted small ok';
      }
      setPageStatus(already ? '调度已在运行' : '调度已启动', 'ok');
      await refreshSchedulerSurfaces();
    } catch (e) {
      if (el.schedError) el.schedError.textContent = runnerErrorLines(e).join('\n');
      if (el.schedStatus) {
        let text = '启动失败';
        if (e && e.status === 409) {
          text = (e.code === 'device_busy' || /device_busy/.test(runnerErrorText(e)))
            ? '启动失败：设备被占用（Studio 试运行 / 临时运行中）'
            : '启动失败：调度锁被其他进程占用';
        }
        el.schedStatus.textContent = text;
        el.schedStatus.className = 'muted small err';
      }
      await loadRunnerSchedulerStatus();
    } finally {
      setSchedBusy(false);
    }
  }

  async function stopRunnerScheduler() {
    if (schedBusy) return;
    setSchedBusy(true);
    if (el.schedError) el.schedError.textContent = '';
    if (el.schedStatus) {
      el.schedStatus.textContent = '停止中…（当前任务跑完才退出）';
      el.schedStatus.className = 'muted small';
    }
    try {
      const body = await runnerFetchJson(RUNNER_API.schedulerStop, { method: 'POST' });
      const text = runnerStopText(body);
      const pending = runnerStopPending(body);
      if (el.schedStatus) {
        el.schedStatus.textContent = text;
        el.schedStatus.className = 'muted small ' + (pending ? 'warn' : 'ok');
      }
      setPageStatus(text, pending ? 'warn' : 'ok');
      setSchedStopping(pending);
      if (pending) startStopPolling();
      else stopStopPolling();
      await refreshSchedulerSurfaces();
    } catch (e) {
      stopStopPolling();
      setSchedStopping(false);
      if (el.schedError) el.schedError.textContent = runnerErrorLines(e).join('\n');
      if (el.schedStatus) {
        el.schedStatus.textContent = '停止失败';
        el.schedStatus.className = 'muted small err';
      }
      await loadRunnerSchedulerStatus();
    } finally {
      setSchedBusy(false);
    }
  }

  // While the backend answered `timeout:true` (a long attempt is finishing), the
  // UI polls `GET /runner/scheduler/status` until running=false (≤30 s) and only
  // then reports "调度已停止"; a timeout is never rendered as a failure.
  function stopStopPolling() {
    if (stopPollTimer !== null) {
      clearTimeout(stopPollTimer);
      stopPollTimer = null;
    }
  }

  function scheduleStopPoll() {
    stopStopPolling();
    stopPollTimer = setTimeout(pollStopOnce, RUNNER_STOP_POLL_MS);
  }

  async function pollStopOnce() {
    stopPollTimer = null;
    if (Date.now() - stopPollStartedMs > RUNNER_STOP_POLL_MAX_MS) {
      // Give the user the controls back after the budget: the scheduler may
      // still be finishing, but the UI must not stay locked forever.
      setSchedStopping(false);
      if (el.schedStatus) {
        el.schedStatus.textContent = '仍在停止：当前任务可能还在执行（可稍后刷新）';
        el.schedStatus.className = 'muted small warn';
      }
      return;
    }
    let status = null;
    try {
      status = await runnerFetchJson(RUNNER_API.schedulerThreadStatus);
    } catch (e) {
      scheduleStopPoll();
      return;
    }
    state.runnerScheduler = status;
    renderQueueSchedulerControl();
    renderOverviewScheduler();
    if (status && status.running === true) {
      scheduleStopPoll();
      return;
    }
    stopStopPolling();
    setSchedStopping(false);
    if (el.schedStatus) {
      el.schedStatus.textContent = '调度已停止';
      el.schedStatus.className = 'muted small ok';
    }
    setPageStatus('调度已停止', 'ok');
    await refreshSchedulerSurfaces();
  }

  function startStopPolling() {
    stopPollStartedMs = Date.now();
    scheduleStopPoll();
  }

  async function saveRunnerSchedJson() {
    if (schedJsonBusy) return;
    const parsed = runnerSchedJsonBody(el.schedJson ? el.schedJson.value : '');
    if (!parsed.ok) {
      if (el.schedJsonError) el.schedJsonError.textContent = '· ' + parsed.error;
      if (el.schedJsonStatus) {
        el.schedJsonStatus.textContent = '未保存：本地校验失败';
        el.schedJsonStatus.className = 'muted small err';
      }
      return;
    }
    setSchedJsonBusy(true);
    if (el.schedJsonError) el.schedJsonError.textContent = '';
    if (el.schedJsonStatus) {
      el.schedJsonStatus.textContent = '保存中…';
      el.schedJsonStatus.className = 'muted small';
    }
    try {
      await runnerFetchJson(RUNNER_API.schedulerConfig, { method: 'POST', body: parsed.body });
      schedJsonDirty = false;
      setPageStatus('已保存 scheduler.json', 'ok');
      await loadRunnerSchedJson();
      await loadQueuePage();
      // loadRunnerSchedJson() rewrites the status line, so the save result is
      // written last (it is the more important message for the user).
      if (el.schedJsonStatus) {
        el.schedJsonStatus.textContent = '已保存；启动/重启调度后生效';
        el.schedJsonStatus.className = 'muted small ok';
      }
    } catch (e) {
      if (el.schedJsonError) el.schedJsonError.textContent = runnerErrorLines(e).join('\n');
      if (el.schedJsonStatus) {
        el.schedJsonStatus.textContent = e && e.status === 409
          ? '保存失败：调度运行中（先停止调度）'
          : '保存失败';
        el.schedJsonStatus.className = 'muted small err';
      }
    } finally {
      setSchedJsonBusy(false);
    }
  }

  function renderRunnerRunStatus(status) {
    if (el.runStatus) {
      let cls = 'muted small';
      if (status && status.running === true) cls = 'muted small';
      else if (status && status.outcome !== undefined && status.outcome !== null &&
          status.outcome !== '') {
        cls = 'muted small ' + (runnerOutcomeClass(status.outcome) === 'ok' ? 'ok' : 'err');
      } else if (status && status.error) cls = 'muted small err';
      el.runStatus.textContent = runnerRunStatusText(status);
      el.runStatus.className = cls;
    }
    if (el.runLog) {
      const tail = status && Array.isArray(status.log_tail) ? status.log_tail : [];
      const lines = tail.slice(-RUNNER_RUN_LOG_TAIL).map(function (line) {
        return String(line === undefined || line === null ? '' : line);
      });
      el.runLog.textContent = lines.join('\n');
    }
    if (el.runError) {
      const err = status ? status.error : null;
      el.runError.textContent = err === undefined || err === null || err === ''
        ? ''
        : runnerErrorText(err);
    }
  }

  function stopRunnerRunPolling() {
    if (runPollTimer !== null) {
      clearTimeout(runPollTimer);
      runPollTimer = null;
    }
  }

  function scheduleRunnerRunPoll() {
    stopRunnerRunPolling();
    runPollTimer = setTimeout(pollRunnerRunOnce, RUNNER_RUN_POLL_MS);
  }

  async function pollRunnerRunOnce() {
    runPollTimer = null;
    if (Date.now() - runPollStartedMs > RUNNER_RUN_POLL_MAX_MS) {
      if (el.runStatus) {
        el.runStatus.textContent = '超时：' + Math.round(RUNNER_RUN_POLL_MAX_MS / 1000) +
          ' 秒内未收到结果（运行可能仍在继续，可稍后刷新本页）';
        el.runStatus.className = 'muted small warn';
      }
      setRunBusy(false);
      return;
    }
    let status = null;
    try {
      status = await runnerFetchJson(RUNNER_API.runStatus);
    } catch (e) {
      if (el.runError) el.runError.textContent = runnerErrorLines(e).join('\n');
      scheduleRunnerRunPoll();
      return;
    }
    renderRunnerRunStatus(status);
    if (status && status.running === true) {
      scheduleRunnerRunPoll();
      return;
    }
    stopRunnerRunPolling();
    setRunBusy(false);
    const text = runnerRunStatusText(status);
    setPageStatus('临时运行：' + text, text.indexOf('失败') >= 0 ? 'err' : 'ok');
    loadQueuePage();
  }

  async function submitRunnerRun() {
    if (runBusy) return;
    const parsed = runnerRunBody(
      el.runTask ? el.runTask.value : '',
      el.runParams ? el.runParams.value : ''
    );
    if (!parsed.ok) {
      if (el.runError) el.runError.textContent = '· ' + parsed.error;
      if (el.runStatus) {
        el.runStatus.textContent = '未运行：参数校验失败';
        el.runStatus.className = 'muted small err';
      }
      return;
    }
    setRunBusy(true);
    if (el.runError) el.runError.textContent = '';
    if (el.runLog) el.runLog.textContent = '';
    if (el.runStatus) {
      el.runStatus.textContent = '已提交，等待 Runner…';
      el.runStatus.className = 'muted small';
    }
    try {
      await runnerFetchJson(RUNNER_API.run, { method: 'POST', body: parsed.body });
      if (el.runStatus) {
        el.runStatus.textContent = '运行中：' + parsed.body.task;
        el.runStatus.className = 'muted small';
      }
      setPageStatus('已提交临时运行：' + parsed.body.task, 'ok');
      runPollStartedMs = Date.now();
      scheduleRunnerRunPoll();
    } catch (e) {
      if (el.runError) el.runError.textContent = runnerErrorLines(e).join('\n');
      if (el.runStatus) {
        const detail = runnerErrorText(e);
        const code = e && e.code ? String(e.code) : '';
        let text = '提交失败';
        if (e && e.status === 409) {
          if (code === 'device_busy' || /device_busy/.test(detail)) {
            text = '未运行：设备被占用（租约冲突）';
          } else if (code === 'run_busy' || /run_busy/.test(detail)) {
            text = '未运行：已有运行在进行';
          } else {
            text = '未运行：设备或调度锁被占用';
          }
        } else if (e && e.status === 404) {
          text = '未运行：任务不存在';
        }
        el.runStatus.textContent = text;
        el.runStatus.className = 'muted small err';
      }
      setRunBusy(false);
    }
  }

  // Queue page polling: refresh the scheduling status every 5s while the page is
  // visible; leaving the page stops the timer.
  function stopQueuePolling() {
    if (queuePollTimer !== null) {
      clearInterval(queuePollTimer);
      queuePollTimer = null;
    }
  }

  function startQueuePolling() {
    stopQueuePolling();
    if (!el.schedControl) return;
    queuePollTimer = setInterval(function () {
      if (state.busy) return; // a manual refresh is already in flight
      loadRunnerSchedulerStatus();
      runnerFetchJson(RUNNER_API.schedulerStatus).then(function (body) {
        state.scheduler = body;
        renderQueue();
      }).catch(function () {
        /* keep the last good render; the next tick retries */
      });
    }, RUNNER_SCHED_POLL_MS);
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
    if (page === 'queue') startQueuePolling();
    else stopQueuePolling();
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
      else if (state.page === 'packages') pageErrors = await loadPackagesPage();
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
    if (el.packagesRefresh) el.packagesRefresh.addEventListener('click', function () { refreshCurrentPage(); });
    if (el.installSubmit) el.installSubmit.addEventListener('click', function () { installRunnerPackage(); });
    if (el.queueRefresh) el.queueRefresh.addEventListener('click', function () { refreshCurrentPage(); });
    if (el.schedStart) el.schedStart.addEventListener('click', function () { startRunnerScheduler(); });
    if (el.schedStop) el.schedStop.addEventListener('click', function () { stopRunnerScheduler(); });
    if (el.schedRefresh) el.schedRefresh.addEventListener('click', function () { refreshCurrentPage(); });
    if (el.runSubmit) el.runSubmit.addEventListener('click', function () { submitRunnerRun(); });
    if (el.schedJsonSave) el.schedJsonSave.addEventListener('click', function () { saveRunnerSchedJson(); });
    if (el.schedJson) {
      el.schedJson.addEventListener('input', function () {
        schedJsonDirty = true;
        if (el.schedJsonStatus) {
          el.schedJsonStatus.textContent = '有未保存修改（用「保存 JSON」写入）';
          el.schedJsonStatus.className = 'muted small';
        }
      });
    }
    if (el.overviewGotoQueue) {
      el.overviewGotoQueue.addEventListener('click', function () {
        showPage('queue');
        refreshCurrentPage();
      });
    }
    if (el.runsRefresh) el.runsRefresh.addEventListener('click', function () { refreshCurrentPage(); });
    if (el.settingsRefresh) el.settingsRefresh.addEventListener('click', function () { refreshCurrentPage(); });
    if (el.configSave) el.configSave.addEventListener('click', function () { saveRunnerConfig(); });
    if (el.configClose) el.configClose.addEventListener('click', closeRunnerConfigDialog);
    if (el.configForm) {
      el.configForm.addEventListener('submit', function (ev) {
        ev.preventDefault();
        saveRunnerConfig();
      });
    }
    if (el.configDialog) {
      el.configDialog.addEventListener('close', function () {
        configTask = '';
        configLoaded = false;
        configBusy = false;
      });
    }
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
