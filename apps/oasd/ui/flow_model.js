// Pure flow-model helpers for the visual flow editor (ENGINE §22.18).
//
// No DOM here on purpose: this file is unit-tested in a headless browser
// (ui/flow_model_test.html) and reused by app.js.
//
// Model:
//   { name, steps: [{op, argsText}], interrupts: [{detect, action}] }
// `argsText` is a JSON string so invalid args can be shown/fixed without losing
// the rest of the flow; buildFlow() parses it and fails loudly.
(function () {
  // 中文短名（界面展示用；flow JSON 里始终是英文 op 键）
  const OP_LABELS = {
    log: '日志',
    wait: '等待',
    wait_until: '等到',
    appear: '出现',
    click: '点击',
    click_point: '点坐标',
    click_target: '定位点击',
    dismiss_popup: '关弹窗',
    swipe: '滑动',
    drag: '拖动（拟人）',
    ocr: '识别文字',
    finish: '结束',
  };

  const OPS = [
    'log',
    'wait',
    'wait_until',
    'appear',
    'click',
    'click_point',
    'click_target',
    'dismiss_popup',
    'swipe',
    'drag',
    'ocr',
    'finish',
  ];

  function defaultArgs(op) {
    switch (op) {
      case 'log':
        return { level: 'info', text: "'hello'" };
      case 'wait':
        return { seconds: 1 };
      case 'wait_until':
        return { cond: "appear('I_A')", timeout_s: 10 };
      case 'appear':
        return { sel: 'I_A' };
      case 'click':
        return { sel: 'I_A' };
      case 'click_point':
        return { x: 100, y: 100 };
      case 'click_target':
        return { find: [{ kind: 'template', sel: 'I_A' }] };
      case 'dismiss_popup':
        return { sel: 'POPUP', timeout_ms: 0 };
      case 'swipe':
        return { from: [1180, 260], to: [1180, 660], ms: 400 };
      case 'drag':
        return {
          from: { kind: 'region', roi: [100, 400, 300, 200] },
          direction: 'up',
          distance: [150, 450],
          duration_ms: [300, 650],
          margin: 0.15,
        };
      case 'ocr':
        return { sel: 'O_A' };
      case 'finish':
        return { exit: 'done' };
      default:
        return {};
    }
  }

  function pretty(obj) {
    return JSON.stringify(obj, null, 2);
  }

  function parseFlow(text) {
    let flow;
    try {
      flow = JSON.parse(text);
    } catch (e) {
      throw new Error('flow 不是合法 JSON：' + e.message);
    }
    if (!flow || typeof flow !== 'object' || Array.isArray(flow)) {
      throw new Error('flow 必须是 JSON 对象');
    }
    if (!Array.isArray(flow.steps)) {
      throw new Error('flow.steps 必须是数组');
    }
    const steps = flow.steps.map((s, i) => {
      if (!s || typeof s.op !== 'string' || !s.op) {
        throw new Error('steps[' + i + '] 缺少字符串 op');
      }
      if (s.op === 'if' || s.op === 'loop' || s.op === 'while') {
        throw new Error(
          'steps[' + i + '] 是 ' + s.op + ' 嵌套块：请用 JSON 文本模式编辑（可视化模式不 flatten，避免丢内容）'
        );
      }
      const args = s.args && typeof s.args === 'object' && !Array.isArray(s.args) ? s.args : {};
      return { op: s.op, argsText: pretty(args) };
    });
    const interrupts = Array.isArray(flow.interrupts)
      ? flow.interrupts.map((r, i) => {
          if (!r || typeof r.detect !== 'string' || !r.detect) {
            throw new Error('interrupts[' + i + '] 缺少字符串 detect');
          }
          return { detect: r.detect, action: typeof r.action === 'string' ? r.action : '' };
        })
      : [];
    return {
      schema_version: flow.schema_version || 1,
      name: typeof flow.name === 'string' ? flow.name : '',
      steps: steps,
      interrupts: interrupts,
    };
  }

  function buildFlow(model) {
    if (!model || !Array.isArray(model.steps)) throw new Error('model.steps 缺失');
    if (model.steps.length === 0) throw new Error('至少需要一个步骤');
    const steps = model.steps.map((s, i) => {
      if (!OPS.includes(s.op)) throw new Error('steps[' + i + '] 未知 op `' + s.op + '`');
      let args;
      try {
        args = JSON.parse(s.argsText || '{}');
      } catch (e) {
        throw new Error('steps[' + i + '] args 不是合法 JSON：' + e.message);
      }
      if (!args || typeof args !== 'object' || Array.isArray(args)) {
        throw new Error('steps[' + i + '] args 必须是 JSON 对象');
      }
      return { op: s.op, args: args };
    });
    const out = {
      schema_version: 1,
      name: (model.name || '').trim() || '未命名',
      steps: steps,
    };
    const ints = (model.interrupts || [])
      .filter((r) => r && typeof r.detect === 'string' && r.detect.trim())
      .map((r) => (r.action && r.action.trim() ? { detect: r.detect, action: r.action } : { detect: r.detect }));
    if (ints.length) out.interrupts = ints;
    return pretty(out);
  }

  // Structural checks that are cheap and deterministic; the server still does the
  // full schema validation on save.
  function validateModel(model) {
    const errs = [];
    if (!model || !Array.isArray(model.steps) || model.steps.length === 0) {
      errs.push('至少需要一个步骤');
      return errs;
    }
    const finishes = model.steps.filter((s) => s.op === 'finish').length;
    if (finishes !== 1) errs.push('必须恰好一个 finish 步骤（当前 ' + finishes + ' 个）');
    if (model.steps[model.steps.length - 1].op !== 'finish') {
      errs.push('finish 必须是最后一步');
    }
    for (let i = 0; i < model.steps.length; i++) {
      const s = model.steps[i];
      try {
        JSON.parse(s.argsText || '{}');
      } catch (e) {
        errs.push('steps[' + i + '] args 不是合法 JSON：' + e.message);
      }
    }
    return errs;
  }

  const api = { OPS, OP_LABELS, defaultArgs, parseFlow, buildFlow, validateModel };
  if (typeof window !== 'undefined') window.OAS_FLOW = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
