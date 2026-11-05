/* Real-browser coverage for the original Catalog declarations and direct docs. */
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium,webkit} = require('playwright');
const origin=process.env.VOLT_WEB_URL || 'http://127.0.0.1:8768';
const output=path.resolve(__dirname,'../target/web-qa');fs.mkdirSync(output,{recursive:true});
const catalog=JSON.parse(fs.readFileSync(path.resolve(__dirname,'../docs/assets/catalog-data.json')));
async function run(type,options,label) {
 const browser=await type.launch(options);const page=await browser.newPage({viewport:{width:1280,height:1000}});
 const errors=[];page.on('pageerror',e=>{errors.push(String(e));console.error('browser error',e)});page.on('console',m=>{if(m.type()==='error'){errors.push(m.text());console.error('browser console',m.text())}});
 async function pointer(root,locator,button='left') {
   const b=JSON.parse(await locator.getAttribute('data-volt-bounds'));const rect=await root.locator('canvas').boundingBox();
   await page.mouse.click(rect.x+b[0]+b[2]/2,rect.y+b[1]+b[3]/2,{button});
 }
 async function ready(root){await root.scrollIntoViewIfNeeded();await root.waitFor();await page.waitForFunction(el=>el.dataset.voltReady==='1',await root.elementHandle());}
 try {
  await page.goto(`${origin}/live/?renderer=software`);
  const sections=catalog.flatMap(p=>p.sections.map(s=>({page:p.slug,id:s.id}))).filter(s=>!process.env.VOLT_SECTION || s.page===process.env.VOLT_SECTION);
  await page.exposeFunction('catalogProgress', message=>console.log(`${label}: ${message}`));
  const results=await page.evaluate(async sections=>{
   const module=await import('./pkg/volt_web_catalog.js');await module.default();const results=[];
   for(const section of sections){
    await window.catalogProgress(`render ${section.page}/${section.id}`);
    const canvas=document.createElement('canvas');document.body.append(canvas);
    const driver=await Promise.race([module.CatalogPreview.mount(canvas,false,false,section.page,section.id,0),new Promise((_,reject)=>setTimeout(()=>reject(new Error(`Mount timed out: ${section.page}/${section.id}`)),15000))]);
    try {
     driver.dispatch(JSON.stringify({op:'resize',width:1000,height:600,scale:1}));driver.frame(performance.now());
     const snapshot=JSON.parse(driver.snapshot());
     if(!snapshot.nodes.length)throw new Error(`Empty semantics: ${section.page}/${section.id}`);
     const pixels=canvas.getContext('2d').getImageData(0,0,1000,600).data;
     let different=false;for(let i=4;i<pixels.length;i+=4)if(pixels[i]!==pixels[0]||pixels[i+1]!==pixels[1]||pixels[i+2]!==pixels[2]){different=true;break;}
     if(!different)throw new Error(`Blank pixels: ${section.page}/${section.id}`);
     results.push(`${section.page}/${section.id}`);
    } finally{driver.free();canvas.remove();}
    await new Promise(resolve=>setTimeout(resolve,0));
   }
   return results;
  },sections);
  assert.equal(results.length,sections.length);
  console.log(`${label}: all ${results.length} original Catalog sections rendered nonblank browser pixels.`);
  // Navigation can be delivered reentrantly by WebKit during a WASM/browser
  // call. Teardown must not free the driver while frame() still borrows it.
  await page.evaluate(async()=>{
   const {attachVolt}=await import('./volt-host.js');
   const root=document.createElement('div');root.style.width='100px';root.style.height='100px';
   root.innerHTML='<canvas></canvas><div class="volt-semantics"></div>';document.body.append(root);root.scrollIntoView();
   let controller,borrowed=false,freed=0;
   const driver={
    snapshot:()=>JSON.stringify({nodes:[],width:100,height:100,animating:false,frames:0,backend:'test'}),
    dispatch:()=>true,
    frame:()=>{borrowed=true;controller.dispose();controller.dispose();borrowed=false;return false;},
    free:()=>{if(borrowed)throw new Error('Freed a borrowed driver');freed++;},
   };
   controller=attachVolt(root,root.querySelector('canvas'),driver);
   await new Promise(resolve=>setTimeout(resolve,100));root.remove();
   if(freed!==1)throw new Error(`Expected one deferred free, got ${freed}`);
  });
  await page.goto(`${origin}/index.html`);
  assert.equal(await page.locator('img[src*="images/catalog"],img[src*="images/theming"],iframe').count(),0);
  const dashboard=page.locator('[data-volt-page="dashboard"]');await ready(dashboard);
  if(label==='chromium')await page.screenshot({path:path.join(output,'landing-live-dashboard.png'),caret:'initial'});
  const counter=page.locator('[data-volt-page="introduction"][data-volt-section="counter"]');await ready(counter);
  await pointer(counter,counter.getByRole('button',{name:'Click me',exact:true}));
  await counter.getByText('Clicked 1 time',{exact:true}).waitFor();
  const plan=page.locator('[data-volt-page="introduction"][data-volt-section="plan"]');await ready(plan);
  await page.locator('#accent-green').check();
  await ready(counter);
  await counter.getByText('Clicked 1 time',{exact:true}).waitFor();
  if(label==='chromium')await page.screenshot({path:path.join(output,'landing-direct-rust.png'),caret:'initial'});
  await page.goto(`${origin}/reference/index.html?renderer=software`);
  const cards=page.locator('[data-volt-mode="directory"]');assert.equal(await cards.count(),49);
  for(let i=0;i<await cards.count();i++)await ready(cards.nth(i));
  assert.equal(await page.locator('.volt-status[data-error="true"]').count(),0);
  assert.equal(await page.locator('img[src*="images/components"]').count(),0);
  assert.ok(Number(await page.locator('html').getAttribute('data-volt-active'))<=8);
  assert.ok(Number(await page.locator('html').getAttribute('data-volt-gpu'))<=2);
  await page.goto(`${origin}/reference/menu.html`);
  const menu=page.locator('[data-volt-page="menu"]');await ready(menu);
  await pointer(menu,menu.getByRole('button',{name:'Options',exact:true}));
  await menu.getByRole('button',{name:'Duplicate',exact:true}).waitFor();
  await pointer(menu,menu.getByRole('button',{name:'Duplicate',exact:true}));
  await menu.getByText('Last action: Duplicate',{exact:true}).waitFor();
  await page.goto(`${origin}/reference/context-menu.html`);
  const context=page.locator('[data-volt-page="context-menu"]');await ready(context);
  const box=await context.locator('canvas').boundingBox();await page.mouse.click(box.x+box.width/2,box.y+box.height/2,{button:'right'});
  await context.getByRole('button',{name:'Reload',exact:true}).waitFor();
  await pointer(context,context.getByRole('button',{name:'Reload',exact:true}));
  await context.getByText('Last action: Reload',{exact:true}).waitFor();
  await page.goto(`${origin}/reference/table.html`);
  const table=page.locator('[data-volt-section="table"]');await ready(table);
  await pointer(table,table.locator('[data-volt-id="inv-5"]'));
  await table.getByText(/INV006 selected/).waitFor();
  const header=table.getByRole('columnheader').filter({hasText:'Invoice'});
  await pointer(table,header);
  await page.waitForFunction(root=>{
   const a=root.querySelector('[data-volt-id="inv-5"]'),b=root.querySelector('[data-volt-id="inv-0"]');
   return a && b && JSON.parse(a.dataset.voltBounds)[1]<JSON.parse(b.dataset.voltBounds)[1];
  },await table.elementHandle());
  const row6=JSON.parse(await table.locator('[data-volt-id="inv-5"]').getAttribute('data-volt-bounds'));
  const row1=JSON.parse(await table.locator('[data-volt-id="inv-0"]').getAttribute('data-volt-bounds'));
  assert.ok(row6[1]<row1[1],'descending Rust sort reorders the actual table rows');
  await page.waitForTimeout(100); // Allow the submitted GPU frame to reach the compositor.
  if(label==='chromium')await page.screenshot({path:path.join(output,'catalog-live-table.png'),caret:'initial'});
  await pointer(table,header);
  await page.waitForFunction(root=>{
   const a=root.querySelector('[data-volt-id="inv-0"]'),b=root.querySelector('[data-volt-id="inv-5"]');
   return a && b && JSON.parse(a.dataset.voltBounds)[1]<JSON.parse(b.dataset.voltBounds)[1];
  },await table.elementHandle());
  await page.goto(`${origin}/reference/date-time.html`);
  const dates=page.locator('[data-volt-section="date-calendar"]');await ready(dates);
  assert.equal(await dates.locator('.volt-status[data-error="true"]').count(),0);
  if(label==='chromium')await page.screenshot({path:path.join(output,'catalog-direct-calendar.png'),fullPage:true,caret:'initial'});
  assert.deepEqual(errors,[]);
  console.log(`${label}: landing counter/state retention, live accent, 49 directory canvases, resource limits, original menu/context callbacks, table sorting/selection and calendar passed.`);
 }catch(e){console.error(label,e,errors);throw e;}finally{await browser.close();}
}
(async()=>{await run(chromium,{channel:process.env.PLAYWRIGHT_CHANNEL || undefined,headless:true},'chromium');if(process.env.VOLT_TEST_WEBKIT==='1')await run(webkit,{headless:true},'webkit');})().catch(e=>{console.error(e);process.exitCode=1});
