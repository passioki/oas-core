// Shared asset card renderer for the editor.
//
// Why this file exists: the workbench "task referenced assets", the workbench
// "asset library", and the builder "available assets" used to render the SAME
// asset with three different DOM structures. One asset must look and behave the
// same everywhere; only the surrounding toolbar/filter differs.
(function () {
  const KIND_LABELS = {
    template: '模板图（template）',
    ocr_line: '单行文字（ocr_line）',
    ocr_lines: '多行文字（ocr_lines）',
    ocr_var: '使用端OCR文字配置项（ocr_var）',
    region: '操作区域（region）',
    template_set: '素材集合（template_set）',
  };

  function kindLabel(kind) {
    return KIND_LABELS[kind] || kind;
  }

  // #6（验收反馈）：素材已存识别多边形在缩略图上的轮廓绘制。
  // 素材 roi 是帧坐标裁剪框；缩略图把 roi 等比缩放放进 64x44（与 canvas 分支的
  // drawImage 布局一致），polygon 顶点按同一映射投影到缩略图坐标。
  function thumbPolygonPoints(polygon, roi, width, height) {
    if (!Array.isArray(polygon) || polygon.length < 3) return null;
    if (!Array.isArray(roi) || roi.length !== 4) return null;
    const cw = Number(width) > 0 ? Number(width) : 64;
    const ch = Number(height) > 0 ? Number(height) : 44;
    const rw = Math.max(1, Number(roi[2]));
    const rh = Math.max(1, Number(roi[3]));
    const scale = Math.min(cw / rw, ch / rh);
    const dw = Math.max(1, rw * scale);
    const dh = Math.max(1, rh * scale);
    const ox = (cw - dw) / 2;
    const oy = (ch - dh) / 2;
    const points = [];
    for (const p of polygon) {
      if (!Array.isArray(p) || p.length < 2) continue;
      const px = Number(p[0]);
      const py = Number(p[1]);
      if (!Number.isFinite(px) || !Number.isFinite(py)) continue;
      points.push([ox + (px - Number(roi[0])) * scale, oy + (py - Number(roi[1])) * scale]);
    }
    return points.length >= 3 ? points : null;
  }

  // 在缩略图 canvas 上描出已存识别多边形（蓝色虚线细线，不喧宾夺主）。
  function drawThumbPolygon(ctx, points) {
    if (!ctx || !Array.isArray(points) || points.length < 3) return;
    ctx.save();
    ctx.beginPath();
    ctx.moveTo(points[0][0], points[0][1]);
    for (let i = 1; i < points.length; i += 1) ctx.lineTo(points[i][0], points[i][1]);
    ctx.closePath();
    ctx.setLineDash([3, 2]);
    ctx.strokeStyle = '#4d9fff';
    ctx.lineWidth = 1;
    ctx.stroke();
    ctx.fillStyle = 'rgba(77,159,255,0.10)';
    ctx.fill();
    ctx.restore();
  }

  function makeThumb(it, task) {
    const box = document.createElement('span');
    box.className = 'asset-thumb';
    if (it.kind === 'template' && it.file) {
      const img = document.createElement('img');
      img.loading = 'lazy';
      img.alt = it.id;
      img.src = `/task/asset/image?task=${encodeURIComponent(task || '')}&id=${encodeURIComponent(it.id)}`;
      img.addEventListener('error', () => {
        box.textContent = 'ROI';
        box.classList.add('asset-thumb-empty');
      });
      box.appendChild(img);
      // #6：模板缩略图叠加已存识别多边形轮廓（ROI 裁剪图上的等比投影）。
      const polyPoints = thumbPolygonPoints(it.polygon, it.roi, 64, 44);
      if (polyPoints) {
        const overlay = document.createElement('canvas');
        overlay.className = 'asset-thumb-poly';
        overlay.width = 64;
        overlay.height = 44;
        const octx = overlay.getContext('2d');
        drawThumbPolygon(octx, polyPoints);
        box.appendChild(overlay);
      }
      return box;
    }
    if (it.roi) {
      // OCR/region assets have no pixels of their own: their thumbnail MUST be
      // one of their declared position frames. Never fall back to the current
      // canvas frame, otherwise switching frames changes the thumbnail and the
      // asset looks like it belongs to the wrong interface.
      const positions = it.positions || [];
      if (!positions.length) {
        box.textContent = '未设位置帧';
        box.classList.add('asset-thumb-empty');
        return box;
      }
      const img = new Image();
      img.onload = () => {
        const c = document.createElement('canvas');
        c.width = 64;
        c.height = 44;
        const ctx = c.getContext('2d');
        ctx.fillStyle = '#0b0d10';
        ctx.fillRect(0, 0, c.width, c.height);
        const rw = Math.max(1, it.roi[2]);
        const rh = Math.max(1, it.roi[3]);
        const scale = Math.min(c.width / rw, c.height / rh);
        const dw = Math.max(1, rw * scale);
        const dh = Math.max(1, rh * scale);
        ctx.drawImage(
          img,
          it.roi[0], it.roi[1], rw, rh,
          (c.width - dw) / 2, (c.height - dh) / 2, dw, dh
        );
        // #6：识别多边形轮廓叠加（与矩形 ROI 同图；蓝色虚线区分）。
        drawThumbPolygon(ctx, thumbPolygonPoints(it.polygon, it.roi, c.width, c.height));
        box.textContent = '';
        box.appendChild(c);
      };
      img.onerror = () => {
        box.textContent = '位置帧失效';
        box.classList.add('asset-thumb-empty');
      };
      img.src = `/frames/image?id=${positions[0]}&ts=${Date.now()}`;
      box.textContent = '位置帧…';
      box.classList.add('asset-thumb-empty');
      return box;
    }
    box.textContent = '—';
    box.classList.add('asset-thumb-empty');
    return box;
  }

  function makeButton(text, title, className) {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = className || 'asset-test';
    b.textContent = text;
    b.title = title || '';
    return b;
  }

  // --- A: selected asset row + the single open settings menu ---------------
  // Selection is keyed by ASSET ID, never by DOM node: the same asset must light
  // up on every surface (task library, global library, frame detail), and a list
  // re-render (save/refresh/delete) must restore the highlight from this state.
  let selectedAssetKey = '';
  // At most ONE settings menu is open at a time, across all surfaces. Selecting
  // a DIFFERENT asset closes it; clicking an item inside the current asset's
  // menu must NOT close it (the settings panel stays open until 设置/Esc).
  let openMenu = null;
  // P2-3: remembers which asset's menu was open so a list re-render (save,
  // rename, threshold, positions, delete) can reopen it on the fresh row
  // instead of silently collapsing the panel.
  let openMenuKey = '';
  let menuRestoreTimer = 0;
  // Every rendered row, so a programmatic selection can restyle rows that are
  // already on screen. Detached rows are pruned lazily via `isConnected`.
  const liveRows = [];
  const SELECTED_CLASS = 'selected';

  function assetKey(id) {
    return id == null ? '' : String(id);
  }

  function syncSelectedRows() {
    for (let i = liveRows.length - 1; i >= 0; i--) {
      const entry = liveRows[i];
      const row = entry.row;
      if (!row) {
        liveRows.splice(i, 1);
        continue;
      }
      // `isConnected` is undefined in the Node DOM stubs, so only a real browser
      // detach prunes the registry.
      if (row.isConnected === false) {
        liveRows.splice(i, 1);
        continue;
      }
      if (row.classList) row.classList.toggle(SELECTED_CLASS, entry.key === selectedAssetKey);
    }
  }

  /** Mark `id` as the selected asset. Returns the stored key ('' = none). */
  function selectAsset(id) {
    const key = assetKey(id);
    if (key !== selectedAssetKey) {
      // Only choosing ANOTHER asset collapses the previous settings panel.
      selectedAssetKey = key;
      closeMenus();
    }
    syncSelectedRows();
    return selectedAssetKey;
  }

  function getSelectedAsset() {
    return selectedAssetKey;
  }

  /** Collapse the (single) open settings menu. Safe to call at any time. */
  function closeMenus() {
    const open = openMenu;
    openMenu = null;
    openMenuKey = '';
    if (!open) return;
    if (open.menu && open.menu.classList) open.menu.classList.remove('open');
    if (open.settings && open.settings.classList) {
      open.settings.classList.remove('open');
      if (open.settings.setAttribute) open.settings.setAttribute('aria-expanded', 'false');
    }
  }

  if (typeof document !== 'undefined' && document && typeof document.addEventListener === 'function') {
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') closeMenus();
    });
  }

  // Every asset row has exactly TWO action buttons: 识别 (the existing onTest
  // behaviour) and 设置 (this menu). The ten entries and their order are a UX
  // contract asserted by ui_pure_test.js; append new entries at the end only.
  // `manage` marks an item as unavailable on read-only surfaces (frame detail).
  const ASSET_MENU_ITEMS = [
    {
      key: 'clickTest', label: '点测', action: 'onClickTest', compactHide: true,
      title: '点击测试（dry-run：验证定位并显示目标点）',
    },
    {
      key: 'dragTest', label: '拖测', action: 'onDragTest', compactHide: true,
      title: '拖动测试（dry-run：生成方向/起终点）',
    },
    {
      key: 'roi', label: '识别区', action: 'onEditRoi', manage: true,
      title: '在画布上统一框选：拖动=矩形，单击=多边形顶点，双击收尾；可载入已存识别多边形继续编辑',
    },
    {
      key: 'action', label: '操作区', action: 'onOpRegion', manage: true,
      title: '设置该素材的操作区（当前素材操作区可直接在画布上画矩形/多边形/反选；或引用素材作识别门槛·链只读）',
    },
    { key: 'rename', label: '重命名', action: 'onRename', manage: true, title: '只重命名素材 id（不改阈值/识别区）' },
    { key: 'threshold', label: '阈值', action: 'onThreshold', manage: true, title: '模板阈值 0~1（template）' },
    { key: 'position', label: '位置帧', action: 'onPosition', manage: true, title: '设置素材位置帧' },
    { key: 'delete', label: '删除', action: 'onDelete', manage: true, title: '删除素材（flow 仍引用时会被拒绝）' },
    {
      key: 'step', label: '步骤', action: 'onStep', compactHide: true,
      title: '生成 click_target 步骤并复制到剪贴板',
    },
    // U-T4（2026-11-06 事项 2）：使用端OCR文字配置项（ocr_var）的默认识别文字。
    // 只能追加在末尾（菜单顺序是 ui_pure_test 的精确契约）。
    {
      key: 'longPress', label: '长按测试', action: 'onLongPress', manage: true,
      title: '长按测试（test_action gesture=long_press；演练/真实 + 候选 + hold_ms 默认 3000）',
    },
    {
      key: 'text', label: '识别文字', action: 'onEditText', manage: true,
      title: '编辑该「使用端OCR文字配置项」的默认识别文字（runtime_text）',
    },
  ];

  /**
   * Resolve the settings menu for one row.
   * Disabled rules:
   *   - template_set: only 删除 is actionable (the other nine do not apply);
   *   - o.manage === false (read-only surface): every management item;
   *   - region: 操作区 is meaningless because the asset itself IS the region;
   *   - region: 阈值 does not exist (only template / OCR text confidence);
   *   - 识别文字: only ocr_var rows are editable (virtual rows use it as a
   *     "please migrate first" hint, so it stays clickable);
   *   - virtual rows: every other management item is read-only;
   *   - template_set handled above.
   * Templates DO expose 识别区 now: the canvas box becomes a re-crop overwrite
   * (`/task/save_asset` + replace:true, rev+1) instead of a rejected ROI patch,
   * so the field is editable on every surface and only `manage` gates it.
   * `threshold` is enabled for template, ocr_line, ocr_lines and ocr_var; its
   * title names the two meanings (match threshold vs OCR minimum confidence).
   * o.compact hides 点测/拖测/步骤 exactly as the old compact row did.
   */
  function assetMenuState(it, opts) {
    const o = opts || {};
    const kind = (it && it.kind) || '';
    const isSet = kind === 'template_set';
    const isVirtual = !!(it && it.virtual === true);
    return ASSET_MENU_ITEMS.map((m) => {
      let disabled = false;
      let title = m.title;
      if (isSet && m.key !== 'delete') {
        disabled = true;
        title = `素材集合不支持「${m.label}」：集合只有成员列表`;
      } else if (m.manage && !o.manage) {
        disabled = true;
        title = `当前列表只读，不能${m.label}；请到任务素材库或全局素材库操作`;
      } else if (m.key === 'action' && kind === 'region') {
        disabled = true;
        title = 'region 素材本身就是操作区域，无需再设置操作区';
      } else if (m.key === 'text') {
        if (kind !== 'ocr_var') {
          disabled = true;
          title = '仅「使用端OCR文字配置项」（ocr_var）素材可编辑识别文字';
        } else if (isVirtual) {
          // 虚拟素材不可编辑：保持可点击，点击后提示先迁移（任务 U-T4）。
          title = '该素材是旧任务参数合成的虚拟素材（待迁移）：点击会提示先迁移';
        }
      } else if (isVirtual && m.manage) {
        disabled = true;
        title = '虚拟素材（待迁移）只读：先迁移为真实 ocr_var 素材后才能编辑';
      } else if (m.key === 'threshold') {
        if (kind === 'template') {
          title = '模板阈值 0~1（NCC 匹配通过线；未设按默认 0.85）';
        } else if (kind === 'ocr_line' || kind === 'ocr_lines' || kind === 'ocr_var') {
          title = 'OCR 最低置信度（0~1；不设或 0 = 不限制）';
        } else {
          disabled = true;
          title = '仅模板图（template）与 OCR 文字素材支持阈值';
        }
      }
      return {
        key: m.key,
        label: m.label,
        action: m.action,
        title,
        disabled,
        hidden: !!(o.compact && m.compactHide),
      };
    });
  }

  function assetTestTitle(it) {
    if (it.kind === 'template' || it.kind === 'template_set') {
      return '识别：返回全部符合阈值的匹配候选（带评分；集合按成员声明序）';
    }
    if (it.kind === 'region') return '识别：显示该操作区域（region 素材本身即操作区域，单候选）';
    if (it.kind === 'ocr_var') {
      return '识别：返回识别区域内的全部候选行（带评分；roi 为空 = 全屏 OCR，较慢）';
    }
    return '识别：返回识别区域内的全部候选行（带评分）';
  }

  // main + 识别 + 设置 + hidden inline menu. The menu is a flex row inside the
  // asset row (flex-basis:100%) instead of an absolutely positioned popover so
  // .asset-list's overflow:auto cannot clip it.
  function appendRowActions(row, it, o, isSet) {
    const test = makeButton(
      '识别',
      isSet ? '识别：与 runtime 同源，测试最佳成员 + 集合阈值' : assetTestTitle(it),
      'asset-test asset-row-action asset-row-action-test'
    );
    test.addEventListener('click', () => {
      // A-1: acting on a row also selects it, so the highlight follows the
      // user's latest focus without a separate click.
      selectAsset(it.id);
      if (isSet) (o.onCollectionTest || o.onTest || (() => {}))(it);
      else (o.onTest || (() => {}))(it);
    });

    const menu = document.createElement('div');
    menu.className = 'asset-row-menu';
    menu.title = '素材设置：点测 / 拖测 / 识别区 / 操作区 / 重命名 / 阈值 / 位置帧 / 删除 / 步骤 / 识别文字';
    menu.setAttribute('aria-label', `${it.id} 的设置菜单`);
    const settings = makeButton(
      '设置',
      '展开设置：点测 / 拖测 / 识别区 / 操作区 / 重命名 / 阈值 / 位置帧 / 删除 / 步骤 / 识别文字',
      'asset-test asset-row-action asset-row-action-settings'
    );
    settings.dataset.menuToggle = '1';
    settings.setAttribute('aria-expanded', 'false');

    function closeMenu() {
      if (openMenu && openMenu.menu === menu) openMenu = null;
      if (openMenuKey === assetKey(it.id)) openMenuKey = '';
      menu.classList.remove('open');
      settings.classList.remove('open');
      settings.setAttribute('aria-expanded', 'false');
    }

    function openThisMenu() {
      // Only one settings panel is ever visible.
      if (openMenu && openMenu.menu !== menu) closeMenus();
      menu.classList.add('open');
      settings.classList.add('open');
      settings.setAttribute('aria-expanded', 'true');
      openMenu = { menu, settings };
      openMenuKey = assetKey(it.id);
    }

    for (const item of assetMenuState(it, o)) {
      if (item.hidden) continue;
      const b = makeButton(item.label, item.title, 'asset-menu-item');
      b.disabled = item.disabled;
      b.dataset.menuKey = item.key;
      b.addEventListener('click', () => {
        // A disabled entry must not change the selection or run anything.
        if (item.disabled) return;
        // Selecting the row the menu already belongs to keeps the panel open:
        // only 设置-again/Esc collapse it (user request 2026-11).
        selectAsset(it.id);
        if (isSet && item.action === 'onDelete') {
          (o.onCollectionDelete || o.onDelete || (() => {}))(it);
        } else if (item.action === 'onDelete') {
          (o.onDelete || (() => {}))(it);
        } else {
          (o[item.action] || (() => {}))(it);
        }
      });
      menu.appendChild(b);
    }

    settings.addEventListener('click', (e) => {
      e.stopPropagation();
      selectAsset(it.id);
      if (menu.classList.contains('open')) closeMenu();
      else openThisMenu();
    });
    menu.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        closeMenu();
        settings.focus();
      }
    });
    settings.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') closeMenu();
    });

    row.appendChild(test);
    row.appendChild(settings);
    row.appendChild(menu);
    return openThisMenu;
  }

  // P2-3: a re-render (save/rename/threshold/positions) replaces the DOM that
  // owned the open menu. Remember the asset id and reopen the panel on the new
  // row; the restore is deferred one tick so the row is already in the document
  // and only the VISIBLE surface (not the hidden rail panel) wins.
  function rowVisibleInBrowser(row) {
    // The Node DOM stubs used by ui_pure_test.js have no offsetParent property.
    if (!row || !('offsetParent' in row)) return true;
    return row.offsetParent !== null;
  }

  function restoreOpenMenu() {
    menuRestoreTimer = 0;
    if (!openMenuKey) return;
    // The old menu is still connected (no re-render happened): nothing to do.
    if (openMenu && openMenu.menu && openMenu.menu.isConnected === true) return;
    const candidates = liveRows.filter(
      (entry) => entry && entry.key === openMenuKey && typeof entry.openMenu === 'function'
    );
    if (!candidates.length) return;
    // Open on the surface the user is looking at. If the matching assets were
    // rendered into a hidden rail panel only, wait for the next render (the
    // other library repaints a moment later) instead of opening an invisible
    // menu that would then block the visible restore.
    const visible = candidates.filter((entry) => rowVisibleInBrowser(entry.row));
    if (!visible.length) return;
    visible[visible.length - 1].openMenu();
  }

  function scheduleMenuRestore() {
    if (!openMenuKey) return;
    // No timers in the pure Node harness: restore synchronously from the
    // freshly created row (the renderer appends synchronously right after).
    if (typeof setTimeout !== 'function') {
      restoreOpenMenu();
      return;
    }
    if (menuRestoreTimer) return;
    menuRestoreTimer = setTimeout(restoreOpenMenu, 0);
  }

  /**
   * Search + candidate picker shared by the builder asset field, asset position
   * picker, capture metadata dialog, and category filter.
   *
   * UX rule (all call sites): clicking the input (or the arrow) opens the FULL
   * candidate list even when the input already contains a selected label.
   * Typing filters; choosing fills the input. This keeps “click -> choose”
   * usable without manually deleting the current text first.
   *
   * opts: { items, value, placeholder, label, filter, onChoose, className }
   */
  function createSearchPicker(opts) {
    const o = opts || {};
    const wrap = document.createElement('span');
    wrap.className = 'asset-search-picker' + (o.className ? ' ' + o.className : '');
    const input = document.createElement('input');
    input.type = 'text';
    input.placeholder = o.placeholder || '输入片段，选择候选项…';
    input.value = o.value || '';
    const toggle = document.createElement('button');
    toggle.type = 'button';
    toggle.className = 'asset-search-toggle';
    toggle.tabIndex = -1;
    toggle.textContent = '▾';
    toggle.title = '展开全部候选';
    const menu = document.createElement('div');
    menu.className = 'asset-search-menu' + (o.menuClass ? ' ' + o.menuClass : '');
    menu.style.display = 'none';

    function labelOf(item) {
      return String(o.label ? o.label(item) : item.id != null ? item.id : item);
    }

    function matches(item, all) {
      if (o.filter && !o.filter(item)) return false;
      if (all) return true;
      const q = input.value.trim().toLowerCase();
      if (!q) return true;
      return labelOf(item).toLowerCase().includes(q);
    }

    function close() {
      menu.style.display = 'none';
      wrap.classList.remove('open');
    }

    function choose(item) {
      close();
      if (o.onChoose) o.onChoose(item, input);
    }

    function show(all) {
      menu.textContent = '';
      const items = (o.items || []).filter((item) => matches(item, all));
      if (!items.length) {
        const empty = document.createElement('div');
        empty.className = 'muted small';
        empty.textContent = '（无匹配候选）';
        menu.appendChild(empty);
      } else {
        for (const item of items) {
          const row = document.createElement('button');
          row.type = 'button';
          row.className = 'asset-search-option';
          row.dataset.id = String(item.id != null ? item.id : item);
          row.textContent = labelOf(item);
          if (o.renderOption) o.renderOption(row, item);
          row.addEventListener('mousedown', (e) => {
            e.preventDefault();
            choose(item);
          });
          menu.appendChild(row);
        }
      }
      menu.style.display = 'block';
      wrap.classList.add('open');
    }

    function openAll() {
      show(true);
    }

    input.addEventListener('focus', () => {
      const opening = menu.style.display === 'none';
      openAll();
      // Select the current label so the first keystroke replaces it instead of
      // appending to the old category/name.
      if (opening) input.select();
    });
    input.addEventListener('click', () => {
      if (menu.style.display === 'none') openAll();
    });
    input.addEventListener('input', () => show(false));
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        close();
        input.blur();
      } else if (e.key === 'Enter') {
        const first = menu.querySelector('.asset-search-option');
        if (o.enterChooses !== false && first) {
          e.preventDefault();
          const items = (o.items || []).filter((item) => matches(item, false));
          if (items[0]) choose(items[0]);
        } else if (o.onEnter) {
          e.preventDefault();
          o.onEnter();
        }
      }
    });
    input.addEventListener('blur', () => setTimeout(close, 120));
    toggle.addEventListener('mousedown', (e) => e.preventDefault());
    toggle.addEventListener('click', (e) => {
      e.stopPropagation();
      if (menu.style.display === 'none') {
        input.focus();
        openAll();
        input.select();
      } else {
        close();
      }
    });
    wrap.append(input, toggle, menu);
    wrap.input = input;
    wrap.close = close;
    wrap.setItems = function (items) {
      o.items = items || [];
      if (menu.style.display !== 'none') show(false);
    };
    wrap.setValue = function (value) {
      input.value = value || '';
    };
    return wrap;
  }

  // U-T4（事项 2/5）：操作区（op_region）的展示摘要。素材行详情用它显示当前操作
  // 区来源；完整链校验/递归对话框在 app.js（阶段 B 接入）。
  function opRegionNodeOf(opRegion) {
    const r = opRegion && typeof opRegion === 'object' && !Array.isArray(opRegion) ? opRegion : {};
    const modeRaw = r.mode === undefined || r.mode === null ? 'self' : String(r.mode);
    const mode = modeRaw.trim().toLowerCase() || 'self';
    const asset = String(r.asset === undefined || r.asset === null ? '' : r.asset).trim();
    const pickRaw = String(r.pick === undefined || r.pick === null ? '' : r.pick).trim().toLowerCase();
    const pick = pickRaw === 'nearest' ? 'nearest' : 'score';
    const offset = Array.isArray(r.offset) ? r.offset.map((v) => Number(v)) : [];
    return { mode, asset, pick, offset };
  }

  function opRegionSummary(opRegion) {
    const n = opRegionNodeOf(opRegion);
    if (n.mode === 'offset') {
      if (n.offset.length !== 4 || n.offset.some((v) => !Number.isFinite(v))) {
        return '相对偏移（偏移值无效）';
      }
      return `相对偏移 [${n.offset.join(',')}]`;
    }
    if (n.mode === 'asset') {
      if (!n.asset) return '引用素材（缺少素材 id）';
      return `引用素材「${n.asset}」` + (n.pick === 'nearest' ? '（离上一节点最近）' : '（评分最高）');
    }
    return '当前素材操作区';
  }

  function assetText(it) {
    const roi = it.roi ? `[${it.roi.join(',')}]` : '—';
    const actionBox = it.action_shape
      ? (it.action_shape.roi || (() => {
          const pts = it.action_shape.points || [];
          if (!pts.length) return null;
          let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
          for (const p of pts) {
            x0 = Math.min(x0, p[0]); y0 = Math.min(y0, p[1]);
            x1 = Math.max(x1, p[0]); y1 = Math.max(y1, p[1]);
          }
          return [x0, y0, x1 - x0 + 1, y1 - y0 + 1];
        })())
      : null;
    const action = actionBox ? ` · 操作区域[${actionBox.join(',')}]${it.action_shape.invert ? '(反选)' : ''}` : '';
    const runtime = it.kind === 'ocr_var'
      ? ` · 识别文字「${it.runtime_text === undefined || it.runtime_text === null ? '—' : String(it.runtime_text)}」`
      : '';
    const opRegion = it.op_region !== undefined && it.op_region !== null
      ? ` · 操作区[${opRegionSummary(it.op_region)}]`
      : '';
    const pos = (it.positions && it.positions.length)
      ? ` · 位置帧[${it.positions.join(',')}]`
      : ' · 位置帧未设置';
    return `${it.id}\n识别 ${roi}${action}${runtime}${opRegion}${pos}`;
  }

  /**
   * One asset row.
   * opts:
   *   task, pick, manage, compact, actions
   *   onPick(it), onTest(it), onPosition(it), onDelete(it),
   *   onAction(it), onClickTest(it), onDragTest(it), onStep(it),
   *   onEditRoi(it), onRename(it), onThreshold(it), onEditText(it),
   *   onCollectionTest(it), onCollectionDelete(it)
   * The 设置 menu calls the split rename/threshold/ROI callbacks. There is no
   * combined onEdit: the legacy editor was dead code and was removed.
   * actions:false hides BOTH 识别 and 设置 (including the management menu).
   */
  function createRow(it, opts) {
    const o = opts || {};
    const task = o.task || '';
    const row = document.createElement('div');
    row.className = 'asset-row' + (o.compact ? ' asset-row-compact' : '');
    row.dataset.assetId = it.id;
    // A-3: every freshly rendered row restores the selection from module state,
    // so a save/refresh re-render never loses the user's highlighted asset.
    const key = assetKey(it.id);
    if (key === selectedAssetKey) row.classList.add(SELECTED_CLASS);
    liveRows.push({ key, row });
    if (liveRows.length > 400) syncSelectedRows();
    // Only lists that actually persist /task/assets/reorder may advertise a
    // drag handle. Referenced/global lists pass draggable:false.
    row.draggable = o.draggable !== false && it.kind !== 'template_set';
    // U-T4：旧 meta.params（role=ocr_text 无 asset_id）合成的虚拟 ocr_var 素材
    // 只读展示，行上挂「待迁移」badge（数据由后端 B-T3 的 virtual_items 提供）。
    if (it.virtual === true) row.classList.add('asset-row-virtual');

    if (it.kind === 'template_set') {
      const main = makeButton('', `${it.id}：flow 引用该集合；运行时按成员顺序尝试匹配`, 'asset-main');
      main.textContent = `${it.id}\n集合成员 ${(it.members || []).length} 张：${(it.members || []).join(' / ')}`;
      main.addEventListener('click', () => {
        selectAsset(it.id);
        if (o.onPick) o.onPick(it);
      });
      row.appendChild(main);
      // Same 识别 + 设置 pair as every other asset row. In the menu only 删除 is
      // enabled (and only on a manageable surface); the other eight are disabled
      // with a "集合不支持" title so the row stays visually uniform.
      const entry = liveRows[liveRows.length - 1];
      if (o.actions !== false) {
        entry.openMenu = appendRowActions(row, it, o, true);
        scheduleMenuRestore();
      }
      return row;
    }

    if (o.pick && it.kind === 'template') {
      const pick = document.createElement('input');
      pick.type = 'checkbox';
      pick.className = 'asset-pick';
      pick.value = it.id;
      pick.title = '勾选后点「新建集合」';
      row.appendChild(pick);
    }

    const main = makeButton('', it.file || '（无图）', 'asset-main');
    main.appendChild(makeThumb(it, task));
    const text = document.createElement('span');
    text.className = 'asset-main-text';
    text.textContent = assetText(it);
    main.appendChild(text);
    main.addEventListener('click', () => {
      // A-1: clicking the row body selects the asset (and still loads its ROI).
      selectAsset(it.id);
      if (o.onPick) o.onPick(it);
    });
    row.appendChild(main);
    if (it.virtual === true) {
      const badge = document.createElement('span');
      badge.className = 'asset-virtual-badge';
      badge.textContent = '待迁移';
      badge.title = '旧任务参数合成的虚拟素材（只读）；迁移为真实 ocr_var 素材后才能编辑';
      row.appendChild(badge);
    }

    // The old 改/帧/删/操作区/点测/拖测/步骤 buttons all live in the settings
    // menu now; the row itself exposes exactly 识别 + 设置.
    if (o.actions !== false) {
      const entry = liveRows[liveRows.length - 1];
      entry.openMenu = appendRowActions(row, it, o, false);
      scheduleMenuRestore();
    }
    return row;
  }

  function renderRows(container, items, opts) {
    if (!container) return;
    const o = opts || {};
    if (o.clear !== false) container.textContent = '';
    if (!items || !items.length) {
      const empty = document.createElement('div');
      empty.className = 'muted small';
      empty.textContent = o.emptyText || '（没有素材）';
      container.appendChild(empty);
      return;
    }
    if (o.group) {
      const groupBy = o.groupBy || 'kind';
      const groups = new Map();
      for (const it of items) {
        const key = groupBy === 'category'
          ? ((it.category || '').trim() || '未分类')
          : it.kind;
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key).push(it);
      }
      for (const [key, list] of groups) {
        const h = document.createElement('h4');
        h.className = 'asset-group';
        h.textContent = groupBy === 'category'
          ? `${key}（${list.length}）`
          : `${kindLabel(key)}（${list.length}）`;
        container.appendChild(h);
        for (const it of list) container.appendChild(createRow(it, o));
      }
    } else {
      for (const it of items) container.appendChild(createRow(it, o));
    }
  }

  window.OASAssetUI = {
    KIND_LABELS,
    kindLabel,
    makeThumb,
    // #6：已存识别多边形 → 缩略图坐标投影（ui_pure_test 直接断言）。
    thumbPolygonPoints,
    drawThumbPolygon,
    createRow,
    renderRows,
    createSearchPicker,
    // A: selected-row state + the single-open settings menu.
    selectAsset,
    getSelectedAsset,
    closeMenus,
    SELECTED_CLASS,
    // Exposed for ui_pure_test.js: the menu order/rules are part of the contract.
    ASSET_MENU_ITEMS,
    assetMenuState,
    // U-T4: row detail (识别文字/op_region 摘要) + op_region display helpers.
    assetText,
    assetTestTitle,
    opRegionSummary,
    opRegionNodeOf,
  };
})();
