// OAS engine editor - MVP front end.
// Plain JS on purpose: no build step, so the same files can be served by the
// Rust daemon today and wrapped by Tauri / an Android WebView later.

const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');
const statusEl = document.getElementById('status');
const roiEl = document.getElementById('roi');
const resultEl = document.getElementById('result');
const logEl = document.getElementById('log');
const btnCapture = document.getElementById('btn-capture');
const btnOcr = document.getElementById('btn-ocr');
const btnClear = document.getElementById('btn-clear');
const btnLines = document.getElementById('btn-lines');

let img = null;
let roi = null; // {x,y,w,h} in frame coordinates
let dragStart = null;
let boxes = []; // detection boxes from the multi-line route

function log(msg) {
  const t = new Date().toLocaleTimeString();
  logEl.textContent = `[${t}] ${msg}\n` + logEl.textContent;
}

function setStatus(text) { statusEl.textContent = text; }

// Map a mouse event to frame coordinates (canvas is scaled by CSS width).
function toFrame(ev) {
  const r = canvas.getBoundingClientRect();
  const sx = canvas.width / r.width;
  const sy = canvas.height / r.height;
  return {
    x: Math.round((ev.clientX - r.left) * sx),
    y: Math.round((ev.clientY - r.top) * sy),
  };
}

function normalizeRect(a, b) {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  const w = Math.abs(a.x - b.x);
  const h = Math.abs(a.y - b.y);
  return { x, y, w, h };
}

function draw() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (img) ctx.drawImage(img, 0, 0);
  if (roi && roi.w > 0 && roi.h > 0) {
    ctx.save();
    ctx.strokeStyle = '#31d0aa';
    ctx.lineWidth = 2;
    ctx.strokeRect(roi.x + 0.5, roi.y + 0.5, roi.w, roi.h);
    ctx.fillStyle = 'rgba(49,208,170,0.12)';
    ctx.fillRect(roi.x, roi.y, roi.w, roi.h);
    ctx.fillStyle = '#31d0aa';
    ctx.font = '14px Consolas, monospace';
    ctx.fillText(`${roi.w}x${roi.h} @ ${roi.x},${roi.y}`, roi.x + 4, Math.max(14, roi.y - 6));
    ctx.restore();
  }
  // detection boxes (multi-line route)
  // High-contrast strategy: every outline is drawn twice - a dark halo first,
  // then the bright line on top - so it stays readable on both light and dark
  // game art. Labels get a dark plate for the same reason.
  for (const b of boxes) {
    const [x, y, w, h] = b.box;
    ctx.save();
    ctx.strokeStyle = 'rgba(0,0,0,0.85)';
    ctx.lineWidth = 5;
    ctx.strokeRect(x + 0.5, y + 0.5, w, h);
    ctx.strokeStyle = '#ffd54a';
    ctx.lineWidth = 2;
    ctx.strokeRect(x + 0.5, y + 0.5, w, h);
    // tiny corner ticks make the exact edges readable on busy art
    ctx.strokeStyle = 'rgba(0,0,0,0.85)';
    ctx.lineWidth = 4;
    const tick = 6;
    ctx.beginPath();
    for (const [cx, cy, dx, dy] of [[x, y, 1, 1], [x + w, y, -1, 1], [x, y + h, 1, -1], [x + w, y + h, -1, -1]]) {
      ctx.moveTo(cx, cy);
      ctx.lineTo(cx + dx * tick, cy);
      ctx.moveTo(cx, cy);
      ctx.lineTo(cx, cy + dy * tick);
    }
    ctx.stroke();
    ctx.fillStyle = 'rgba(232,180,49,0.08)';
    ctx.fillRect(x, y, w, h);
    // label plate
    const label = `${b.text} ${b.score.toFixed(2)}`;
    ctx.font = '13px Consolas, monospace';
    const tw = ctx.measureText(label).width;
    const ly = y >= 16 ? y - 15 : y + 1;
    ctx.fillStyle = 'rgba(0,0,0,0.72)';
    ctx.fillRect(x, ly, tw + 8, 15);
    ctx.fillStyle = '#ffd54a';
    ctx.fillText(label, x + 4, ly + 12);
    ctx.restore();
  }
}

function updateRoiLabel() {
  if (!roi) {
    roiEl.textContent = '未选择';
    btnOcr.disabled = true;
    btnLines.disabled = true;
    return;
  }
  roiEl.textContent = `x=${roi.x} y=${roi.y} w=${roi.w} h=${roi.h}`;
  btnOcr.disabled = roi.w < 2 || roi.h < 2;
  btnLines.disabled = roi.w < 2 || roi.h < 2;
}

