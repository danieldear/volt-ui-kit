import { attachVolt } from './volt-host.js';
const root = document.querySelector('#demo');
const status = document.querySelector('#status');
const backend = document.querySelector('#backend');
const selector = document.querySelector('#renderer');
const params = new URLSearchParams(location.search);
const catalogPage = params.get('catalog');
const catalogSection = params.get('section') || '';
const initialPage = ({'text-field': 1, 'scroll-area': 2})[params.get('embed')] || 0;
if (params.has('embed')) document.documentElement.dataset.embed = 'true';
if (params.get('renderer') === 'software') selector.value = 'software';
let controller, generation = 0, runtime;
function report(message, error = false, snapshot) {
  status.textContent = message; status.dataset.error = String(error);
  if (snapshot) { backend.textContent = `${snapshot.backend} · ${Math.round(snapshot.width)} × ${Math.round(snapshot.height)}`; root.dataset.frames = snapshot.frames; root.dataset.backend = snapshot.backend; }
}
async function mount() {
  const token = ++generation;
  controller?.dispose(); controller = null;
  root.dataset.frames = '0'; root.dataset.backend = '';
  let canvas = root.querySelector('canvas');
  const fresh = canvas.cloneNode(false); canvas.replaceWith(fresh); canvas = fresh;
  report('Loading the Rust runtime…');
  try {
    runtime ||= import('./pkg/volt_web_catalog.js').then(async module => { await module.default(); return module; });
    const module = await runtime;
    if (token !== generation) return;
    let demo;
    const dark = matchMedia('(prefers-color-scheme: dark)').matches;
    async function create(gpu) {
      return catalogPage ? module.CatalogPreview.mount(canvas,gpu,dark,catalogPage,catalogSection,0) : module.CatalogDemo.mount(canvas,gpu,dark,initialPage);
    }
    if (selector.value === 'gpu') {
      try { demo = await create(true); }
      catch (error) {
        if (token !== generation) return;
        const fallback = canvas.cloneNode(false); canvas.replaceWith(fallback); canvas = fallback;
        report(`WebGPU unavailable (${String(error)}). Using software rendering.`);
        demo = await create(false);
      }
    } else demo = await create(false);
    if (token !== generation) { demo.free(); return; }
    controller = attachVolt(root, canvas, demo, { onStatus: report });
  } catch (error) { runtime = null; report(`Live preview could not start: ${String(error)}. Build it with python3 scripts/build_web.py, or use the native Catalog.`, true); }
}
selector.addEventListener('change', mount);
document.querySelector('#restart').addEventListener('click', mount);
window.addEventListener('pagehide', () => { generation++; controller?.dispose(); controller = null; });
// Lazy initialization, not one renderer/device for every offscreen docs card.
const observer = new IntersectionObserver(([entry]) => {
  if (entry.isIntersecting) { observer.disconnect(); mount(); }
});
observer.observe(root);
