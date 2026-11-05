/* Real browser input regression tests. Composition events are synthetic, not
   a claim of physical input-method/device acceptance. */
'use strict';
const assert=require('node:assert/strict');const {chromium,webkit}=require('playwright');
const url=process.env.VOLT_WEB_URL || 'http://127.0.0.1:8768';
async function run(type,options,label){
 const browser=await type.launch(options),page=await browser.newPage({viewport:{width:1100,height:1100}}),errors=[];
 page.on('pageerror',e=>errors.push(String(e)));page.on('console',m=>{if(m.type()==='error')errors.push(m.text())});
 try {
  await page.goto(`${url}/live/?renderer=software`);
  await page.waitForFunction(()=>Number(document.querySelector('#demo').dataset.frames)>0);
  const stale=await page.evaluate(async()=>{
   const {wireTextProxy}=await import('./text-input.js');
   const el=document.createElement('input');el.type='password';document.body.append(el);
   const events=[],node={id:'qa-secret',password:true,enabled:true,focused:true};
   wireTextProxy(el,{listen:(target,event,fn)=>target.addEventListener(event,fn),send:event=>events.push(event),node:()=>node,clipboard:()=>null,invalidate:()=>{}});
   el.focus();el.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));el.value='draft';el.blur();
   el.value='late';el.dispatchEvent(new InputEvent('input',{bubbles:true,inputType:'insertFromComposition'}));
   const result=[el.value,events.filter(e=>e.op==='insert' || (e.op==='composition' && e.phase==='commit')).length];el.remove();return result;
  });assert.deepEqual(stale,['',0]);
  const conversion=await page.evaluate(async()=>{
   const {utf16ToByte,byteToUtf16}=await import('./text-input.js');
   const value='A🌿é東京';return [utf16ToByte(value,1),utf16ToByte(value,2),utf16ToByte(value,3),byteToUtf16(value,5),byteToUtf16(value,2)];
  });assert.deepEqual(conversion,[1,null,5,3,null]);
  await page.getByRole('tab',{name:'Editable form',exact:true}).press('Enter');
  const name=page.getByRole('textbox',{name:'Display name',exact:true});await name.fill('A🌿é東京');
  await name.evaluate(el=>{el.setSelectionRange(1,3);el.dispatchEvent(new Event('select',{bubbles:true}));});
  await page.waitForTimeout(100);assert.deepEqual(await name.evaluate(el=>[el.selectionStart,el.selectionEnd]),[1,3]);
  await page.keyboard.insertText('😀');
  assert.equal(await name.inputValue(),'A😀é東京');
  await name.evaluate(el=>{el.select();el.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));el.value='取消';el.dispatchEvent(new CompositionEvent('compositionupdate',{bubbles:true,data:'取消'}));el.dispatchEvent(new InputEvent('input',{bubbles:true,isComposing:true,inputType:'insertCompositionText'}));el.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true,data:''}));});
  await page.waitForTimeout(100);assert.equal(await name.inputValue(),'A😀é東京');
  const password=page.getByRole('textbox',{name:'Password',exact:true});await password.fill('private🔐value');
  await page.locator('[data-volt-id="password-status"]').filter({hasText:'Password updated in Rust.'}).waitFor();
  assert.equal(await password.getAttribute('type'),'password');assert.equal(await password.inputValue(),'');
  assert.ok(!(await page.locator('.volt-semantics').innerHTML()).includes('private'));
  const copied=await password.evaluate(el=>{const data=new DataTransfer();const event=new ClipboardEvent('copy',{bubbles:true,cancelable:true,clipboardData:data});el.dispatchEvent(event);return [event.defaultPrevented,data.getData('text/plain')];});assert.deepEqual(copied,[true,'']);
  await password.evaluate(el=>{el.focus();el.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));el.value='draft-secret';el.dispatchEvent(new CompositionEvent('compositionupdate',{bubbles:true,data:'draft-secret'}));});
  await name.focus();assert.equal(await password.inputValue(),'');
  assert.ok(!(await page.locator('.volt-semantics').innerHTML()).includes('draft-secret'));
  await page.getByRole('tab',{name:'Editor',exact:true}).press('Enter');
  const code=page.getByRole('textbox',{name:'Rust source editor',exact:true});await code.focus();
  await code.evaluate(el=>{el.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));el.value='東京';el.dispatchEvent(new CompositionEvent('compositionupdate',{bubbles:true,data:'東京'}));el.dispatchEvent(new InputEvent('input',{bubbles:true,isComposing:true,inputType:'insertCompositionText'}));});
  await page.waitForTimeout(100);await page.locator('[data-volt-id="code-edits"]').filter({hasText:'0 committed edits'}).waitFor({state:'attached'});
  await code.evaluate(el=>el.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true,data:'東京'})));
  await page.locator('[data-volt-id="code-edits"]').filter({hasText:'1 committed edits'}).waitFor({state:'attached'});
  assert.equal(await code.inputValue(),'');
  await code.press('Control+a');
  const source=await code.evaluate(el=>{const data=new DataTransfer();el.dispatchEvent(new ClipboardEvent('copy',{bubbles:true,cancelable:true,clipboardData:data}));return data.getData('text/plain');});assert.match(source,/^東京fn main/);
  await code.evaluate(el=>{const data=new DataTransfer();data.setData('text/plain','cat🌿');el.dispatchEvent(new ClipboardEvent('paste',{bubbles:true,cancelable:true,clipboardData:data}));});
  await page.locator('[data-volt-id="code-edits"]').filter({hasText:'2 committed edits'}).waitFor({state:'attached'});
  await code.press('Backspace');await page.locator('[data-volt-id="code-edits"]').filter({hasText:'3 committed edits'}).waitFor({state:'attached'});
  await code.press('Control+z');await page.locator('[data-volt-id="code-edits"]').filter({hasText:'4 committed edits'}).waitFor({state:'attached'});
  await code.press('Control+a');
  const restored=await code.evaluate(el=>{const data=new DataTransfer();el.dispatchEvent(new ClipboardEvent('copy',{bubbles:true,cancelable:true,clipboardData:data}));return data.getData('text/plain');});assert.equal(restored,'cat🌿');
  await page.getByRole('checkbox',{name:'Read-only editor',exact:true}).press('Space');
  await page.waitForFunction(()=>document.querySelector('[data-volt-id="code"]').readOnly);
  await code.focus();await code.press('Backspace');
  await code.evaluate(el=>{const data=new DataTransfer();data.setData('text/plain','blocked');el.dispatchEvent(new ClipboardEvent('paste',{bubbles:true,cancelable:true,clipboardData:data}));});
  await page.waitForTimeout(100);
  assert.match(await page.locator('[data-volt-id="code-edits"]').innerText(),/^4 committed edits$/);
  if(label==='chromium')await page.screenshot({path:'target/web-qa/input-code-editor.png',caret:'initial'});
  assert.deepEqual(errors,[]);console.log(`${label}: UTF-16/UTF-8 selection, composition cancellation/commit, write-only password/clipboard, code IME, paste, Unicode deletion and undo passed.`);
 }finally{await browser.close();}
}
(async()=>{await run(chromium,{channel:process.env.PLAYWRIGHT_CHANNEL||undefined,headless:true},'chromium');if(process.env.VOLT_TEST_WEBKIT==='1')await run(webkit,{headless:true},'webkit');})().catch(e=>{console.error(e);process.exitCode=1});
