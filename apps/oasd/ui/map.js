// seq138 V2-C: workbench map tab runtime bundle (served at GET /map.js).
// Canonical source is map_ui.js; kept in sync by ui_pure_test.
// seq138 V2-C：工作台「地图」tab（全局地图 V2：所在地 / 路标 / 路径）。
// 独立经典脚本（无构建）；DOM 在 index.html#panel-map；共享工作台当前帧。
// 设计：docs/MAP-V2-DESIGN.md §2/§3/§8；API 由 seq138 V2-B 提供（/map* v2）。
(function () {
  'use strict';

  // ---------------- 纯函数（ui_pure_test 直接执行 window.OAS_MAP） ------------
  function mapV2Empty() {
    return { schema_version: 2, fallback_task: '返回庭院', locations: {} };
  }

  function mapV2LocationNames(map) {
    const locations = map && map.locations && typeof map.locations === 'object' ? map.locations : {};
    return Object.keys(locations).sort((a, b) => a.localeCompare(b, 'zh'));
  }

  function mapV2LocationNameValid(name) {
    const s = String(name === undefined || name === null ? '' : name).trim();
    if (!s || s.length > 64) return false;
    return !/[\u0000-\u001f\u007f]/.test(s);
  }

  function mapV2FeatureSummary(feature) {
    const f = feature && typeof feature === 'object' ? feature : {};
    if (f.kind === 'ocr_text') return 'OCR「' + String(f.text || '') + '」' +
      (Array.isArray(f.roi) && f.roi.length === 4 ? ' · ROI[' + f.roi.join(',') + ']' : '');
    if (f.kind === 'asset') return '素材 ' + String(f.asset || '');
    return '未知特征(' + String(f.kind || '') + ')';
  }

  function mapV2LocationFeatureSummary(loc) {
    const l = loc && typeof loc === 'object' ? loc : {};
    const rec = l.recognize && typeof l.recognize === 'object' ? l.recognize : {};
    const fs = Array.isArray(rec.features) ? rec.features : [];
    return fs.map(mapV2FeatureSummary).join('；') + ' · min_hits=' + (Number(rec.min_hits) || 1);
  }

  function mapV2LandmarkPathSummary(lm) {
    const l = lm && typeof lm === 'object' ? lm : {};
    const path = l.path && typeof l.path === 'object' ? l.path : {};
    const action = String(path.action || 'click');
    const asset = String(path.asset || '');
    const wait = path.wait && typeof path.wait === 'object' ? Number(path.wait.settle_ms) : null;
    return '目标「' + String(l.name || '?') + '」· ' + action + ' 素材 ' + (asset || '（未选）') +
      (wait !== null ? ' · settle ' + wait + 'ms' : '');
  }

  function mapV2SearchLocations(map, query) {
    const q = String(query === undefined || query === null ? '' : query).trim().toLowerCase();
    const locations = map && map.locations && typeof map.locations === 'object' ? map.locations : {};
    return mapV2LocationNames(map).filter((name) => {
      if (!q) return true;
      const label = String((locations[name] && locations[name].label) || '').toLowerCase();
      return name.toLowerCase().indexOf(q) >= 0 || label.indexOf(q) >= 0;
    });
  }

  function mapV2FilterFrames(frames, query) {
    const q = String(query === undefined || query === null ? '' : query).trim().toLowerCase();
    const list = Array.isArray(frames) ? frames : [];
    if (!q) return list;
    return list.filter((f) => {
      const hay = ['#' + String(f.id || ''), String(f.name || ''), String(f.category || '')].join(' ').toLowerCase();
      return hay.indexOf(q) >= 0;
    });
  }

  function mapV2FilterAssets(assets, query) {
    const q = String(query === undefined || query === null ? '' : query).trim().toLowerCase();
    const list = Array.isArray(assets) ? assets : [];
    if (!q) return list;
    return list.filter((a) => {
      const hay = [String(a.id || ''), String(a.kind || ''), String(a.label || '')].join(' ').toLowerCase();
      return hay.indexOf(q) >= 0;
    });
  }

  function mapV2LandmarkTargets(map, selfName) {
    return mapV2LocationNames(map).filter((n) => n !== String(selfName || ''));
  }

  function mapV2CurrentFeatureAssets(loc) {
    const l = loc && typeof loc === 'object' ? loc : {};
    const rec = l.recognize && typeof l.recognize === 'object' ? l.recognize : {};
    const out = [];
    for (const f of Array.isArray(rec.features) ? rec.features : []) {
      if (f && f.kind === 'asset' && f.asset) out.push(String(f.asset));
    }
    return [...new Set(out)];
  }

  function mapV2LandmarkAssets(loc) {
    const l = loc && typeof loc === 'object' ? loc : {};
    const out = [];
    for (const key of ['return_landmarks', 'outbound_landmarks']) {
      for (const lm of Array.isArray(l[key]) ? l[key] : []) {
        if (lm && lm.path && lm.path.asset) out.push(String(lm.path.asset));
      }
    }
    return [...new Set(out)];
  }

  function mapV2ValidationLocal(map) {
    const m = map && typeof map === 'object' ? map : {};
    const errors = [];
    const warnings = [];
    if (Number(m.schema_version) !== 2) errors.push({ path: 'schema_version', message: 'schema_version 必须为 2' });
    if (!String(m.fallback_task === undefined || m.fallback_task === null ? '' : m.fallback_task).trim()) {
      errors.push({ path: 'fallback_task', message: 'fallback_task 不能为空' });
    }
    const locations = m.locations && typeof m.locations === 'object' && !Array.isArray(m.locations) ? m.locations : null;
    if (!locations) {
      errors.push({ path: 'locations', message: 'locations 必须是对象（所在地名称→定义）' });
      return { errors, warnings };
    }
    const names = Object.keys(locations);
    if (!names.length) warnings.push({ path: 'locations', message: '地图还没有任何所在地' });
    for (const name of names) {
      const loc = locations[name] || {};
      const p = 'locations[' + JSON.stringify(name) + ']';
      if (!mapV2LocationNameValid(name)) errors.push({ path: p, message: '非法所在地名称（非空、<=64、无控制字符）' });
      const rec = loc.recognize && typeof loc.recognize === 'object' ? loc.recognize : {};
      const feats = Array.isArray(rec.features) ? rec.features : [];
      if (!feats.length) errors.push({ path: p + '.recognize.features', message: '至少需要 1 个识别特征' });
      const min = Number(rec.min_hits !== undefined && rec.min_hits !== null ? rec.min_hits : 1);
      if (!Number.isInteger(min) || min < 1 || min > Math.max(1, feats.length)) {
        errors.push({ path: p + '.recognize.min_hits', message: 'min_hits 必须是 1..特征数 的整数' });
      }
      feats.forEach((f, i) => {
        const fp = p + '.recognize.features[' + i + ']';
        if (!f || typeof f !== 'object') errors.push({ path: fp, message: '特征必须是对象' });
        else if (f.kind === 'ocr_text' && !String(f.text || '').trim()) errors.push({ path: fp + '.text', message: 'OCR 特征文字不能为空' });
        else if (f.kind === 'asset' && !String(f.asset || '').trim()) errors.push({ path: fp + '.asset', message: '素材特征不能为空' });
        else if (f.kind !== 'ocr_text' && f.kind !== 'asset') errors.push({ path: fp + '.kind', message: '特征类型必须是 ocr_text/asset' });
      });
      for (const group of ['return_landmarks', 'outbound_landmarks']) {
        const list = Array.isArray(loc[group]) ? loc[group] : [];
        const seen = {};
        list.forEach((lm, li) => {
          const lp = p + '.' + group + '[' + li + ']';
          const target = String((lm && lm.name) || '').trim();
          if (!target) errors.push({ path: lp + '.name', message: '路标目标不能为空' });
          else if (target === name) errors.push({ path: lp + '.name', message: '路标不能指向自身所在地' });
          else if (!Object.prototype.hasOwnProperty.call(locations, target)) {
            errors.push({ path: lp + '.name', message: '路标目标所在地「' + target + '」不存在' });
          }
          if (target && seen[target]) warnings.push({ path: lp + '.name', message: '同一组内重复路标目标「' + target + '」' });
          seen[target] = true;
          const path = lm && lm.path && typeof lm.path === 'object' ? lm.path : {};
          if (String(path.action || 'click') !== 'click') errors.push({ path: lp + '.path.action', message: 'V2 只支持 action=click' });
          if (!String(path.asset || '').trim()) errors.push({ path: lp + '.path.asset', message: '路径素材不能为空' });
          if (path.wait && typeof path.wait === 'object') {
            const ranges = [['settle_ms', 0, 5000], ['stable_ms', 0, 2000], ['timeout_ms', 100, 15000], ['poll_ms', 50, 1000]];
            for (const [k, lo, hi] of ranges) {
              if (path.wait[k] === undefined || path.wait[k] === null) continue;
              const v = Number(path.wait[k]);
              if (!Number.isFinite(v) || v < lo || v > hi) errors.push({ path: lp + '.path.wait.' + k, message: k + ' 必须在 ' + lo + '..=' + hi });
            }
          }
        });
      }
    }
    return { errors, warnings };
  }

  function mapV2IssueText(issues) {
    const list = Array.isArray(issues) ? issues : [];
    return list.map((i) => String(i && i.path ? i.path : '(root)') + '：' + String(i && i.message ? i.message : i)).join('\n');
  }

  function mapV2NewLocation(name) {
    return {
      label: undefined,
      recognize: { min_hits: 1, features: [] },
      return_landmarks: [],
      outbound_landmarks: [],
    };
  }

  function mapV2RenameLocation(map, oldName, newName) {
    const m = map && typeof map === 'object' ? map : null;
    const from = String(oldName || '').trim();
    const to = String(newName || '').trim();
    if (!m || !m.locations) return { ok: false, error: '地图未加载' };
    if (!mapV2LocationNameValid(to)) return { ok: false, error: '所在地名称非法（非空、<=64、无控制字符）' };
    if (from === to) return { ok: true, map: m };
    if (Object.prototype.hasOwnProperty.call(m.locations, to)) return { ok: false, error: '所在地名称已存在：' + to };
    const loc = m.locations[from];
    if (!loc) return { ok: false, error: '所在地不存在：' + from };
    const next = {};
    for (const [name, value] of Object.entries(m.locations)) next[name === from ? to : name] = value;
    for (const value of Object.values(next)) {
      for (const group of ['return_landmarks', 'outbound_landmarks']) {
        for (const lm of Array.isArray(value[group]) ? value[group] : []) {
          if (lm && lm.name === from) lm.name = to;
        }
      }
    }
    m.locations = next;
    return { ok: true, map: m };
  }

  function mapV2RemoveLocation(map, name) {
    const m = map && typeof map === 'object' ? map : null;
    const n = String(name || '');
    if (!m || !m.locations || !m.locations[n]) return { ok: false, error: '所在地不存在：' + n };
    const refs = [];
    for (const [owner, loc] of Object.entries(m.locations)) {
      for (const group of ['return_landmarks', 'outbound_landmarks']) {
        for (const lm of Array.isArray(loc[group]) ? loc[group] : []) {
          if (lm && lm.name === n) refs.push(owner + '.' + group + '「' + n + '」');
        }
      }
    }
    if (refs.length) return { ok: false, error: '被路标引用，不能删除：' + refs.slice(0, 5).join('；') };
    delete m.locations[n];
    return { ok: true, map: m };
  }

  function mapV2PlanChain(plan) {
    const p = plan && typeof plan === 'object' ? plan : {};
    const route = Array.isArray(p.route) ? p.route : [];
    if (p.ok === false) return '路径预览失败：' + ((p.errors || []).join('；') || '未知错误');
    if (!route.length) return '路径：' + String(p.from || '?') + ' → ' + String(p.target || '?') + '（同层/直达，无路标）';
    const parts = [String(p.from || route[0].from || '?')];
    for (const hop of route) {
      parts.push('--[' + String(hop.action || hop.asset || '?') + (hop.is_return ? '/回程' : '') + ']--> ' + String(hop.to));
    }
    return '路标链：' + parts.join(' ');
  }

  function mapV2PlanSummary(plan) {
    const p = plan && typeof plan === 'object' ? plan : {};
    if (p.ok === false) return '路径预览失败：' + ((p.errors || []).join('；') || '未知错误');
    const route = Array.isArray(p.route) ? p.route : [];
    const lines = ['起点「' + String(p.from || '?') + '」 → 目标「' + String(p.target || '?') + '」 · hops=' + Number(p.hops || route.length)];
    lines.push(mapV2PlanChain(plan));
    route.forEach((hop, i) => {
      lines.push('  ' + (i + 1) + '. ' + String(hop.from) + ' → ' + String(hop.to) + ' · 点击素材 ' +
        String(hop.asset || hop.action || '') + (hop.is_return ? '（回程路标）' : '') +
        (hop.wait ? ' · settle=' + Number(hop.wait.settle_ms || 0) + 'ms' : ''));
    });
    return lines.join('\n');
  }

  function mapV2StateSummary(state) {
    const s = state && typeof state === 'object' ? state : {};
    if (s.ok === false) return '识别失败：' + ((s.errors || []).join('；') || s.error || '未知错误');
    const cur = s.current;
    if (!cur) return '未识别到当前所在地' + (s.error ? '：' + s.error : '') + '（帧来源：' + String(s.frame_source || '?') + '）';
    return '当前所在地：「' + String(cur.location || cur.node || '?') + '」' +
      (cur.name && cur.name !== cur.location ? '（' + cur.name + '）' : '') +
      ' · hits=' + Number(cur.hits || 0) + (cur.ambiguous ? ' · 多个所在地命中' : '') +
      ' · 帧来源：' + String(s.frame_source || '?');
  }

  function mapV2NavigateSummary(res) {
    const r = res && typeof res === 'object' ? res : {};
    if (r.ok === false) return '导航失败：' + ((r.errors || []).join('；') || r.error || '未知错误');
    if (r.real === true) {
      const steps = Array.isArray(r.steps) ? r.steps : [];
      const lines = ['真实导航：目标「' + String(r.target || '?') + '」 · exit=' + String(r.exit_code) +
        ' · seed=' + String(r.seed) + ' · clicks=' + (Array.isArray(r.clicks) ? r.clicks.length : 0)];
      for (const st of steps) lines.push('  #' + Number(st.seq) + ' ' + String(st.op || '') + ' · ' + String(st.result || ''));
      return lines.join('\n');
    }
    return mapV2PlanSummary(r);
  }

  function mapV2FallbackText(map) {
    const m = map && typeof map === 'object' ? map : {};
    const task = String(m.fallback_task || '返回庭院');
    return '失败兜底任务：' + task + (task === '返回庭院' ? '（默认：返回庭院）' : '（默认：返回庭院）');
  }

  // 当前帧展示 label：name/category 为空时给可读默认（仅展示，不持久化）。
  function mapV2FrameLabel(frame) {
    const f = frame && typeof frame === 'object' ? frame : {};
    const name = String(f.name === undefined || f.name === null ? '' : f.name).trim();
    const category = String(f.category === undefined || f.category === null ? '' : f.category).trim();
    const id = f.id === undefined || f.id === null ? '' : String(f.id);
    if (!name && !category) return '当前帧' + (id ? ' #' + id : '');
    return '当前帧 ' + (name || ('#' + id)) + (category ? ' · ' + category : '');
  }

  // seq139 #2：所在地展示名——label 为空时用当前帧 name · category（不持久化）。
  function mapV2LocationDisplayName(label, frame) {
    const lbl = String(label === undefined || label === null ? '' : label).trim();
    if (lbl) return lbl;
    const f = frame && typeof frame === 'object' ? frame : {};
    const name = String(f.name === undefined || f.name === null ? '' : f.name).trim();
    const category = String(f.category === undefined || f.category === null ? '' : f.category).trim();
    const id = f.id === undefined || f.id === null ? '' : String(f.id);
    if (name && category && name !== category) return name + ' · ' + category;
    if (name) return name;
    if (category) return category;
    return id ? '帧 #' + id : '（未命名帧）';
  }

  function mapV2HitSummary(probe, loc) {
    const p = probe && typeof probe === 'object' ? probe : {};
    const lines = Array.isArray(p.ocr_lines) ? p.ocr_lines : [];
    const hints = Array.isArray(p.assets) ? p.assets : [];
    const rec = loc && loc.recognize && typeof loc.recognize === 'object' ? loc.recognize : {};
    const feats = Array.isArray(rec.features) ? rec.features : [];
    let hits = 0;
    for (const f of feats) {
      if (!f) continue;
      if (f.kind === 'ocr_text') {
        if (lines.some((l) => String((l && l.text) || '').indexOf(String(f.text || '')) >= 0)) hits += 1;
      } else if (f.kind === 'asset') {
        if (hints.some((h) => h && String(h.asset) === String(f.asset) && h.pass !== false)) hits += 1;
      }
    }
    const min = Number(rec.min_hits) || 1;
    return '命中 ' + hits + '/' + feats.length + ' · min_hits=' + min +
      (hits >= min ? ' ✅ 满足' : ' ⚠ 未满足') +
      (p.scope ? ' · probe scope=' + p.scope : '');
  }

  const PURE = {
    mapV2Empty, mapV2LocationNames, mapV2LocationNameValid, mapV2FeatureSummary,
    mapV2LocationFeatureSummary, mapV2LandmarkPathSummary, mapV2SearchLocations,
    mapV2FilterFrames, mapV2FilterAssets, mapV2LandmarkTargets, mapV2CurrentFeatureAssets,
    mapV2LandmarkAssets, mapV2ValidationLocal, mapV2IssueText, mapV2NewLocation,
    mapV2RenameLocation, mapV2RemoveLocation, mapV2PlanChain, mapV2PlanSummary,
    mapV2StateSummary, mapV2NavigateSummary, mapV2FallbackText, mapV2HitSummary,
    mapV2FrameLabel,
    mapV2LocationDisplayName,
  };

  function boot() {
    const $ = (id) => document.getElementById(id);
    const state = {
      map: null, loaded: false, dirty: false, active: false, selected: '',
      assets: [], frameQuery: '', locationQuery: '', result: '', probe: null,
      busy: false, identifying: false,
    };
    const el = (id) => $(id);
    function setStatus(text, color) {
      const s = el('map-v2-status');
      if (s) { s.textContent = text || '—'; s.style.color = color || ''; }
    }
    function setResult(text) {
      state.result = String(text || '');
      const p = el('map-v2-result');
      if (p) p.textContent = state.result || '—';
    }
    async function api(url, opts) {
      const r = await fetch(url, opts);
      let body = null;
      try { body = await r.json(); } catch (e) { throw new Error('服务端响应不是 JSON（HTTP ' + r.status + '）'); }
      if (!r.ok || (body && body.ok === false)) {
        const msg = body && Array.isArray(body.errors) ? body.errors.join('；') : ((body && body.error) || ('HTTP ' + r.status));
        const err = new Error(msg);
        err.body = body;
        throw err;
      }
      return body;
    }
    function postJson(url, payload) {
      return api(url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
    }
    function currentMap() { return state.map || mapV2Empty(); }
    function locations() { return currentMap().locations || {}; }
    function selectedLocation() { return locations()[state.selected] || null; }
    function markDirty() {
      state.dirty = true;
      const d = el('map-v2-dirty');
      if (d) { d.textContent = '未保存修改'; d.style.color = '#e8c46a'; }
      setStatus('地图有未保存修改（保存后写 map.json + .bak）', '#e8c46a');
    }
    function clearDirty() {
      state.dirty = false;
      const d = el('map-v2-dirty');
      if (d) { d.textContent = '已保存'; d.style.color = ''; }
    }
    function currentFrameList() {
      return typeof framesCache !== 'undefined' && Array.isArray(framesCache) ? framesCache : [];
    }
    function currentFrameIdSafe() {
      return typeof currentFrameId !== 'undefined' && currentFrameId !== null ? Number(currentFrameId) : null;
    }
    function currentFrame() {
      const id = currentFrameIdSafe();
      return currentFrameList().find((f) => Number(f.id) === id) || null;
    }
    function selectWorkbenchFrame(id) {
      if (typeof selectFrame === 'function') {
        Promise.resolve(selectFrame(id)).catch((e) => setStatus('切换帧失败：' + (e && e.message ? e.message : e), '#ff8a80'));
      }
    }

    async function refreshAll() {
      setStatus('加载地图/素材/帧…');
      await Promise.all([loadAssets(), loadMap()]);
      renderAll();
      if (currentFrameIdSafe() !== null) previewHits().catch(() => {});
      setStatus(state.loaded
        ? '已加载 V2 地图：' + Object.keys(locations()).length + ' 个所在地'
        : '无地图：新建所在地并保存后生成 map.json（V2）', '');
    }

    async function loadAssets() {
      try {
        const s = await api('/task/assets?task=');
        const items = (Array.isArray(s.items) ? s.items : []).filter((a) => a && a.id)
          .map((a) => ({ id: String(a.id), kind: String(a.kind || 'template'), label: a.label || '' }));
        const collections = (Array.isArray(s.collections) ? s.collections : []).filter((c) => c && c.id)
          .map((c) => ({ id: String(c.id), kind: 'template_set', label: c.id }));
        const seen = {};
        state.assets = [];
        for (const a of items.concat(collections)) {
          if (seen[a.id]) continue;
          seen[a.id] = true;
          state.assets.push(a);
        }
      } catch (e) {
        state.assets = [];
        setStatus('全局素材读取失败：' + e.message, '#ff8a80');
      }
    }

    async function loadMap() {
      setStatus('加载地图…');
      try {
        const s = await api('/map');
        state.map = s.map || mapV2Empty();
        state.loaded = !!s.exists;
        clearDirty();
        if (!state.selected || !locations()[state.selected]) {
          state.selected = mapV2LocationNames(state.map)[0] || '';
        }
        const p = el('map-v2-fallback');
        if (p) p.value = String(currentMap().fallback_task || '返回庭院');
        if (s.validation) renderValidation(s.validation);
        renderAll();
        setStatus(state.loaded ? '已加载：' + Number(s.location_count || 0) + ' 所在地 / ' + Number(s.edge_count || 0) + ' 路标' : '无地图：新建所在地并保存', '');
      } catch (e) {
        state.map = mapV2Empty();
        state.loaded = false;
        setStatus('地图加载失败：' + e.message, '#ff8a80');
        renderAll();
      }
    }

    function renderAll() {
      const count = el('map-v2-location-count');
      if (count) count.textContent = '所在地 ' + Object.keys(locations()).length;
      renderLocationList();
      renderEditor();
      renderLandmarks();
      renderTargets();
      renderFramePane();
      drawPreview();
      const fb = el('map-v2-fallback');
      if (fb) fb.value = String(currentMap().fallback_task || '返回庭院');
    }

    function renderLocationList() {
      const host = el('map-v2-location-list');
      if (!host) return;
      host.textContent = '';
      const names = mapV2SearchLocations(state.map, state.locationQuery);
      if (!names.length) { host.textContent = '（无匹配所在地；点「新建所在地」）'; return; }
      for (const name of names) {
        const row = document.createElement('button');
        row.type = 'button';
        row.className = 'map-v2-location-row' + (name === state.selected ? ' selected' : '');
        const display = mapV2LocationDisplayName((locations()[name] || {}).label, currentFrame());
        row.textContent = name + (display && display !== name ? ' · ' + display : '');
        row.title = mapV2LocationFeatureSummary(locations()[name]);
        row.addEventListener('click', () => {
          state.selected = name;
          state.probe = null;
          renderAll();
          previewHits().catch(() => {});
        });
        host.appendChild(row);
      }
    }

    function renderEditor() {
      const host = el('map-v2-editor');
      if (!host) return;
      host.textContent = '';
      const loc = selectedLocation();
      if (!loc) { host.textContent = '（选择左侧所在地，或「新建所在地」）'; return; }
      const nameInput = document.createElement('input');
      nameInput.type = 'text'; nameInput.value = state.selected; nameInput.placeholder = '所在地名称（全局唯一，路标按此名引用）';
      const rename = document.createElement('button');
      rename.type = 'button'; rename.textContent = '保存名称/别名';
      const labelInput = document.createElement('input');
      const frameDefaultLabel = mapV2FrameLabel(currentFrame());
      labelInput.type = 'text'; labelInput.value = String(loc.label || '');
      labelInput.placeholder = '默认展示：' + frameDefaultLabel;
      const row = document.createElement('div');
      row.className = 'map-v2-field';
      row.append(document.createTextNode('名称'), nameInput, document.createTextNode('别名'), labelInput, rename);
      host.appendChild(row);
      // seq139 #2：label 为空时展示默认「当前帧 name · category」，仅展示不持久化。
      if (!String(loc.label || '').trim()) {
        const labelHint = document.createElement('div');
        labelHint.className = 'muted small';
        labelHint.textContent = 'label 为空时展示：' + frameDefaultLabel + '（不写入 map.json）';
        host.appendChild(labelHint);
      }
      rename.addEventListener('click', () => {
        const next = nameInput.value.trim();
        const res = mapV2RenameLocation(currentMap(), state.selected, next);
        if (!res.ok) { setStatus('改名失败：' + res.error, '#ff8a80'); return; }
        state.selected = next;
        loc.label = labelInput.value.trim() || undefined;
        if (!loc.label) delete loc.label;
        markDirty();
        renderAll();
        setStatus('所在地名称已更新（未保存）', '#7bd88f');
      });
      // 识别条件
      const rec = loc.recognize && typeof loc.recognize === 'object' ? loc.recognize : { min_hits: 1, features: [] };
      if (!Array.isArray(rec.features)) rec.features = [];
      loc.recognize = rec;
      const head = document.createElement('div');
      head.className = 'muted small';
      head.textContent = '判定条件（OCR 文字 / 素材特征）：' + mapV2LocationFeatureSummary(loc);
      host.appendChild(head);
      const featHost = document.createElement('div');
      host.appendChild(featHost);
      function renderFeatures() {
        featHost.textContent = '';
        rec.features.forEach((f, idx) => {
          const r = document.createElement('div');
          r.className = 'map-v2-feature-row';
          const kind = document.createElement('select');
          kind.appendChild(new Option('OCR 文字', 'ocr_text'));
          kind.appendChild(new Option('素材', 'asset'));
          kind.value = f.kind === 'asset' ? 'asset' : 'ocr_text';
          kind.addEventListener('change', () => {
            if (kind.value === 'asset') { delete f.text; delete f.roi; f.kind = 'asset'; f.asset = ''; }
            else { delete f.asset; f.kind = 'ocr_text'; f.text = ''; }
            markDirty(); renderFeatures();
          });
          r.appendChild(kind);
          if (f.kind === 'asset') {
            r.appendChild(assetPicker(f.asset, (id) => { f.asset = id; markDirty(); head.textContent = '判定条件：' + mapV2LocationFeatureSummary(loc); }));
          } else {
            const t = document.createElement('input');
            t.type = 'text'; t.value = String(f.text || ''); t.placeholder = '期望 OCR 文字';
            t.addEventListener('change', () => { f.text = t.value; markDirty(); head.textContent = '判定条件：' + mapV2LocationFeatureSummary(loc); });
            r.appendChild(t);
            const addOcr = document.createElement('button');
            addOcr.type = 'button'; addOcr.textContent = '从当前帧 OCR 取…';
            addOcr.addEventListener('click', () => pickOcrLine((line) => {
              f.text = line.text;
              if (Array.isArray(line.rect) && line.rect.length === 4) f.roi = line.rect.slice();
              markDirty(); renderFeatures();
              setStatus('已填入 OCR 特征「' + line.text + '」', '#7bd88f');
            }));
            r.appendChild(addOcr);
          }
          const del = document.createElement('button');
          del.type = 'button'; del.textContent = '删';
          del.addEventListener('click', () => { rec.features.splice(idx, 1); markDirty(); renderFeatures(); });
          r.appendChild(del);
          featHost.appendChild(r);
        });
        const addRow = document.createElement('div');
        addRow.className = 'map-v2-actions';
        const addOcr = document.createElement('button');
        addOcr.type = 'button'; addOcr.textContent = '+ OCR 文字特征';
        addOcr.addEventListener('click', () => { rec.features.push({ kind: 'ocr_text', text: '' }); renderFeatures(); markDirty(); });
        const addAsset = document.createElement('button');
        addAsset.type = 'button'; addAsset.textContent = '+ 素材特征';
        addAsset.addEventListener('click', () => { rec.features.push({ kind: 'asset', asset: '' }); renderFeatures(); markDirty(); });
        addRow.append(addOcr, addAsset);
        featHost.appendChild(addRow);
        const minRow = document.createElement('label');
        minRow.className = 'opt';
        minRow.textContent = 'min_hits ';
        const min = document.createElement('input');
        min.type = 'number'; min.min = '1'; min.value = String(Number(rec.min_hits) || 1);
        min.addEventListener('change', () => { rec.min_hits = Number(min.value) || 1; markDirty(); });
        minRow.appendChild(min);
        featHost.appendChild(minRow);
      }
      renderFeatures();
    }

    function assetPicker(current, onChoose) {
      // 复用现有素材搜索选择器（与构建器同一组件/样式）。
      if (window.OASAssetUI && window.OASAssetUI.createSearchPicker) {
        return window.OASAssetUI.createSearchPicker({
          items: state.assets,
          value: current || '',
          placeholder: '搜索素材/集合…',
          className: 'asset-search',
          label: (a) => a.id + ' [' + a.kind + ']',
          onChoose: (a, input) => { input.value = a.id; onChoose(a.id, input); },
        });
      }
      const input = document.createElement('input');
      input.type = 'text'; input.value = String(current || '');
      input.placeholder = '素材 id';
      input.addEventListener('change', () => onChoose(input.value.trim(), input));
      return input;
    }

    function renderLandmarks() {
      const host = el('map-v2-landmarks');
      if (!host) return;
      host.textContent = '';
      const loc = selectedLocation();
      if (!loc) { host.textContent = '（先选择所在地）'; return; }
      for (const group of ['return_landmarks', 'outbound_landmarks']) {
        if (!Array.isArray(loc[group])) loc[group] = [];
        const box = document.createElement('div');
        box.className = 'map-v2-landmark-group';
        const title = document.createElement('h4');
        title.textContent = group === 'return_landmarks' ? '回程路标（返回）' : '去程路标（去往）';
        box.appendChild(title);
        loc[group].forEach((lm, idx) => {
          const row = document.createElement('div');
          row.className = 'map-v2-landmark-row';
          const target = document.createElement('select');
          target.appendChild(new Option('（选择目标所在地）', ''));
          for (const n of mapV2LandmarkTargets(state.map, state.selected)) target.appendChild(new Option(n, n));
          if (lm.name && !mapV2LandmarkTargets(state.map, state.selected).includes(lm.name)) {
            target.appendChild(new Option('不存在：' + lm.name, lm.name));
          }
          target.value = String(lm.name || '');
          target.addEventListener('change', () => { lm.name = target.value; markDirty(); renderLandmarks(); });
          row.appendChild(target);
          const action = document.createElement('select');
          action.appendChild(new Option('点击（V1）', 'click'));
          action.value = String((lm.path && lm.path.action) || 'click');
          action.addEventListener('change', () => {
            if (!lm.path) lm.path = {};
            lm.path.action = action.value;
            markDirty();
          });
          row.appendChild(action);
          if (!lm.path || typeof lm.path !== 'object') lm.path = { action: 'click', asset: '' };
          const assetWrap = document.createElement('span');
          assetWrap.className = 'map-v2-landmark-asset';
          assetWrap.appendChild(assetPicker(lm.path.asset || '', (id) => {
            lm.path.asset = id;
            markDirty();
            renderLandmarks();
            previewHits().catch(() => {});
          }));
          row.appendChild(assetWrap);
          if (lm.path.asset) {
            const thumb = document.createElement('img');
            thumb.className = 'map-v2-asset-thumb';
            thumb.src = '/task/asset/image?task=&id=' + encodeURIComponent(lm.path.asset) + '&ts=' + Date.now();
            thumb.title = '路径素材预览：' + lm.path.asset;
            thumb.onerror = () => { thumb.remove(); };
            row.appendChild(thumb);
          }
          const wait = document.createElement('input');
          wait.type = 'number'; wait.min = '0'; wait.max = '5000';
          wait.value = String((lm.path.wait && lm.path.wait.settle_ms) || 600);
          wait.title = '点击后 settle_ms';
          wait.addEventListener('change', () => {
            lm.path.wait = { settle_ms: Math.max(0, Math.min(5000, Number(wait.value) || 0)), timeout_ms: 3000 };
            markDirty();
          });
          row.appendChild(wait);
          const del = document.createElement('button');
          del.type = 'button'; del.textContent = '删路标';
          del.addEventListener('click', () => { loc[group].splice(idx, 1); markDirty(); renderLandmarks(); });
          row.appendChild(del);
          box.appendChild(row);
          const summary = document.createElement('div');
          summary.className = 'muted small';
          summary.textContent = mapV2LandmarkPathSummary(lm);
          box.appendChild(summary);
        });
        const add = document.createElement('button');
        add.type = 'button'; add.textContent = '+ 新增' + (group === 'return_landmarks' ? '回程' : '去程') + '路标';
        add.addEventListener('click', () => {
          const targets = mapV2LandmarkTargets(state.map, state.selected);
          loc[group].push({ name: targets[0] || '', path: { action: 'click', asset: '', wait: { settle_ms: 600, timeout_ms: 3000 } } });
          markDirty();
          renderLandmarks();
        });
        box.appendChild(add);
        host.appendChild(box);
      }
    }

    function renderTargets() {
      const sel = el('map-v2-target');
      if (!sel) return;
      const keep = sel.value;
      sel.textContent = '';
      const frame = currentFrame();
      for (const n of mapV2LocationNames(state.map)) {
        const display = mapV2LocationDisplayName((locations()[n] || {}).label, frame);
        sel.appendChild(new Option(n + (display && display !== n ? ' · ' + display : ''), n));
      }
      if (keep && mapV2LocationNames(state.map).includes(keep)) sel.value = keep;
      else if (state.selected) sel.value = state.selected;
    }

    function frameImageUrl(fid) {
      return '/frames/image?id=' + encodeURIComponent(fid) + '&ts=' + Date.now();
    }

    function renderFramePane() {
      const info = el('map-v2-frame-info');
      const f = currentFrame();
      if (info) {
        info.textContent = f
          ? mapV2FrameLabel(f) + '（共享工作台）'
          : '帧：—（工作台暂无可用帧）';
      }
      const list = el('map-v2-frame-list');
      if (list) {
        list.textContent = '';
        // 复用工作台现有帧搜索逻辑（filterFramesByQuery），保证同一查询语义。
        const frames = (typeof filterFramesByQuery === 'function')
          ? filterFramesByQuery(currentFrameList(), state.frameQuery)
          : mapV2FilterFrames(currentFrameList(), state.frameQuery);
        if (!frames.length) list.textContent = '（无匹配帧）';
        for (const fr of frames.slice(0, 40)) {
          const row = document.createElement('button');
          row.type = 'button';
          row.className = 'map-v2-frame-row' + (Number(fr.id) === currentFrameIdSafe() ? ' selected' : '');
          row.textContent = '#' + fr.id + ' ' + (fr.name || '') + (fr.category ? ' · ' + fr.category : '');
          row.addEventListener('click', () => selectWorkbenchFrame(fr.id));
          list.appendChild(row);
        }
      }
    }

    // seq139 #1：切帧/轮询只刷新帧相关区域，绝不 renderAll（保留表单输入/OCR 结果）。
    function refreshFrameOnly() {
      state.probe = null;
      renderFramePane();
      drawPreview();
      const hit = el('map-v2-hit-summary');
      if (hit) hit.textContent = '帧已切换；点「识别当前所在地」刷新命中预览';
      previewHits().catch(() => {});
    }

    function drawPreview() {
      const canvas = el('map-v2-preview');
      if (!canvas) return;
      const f = currentFrame();
      const ctx = canvas.getContext('2d');
      if (!f) {
        canvas.width = 480; canvas.height = 270;
        ctx.fillStyle = '#0b0d10'; ctx.fillRect(0, 0, 480, 270);
        ctx.fillStyle = '#8fb8e8'; ctx.font = '13px Consolas, monospace';
        ctx.fillText('无当前帧（请在工作台选择图片）', 12, 24);
        return;
      }
      const img = new Image();
      img.onload = () => {
        const dpr = window.devicePixelRatio || 1;
        const cssW = Math.max(1, canvas.clientWidth || 480);
        const w = Math.round(cssW * dpr);
        const h = Math.round((w * img.height) / Math.max(1, img.width));
        canvas.width = w; canvas.height = h;
        ctx.setTransform(w / img.width, 0, 0, h / img.height, 0, 0);
        ctx.drawImage(img, 0, 0);
        const probe = state.probe;
        if (probe) {
          ctx.save(); ctx.lineWidth = 2; ctx.strokeStyle = '#7bd88f';
          for (const line of Array.isArray(probe.ocr_lines) ? probe.ocr_lines : []) {
            const r = line && Array.isArray(line.rect) ? line.rect : null;
            if (r && r.length >= 4) ctx.strokeRect(Number(r[0]) + 0.5, Number(r[1]) + 0.5, Number(r[2]), Number(r[3]));
          }
          for (const hint of Array.isArray(probe.assets) ? probe.assets : []) {
            const r = hint && Array.isArray(hint.real) ? hint.real : null;
            if (r && r.length >= 4 && hint.pass !== false) {
              ctx.strokeStyle = '#ffd54a';
              ctx.strokeRect(Number(r[0]) + 0.5, Number(r[1]) + 0.5, Number(r[2]), Number(r[3]));
            }
          }
          ctx.restore();
        }
      };
      img.onerror = () => {
        canvas.width = 480; canvas.height = 270;
        ctx.fillStyle = '#0b0d10'; ctx.fillRect(0, 0, 480, 270);
        ctx.fillStyle = '#ff8a80'; ctx.font = '13px Consolas, monospace';
        ctx.fillText('当前帧图片不可用', 12, 24);
      };
      img.src = frameImageUrl(f.id);
    }

    async function previewHits() {
      const loc = selectedLocation();
      const fid = currentFrameIdSafe();
      if (!loc || fid === null) {
        const elx = el('map-v2-hit-summary');
        if (elx) elx.textContent = loc ? '（无当前帧）' : '（先选择所在地）';
        return;
      }
      const assets = mapV2CurrentFeatureAssets(loc).concat(mapV2LandmarkAssets(loc));
      const box = el('map-v2-hit-summary');
      if (box) box.textContent = '读取命中预览…（probe 限定 ' + assets.length + ' 个素材）';
      try {
        const s = await postJson('/map/probe', { frame_id: Number(fid), assets: assets });
        state.probe = s;
        if (box) box.textContent = mapV2HitSummary(s, loc);
        drawPreview();
      } catch (e) {
        if (box) box.textContent = '命中预览失败：' + e.message;
      }
    }

    async function pickOcrLine(onPick) {
      const fid = currentFrameIdSafe();
      if (fid === null) { setStatus('请先在工作台选一张帧', '#e8c46a'); return; }
      setStatus('读取当前帧 OCR 行…');
      try {
        const s = await postJson('/map/probe', { frame_id: Number(fid), assets: [] });
        state.probe = s;
        const lines = Array.isArray(s.ocr_lines) ? s.ocr_lines : [];
        if (!lines.length) { setStatus('当前帧没有 OCR 行', '#e8c46a'); return; }
        const host = el('map-v2-editor');
        const list = document.createElement('div');
        list.className = 'map-v2-ocr-pick';
        lines.slice(0, 40).forEach((line) => {
          const b = document.createElement('button');
          b.type = 'button';
          b.textContent = String(line.text || '') + ' (' + Number(line.score || 0).toFixed(2) + ')';
          b.addEventListener('click', () => { list.remove(); onPick(line); });
          list.appendChild(b);
        });
        const cancel = document.createElement('button');
        cancel.type = 'button'; cancel.textContent = '取消';
        cancel.addEventListener('click', () => list.remove());
        list.appendChild(cancel);
        host.appendChild(list);
        drawPreview();
        setStatus('请选择 OCR 行', '#8fb8e8');
      } catch (e) {
        setStatus('读取 OCR 失败：' + e.message, '#ff8a80');
      }
    }

    async function identifyCurrent() {
      const fid = currentFrameIdSafe();
      setStatus('识别当前所在地…');
      setResult('识别中…');
      try {
        const qs = fid !== null ? '?frame_id=' + encodeURIComponent(fid) : '?live=1';
        const s = await api('/map/state' + qs);
        setResult(mapV2StateSummary(s));
        if (s.current && (s.current.location || s.current.node)) {
          state.selected = String(s.current.location || s.current.node);
          renderAll();
          previewHits().catch(() => {});
        }
      } catch (e) {
        setResult('识别失败：' + e.message);
      }
    }

    async function planRoute() {
      const target = el('map-v2-target') ? el('map-v2-target').value : '';
      if (!target) { setResult('请选择目标所在地'); return; }
      const fid = currentFrameIdSafe();
      const qs = new URLSearchParams({ from: 'auto', to: target });
      if (fid !== null) qs.set('frame_id', String(fid));
      setResult('路径预览中…');
      try {
        const s = await api('/map/plan?' + qs.toString());
        setResult(mapV2PlanSummary(s));
      } catch (e) {
        setResult('路径预览失败：' + e.message);
      }
    }

    async function navigateDebug(real) {
      const target = el('map-v2-target') ? el('map-v2-target').value : '';
      if (!target) { setResult('请选择目标所在地'); return; }
      if (real && !window.confirm('对真实设备执行导航到「' + target + '」（临时任务，不写用户任务）？')) return;
      const body = { target: target, real: !!real };
      if (currentFrameIdSafe() !== null) body.frame_id = Number(currentFrameIdSafe());
      setResult(real ? '真实导航执行中…' : 'dry 执行中…');
      try {
        const s = await postJson('/map/navigate', body);
        setResult(mapV2NavigateSummary(s));
      } catch (e) {
        setResult('导航失败（当前位置/路标/目标见服务端错误）：' + e.message);
      }
    }

    async function validateMap() {
      const local = mapV2ValidationLocal(currentMap());
      renderValidation(local);
      try {
        const s = await postJson('/map/validate', { map: currentMap() });
        renderValidation(s.validation || local);
        if (s.ok) setStatus('校验通过（服务端权威）', '#7bd88f');
        else setStatus('校验失败：见校验面板', '#ff8a80');
      } catch (e) {
        setStatus('校验请求失败：' + e.message, '#ff8a80');
      }
    }

    function renderValidation(v) {
      const host = el('map-v2-validation');
      if (!host) return;
      host.textContent = '';
      const errors = (v && v.errors) || [];
      const warnings = (v && v.warnings) || [];
      if (!errors.length && !warnings.length) { host.textContent = '✅ 校验通过'; return; }
      host.textContent = (errors.length ? '错误 ' + errors.length + '：\n' + mapV2IssueText(errors) : '') +
        (warnings.length ? (errors.length ? '\n' : '') + '警告 ' + warnings.length + '：\n' + mapV2IssueText(warnings) : '');
      host.style.whiteSpace = 'pre-wrap';
    }

    async function saveMap() {
      const local = mapV2ValidationLocal(currentMap());
      renderValidation(local);
      if (local.errors.length) {
        setStatus('本地校验失败，未保存（见左侧校验面板）', '#ff8a80');
        return;
      }
      setStatus('保存地图…');
      try {
        const s = await postJson('/map', { map: currentMap() });
        clearDirty();
        setStatus('已保存 map.json（V2）' + (s.backup ? ' · 备份 ' + s.backup : ''), '#7bd88f');
        if (s.validation) renderValidation(s.validation);
        await loadMap();
      } catch (e) {
        if (e.body && e.body.errors) renderValidation({ errors: e.body.errors, warnings: e.body.warnings || [] });
        setStatus('保存失败：' + e.message, '#ff8a80');
      }
    }

    function newLocation() {
      let base = '新所在地';
      let name = base;
      let i = 2;
      while (Object.prototype.hasOwnProperty.call(locations(), name)) { name = base + i; i += 1; }
      currentMap().locations[name] = mapV2NewLocation(name);
      state.selected = name;
      markDirty();
      renderAll();
      setStatus('已新建所在地「' + name + '」（未保存；先填判定条件与路标）', '#7bd88f');
    }

    function newLocationFromFrame() {
      const f = currentFrame();
      if (!f) { setStatus('工作台没有当前帧', '#e8c46a'); return; }
      let name = '帧' + f.id;
      let i = 2;
      while (Object.prototype.hasOwnProperty.call(locations(), name)) { name = '帧' + f.id + '-' + i; i += 1; }
      const loc = mapV2NewLocation(name);
      loc.recognize.features = [{ kind: 'ocr_text', text: '' }];
      currentMap().locations[name] = loc;
      state.selected = name;
      markDirty();
      renderAll();
      setStatus('已用当前帧 #' + f.id + ' 新建所在地「' + name + '」（未保存；可「从当前帧 OCR 取…」填判定条件）', '#7bd88f');
    }

    // seq139 #5：删除所在地放在左侧始终可见操作区；保留被引用阻止/确认/删除后选中逻辑。
    function deleteSelectedLocation() {
      if (!state.selected) { setStatus('请先选择所在地', '#e8c46a'); return; }
      if (!window.confirm('删除所在地「' + state.selected + '」？（未保存）')) return;
      const res = mapV2RemoveLocation(currentMap(), state.selected);
      if (!res.ok) { setStatus('删除失败：' + res.error, '#ff8a80'); return; }
      state.selected = mapV2LocationNames(state.map)[0] || '';
      markDirty();
      renderAll();
      setStatus('已删除所在地（未保存）', '#7bd88f');
    }

    function stepFrame(delta) {
      const frames = currentFrameList();
      if (!frames.length) return;
      const ids = frames.map((f) => Number(f.id));
      let idx = ids.indexOf(Number(currentFrameIdSafe()));
      if (idx < 0) idx = 0;
      idx = Math.max(0, Math.min(ids.length - 1, idx + delta));
      selectWorkbenchFrame(ids[idx]);
    }

    function bind() {
      const on = (id, fn) => { const b = el(id); if (b) b.addEventListener('click', fn); };
      on('map-v2-close', () => { if (window.__oasActivatePanel) window.__oasActivatePanel('panel-assets'); });
      on('map-v2-reload', () => { if (state.dirty && !window.confirm('放弃未保存修改并重载地图？')) return; loadMap(); });
      on('map-v2-validate', validateMap);
      on('map-v2-save', saveMap);
      on('map-v2-location-new', newLocation);
      on('map-v2-location-delete', deleteSelectedLocation);
      on('map-v2-use-frame', newLocationFromFrame);
      on('map-v2-frame-prev', () => stepFrame(-1));
      on('map-v2-frame-next', () => stepFrame(1));
      on('map-v2-identify', identifyCurrent);
      on('map-v2-plan', planRoute);
      on('map-v2-dry', () => navigateDebug(false));
      on('map-v2-real', () => navigateDebug(true));
      const search = el('map-v2-location-search');
      if (search) search.addEventListener('input', () => { state.locationQuery = search.value; renderLocationList(); });
      const frameSearch = el('map-v2-frame-search');
      if (frameSearch) frameSearch.addEventListener('input', () => { state.frameQuery = frameSearch.value; renderFramePane(); });
      const fallback = el('map-v2-fallback');
      if (fallback) fallback.addEventListener('change', () => {
        currentMap().fallback_task = fallback.value.trim() || '返回庭院';
        markDirty();
      });
      document.addEventListener('oas:frame-changed', () => {
        if (!state.active) return;
        // seq139 #1：只刷新帧信息/列表/canvas/命中，不重建所在地表单（保留输入与结果）。
        refreshFrameOnly();
      });
    }

    let bound = false;
    function bindOnce() {
      if (bound) return;
      bound = true;
      bind();
    }
    window.OAS_MAP_UI = {
      onActivate: async function () {
        state.active = true;
        bindOnce();
        await refreshAll();
      },
      onDeactivate: function () { state.active = false; },
      refresh: refreshAll,
      state: state,
      getMap: currentMap,
    };
  }

  window.OAS_MAP = PURE;
  if (typeof document !== 'undefined') {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', boot);
    else boot();
  }
})();