async function loadFrame() {
  return new Promise((resolve, reject) => {
    const im = new Image();
    im.onload = () => {
      img = im;
      canvas.width = im.width;
      canvas.height = im.height;
      draw();
      resolve(im);
    };
    im.onerror = () => reject(new Error('帧加载失败'));
    im.src = '/frame.png?t=' + Date.now();
  });
}

async function refreshState() {
  const r = await fetch('/state');
  const s = await r.json();
  if (!s.captured) {
    setStatus(s.error ? `未截到帧：${s.error}` : '没有帧');
    return;
  }
  setStatus(`${s.device} · ${s.width}x${s.height}`);
}

async function doCapture() {
  btnCapture.disabled = true;
  setStatus('截图中…');
  try {
    const r = await fetch('/capture', { method: 'POST' });
    const s = await r.json();
    if (!s.ok) {
      setStatus(`截图失败：${s.error}`);
      log(`截图失败：${s.error}`);
      return;
    }
    await loadFrame();
    await refreshState();
    log(`已截图 ${s.width}x${s.height}`);
  } catch (e) {
    setStatus('截图异常');
    log('截图异常：' + e);
  } finally {
    btnCapture.disabled = false;
  }
}

canvas.addEventListener('mousedown', (ev) => {
  dragStart = toFrame(ev);
  roi = null;
  boxes = [];
  draw();
});

canvas.addEventListener('mousemove', (ev) => {
  if (!dragStart) return;
  roi = normalizeRect(dragStart, toFrame(ev));
  updateRoiLabel();
  draw();
});

canvas.addEventListener('mouseup', () => {
  if (!dragStart) return;
  dragStart = null;
  updateRoiLabel();
  draw();
  if (roi) log(`选中 ROI ${roi.w}x${roi.h} @ ${roi.x},${roi.y}`);
});

btnClear.addEventListener('click', () => {
  roi = null;
  updateRoiLabel();
  draw();
  resultEl.textContent = '—';
});

btnOcr.addEventListener('click', async () => {
  if (!roi) return;
  btnOcr.disabled = true;
  resultEl.textContent = '识别中…';
  try {
    // Wired up in the next step; the route already exists on the backend side.
    const q = `x=${roi.x}&y=${roi.y}&w=${roi.w}&h=${roi.h}`;
    const r = await fetch('/ocr?' + q);
    const s = await r.json();
    if (s.error) {
      resultEl.textContent = '错误：' + s.error;
    } else {
      resultEl.textContent = `text = ${JSON.stringify(s.text)}\nscore = ${s.score.toFixed(4)}\nelapsed = ${s.elapsed_ms.toFixed(1)} ms`;
      log(`OCR → ${s.text} (${s.score.toFixed(3)})`);
    }
  } catch (e) {
    resultEl.textContent = '识别异常：' + e;
  } finally {
    btnOcr.disabled = false;
  }
});

btnCapture.addEventListener('click', doCapture);

btnLines.addEventListener('click', async () => {
  if (!roi) return;
  btnLines.disabled = true;
  resultEl.textContent = '多行识别中…';
  try {
    const q = `x=${roi.x}&y=${roi.y}&w=${roi.w}&h=${roi.h}`;
    const r = await fetch('/lines?' + q);
    const s = await r.json();
    if (s.error) {
      resultEl.textContent = '错误：' + s.error;
      log('多行识别错误：' + s.error);
    } else {
      // /lines returns boxes in ROI-LOCAL coordinates; the canvas draws in
      // frame coordinates, so shift by the ROI origin (this was the visible
      // "boxes drift to the upper-left" bug).
      boxes = (s.lines || []).map((b) => ({
        ...b,
        box: [roi.x + b.box[0], roi.y + b.box[1], b.box[2], b.box[3]],
      }));
      draw();
      const txt = boxes.map((b, i) => `${i + 1}. ${b.text} (${b.score.toFixed(3)})`).join('\n');
      resultEl.textContent = `${boxes.length} 行 / ${s.elapsed_ms.toFixed(0)} ms\n` + txt;
      log(`多行识别 → ${boxes.length} 行`);
    }
  } catch (e) {
    resultEl.textContent = '多行识别异常：' + e;
  } finally {
    btnLines.disabled = false;
  }
});

(async function boot() {
  try {
    await loadFrame();
    await refreshState();
    btnCapture.disabled = false;
    log('已连接，画面已加载');
  } catch (e) {
    setStatus('没有可用帧，点「截图」');
    btnCapture.disabled = false;
  }
})();
