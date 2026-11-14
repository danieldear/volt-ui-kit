/* Shared Rust gallery workflows in a real browser; no JS component replicas. */
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium, webkit} = require('playwright');
const origin = process.env.VOLT_WEB_URL || 'http://127.0.0.1:8768';
const output = path.resolve(__dirname, '../target/web-qa');
fs.mkdirSync(output, {recursive:true});

async function run(type, options, name) {
 const browser = await type.launch(options);
 const page = await browser.newPage({viewport:{width:1200,height:900}});
 const errors=[];page.on('pageerror',e=>errors.push(String(e)));
 try {
  await page.goto(`${origin}/live/?renderer=software`);
  await page.evaluate(async()=>{
   const m=await import('./pkg/volt_web_catalog.js');await m.default();
   const check=(v,s)=>{if(!v)throw new Error(s)};
   for(const dpi of [1,1.25,2])for(const [p,s] of [
    ['motion-studio','studio-carousel'],['motion-studio','studio-card'],
    ['motion-studio','studio-sheet'],['motion-studio','studio-fab'],
    ['motion-studio','studio-pointer'],['motion-studio','studio-scroll-reveal'],
    ['instruments','instruments-controls'],['image','image-blend'],
   ]) {
    const c=document.createElement('canvas');document.body.append(c);
    const d=await m.CatalogPreview.mount(c,false,false,p,s,0);
    const send=e=>d.dispatch(JSON.stringify(e));
    const state=()=>JSON.parse(d.snapshot());
    const node=id=>state().nodes.find(n=>n.id===id);
    const hasLabel=text=>state().nodes.some(n=>n.label===text);
    const activate=id=>send({op:'activate',id});
    const center=id=>{const b=node(id).bounds;return {x:b[0]+b[2]/2,y:b[1]+b[3]/2}};
    try {
     send({op:'resize',width:700,height:650,scale:dpi});
     send({op:'motion',reduced:false});d.frame(0);
     if(s==='studio-carousel') {
      check(hasLabel('The quiet coast'),'first retained slide visible');
      activate('studio-carousel-control-next');d.frame(1);d.frame(200);d.frame(600);
      check(hasLabel('Beyond the ridge')&&!hasLabel('The quiet coast'),'slide semantics track active page');
      activate('studio-carousel-control-prev');d.frame(610);d.frame(1100);
      check(hasLabel('The quiet coast'),'previous action');
     } else if(s==='studio-card') {
      activate('studio-card-reveal');d.frame(1);d.frame(500);
      check(hasLabel('✓  Battery charged'),'card details mount');
      activate('studio-card-reveal');d.frame(510);d.frame(1100);
      check(!hasLabel('✓  Battery charged'),'removed details not accessible');
     } else if(s==='studio-sheet') {
      activate('studio-sheet-open');d.frame(1);d.frame(500);
      check(!!node('studio-sheet-confirm'),'sheet opens');
      check(!node('studio-sheet-open'),'modal scopes out background');
      activate('studio-sheet-confirm');d.frame(510);d.frame(1100);
      check(!node('studio-sheet-confirm')&&!!node('studio-sheet-open'),'sheet closes');
     } else if(s==='studio-fab') {
      check(!node('studio-speed-dial-action-1'),'closed dial has no actions');
      activate('studio-speed-dial-toggle');d.frame(1);d.frame(500);
      activate('studio-speed-dial-action-1');d.frame(510);d.frame(1100);
      check(hasLabel('Waypoint added')&&!node('studio-speed-dial-action-1'),'typed action closes speed dial');
     } else if(s==='studio-pointer') {
      const card=()=>state().nodes.find(n=>n.label==='Point. Explore. Discover.');
      const before=card().bounds;
      const b=node('studio-pointer-surface').bounds;
      send({op:'move',x:b[0]+b[2]*0.8,y:b[1]+b[3]*0.8});d.frame(1);d.frame(200);
      check(JSON.stringify(card().bounds)!==JSON.stringify(before),'pointer response transforms retained card');
      activate('studio-pointer-reset');d.frame(210);d.frame(500);
     } else if(s==='studio-scroll-reveal') {
      const at=center('studio-reveal-scroller');
      send({op:'wheel',...at,dx:0,dy:300});d.frame(1);d.frame(600);
      check(node('studio-reveal-scroller').scroll[1]>100,'real Rust scroll offset');
      activate('studio-scroll-replay');d.frame(610);d.frame(1200);
     } else if(s==='instruments-controls') {
      activate('instrument-charge');d.frame(1);
      check(hasLabel('77%'),'gauge responds to application value');
      send({op:'number',id:'instrument-temperature',value:24});d.frame(2);
      check(hasLabel('24.0 °C'),'rotary assistive value callback');
      activate('instrument-reset');d.frame(3);check(hasLabel('21.5 °C'),'rotary reset');
     } else {
      activate('image-blend-mode-4');d.frame(1);
      check(!!node('image-blend-mode-4'),'blend mode callback remains mounted');
     }
     d.frame(2500);check(!state().animating,`${s}: finite motion must idle`);
     send({op:'motion',reduced:true});d.frame(3000);
     check(!state().animating,`${s}: reduced motion must idle`);
    } finally {d.free();c.remove()}
   }
  });
  const backends=await page.evaluate(async()=>{
   const m=await import('./pkg/volt_web_catalog.js');await m.default();
   const backends=[];
   for(const [p,s,action] of [
    ['motion-studio','studio-carousel','studio-carousel-control-next'],
    ['motion-studio','studio-sheet','studio-sheet-open'],
    ['instruments','instruments-controls','instrument-charge'],
    ['image','image-mask-operations',null],['image','image-blend','image-blend-mode-4'],
   ]) {
    const c=document.createElement('canvas');document.body.append(c);
    const d=await m.CatalogPreview.mount(c,true,true,p,s,0);
    try {
     d.dispatch(JSON.stringify({op:'resize',width:700,height:650,scale:1.25}));d.frame(0);
     if(action)d.dispatch(JSON.stringify({op:'activate',id:action}));
     for(const t of [1,100,600])d.frame(t);
     backends.push(JSON.parse(d.snapshot()).backend);
    } finally {d.free();c.remove()}
   }
   return backends;
  });
  assert.equal(backends.length,5);
  console.log(`${name}: preferred gallery renderers: ${[...new Set(backends)].join(', ')}`);
  // DOM bridge: actual pointer/keyboard and rendered canvas, not just dispatch.
  await page.goto(`${origin}/reference/motion-studio.html?renderer=software`);
  const root=page.locator('[data-volt-section="studio-carousel"]');
  await root.scrollIntoViewIfNeeded();
  await page.waitForFunction(el=>el.dataset.voltReady==='1',await root.elementHandle());
  const next=root.locator('[data-volt-id="studio-carousel-control-next"]');
  const b=JSON.parse(await next.getAttribute('data-volt-bounds'));
  const r=await root.locator('canvas').boundingBox();
  await page.mouse.click(r.x+b[0]+b[2]/2,r.y+b[1]+b[3]/2);
  await root.getByText('Beyond the ridge',{exact:true}).waitFor();
  await page.waitForTimeout(450);
  if(name==='chromium')await page.screenshot({path:path.join(output,'motion-studio-live.png')});
  await page.goto(`${origin}/reference/instruments.html?renderer=software`);
  const instruments=page.locator('[data-volt-section="instruments-controls"]');
  await instruments.scrollIntoViewIfNeeded();
  await page.waitForFunction(el=>el.dataset.voltReady==='1',await instruments.elementHandle());
  const knob=instruments.locator('[data-volt-id="instrument-temperature"]');
  await knob.press('ArrowUp');await instruments.getByText('22.0 °C',{exact:true}).waitFor();
  if(name==='chromium')await page.screenshot({path:path.join(output,'instruments-live.png')});
  assert.deepEqual(errors,[]);
  console.log(`${name}: gallery callbacks, pointer/keyboard, modal scoping, finite idle and reduced motion passed at 100/125/200% DPI.`);
 } finally {await browser.close()}
}
(async()=>{
 await run(chromium,{channel:process.env.PLAYWRIGHT_CHANNEL||undefined,headless:true},'chromium');
 if(process.env.VOLT_TEST_WEBKIT==='1')await run(webkit,{headless:true},'webkit');
})().catch(e=>{console.error(e);process.exitCode=1});
