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
        box.textContent = 'ROI';
        box.classList.add('asset-thumb-empty');
      };
      img.src = (it.positions && it.positions.length)
        ? `/frames/image?id=${it.positions[0]}&ts=${Date.now()}`
        : '/frame.png?ts=' + Date.now();
      box.textContent = 'ROI';
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
        if (first) {
          e.preventDefault();
          const items = (o.items || []).filter((item) => matches(item, false));
          if (items[0]) choose(items[0]);
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
    const action = it.action_roi ? ` · 操作区域[${it.action_roi.join(',')}]` : '';
    const pos = (it.positions && it.positions.length)
      ? ` · 位置帧[${it.positions.join(',')}]`
      : ' · 位置帧未设置';
    return `${it.id}\n识别 ${roi}${action}${pos}`;
  }

  /**
   * One asset row.
   * opts:
   *   task, pick, manage, compact, actions
   *   onPick(it), onTest(it), onEdit(it), onPosition(it), onDelete(it),
   *   onAction(it), onClickTest(it), onDragTest(it), onStep(it),
   *   onCollectionTest(it), onCollectionDelete(it)
   */
  function createRow(it, opts) {
    const o = opts || {};
    const task = o.task || '';
    const row = document.createElement('div');
    row.className = 'asset-row' + (o.compact ? ' asset-row-compact' : '');

    if (it.kind === 'template_set') {
      const main = makeButton('', `${it.id}：flow 引用该集合；运行时按成员顺序尝试匹配`, 'asset-main');
      main.textContent = `${it.id}\n集合成员 ${(it.members || []).length} 张：${(it.members || []).join(' / ')}`;
      if (o.onPick) main.addEventListener('click', () => o.onPick(it));
      row.appendChild(main);
      if (o.actions !== false) {
        const test = makeButton('测集合', '与 runtime 同源：最佳成员 + 集合阈值', 'asset-test');
        test.addEventListener('click', () => (o.onCollectionTest || o.onTest || (() => {}))(it));
        row.appendChild(test);
      }
      if (o.manage) {
        const del = makeButton('删', '删除集合（flow 引用时阻止）', 'asset-test');
        del.addEventListener('click', () => (o.onCollectionDelete || (() => {}))(it));
        row.appendChild(del);
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
    if (o.onPick) main.addEventListener('click', () => o.onPick(it));
    row.appendChild(main);

    if (o.actions !== false) {
      const test = makeButton('测试', '在当前帧用该素材测试', 'asset-test');
      if (it.kind === 'region') {
        test.textContent = '区域';
        test.title = '选中该操作区域（区域点击/拖动可直接引用）';
      } else if (it.kind.startsWith('ocr')) {
        test.textContent = '识别';
        test.title = '在当前帧用该 ROI 试识别文字';
      }
      test.addEventListener('click', () => (o.onTest || (() => {}))(it));
      row.appendChild(test);
    }

    if (o.manage) {
      const edit = makeButton('改', '重命名 / 修改 OCR ROI / 模板阈值', 'asset-test');
      edit.addEventListener('click', () => (o.onEdit || (() => {}))(it));
      const pos = makeButton('帧', '设置素材位置帧', 'asset-test');
      pos.addEventListener('click', () => (o.onPosition || (() => {}))(it));
      const del = makeButton('删', '删除素材（flow 仍引用时会被拒绝）', 'asset-test');
      del.addEventListener('click', () => (o.onDelete || (() => {}))(it));
      row.append(edit, pos, del);
      if (it.kind !== 'region') {
        const action = makeButton('操作区', '在画布上拖拽设置该素材的操作区域', 'asset-test');
        action.addEventListener('click', () => (o.onAction || (() => {}))(it));
        row.appendChild(action);
      }
    }

    if (o.actions !== false && !o.compact) {
      const clickTest = makeButton('点测', '点击测试（dry-run：验证定位并显示目标点）', 'asset-test');
      clickTest.addEventListener('click', () => (o.onClickTest || (() => {}))(it));
      const dragTest = makeButton('拖测', '拖动测试（dry-run：生成方向/起终点）', 'asset-test');
      dragTest.addEventListener('click', () => (o.onDragTest || (() => {}))(it));
      const step = makeButton('步骤', '生成 click_target 步骤并复制到剪贴板', 'asset-test');
      step.addEventListener('click', () => (o.onStep || (() => {}))(it));
      row.append(clickTest, dragTest, step);
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

  window.OASAssetUI = { KIND_LABELS, kindLabel, makeThumb, createRow, renderRows, createSearchPicker };
})();
