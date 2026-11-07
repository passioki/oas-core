// seq136 D1-M3：Studio 全局地图页（map.json 编辑器）。
// 独立经典脚本（无构建步骤），依赖 asset_ui.js 的 createSearchPicker；DOM 在 index.html#page-map。
// 设计：_verify/seq136-map/DESIGN.md §6；服务端 /map* API 由 D1-M2 提供（Studio-only）。
(function () {
  'use strict';

  // ---------------- 纯函数（ui_pure_test 可直接抽取/执行） -------------------
  function mapIdValid(id) {
    const s = String(id === undefined || id === null ? '' : id).trim();
    if (!s || s.length > 64) return false;
    if (/[\u0000-\u001f\u007f]/.test(s)) return false;
    if (/[\/\\:*?"<>|]/.test(s)) return false;
    if (s.indexOf('..') >= 0) return false;
    return true;
  }

  function mapNodeName(node) {
    const n = node && typeof node === 'object' ? node : {};
    const name = String(n.name === undefined || n.name === null ? '' : n.name).trim();
    return name || String(n.id === undefined || n.id === null ? '' : n.id);
  }

  function mapEdgeActionLabel(action) {
    const a = action && typeof action === 'object' && !Array.isArray(action) ? action : {};
    const kind = String(a.kind === undefined || a.kind === null ? '' : a.kind).trim();
    if (kind === 'click_asset') {
      return '点击素材 ' + String(a.asset === undefined || a.asset === null ? '' : a.asset) +
        (a.hold_ms !== undefined && a.hold_ms !== null ? '（hold ' + Number(a.hold_ms) + 'ms）' : '');
    }
    if (kind === 'key') return '按键 ' + String(a.keycode === undefined || a.keycode === null ? '' : a.keycode);
    if (kind === 'swipe') {
      return '滑动 ' + JSON.stringify(a.from || []) + '→' + JSON.stringify(a.to || []) +
        (a.ms !== undefined && a.ms !== null ? ' ' + Number(a.ms) + 'ms' : '');
    }
    return kind || '（未设置动作）';
  }

  function mapEdgeLabel(edge) {
    const e = edge && typeof edge === 'object' ? edge : {};
    return String(e.from || '?') + ' → ' + String(e.to || '?') + ' · ' + mapEdgeActionLabel(e.action);
  }

  function mapFeatureSummary(feature) {
    const f = feature && typeof feature === 'object' && !Array.isArray(feature) ? feature : {};
    const kind = String(f.kind === undefined || f.kind === null ? '' : f.kind).trim();
    if (kind === 'ocr_text') {
      return 'OCR 文字「' + String(f.text === undefined || f.text === null ? '' : f.text) + '」' +
        (Array.isArray(f.roi) && f.roi.length === 4 ? ' · ROI[' + f.roi.join(',') + ']' : '');
    }
    if (kind === 'asset') return '素材 ' + String(f.asset === undefined || f.asset === null ? '' : f.asset);
    return kind || '（未知特征）';
  }

  function mapNodeFeatureSummary(node) {
    const n = node && typeof node === 'object' ? node : {};
    const rec = n.recognize && typeof n.recognize === 'object' ? n.recognize : {};
    const features = Array.isArray(rec.features) ? rec.features : [];
    if (!features.length) return '（无识别特征）';
    const minHits = Number(rec.min_hits);
    return features.map(mapFeatureSummary).join('；') +
      ' · min_hits=' + (Number.isFinite(minHits) ? minHits : 1);
  }

  // 节点树：parent 分组；环/孤儿单独列出（不丢节点）。
  function mapNodeTree(nodes) {
    const list = Array.isArray(nodes) ? nodes.filter((n) => n && typeof n === 'object') : [];
    const byId = {};
    for (const n of list) {
      const id = String(n.id === undefined || n.id === null ? '' : n.id);
      if (id && !byId[id]) byId[id] = n;
    }
    const children = {};
    const roots = [];
    const orphans = [];
    const errors = [];
    for (const n of list) {
      const id = String(n.id || '');
      const parent = n.parent === undefined || n.parent === null ? '' : String(n.parent).trim();
      if (!parent) {
        roots.push(n);
        continue;
      }
      if (!byId[parent]) {
        orphans.push({ node: n, reason: '父节点「' + parent + '」不存在' });
        continue;
      }
      (children[parent] = children[parent] || []).push(n);
    }
    for (const n of list) {
      const seen = {};
      let cur = n;
      let depth = 0;
      while (cur && cur.parent !== undefined && cur.parent !== null && String(cur.parent).trim()) {
        const pid = String(cur.parent).trim();
        if (seen[pid]) {
          errors.push('节点「' + String(n.id || '') + '」的 parent 链成环（' + pid + '）');
          break;
        }
        seen[pid] = true;
        depth += 1;
        if (depth > 16) {
          errors.push('节点「' + String(n.id || '') + '」parent 深度超过 16');
          break;
        }
        cur = byId[pid];
      }
    }
    return { roots: roots, children: children, orphans: orphans, errors: errors };
  }

  function mapNodeReturnStatus(map, node) {
    const m = map && typeof map === 'object' ? map : {};
    const n = node && typeof node === 'object' ? node : {};
    const id = String(n.id || '');
    const parent = n.parent === undefined || n.parent === null ? '' : String(n.parent).trim();
    if (!parent) return { status: 'root', text: '根节点（无返回边）' };
    const edges = Array.isArray(m.edges) ? m.edges : [];
    if (edges.some((e) => e && String(e.from) === id && String(e.to) === parent)) {
      return { status: 'explicit', text: '返回边：显式 ' + id + ' → ' + parent };
    }
    const asset = String((n.return_asset === undefined || n.return_asset === null ? '' : n.return_asset) ||
      (m.default_return_asset === undefined || m.default_return_asset === null ? '' : m.default_return_asset));
    if (asset) return { status: 'derived', text: '返回边：派生（return_asset=' + asset + '）' };
    return { status: 'none', text: '无返回素材：请设置 return_asset 或全局 default_return_asset' };
  }

  // 前端本地校验（服务端 /map/validate 是权威；这里即时反馈基本 schema/引用/范围）。
  function mapValidationLocal(map) {
    const m = map && typeof map === 'object' ? map : {};
    const errors = [];
    const warnings = [];
    const nodes = Array.isArray(m.nodes) ? m.nodes : [];
    const edges = Array.isArray(m.edges) ? m.edges : [];
    if (Number(m.schema_version) !== 1) errors.push({ path: 'schema_version', message: 'schema_version 必须为 1' });
    const hub = String(m.hub === undefined || m.hub === null ? '' : m.hub).trim();
    const ids = {};
    nodes.forEach((n, i) => {
      const path = 'nodes[' + i + ']';
      const id = String((n && n.id) === undefined || (n && n.id) === null ? '' : n.id).trim();
      if (!mapIdValid(id)) errors.push({ path: path + '.id', message: '节点 id 非法或为空：' + JSON.stringify(id) });
      else if (ids[id]) errors.push({ path: path + '.id', message: '节点 id 重复：' + id });
      else ids[id] = n;
      if (!n || typeof n !== 'object') {
        errors.push({ path: path, message: '节点必须是对象' });
        return;
      }
      const rec = n.recognize && typeof n.recognize === 'object' ? n.recognize : null;
      const features = rec && Array.isArray(rec.features) ? rec.features : [];
      if (!features.length) errors.push({ path: path + '.recognize.features', message: '识别特征至少需要 1 个' });
      const minHits = rec && rec.min_hits !== undefined && rec.min_hits !== null ? Number(rec.min_hits) : 1;
      if (!Number.isInteger(minHits) || minHits < 1 || minHits > Math.max(1, features.length)) {
        errors.push({ path: path + '.recognize.min_hits', message: 'min_hits 必须是 1..特征数 的整数' });
      }
      if (n.priority !== undefined && n.priority !== null && !Number.isInteger(Number(n.priority))) {
        warnings.push({ path: path + '.priority', message: 'priority 不是整数（服务端可能拒绝）' });
      }
      if (n.frames !== undefined && n.frames !== null && !Array.isArray(n.frames)) {
        errors.push({ path: path + '.frames', message: 'frames 必须是帧 id 数组' });
      }
    });
    if (!hub) errors.push({ path: 'hub', message: 'hub（兜底路由节点）必填' });
    else if (!ids[hub]) errors.push({ path: 'hub', message: 'hub 节点「' + hub + '」不存在' });
    nodes.forEach((n, i) => {
      if (!n) return;
      const parent = n.parent === undefined || n.parent === null ? '' : String(n.parent).trim();
      if (parent && !ids[parent]) {
        errors.push({ path: 'nodes[' + i + '].parent', message: '父节点「' + parent + '」不存在' });
      }
    });
    edges.forEach((e, i) => {
      const path = 'edges[' + i + ']';
      const from = String((e && e.from) === undefined || (e && e.from) === null ? '' : e.from);
      const to = String((e && e.to) === undefined || (e && e.to) === null ? '' : e.to);
      if (!ids[from]) errors.push({ path: path + '.from', message: 'from 节点「' + from + '」不存在' });
      if (!ids[to]) errors.push({ path: path + '.to', message: 'to 节点「' + to + '」不存在' });
      if (from && from === to) errors.push({ path: path, message: '不允许自环（from == to）' });
      const action = e && e.action && typeof e.action === 'object' ? e.action : {};
      const kind = String(action.kind === undefined || action.kind === null ? '' : action.kind).trim();
      if (kind === 'click_asset') {
        if (!String(action.asset === undefined || action.asset === null ? '' : action.asset).trim()) {
          errors.push({ path: path + '.action.asset', message: 'click_asset 需要素材 id' });
        }
      } else if (kind === 'key') {
        const code = Number(action.keycode);
        if (!Number.isInteger(code) || code < 1 || code > 300) {
          errors.push({ path: path + '.action.keycode', message: 'key 的 keycode 必须是 1..=300 的整数' });
        }
      } else if (kind === 'swipe') {
        const okBox = (v) => Array.isArray(v) && v.length === 2 && v.every((x) => Number.isFinite(Number(x)));
        if (!okBox(action.from) || !okBox(action.to)) {
          errors.push({ path: path + '.action', message: 'swipe 需要 from/to [x,y] 坐标' });
        }
        if (action.ms !== undefined && action.ms !== null && !(Number(action.ms) > 0)) {
          errors.push({ path: path + '.action.ms', message: 'swipe ms 必须大于 0' });
        }
      } else {
        errors.push({ path: path + '.action.kind', message: '未知边动作类型：' + (kind || '（空）') });
      }
      const wait = e && e.wait && typeof e.wait === 'object' ? e.wait : null;
      if (wait) {
        const ranges = [
          ['settle_ms', 0, 5000], ['stable_ms', 0, 2000], ['timeout_ms', 100, 15000], ['poll_ms', 50, 1000],
        ];
        for (const [key, lo, hi] of ranges) {
          if (wait[key] === undefined || wait[key] === null) continue;
          const v = Number(wait[key]);
          if (!Number.isFinite(v) || v < lo || v > hi) {
            errors.push({ path: path + '.wait.' + key, message: key + ' 必须在 ' + lo + '..=' + hi });
          }
        }
      }
    });
    return { errors: errors, warnings: warnings };
  }

  function mapIssueText(issues) {
    const list = Array.isArray(issues) ? issues : [];
    return list.map((i) => String(i && i.path ? i.path : '(root)') + '：' + String(i && i.message ? i.message : i)).join('\n');
  }

  function mapPlanSummary(plan) {
    const p = plan && typeof plan === 'object' ? plan : {};
    if (p.ok === false) return '路径预览失败：' + ((p.errors || []).join('；') || p.error || '未知错误');
    const route = Array.isArray(p.route) ? p.route : [];
    const lines = ['起点「' + String(p.from || '?') + '」 → 目标「' + String(p.target || '?') + '」 · hops=' +
      Number(p.hops || route.length) + (p.hub_used ? ' · 经 hub' : '')];
    route.forEach((hop, i) => {
      lines.push('  ' + (i + 1) + '. ' + String(hop.from) + ' → ' + String(hop.to) +
        ' · ' + String(hop.action || '') +
        (hop.derived ? '（派生返回边）' : '') +
        (hop.wait ? ' · wait settle=' + Number(hop.wait.settle_ms || 0) + 'ms' : ''));
    });
    return lines.join('\n');
  }

  function mapStateSummary(state) {
    const s = state && typeof state === 'object' ? state : {};
    if (s.ok === false) return '识别失败：' + ((s.errors || []).join('；') || s.error || '未知错误');
    const cur = s.current;
    if (!cur) return '未识别到当前节点' + (s.error ? '：' + s.error : '') + '（帧来源：' + String(s.frame_source || '?') + '）';
    return '当前节点：「' + String(cur.node || '?') + '」' + (cur.name ? '（' + cur.name + '）' : '') +
      ' · hits=' + Number(cur.hits || 0) + (cur.ambiguous ? ' · 命中多个节点（priority 裁决）' : '') +
      ' · 帧来源：' + String(s.frame_source || '?');
  }

  function mapNavigateSummary(res) {
    const r = res && typeof res === 'object' ? res : {};
    if (r.ok === false) return '导航失败：' + ((r.errors || []).join('；') || r.error || '未知错误');
    if (r.real === true) {
      const steps = Array.isArray(r.steps) ? r.steps : [];
      const lines = ['真实导航执行：target「' + String(r.target || '?') + '」 · exit=' + String(r.exit_code) +
        ' · seed=' + String(r.seed) + ' · clicks=' + (Array.isArray(r.clicks) ? r.clicks.length : 0)];
      for (const st of steps) {
        lines.push('  #' + Number(st.seq) + ' ' + String(st.op || '') + ' · ' + String(st.result || ''));
      }
      return lines.join('\n');
    }
    return mapPlanSummary(r);
  }

  function mapNewNodeDefaults(input) {
    const o = input && typeof input === 'object' ? input : {};
    const frameId = Number.isFinite(Number(o.frameId)) ? Number(o.frameId) : null;
    const lines = Array.isArray(o.ocrLines) ? o.ocrLines : [];
    let name = '';
    for (const line of lines) {
      const text = String((line && line.text) || '').trim();
      if (text) { name = text.slice(0, 32); break; }
    }
    if (!name) name = frameId !== null ? ('页面' + frameId) : '新节点';
    let id = name.replace(/[\/\\:*?"<>|]/g, '').trim().slice(0, 40);
    if (!mapIdValid(id)) id = 'node' + (frameId !== null ? frameId : Date.now());
    return {
      id: id,
      name: name,
      parent: null,
      return_asset: '',
      priority: 0,
      frames: frameId !== null ? [frameId] : [],
      recognize: {
        min_hits: 1,
        features: name ? [{ kind: 'ocr_text', text: name }] : [],
      },
    };
  }

  function mapNavigateFlowPreset(target) {
    return {
      schema_version: 1,
      name: '导航任务',
      steps: [
        { op: 'navigate', args: { target: String(target === undefined || target === null ? '' : target) } },
        { op: 'finish', args: { exit: 'done' } },
      ],
    };
  }

  function mapEdgeDefaults(kind) {
    const k = String(kind === undefined || kind === null ? 'click_asset' : kind);
    const action = k === 'key'
      ? { kind: 'key', keycode: 4 }
      : (k === 'swipe'
        ? { kind: 'swipe', from: [640, 600], to: [640, 300], ms: 300 }
        : { kind: 'click_asset', asset: '' });
    return { from: '', to: '', action: action, wait: { settle_ms: 500, timeout_ms: 3000 } };
  }

  // ---------------- UI 实现（浏览器；无 document 时不执行） -------------------
  const PURE = {
    mapIdValid, mapNodeName, mapEdgeActionLabel, mapEdgeLabel, mapFeatureSummary,
    mapNodeFeatureSummary, mapNodeTree, mapNodeReturnStatus, mapValidationLocal,
    mapIssueText, mapPlanSummary, mapStateSummary, mapNavigateSummary,
    mapNewNodeDefaults, mapNavigateFlowPreset, mapEdgeDefaults,
  };

  function boot() {
    const $ = (id) => document.getElementById(id);
    const state = {
      map: null,
      frames: [],
      assets: [],
      selectedNodeId: '',
      selectedEdgeIndex: -1,
      frameId: null,
      probe: null,
      loaded: false,
      busy: false,
      dirty: false,
    };
    function el(id) { return $(id); }
    function setStatus(text, color) {
      const s = el('map-status');
      if (s) { s.textContent = text || '—'; s.style.color = color || ''; }
    }
    function setResult(text) {
      const p = el('map-plan-result');
      if (p) p.textContent = text || '—';
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
      return api(url, {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload),
      });
    }
    function emptyMap() {
      return { schema_version: 1, hub: '', default_return_asset: '', nodes: [], edges: [] };
    }
    function currentMap() { return state.map || emptyMap(); }
    function selectedNode() {
      const nodes = currentMap().nodes || [];
      return nodes.find((n) => String(n.id) === state.selectedNodeId) || null;
    }
    function nodeIds() {
      return (currentMap().nodes || []).map((n) => String(n.id)).filter(Boolean);
    }
    function markDirty() { state.dirty = true; setStatus('地图有未保存修改', '#e8c46a'); }

    async function loadFrames() {
      try {
        const s = await api('/frames');
        state.frames = Array.isArray(s.frames) ? s.frames : [];
        const cur = state.frames.find((f) => f.current);
        if (cur) state.frameId = cur.id;
        else if (state.frames.length) state.frameId = state.frames[0].id;
        renderFrameInfo();
      } catch (e) {
        state.frames = [];
        setStatus('帧列表读取失败：' + e.message, '#ff8a80');
      }
    }
    async function loadAssets() {
      try {
        const s = await api('/library');
        state.assets = (Array.isArray(s.items) ? s.items : []).filter((a) => a && a.kind !== 'region');
      } catch (e) {
        state.assets = [];
      }
    }

    async function loadMap() {
      setStatus('加载地图…');
      try {
        const s = await api('/map');
        state.map = s.map || emptyMap();
        state.loaded = !!s.exists;
        state.dirty = false;
        state.selectedNodeId = '';
        state.selectedEdgeIndex = -1;
        const p = el('map-path');
        if (p) p.textContent = s.exists ? String(s.path || '') : '（尚未创建 map.json）';
        setStatus(s.exists
          ? '已加载：' + Number(s.node_count || 0) + ' 节点 / ' + Number(s.edge_count || 0) + ' 边'
          : '无地图：先用「新建节点/用当前帧新建节点」创建，保存后生成全局 map.json', '');
        renderAll();
        if (s.validation) renderValidation(s.validation);
      } catch (e) {
        state.map = emptyMap();
        state.loaded = false;
        setStatus('地图加载失败：' + e.message, '#ff8a80');
        renderAll();
      }
    }

    async function refreshAll() {
      await Promise.all([loadFrames(), loadAssets(), loadMap()]);
    }

    function renderAll() {
      renderTree();
      renderNodeEditor();
      renderHubEditor();
      renderEdgeEditor();
      renderEdgeList();
      renderPlanTargets();
      drawFrame();
    }

    function renderFrameInfo() {
      const info = el('map-frame-info');
      if (info) info.textContent = state.frameId === null ? '帧：—（无帧）'
        : '帧：#' + state.frameId + '（' + state.frames.length + ' 帧可选）';
      const sel = el('map-node-frame-select');
      if (sel) {
        sel.textContent = '';
        sel.appendChild(new Option('（选择节点帧用于预览）', ''));
        for (const f of state.frames) sel.appendChild(new Option('#' + f.id + ' ' + (f.name || ''), String(f.id)));
        const node = selectedNode();
        if (node && Array.isArray(node.frames) && node.frames.length) sel.value = String(node.frames[0]);
      }
    }

    function renderTree() {
      const host = el('map-node-tree');
      if (!host) return;
      host.textContent = '';
      const map = currentMap();
      const nodes = map.nodes || [];
      if (!nodes.length) {
        host.textContent = '（空地图：先新建节点）';
        return;
      }
      const tree = mapNodeTree(nodes);
      const renderNode = (node, depth) => {
        const row = document.createElement('div');
        row.className = 'map-node-row' + (String(node.id) === state.selectedNodeId ? ' selected' : '');
        row.style.paddingLeft = (6 + depth * 14) + 'px';
        row.textContent = (depth ? '└ ' : '') + mapNodeName(node) + '（' + String(node.id) + '）' +
          (map.hub === node.id ? ' · hub' : '');
        row.addEventListener('click', () => {
          state.selectedNodeId = String(node.id);
          if (Array.isArray(node.frames) && node.frames.length) state.frameId = Number(node.frames[0]);
          renderFrameInfo();
          renderAll();
          previewSelectedNode();
        });
        host.appendChild(row);
        const kids = tree.children[node.id] || [];
        for (const kid of kids) renderNode(kid, depth + 1);
      };
      for (const root of tree.roots) renderNode(root, 0);
      for (const orphan of tree.orphans) {
        const row = document.createElement('div');
        row.className = 'map-node-row orphan';
        row.textContent = String(orphan.node.id) + ' · ⚠ ' + orphan.reason;
        host.appendChild(row);
      }
      for (const err of tree.errors) {
        const row = document.createElement('div');
        row.className = 'map-node-row error';
        row.textContent = '⚠ ' + err;
        host.appendChild(row);
      }
    }

    function assetSelect(current, includeEmpty, placeholder) {
      const sel = document.createElement('select');
      sel.appendChild(new Option(placeholder || '（无）', ''));
      for (const a of state.assets) sel.appendChild(new Option(a.id + ' · ' + (a.kind || ''), String(a.id)));
      sel.value = String(current === undefined || current === null ? '' : current);
      if (sel.value !== '' && !state.assets.some((a) => String(a.id) === sel.value)) {
        sel.appendChild(new Option('不存在：' + sel.value, sel.value));
      }
      return sel;
    }

    function renderNodeEditor() {
      const host = el('map-node-editor');
      if (!host) return;
      host.textContent = '';
      const map = currentMap();
      const cur = state.selectedNodeId ? selectedNode() : null;
      const node = cur ? JSON.parse(JSON.stringify(cur)) : mapNewNodeDefaults({ frameId: state.frameId });
      const isNew = !cur;
      function field(labelText, input) {
        const lab = document.createElement('label');
        lab.className = 'opt';
        lab.textContent = labelText;
        const row = document.createElement('div');
        row.className = 'map-field-row';
        row.appendChild(input);
        lab.appendChild(row);
        host.appendChild(lab);
        return input;
      }
      const idInput = document.createElement('input');
      idInput.type = 'text'; idInput.value = String(node.id || ''); idInput.placeholder = '节点 id（全局唯一，navigate.target 引用）';
      idInput.disabled = !isNew;
      field('节点 id', idInput);
      const nameInput = document.createElement('input');
      nameInput.type = 'text'; nameInput.value = String(node.name || ''); nameInput.placeholder = '展示名（缺省=id）';
      field('名称', nameInput);
      const parentSel = document.createElement('select');
      parentSel.appendChild(new Option('（根节点）', ''));
      for (const id of nodeIds()) if (id !== String(node.id || '')) parentSel.appendChild(new Option(id, id));
      parentSel.value = node.parent === undefined || node.parent === null ? '' : String(node.parent);
      field('父节点 parent', parentSel);
      const returnSel = assetSelect(node.return_asset, true, '（缺省用 default_return_asset）');
      field('返回素材 return_asset', returnSel);
      const prioInput = document.createElement('input');
      prioInput.type = 'number'; prioInput.value = String(Number.isInteger(Number(node.priority)) ? Number(node.priority) : 0);
      field('优先级 priority', prioInput);
      const framesInput = document.createElement('input');
      framesInput.type = 'text';
      framesInput.value = Array.isArray(node.frames) ? node.frames.join(',') : '';
      framesInput.placeholder = '帧 id 列表，如 12,31（仅编辑器用）';
      field('帧 frames（逗号分隔）', framesInput);
      const useFrameBtn = document.createElement('button');
      useFrameBtn.type = 'button';
      useFrameBtn.textContent = '把当前帧加入 frames';
      useFrameBtn.addEventListener('click', () => {
        if (state.frameId === null) return;
        const ids = String(framesInput.value || '').split(',').map((s) => s.trim()).filter(Boolean);
        if (ids.indexOf(String(state.frameId)) < 0) ids.push(String(state.frameId));
        framesInput.value = ids.join(',');
      });
      host.appendChild(useFrameBtn);
      // 识别特征
      const recBox = document.createElement('div');
      recBox.className = 'map-recognize-box';
      const recHead = document.createElement('div');
      recHead.textContent = '识别特征（服务端识别；min_hits 控制命中数）';
      recHead.className = 'muted small';
      recBox.appendChild(recHead);
      const rec = node.recognize && typeof node.recognize === 'object' ? node.recognize : { min_hits: 1, features: [] };
      const features = Array.isArray(rec.features) ? rec.features : [];
      const featureHost = document.createElement('div');
      recBox.appendChild(featureHost);
      function renderFeatures() {
        featureHost.textContent = '';
        features.forEach((f, idx) => {
          const row = document.createElement('div');
          row.className = 'map-feature-row';
          const kind = document.createElement('select');
          kind.appendChild(new Option('OCR 文字', 'ocr_text'));
          kind.appendChild(new Option('素材', 'asset'));
          kind.value = f.kind === 'asset' ? 'asset' : 'ocr_text';
          kind.addEventListener('change', () => { f.kind = kind.value; if (f.kind === 'asset') { delete f.text; f.asset = ''; } else { delete f.asset; f.text = ''; } renderFeatures(); });
          row.appendChild(kind);
          if (f.kind === 'asset') {
            const aSel = assetSelect(f.asset, true, '（选择素材）');
            aSel.addEventListener('change', () => { f.asset = aSel.value; });
            row.appendChild(aSel);
          } else {
            const text = document.createElement('input');
            text.type = 'text'; text.value = String(f.text || ''); text.placeholder = '期望 OCR 文字（严格/归一化匹配由服务端）';
            text.addEventListener('change', () => { f.text = text.value; });
            row.appendChild(text);
          }
          const del = document.createElement('button');
          del.type = 'button'; del.textContent = '删'; del.title = '删除特征';
          del.addEventListener('click', () => { features.splice(idx, 1); renderFeatures(); });
          row.appendChild(del);
          featureHost.appendChild(row);
        });
        const addRow = document.createElement('div');
        addRow.className = 'map-feature-add';
        const addOcr = document.createElement('button');
        addOcr.type = 'button'; addOcr.textContent = '+ OCR 文字特征';
        addOcr.addEventListener('click', () => { features.push({ kind: 'ocr_text', text: '' }); renderFeatures(); });
        const addAsset = document.createElement('button');
        addAsset.type = 'button'; addAsset.textContent = '+ 素材特征';
        addAsset.addEventListener('click', () => { features.push({ kind: 'asset', asset: '' }); renderFeatures(); });
        const addFromProbe = document.createElement('button');
        addFromProbe.type = 'button'; addFromProbe.textContent = '从当前帧 OCR 添加…';
        addFromProbe.addEventListener('click', () => pickFeatureFromProbe(features, renderFeatures));
        addRow.append(addOcr, addAsset, addFromProbe);
        featureHost.appendChild(addRow);
      }
      renderFeatures();
      const minHits = document.createElement('input');
      minHits.type = 'number'; minHits.min = '1';
      minHits.value = String(Number.isFinite(Number(rec.min_hits)) ? Number(rec.min_hits) : 1);
      minHits.addEventListener('change', () => { rec.min_hits = Number(minHits.value) || 1; });
      const minLab = document.createElement('label');
      minLab.className = 'opt';
      minLab.textContent = 'min_hits';
      minLab.appendChild(minHits);
      recBox.appendChild(minLab);
      host.appendChild(recBox);

      const saveBtn = document.createElement('button');
      saveBtn.type = 'button';
      saveBtn.textContent = isNew ? '添加节点到地图（未保存）' : '应用节点修改（未保存）';
      saveBtn.addEventListener('click', () => {
        const id = String(idInput.value || '').trim();
        if (!mapIdValid(id)) { setStatus('节点 id 非法：' + id, '#ff8a80'); return; }
        const next = {
          id: id,
          name: String(nameInput.value || '').trim(),
          parent: parentSel.value ? parentSel.value : null,
          return_asset: returnSel.value || undefined,
          priority: Number(prioInput.value) || 0,
          frames: String(framesInput.value || '').split(',').map((s) => Number(s.trim())).filter((n) => Number.isFinite(n)),
          recognize: { min_hits: Number(minHits.value) || 1, features: features.slice() },
        };
        if (!next.name) delete next.name;
        if (!next.return_asset) delete next.return_asset;
        const nodes = map.nodes;
        if (isNew) {
          if (nodes.some((n) => String(n.id) === id)) { setStatus('节点 id 已存在：' + id, '#ff8a80'); return; }
          nodes.push(next);
          state.selectedNodeId = id;
        } else {
          const idx = nodes.findIndex((n) => String(n.id) === state.selectedNodeId);
          if (idx >= 0) nodes[idx] = next;
          state.selectedNodeId = id;
        }
        if (!map.hub && nodes.length === 1) map.hub = next.id;
        markDirty();
        renderAll();
      });
      host.appendChild(saveBtn);
      const ret = mapNodeReturnStatus(map, node);
      const retLine = document.createElement('div');
      retLine.className = 'muted small' + (ret.status === 'none' ? ' warn' : '');
      retLine.textContent = ret.text;
      host.appendChild(retLine);
    }

    function pickFeatureFromProbe(features, renderFeatures) {
      setStatus('读取当前帧 OCR 行…');
      const body = state.frameId !== null ? { frame_id: Number(state.frameId) } : { live: true };
      postJson('/map/probe', body).then((s) => {
        state.probe = s;
        const lines = Array.isArray(s.ocr_lines) ? s.ocr_lines : [];
        if (!lines.length) { setStatus('当前帧没有 OCR 行', '#e8c46a'); return; }
        const chosen = window.prompt('选择要添加的 OCR 文字（输入序号，0=取消）：\n' +
          lines.slice(0, 30).map((l, i) => (i + 1) + '. ' + l.text + ' (' + Number(l.score || 0).toFixed(2) + ')').join('\n'), '1');
        const idx = Number(chosen);
        if (!Number.isInteger(idx) || idx < 1 || idx > lines.length) return;
        const line = lines[idx - 1];
        features.push({ kind: 'ocr_text', text: line.text, roi: line.rect });
        renderFeatures();
        setStatus('已添加 OCR 特征「' + line.text + '」', '#7bd88f');
      }).catch((e) => setStatus('取特征失败：' + e.message, '#ff8a80'));
    }

    function renderHubEditor() {
      const host = el('map-hub-editor');
      if (!host) return;
      host.textContent = '';
      const map = currentMap();
      const hubSel = document.createElement('select');
      hubSel.appendChild(new Option('（选择 hub）', ''));
      for (const id of nodeIds()) hubSel.appendChild(new Option(id, id));
      hubSel.value = String(map.hub || '');
      hubSel.addEventListener('change', () => { map.hub = hubSel.value; markDirty(); renderAll(); });
      const hubLab = document.createElement('label');
      hubLab.className = 'opt'; hubLab.textContent = 'hub（兜底路由节点）'; hubLab.appendChild(hubSel);
      host.appendChild(hubLab);
      const retSel = assetSelect(map.default_return_asset, true, '（无默认返回素材）');
      retSel.addEventListener('change', () => { map.default_return_asset = retSel.value; markDirty(); });
      const retLab = document.createElement('label');
      retLab.className = 'opt'; retLab.textContent = 'default_return_asset'; retLab.appendChild(retSel);
      host.appendChild(retLab);
    }

    function renderEdgeEditor() {
      const host = el('map-edge-editor');
      if (!host) return;
      host.textContent = '';
      const map = currentMap();
      const ids = nodeIds();
      if (ids.length < 1) { host.textContent = '（先添加节点）'; return; }
      const fromSel = document.createElement('select');
      const toSel = document.createElement('select');
      for (const id of ids) { fromSel.appendChild(new Option(id, id)); toSel.appendChild(new Option(id, id)); }
      const kindSel = document.createElement('select');
      for (const k of ['click_asset', 'key', 'swipe']) kindSel.appendChild(new Option(k, k));
      const assetSel = assetSelect('', true, '（click_asset 素材）');
      const keyInput = document.createElement('input');
      keyInput.type = 'number'; keyInput.value = '4'; keyInput.placeholder = 'keycode（BACK=4）';
      const fromXY = document.createElement('input');
      fromXY.type = 'text'; fromXY.value = '640,600'; fromXY.placeholder = 'swipe from x,y';
      const toXY = document.createElement('input');
      toXY.type = 'text'; toXY.value = '640,300'; toXY.placeholder = 'swipe to x,y';
      const msInput = document.createElement('input');
      msInput.type = 'number'; msInput.value = '300'; msInput.placeholder = 'swipe ms';
      const settleInput = document.createElement('input');
      settleInput.type = 'number'; settleInput.value = '500'; settleInput.placeholder = 'wait settle_ms';
      const row1 = document.createElement('div');
      row1.className = 'map-edge-row';
      row1.append(document.createTextNode('from'), fromSel, document.createTextNode('to'), toSel, kindSel);
      host.appendChild(row1);
      const row2 = document.createElement('div');
      row2.className = 'map-edge-row';
      row2.append(assetSel, keyInput, fromXY, toXY, msInput);
      host.appendChild(row2);
      const row3 = document.createElement('div');
      row3.className = 'map-edge-row';
      row3.append(document.createTextNode('wait settle_ms'), settleInput);
      host.appendChild(row3);
      const addBtn = document.createElement('button');
      addBtn.type = 'button'; addBtn.textContent = '添加边（未保存）';
      addBtn.addEventListener('click', () => {
        const kind = kindSel.value;
        const action = kind === 'click_asset'
          ? { kind: 'click_asset', asset: assetSel.value }
          : (kind === 'key'
            ? { kind: 'key', keycode: Number(keyInput.value) || 4 }
            : {
              kind: 'swipe',
              from: String(fromXY.value || '').split(',').map((s) => Number(s.trim())),
              to: String(toXY.value || '').split(',').map((s) => Number(s.trim())),
              ms: Number(msInput.value) || 300,
            });
        if (!assetSel.value && kind === 'click_asset') { setStatus('请选择 click_asset 素材', '#ff8a80'); return; }
        map.edges.push({
          from: fromSel.value, to: toSel.value, action: action,
          wait: { settle_ms: Number(settleInput.value) || 500, timeout_ms: 3000 },
        });
        markDirty();
        renderEdgeList();
        renderAll();
      });
      host.appendChild(addBtn);
    }

    function renderEdgeList() {
      const host = el('map-edge-list');
      if (!host) return;
      host.textContent = '';
      const edges = currentMap().edges || [];
      if (!edges.length) { host.textContent = '（无）'; return; }
      edges.forEach((edge, i) => {
        const row = document.createElement('div');
        row.className = 'map-node-row';
        row.textContent = mapEdgeLabel(edge);
        const del = document.createElement('button');
        del.type = 'button'; del.textContent = '删';
        del.addEventListener('click', () => { edges.splice(i, 1); markDirty(); renderEdgeList(); renderAll(); });
        row.appendChild(del);
        host.appendChild(row);
      });
    }

    function renderPlanTargets() {
      const ids = nodeIds();
      for (const id of ['map-plan-target', 'map-plan-from']) {
        const sel = el(id);
        if (!sel) continue;
        const keep = sel.value;
        sel.textContent = '';
        if (id === 'map-plan-from') sel.appendChild(new Option('自动识别当前节点', 'auto'));
        for (const nid of ids) sel.appendChild(new Option(nid, nid));
        if (keep && (id === 'map-plan-from' || ids.indexOf(keep) >= 0)) sel.value = keep;
      }
    }

    function renderValidation(v) {
      const host = el('map-validation');
      if (!host) return;
      host.textContent = '';
      const errors = (v && v.errors) || [];
      const warnings = (v && v.warnings) || [];
      if (!errors.length && !warnings.length) { host.textContent = '✅ 校验通过'; return; }
      const pre = document.createElement('div');
      pre.textContent = (errors.length ? '错误 ' + errors.length + '：\n' + mapIssueText(errors) : '') +
        (warnings.length ? (errors.length ? '\n' : '') + '警告 ' + warnings.length + '：\n' + mapIssueText(warnings) : '');
      pre.style.whiteSpace = 'pre-wrap';
      host.appendChild(pre);
    }

    function currentFrameImageUrl() {
      if (state.frameId === null) return '/frame.png?ts=' + Date.now();
      return '/frames/image?id=' + encodeURIComponent(state.frameId) + '&ts=' + Date.now();
    }

    function countNodeHits(node, probe) {
      const n = node && typeof node === 'object' ? node : {};
      const rec = n.recognize && typeof n.recognize === 'object' ? n.recognize : {};
      const features = Array.isArray(rec.features) ? rec.features : [];
      const lines = probe && Array.isArray(probe.ocr_lines) ? probe.ocr_lines : [];
      const assetHints = probe && Array.isArray(probe.assets) ? probe.assets : [];
      let hits = 0;
      for (const f of features) {
        if (!f || typeof f !== 'object') continue;
        if (f.kind === 'ocr_text') {
          const want = String(f.text || '').trim();
          if (want && lines.some((l) => String((l && l.text) || '').indexOf(want) >= 0)) hits += 1;
        } else if (f.kind === 'asset') {
          const aid = String(f.asset || '').trim();
          if (aid && assetHints.some((h) => h && String(h.asset) === aid && h.pass !== false)) hits += 1;
        }
      }
      const minHits = Math.max(1, Number(rec.min_hits) || 1);
      return { hits: hits, total: features.length, minHits: minHits, satisfied: hits >= minHits };
    }

    function drawProbeOverlay(ctx) {
      const probe = state.probe;
      if (!ctx || !probe) return;
      const lines = Array.isArray(probe.ocr_lines) ? probe.ocr_lines : [];
      ctx.save();
      ctx.lineWidth = 1.5;
      ctx.strokeStyle = '#7bd88f';
      for (const line of lines) {
        const r = line && Array.isArray(line.rect) ? line.rect : null;
        if (!r || r.length < 4) continue;
        ctx.strokeRect(Number(r[0]) + 0.5, Number(r[1]) + 0.5, Number(r[2]), Number(r[3]));
      }
      ctx.restore();
      const node = selectedNode();
      if (node) {
        const c = countNodeHits(node, probe);
        const hitsEl = el('map-feature-hits');
        if (hitsEl) {
          hitsEl.textContent = '特征命中 ' + c.hits + '/' + c.total + ' · min_hits=' + c.minHits +
            (c.satisfied ? ' ✅' : ' ⚠ 未满足');
          hitsEl.style.color = c.satisfied ? '#7bd88f' : '#e8c46a';
        }
        const detail = el('map-feature-detail');
        if (detail) {
          detail.textContent = '节点 ' + String(node.id) + ' 识别特征：\n' +
            ((node.recognize && node.recognize.features) || []).map((f, i) => (i + 1) + '. ' + mapFeatureSummary(f) +
              (probe ? ' · ' + (countNodeHits({ recognize: { min_hits: 1, features: [f] } }, probe).hits ? '命中' : '未命中') : '')).join('\n');
        }
      }
    }

    function drawFrame() {
      const canvas = el('map-canvas');
      if (!canvas) return;
      const ctx = canvas.getContext('2d');
      const img = new Image();
      img.onload = () => {
        canvas.width = img.width;
        canvas.height = img.height;
        ctx.drawImage(img, 0, 0);
        drawProbeOverlay(ctx);
        const hits = el('map-feature-hits');
        if (hits && !state.probe) hits.textContent = '—';
      };
      img.onerror = () => {
        ctx.fillStyle = '#0b0d10';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        ctx.fillStyle = '#8fb8e8';
        ctx.font = '14px Consolas, monospace';
        ctx.fillText('帧图片不可用（无设备/无帧）', 12, 24);
      };
      img.src = currentFrameImageUrl();
    }

    // 选中节点：按节点帧取 OCR/素材特征命中（/map/probe），并展示是否满足 min_hits。
    async function previewSelectedNode() {
      const node = selectedNode();
      if (!node) return;
      const body = state.frameId !== null ? { frame_id: Number(state.frameId) } : { live: true };
      try {
        const s = await postJson('/map/probe', body);
        state.probe = s;
        drawFrame();
      } catch (e) {
        setStatus('特征预览失败：' + e.message, '#e8c46a');
      }
    }

    async function validateMap() {
      const local = mapValidationLocal(currentMap());
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

    async function saveMap() {
      const local = mapValidationLocal(currentMap());
      renderValidation(local);
      if (local.errors.length) {
        setStatus('本地校验失败，未保存（见校验面板）', '#ff8a80');
        return;
      }
      setStatus('保存地图…');
      try {
        const s = await postJson('/map', { map: currentMap() });
        state.dirty = false;
        setStatus('已保存 map.json' + (s.backup ? '（备份 ' + s.backup + '）' : ''), '#7bd88f');
        renderValidation(s.validation || { errors: [], warnings: [] });
        await loadMap();
      } catch (e) {
        const body = e.body;
        if (body && body.validation) renderValidation(body.validation);
        else renderValidation({ errors: [{ path: '(save)', message: e.message }], warnings: [] });
        setStatus('保存失败：' + e.message, '#ff8a80');
      }
    }

    async function identifyState() {
      setStatus('识别当前节点…');
      setResult('识别中…');
      const qs = state.frameId !== null ? '?frame_id=' + encodeURIComponent(state.frameId) : '?live=1';
      try {
        const s = await api('/map/state' + qs);
        setResult(mapStateSummary(s));
        const lines = ['识别当前节点：'];
        lines.push(mapStateSummary(s));
        setResult(lines.join('\n'));
      } catch (e) {
        setResult('识别失败：' + e.message);
      }
    }

    async function planRoute() {
      const target = el('map-plan-target') ? el('map-plan-target').value : '';
      if (!target) { setResult('请选择目标节点'); return; }
      const from = el('map-plan-from') ? el('map-plan-from').value : 'auto';
      const qs = new URLSearchParams({ from: from || 'auto', to: target });
      if (from !== 'auto' && state.frameId === null) qs.set('live', '1');
      setResult('路径预览中…');
      try {
        const s = await api('/map/plan?' + qs.toString());
        setResult(mapPlanSummary(s));
      } catch (e) {
        setResult('路径预览失败：' + e.message);
      }
    }

    async function navigateDebug(real) {
      const target = el('map-plan-target') ? el('map-plan-target').value : '';
      if (!target) { setResult('请选择目标节点'); return; }
      if (real) {
        if (!window.confirm('即将对真实设备执行导航到「' + target + '」（服务端临时任务，不写入用户任务）。\n确认执行？')) return;
      }
      setResult(real ? '真实导航执行中…' : 'dry 预览中…');
      const body = { target: target, real: !!real };
      const from = el('map-plan-from') ? el('map-plan-from').value : 'auto';
      if (from && from !== 'auto') body.from = from;
      if (state.frameId !== null) body.frame_id = Number(state.frameId);
      try {
        const s = await postJson('/map/navigate', body);
        setResult(mapNavigateSummary(s));
      } catch (e) {
        setResult('导航失败：' + e.message);
      }
    }

    async function createNavigateTask() {
      const target = el('map-plan-target') ? el('map-plan-target').value : '';
      if (!target) { setResult('请先选择目标节点'); return; }
      const name = (window.prompt('新导航任务名称（目录名，可用中文）', '导航-' + target) || '').trim();
      if (!name) return;
      try {
        await postJson('/task/create', { name: name, template: 'utility', description: '全局地图导航任务（navigate）' });
        await postJson('/task/save_flow', { task: name, flow: mapNavigateFlowPreset(target) });
        setStatus('已创建导航任务「' + name + '」（flow=navigate+finish）', '#7bd88f');
        setResult('已创建导航任务「' + name + '」。可到「任务构建」继续编辑/运行。');
        const btn = document.querySelector('.app-nav-btn[data-page="builder"]');
        if (btn) btn.click();
      } catch (e) {
        setStatus('创建导航任务失败：' + e.message, '#ff8a80');
      }
    }

    function deleteSelectedNode() {
      const node = selectedNode();
      if (!node) { setStatus('请先选择节点', '#e8c46a'); return; }
      const map = currentMap();
      const id = String(node.id);
      const refs = [];
      for (const e of map.edges || []) {
        if (String(e.from) === id || String(e.to) === id) refs.push('边 ' + String(e.from) + '→' + String(e.to));
      }
      if (map.hub === id) refs.push('hub');
      if (refs.length) {
        setStatus('无法删除：节点被引用（' + refs.slice(0, 5).join('；') + '）', '#ff8a80');
        return;
      }
      if (!window.confirm('删除节点「' + id + '」（未保存）？')) return;
      map.nodes = (map.nodes || []).filter((n) => String(n.id) !== id);
      for (const n of map.nodes || []) if (String(n.parent || '') === id) n.parent = null;
      state.selectedNodeId = '';
      markDirty();
      renderAll();
    }

    function bind() {
      const on = (id, fn) => { const b = el(id); if (b) b.addEventListener('click', fn); };
      on('map-refresh', loadMap);
      on('map-validate', validateMap);
      on('map-save', saveMap);
      on('map-node-new', () => {
        state.selectedNodeId = '';
        renderAll();
      });
      on('map-node-from-frame', () => {
        setStatus('用当前帧新建节点…');
        const body = state.frameId !== null ? { frame_id: Number(state.frameId) } : { live: true };
        postJson('/map/probe', body).then((s) => {
          state.probe = s;
          const def = mapNewNodeDefaults({ frameId: state.frameId, ocrLines: s.ocr_lines });
          const map = currentMap();
          map.nodes.push(def);
          if (!map.hub) map.hub = def.id;
          state.selectedNodeId = def.id;
          markDirty();
          renderAll();
          setStatus('已用当前帧创建节点「' + def.id + '」（未保存）', '#7bd88f');
        }).catch((e) => setStatus('从当前帧建节点失败：' + e.message, '#ff8a80'));
      });
      on('map-node-delete', deleteSelectedNode);
      on('map-state-run', identifyState);
      on('map-plan-run', planRoute);
      on('map-nav-dry', () => navigateDebug(false));
      on('map-nav-real', () => navigateDebug(true));
      on('map-new-task', createNavigateTask);
      const frameSel = el('map-node-frame-select');
      if (frameSel) frameSel.addEventListener('change', () => {
        if (frameSel.value) { state.frameId = Number(frameSel.value); drawFrame(); }
      });
      on('map-frame-prev', () => stepFrame(-1));
      on('map-frame-next', () => stepFrame(1));
    }

    function stepFrame(delta) {
      if (!state.frames.length) return;
      const ids = state.frames.map((f) => Number(f.id));
      let idx = ids.indexOf(Number(state.frameId));
      if (idx < 0) idx = 0;
      idx = Math.max(0, Math.min(ids.length - 1, idx + delta));
      state.frameId = ids[idx];
      renderFrameInfo();
      drawFrame();
    }

    async function onActivate() {
      bindOnce();
      await refreshAll();
    }
    let bound = false;
    function bindOnce() {
      if (bound) return;
      bound = true;
      bind();
    }
    // 首次载入页面时也准备 DOM（不自动请求，等切到地图页再加载）。
    state.prepare = function () { bindOnce(); };
    window.OAS_MAP_UI = {
      onActivate: onActivate,
      refresh: refreshAll,
      getMap: currentMap,
      state: state,
    };
    bindOnce();
  }

  window.OAS_MAP = PURE;
  if (typeof document !== 'undefined') {
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', boot);
    } else {
      boot();
    }
  }
})();
