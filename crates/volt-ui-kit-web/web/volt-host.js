import {wireTextProxy} from './text-input.js';
// Browser/platform glue only. Rust owns every component, callback and pixel.
export function attachVolt(root, canvas, driver, { onStatus = () => {}, passive = false, virtualWidth = 0 } = {}) {
  const abort = new AbortController();
  const semantics = root.querySelector('.volt-semantics');
  const elements = new Map();
  const proxies = new Map();
  const nodes = new Map();
  let raf = 0, disposed = false, visible = true, dirty = true, retries = 0;
  let lastRustFocus = null, pointerFocus = false;
  let viewScale = 1;
  semantics.setAttribute('aria-hidden', String(passive));
  let state = JSON.parse(driver.snapshot());
  const listen = (target, type, fn, options = {}) => target.addEventListener(type, fn, { ...options, signal: abort.signal });
  function schedule() {
    if (!raf && !disposed && visible && !document.hidden) raf = requestAnimationFrame(frame);
  }
  function send(event) {
    if (disposed) return false;
    try {
      const changed = driver.dispatch(JSON.stringify(event));
      if (changed) { dirty = true; retries = 0; schedule(); }
      return changed;
    } catch (error) { onStatus(`Preview error: ${String(error)}`, true); return false; }
  }
  function sync() {
    state = JSON.parse(driver.snapshot());
    const alive = new Set();
    const focused = state.nodes.find(n => n.focused)?.id || null;
    const transferFocus = pointerFocus || focused !== lastRustFocus;
    const parents = [];
    for (const [index, n] of (passive ? [] : state.nodes.entries())) {
      const id=n.id; // Listeners retain an identity, never an old value snapshot.
      const [x, y, w, h] = n.bounds.map(v => v * viewScale);
      const [cx, cy, cw, ch] = n.clip.map(v => v * viewScale);
      const left = Math.max(0, x, cx), top = Math.max(0, y, cy);
      const right = Math.min(state.width * viewScale, x + w, cx + cw), bottom = Math.min(state.height * viewScale, y + h, cy + ch);
      const onscreen = right > left && bottom > top;
      const editable = n.role === 'textbox';
      nodes.set(n.id,n);
      const kind = editable ? (n.multiline ? 'textarea' : 'input') : (n.numeric ? 'input' : (n.clickable ? 'button' : 'div'));
      let el = elements.get(n.id);
      if (el && el.localName !== kind) { proxies.get(n.id)?.cancel();proxies.delete(n.id);if('value' in el)el.value='';el.remove(); elements.delete(n.id); el = null; }
      if (!el) {
        el = document.createElement(kind);
        el.dataset.voltId = n.id;
        if (kind === 'button') el.type = 'button';
        if (n.numeric) el.type = 'range';
        if (editable) {
          el.className = 'volt-text-proxy';
          if(n.password) el.type = 'password';
          el.autocomplete = 'off'; el.spellcheck = false;
          el.maxLength = n.multiline ? 65536 : 8192;
          proxies.set(n.id,wireTextProxy(el,{
            listen,send,node:()=>nodes.get(id),
            clipboard:cut=>driver.clipboard(cut),
            invalidate:()=>{dirty=true;schedule();},
          }));
        } else {
          el.className = 'volt-access-node';
          if (n.numeric) listen(el, 'input', () => send({ op: 'number', id, value: Number(el.value) }));
          else if (n.clickable) listen(el, 'click', () => send({ op: 'activate', id }));
          listen(el, 'keydown', e => {
            if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End'].includes(e.key) && ['tab', 'radio', 'option', 'treeitem', 'button', 'row'].includes(el.getAttribute('role'))) {
              if(send({ op: 'arrow', key: e.key })) { e.preventDefault(); e.stopPropagation(); }
            } else if (el.dataset.voltScroll && ['ArrowUp','ArrowDown','ArrowLeft','ArrowRight','PageUp','PageDown','Home','End'].includes(e.key)) {
              const [x,y,w,h] = JSON.parse(el.dataset.voltBounds);
              const delta = { ArrowUp:[0,-32], ArrowDown:[0,32], ArrowLeft:[-32,0], ArrowRight:[32,0], PageUp:[0,-h], PageDown:[0,h], Home:[0,-16384], End:[0,16384] }[e.key];
              if (send({op:'wheel', x:x+w/2, y:y+h/2, dx:delta[0], dy:delta[1]})) { e.preventDefault(); e.stopPropagation(); }
            } else if (e.key === 'Escape') { e.preventDefault(); send({ op: 'escape' }); }
          });
        }
        listen(el, 'focus', () => send({ op: 'focus', id }));
        elements.set(n.id, el);
      }
      alive.add(n.id);
      el.dataset.voltBounds = JSON.stringify(n.bounds);
      if (n.scroll) el.dataset.voltScroll = JSON.stringify(n.scroll);
      parents[index] = el;
      // Keep meaningful grouping/order for the accessibility tree, while field
      // proxies use viewport-relative positioning in the overlay root.
      const parent = editable ? semantics : (parents[n.parent] || semantics);
      if (el.parentElement !== parent) parent.append(el);
      if (n.role === 'text') el.removeAttribute('role'); else el.setAttribute('role', n.role);
      if (n.label && n.role !== 'text') el.setAttribute('aria-label', n.label); else el.removeAttribute('aria-label');
      for (const [name, value] of [['checked', n.checked], ['selected', n.selected], ['expanded', n.expanded]]) {
        if (value === null) el.removeAttribute(`aria-${name}`); else el.setAttribute(`aria-${name}`, String(value));
      }
      el.setAttribute('aria-disabled', String(!n.enabled));
      el.hidden = !onscreen;
      if (editable) {
        if(el.localName==='input')el.type=n.password?'password':'text';
        el.readOnly = n.readOnly || !n.enabled;
        el.disabled = !n.enabled;
        el.placeholder = n.placeholder || '';
        proxies.get(n.id)?.sync(n);
        if(n.password || n.code) {
          // A write-only IME/keyboard sink at the painted caret. No password or
          // virtualized code document is copied back into the DOM editor.
          const caret=n.focused && state.caret ? state.caret.map(v=>v*viewScale) : [x,y,2,18];
          el.style.left=`${caret[0]}px`;el.style.top=`${caret[1]}px`;el.style.width='2px';el.style.height=`${caret[3]}px`;
          el.style.pointerEvents='none';el.style.clipPath='none';el.style.padding='0';
        } else {
        el.style.pointerEvents='auto';el.style.padding='';
        el.style.clipPath = `inset(${Math.max(0, top-y)}px ${Math.max(0,x+w-right)}px ${Math.max(0,y+h-bottom)}px ${Math.max(0,left-x)}px)`;
        el.style.left = `${x}px`; el.style.top = `${y}px`; el.style.width = `${w}px`; el.style.height = `${h}px`;
        }
      } else {
        el.tabIndex = onscreen && n.enabled && (n.clickable || n.numeric || n.scroll) ? 0 : -1;
        if (n.numeric) {
          const [value, min, max, step] = n.numeric;
          Object.assign(el, { value: String(value), min: String(min), max: String(max), step: String(step) });
        } else if (!n.clickable && n.role === 'text') el.textContent = n.label;
      }
      if (transferFocus && n.focused && onscreen && document.activeElement !== el && (n.clickable || editable || n.numeric)) {
        el.focus({ preventScroll: true });
      }
    }
    for (const [id, el] of elements) if (!alive.has(id)) { proxies.get(id)?.cancel();proxies.delete(id);nodes.delete(id);el.value='';el.remove(); elements.delete(id); }
    lastRustFocus = focused; pointerFocus = false;
    onStatus(state.backend, false, state);
  }
  function frame(ms) {
    raf = 0;
    if (disposed || !visible || document.hidden) return;
    try {
      if (dirty || state.animating) {
        const retry = driver.frame(ms);
        if (disposed) return;
        dirty = retry && ++retries <= 3;
        sync();
        if (retry && retries > 3) onStatus('Surface busy; interact to retry.', true);
      }
      if (dirty || state.animating) schedule();
    } catch (error) { dirty = false; onStatus(`Rendering stopped: ${String(error)}`, true); }
  }
  function point(e) { const r = canvas.getBoundingClientRect(); return { x: (e.clientX - r.left) / viewScale, y: (e.clientY - r.top) / viewScale }; }
  if (!passive) {
  listen(canvas, 'contextmenu', e => { if(send({ op: 'context', ...point(e) })) { e.preventDefault(); e.stopPropagation(); } });
  listen(canvas, 'pointerdown', e => {
    if (e.button !== 0) return;
    e.preventDefault(); pointerFocus = true; canvas.setPointerCapture(e.pointerId); send({ op: 'down', ...point(e) });
  });
  listen(canvas, 'pointermove', e => send({ op: 'move', ...point(e) }));
  listen(canvas, 'pointerup', e => { if (e.button === 0) { send({ op: 'up', ...point(e) }); if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId); } });
  listen(canvas, 'pointercancel', () => send({ op: 'cancel' }));
  listen(canvas, 'lostpointercapture', () => send({ op: 'cancel' }));
  listen(canvas, 'pointerleave', () => send({ op: 'leave' }));
  listen(root, 'wheel', e => {
    const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? state.height : 1;
    const changed = send({ op: 'wheel', ...point(e), dx: Math.max(-4096, Math.min(4096, e.deltaX * unit)), dy: Math.max(-4096, Math.min(4096, e.deltaY * unit)) });
    if (changed) e.preventDefault(); // At the scroll boundary, let the page scroll.
  }, { passive: false });
  }
  function resizeView() {
    const r = { width: root.clientWidth, height: root.clientHeight };
    viewScale = virtualWidth > 0 ? Math.max(0.05, r.width / virtualWidth) : 1;
    send({ op: 'resize', width: Math.max(1, r.width / viewScale), height: Math.max(1, r.height / viewScale), scale: Math.max(0.25, Math.min(2, (devicePixelRatio || 1) * viewScale)) });
  }
  const resize = new ResizeObserver(resizeView);
  listen(window, 'resize', resizeView);
  resize.observe(root);
  const intersect = new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
    send({ op: 'suspend', value: !visible || document.hidden });
    if (visible) { resizeView(); dirty = true; schedule(); } else if (raf) { cancelAnimationFrame(raf); raf = 0; }
  });
  intersect.observe(root);
  listen(document, 'visibilitychange', () => {
    send({ op: 'suspend', value: document.hidden || !visible });
    if (!document.hidden && visible) { resizeView(); dirty = true; schedule(); } else if (raf) { cancelAnimationFrame(raf); raf = 0; }
  });
  const motion = matchMedia('(prefers-reduced-motion: reduce)');
  listen(motion, 'change', () => send({ op: 'motion', reduced: motion.matches }));
  resizeView(); // Size before the first paint, including on WebKit.
  send({ op: 'motion', reduced: motion.matches });
  schedule();
  return { snapshot: () => state, invalidate() { dirty = true; schedule(); }, dispose(free = true) {
    if (disposed) return;
    disposed = true; abort.abort(); resize.disconnect(); intersect.disconnect();
    if (raf) cancelAnimationFrame(raf);
    for(const [id,el] of elements) {proxies.get(id)?.cancel();if('value' in el)el.value='';}
    proxies.clear();nodes.clear();elements.clear(); semantics.replaceChildren();
    // WebKit may deliver pagehide reentrantly during a browser call made by a
    // Rust frame. Wait for that synchronous borrow to unwind before ownership
    // is released; disposing twice still schedules only one release.
    if (free) queueMicrotask(() => driver.free());
  } };
}
