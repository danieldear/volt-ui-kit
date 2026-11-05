// One shared WASM module per document. UI declarations and callbacks are Rust.
import { attachVolt } from './volt-host.js';
const roots = [...document.querySelectorAll('[data-volt-page]')];
const active = new Set();
const MAX_ACTIVE = 8, MAX_GPU = 2;
let pumping = false, stopped = false;
let modulePromise;
const accents = ['zinc', 'blue', 'violet', 'rose', 'green', 'orange'];
const mode = matchMedia('(prefers-color-scheme: dark)');
const forcedSoftware = new URLSearchParams(location.search).get('renderer') === 'software';
const items = roots.map(root => ({ root, stage: root.querySelector('.volt-root'), status: root.querySelector('.volt-status'), driver: null, controller: null, visible: false, busy: false, rendered: false, gpu: false, failed: false }));
function accent() { return Math.max(0, accents.indexOf(document.querySelector('input[name="accent"]:checked')?.id?.replace('accent-', '') || 'zinc')); }
function report(item, text, error = false) { item.status.textContent = text; item.status.dataset.error = String(error); }
function freshCanvas(item) { const old = item.stage.querySelector('canvas'); const canvas = old.cloneNode(false); old.replaceWith(canvas); return canvas; }
function record() {
  document.documentElement.dataset.voltActive = String(active.size);
  document.documentElement.dataset.voltGpu = String([...active].filter(i => i.gpu).length);
}
function pause(item) {
  if (item.busy) return;
  item.controller?.dispose(false); item.controller = null;
  if (item.driver) {
    item.driver.pause();
    item.root.dataset.voltReady = '0';
    report(item, 'Paused offscreen · Rust state retained');
  }
  item.gpu = false; active.delete(item); record();
}
async function start(item) {
  item.busy = true; item.root.dataset.voltReady = '0';
  active.add(item); record();
  const passive = item.root.dataset.voltMode === 'directory';
  try {
    modulePromise ||= import('./pkg/volt_web_catalog.js').then(async module => { await module.default(); return module; });
    const module = await modulePromise;
    if (stopped || !item.visible) { active.delete(item); return; }
    const gpu = !forcedSoftware && !passive && [...active].filter(i => i.gpu).length < MAX_GPU;
    item.gpu = gpu; record();
    let canvas = freshCanvas(item);
    const page = item.root.dataset.voltPage, section = item.root.dataset.voltSection || '';
    async function open(useGpu) {
      if (item.driver) await item.driver.resume(canvas, useGpu);
      else item.driver = await module.CatalogPreview.mount(canvas, useGpu, mode.matches, page, section, accent());
    }
    try { await open(gpu); }
    catch (error) {
      if (!gpu) throw error;
      item.gpu = false; canvas = freshCanvas(item); await open(false); record();
    }
    if (stopped || !item.visible) { item.busy = false; pause(item); return; }
    item.driver.appearance(mode.matches, accent());
    item.root.dataset.voltReady = '0';
    item.controller = attachVolt(item.stage, canvas, item.driver, {
      passive, virtualWidth: passive ? 1000 : 0,
      onStatus(text, error, snapshot) {
        report(item, error ? text : `${snapshot?.backend || text} · original Rust Catalog`, error);
        if (snapshot) {
          item.root.dataset.voltFrames = String(snapshot.frames);
          item.root.dataset.voltBackend = snapshot.backend;
          item.root.dataset.voltReady = '1';
          item.rendered = true;
          if (passive) queueMicrotask(() => {
            if (item.busy || !item.controller) return;
            // Directory cards are actual runtime draws, not PNGs. They retain
            // their canvas pixels, not a device or a second WASM instance.
            item.controller.dispose(false); item.controller = null;
            item.driver.free(); item.driver = null;
            active.delete(item); item.gpu = false; record(); pump();
          });
        }
      },
    });
    report(item, item.gpu ? 'Vello / WebGPU' : 'Rust software / Canvas2D');
  } catch (error) {
    item.failed = true;
    report(item, `Live component unavailable: ${String(error)}`, true);
    active.delete(item); item.driver?.free(); item.driver = null;
  } finally {
    item.busy = false; record();
  }
}
async function pump() {
  if (pumping || stopped) return;
  pumping = true;
  try {
    for (const item of items) {
      if (active.size >= MAX_ACTIVE) break;
      if (item.visible && !item.busy && !item.failed && !active.has(item) && (!item.rendered || item.root.dataset.voltMode !== 'directory')) await start(item);
    }
  } finally { pumping = false; }
}
const observer = new IntersectionObserver(entries => {
  for (const entry of entries) {
    const item = items.find(i => i.stage === entry.target);
    item.visible = entry.isIntersecting && !document.hidden;
    if (!item.visible) pause(item);
  }
  pump();
}, { threshold: 0.01 });
for (const item of items) {
  const height = Number(item.root.dataset.voltHeight || 320);
  item.stage.style.height = `${Math.max(160, Math.min(760, height))}px`;
  observer.observe(item.stage);
}
function appearance() {
  for (const item of items) {
    if (item.busy) continue;
    if (item.driver) { item.driver.appearance(mode.matches, accent()); item.controller?.invalidate(); }
    else item.rendered = false;
  }
  pump();
}
mode.addEventListener('change', appearance);
const accentInputs = [...document.querySelectorAll('input[name="accent"]')];
for (const input of accentInputs) input.addEventListener('change', appearance);
document.addEventListener('visibilitychange', () => {
  for (const item of items) {
    const r = item.stage.getBoundingClientRect();
    item.visible = !document.hidden && r.bottom > 0 && r.top < innerHeight;
    if (!item.visible) pause(item);
  }
  if (!document.hidden) pump();
});
window.addEventListener('pagehide', () => {
  stopped = true; observer.disconnect(); mode.removeEventListener('change', appearance);
  for (const input of accentInputs) input.removeEventListener('change', appearance);
  for (const item of items) { item.controller?.dispose(false); item.controller = null; if (!item.busy) item.driver?.free(); item.driver = null; }
});
