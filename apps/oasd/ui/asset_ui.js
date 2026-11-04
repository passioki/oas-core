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
    region: '操作区域（region）',
    template_set: '素材集合（template_set）',
  };

  function kindLabel(kind) {
    return KIND_LABELS[kind] || kind;
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

  // Every asset row has exactly TWO action buttons: 识别 (the existing onTest
  // behaviour) and 设置 (this menu). The nine entries and their order are a UX
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
    { key: 'roi', label: '识别区', action: 'onEditRoi', manage: true, title: '编辑识别区域 x,y,w,h' },
    {
      key: 'action', label: '操作区', action: 'onAction', manage: true,
      title: '在画布上拖拽设置该素材的操作区域',
    },
    { key: 'rename', label: '重命名', action: 'onRename', manage: true, title: '只重命名素材 id（不改阈值/识别区）' },
    { key: 'threshold', label: '阈值', action: 'onThreshold', manage: true, title: '模板阈值 0~1（仅 template）' },
    { key: 'position', label: '位置帧', action: 'onPosition', manage: true, title: '设置素材位置帧' },
    { key: 'delete', label: '删除', action: 'onDelete', manage: true, title: '删除素材（flow 仍引用时会被拒绝）' },
    {
      key: 'step', label: '步骤', action: 'onStep', compactHide: true,
      title: '生成 click_target 步骤并复制到剪贴板',
    },
  ];

  /**
   * Resolve the settings menu for one row.
   * Disabled rules:
   *   - template_set: only 删除 is actionable (the other eight do not apply);
   *   - o.manage === false (frame detail is read-only): every management item;
   *   - template: 识别区 does not exist as an editable field — the server rejects
   *     template ROI updates with "needs re-crop" (a template's ROI is baked into
   *     its cropped image), so the menu item must not look actionable;
   *   - region: 操作区 is meaningless because the asset itself IS the region;
   *   - non-template: 阈值 does not exist.
   * o.compact hides 点测/拖测/步骤 exactly as the old compact row did.
   */
  function assetMenuState(it, opts) {
    const o = opts || {};
    const kind = (it && it.kind) || '';
    const isSet = kind === 'template_set';
    return ASSET_MENU_ITEMS.map((m) => {
      let disabled = false;
      let title = m.title;
      if (isSet && m.key !== 'delete') {
        disabled = true;
        title = `素材集合不支持「${m.label}」：集合只有成员列表`;
      } else if (m.manage && !o.manage) {
        disabled = true;
        title = `当前列表只读，不能${m.label}；请到任务素材库或全局素材库操作`;
      } else if (m.key === 'roi' && kind === 'template') {
        disabled = true;
        title = '模板无识别区（服务端拒绝修改模板 ROI）；如需变更请重新截图保存并替换素材';
      } else if (m.key === 'action' && kind === 'region') {
        disabled = true;
        title = 'region 素材本身就是操作区域，无需再设置操作区';
      } else if (m.key === 'threshold' && kind !== 'template') {
        disabled = true;
        title = '仅模板图（template）支持阈值';
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
    if (it.kind === 'template') return '识别：在当前帧用该素材测试模板匹配';
    if (it.kind === 'region') return '识别：选中该操作区域（区域点击/拖动可直接引用）';
    return '识别：在当前帧用该 ROI 试识别文字';
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
      if (isSet) (o.onCollectionTest || o.onTest || (() => {}))(it);
      else (o.onTest || (() => {}))(it);
    });

    const menu = document.createElement('div');
    menu.className = 'asset-row-menu';
    menu.title = '素材设置：点测 / 拖测 / 识别区 / 操作区 / 重命名 / 阈值 / 位置帧 / 删除 / 步骤';
    menu.setAttribute('aria-label', `${it.id} 的设置菜单`);
    const settings = makeButton(
      '设置',
      '展开设置：点测 / 拖测 / 识别区 / 操作区 / 重命名 / 阈值 / 位置帧 / 删除 / 步骤',
      'asset-test asset-row-action asset-row-action-settings'
    );
    settings.dataset.menuToggle = '1';
    settings.setAttribute('aria-expanded', 'false');

    function closeMenu() {
      menu.classList.remove('open');
      settings.classList.remove('open');
      settings.setAttribute('aria-expanded', 'false');
    }

    for (const item of assetMenuState(it, o)) {
      if (item.hidden) continue;
      const b = makeButton(item.label, item.title, 'asset-menu-item');
      b.disabled = item.disabled;
      b.dataset.menuKey = item.key;
      b.addEventListener('click', () => {
        if (item.disabled) return;
        closeMenu();
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
      const open = menu.classList.toggle('open');
      settings.classList.toggle('open', open);
      settings.setAttribute('aria-expanded', String(open));
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
    const pos = (it.positions && it.positions.length)
      ? ` · 位置帧[${it.positions.join(',')}]`
      : ' · 位置帧未设置';
    return `${it.id}\n识别 ${roi}${action}${pos}`;
  }

  /**
   * One asset row.
   * opts:
   *   task, pick, manage, compact, actions
   *   onPick(it), onTest(it), onPosition(it), onDelete(it),
   *   onAction(it), onClickTest(it), onDragTest(it), onStep(it),
   *   onEditRoi(it), onRename(it), onThreshold(it),
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
    // Only lists that actually persist /task/assets/reorder may advertise a
    // drag handle. Referenced/global lists pass draggable:false.
    row.draggable = o.draggable !== false && it.kind !== 'template_set';

    if (it.kind === 'template_set') {
      const main = makeButton('', `${it.id}：flow 引用该集合；运行时按成员顺序尝试匹配`, 'asset-main');
      main.textContent = `${it.id}\n集合成员 ${(it.members || []).length} 张：${(it.members || []).join(' / ')}`;
      if (o.onPick) main.addEventListener('click', () => o.onPick(it));
      row.appendChild(main);
      // Same 识别 + 设置 pair as every other asset row. In the menu only 删除 is
      // enabled (and only on a manageable surface); the other eight are disabled
      // with a "集合不支持" title so the row stays visually uniform.
      if (o.actions !== false) appendRowActions(row, it, o, true);
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
    if (o.onPick) main.addEventListener('click', () => o.onPick(it));
    row.appendChild(main);

    // The old 改/帧/删/操作区/点测/拖测/步骤 buttons all live in the settings
    // menu now; the row itself exposes exactly 识别 + 设置.
    if (o.actions !== false) appendRowActions(row, it, o, false);
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
    createRow,
    renderRows,
    createSearchPicker,
    // Exposed for ui_pure_test.js: the menu order/rules are part of the contract.
    ASSET_MENU_ITEMS,
    assetMenuState,
  };
})();
