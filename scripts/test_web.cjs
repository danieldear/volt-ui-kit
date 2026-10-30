/* Real browser smoke tests. Requires playwright 1.62.1; run against a locally
 * served docs/ directory after scripts/build_web.py. Does not mutate repo data. */
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium, webkit } = require('playwright');
const url = process.env.VOLT_WEB_URL || 'http://127.0.0.1:8768';
const output = path.resolve(__dirname, '../target/web-qa');
fs.mkdirSync(output, { recursive: true });
async function run(browserType, options, label) {
  const browser = await browserType.launch(options);
  const page = await browser.newPage({ viewport: { width: 1100, height: 1000 }, deviceScaleFactor: 1 });
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  // Playwright WebKit's screenshot preparer injects `style { body {} }` to
  // synchronize animations, which violates our intentionally strict CSP.
  // Keep WebKit runtime assertions clean; capture visual proof in Chromium.
  const capture = async file => {
    if (label !== 'webkit') await page.screenshot({ path: path.join(output, file), fullPage: true, caret: 'initial' });
  };
  const waitFrame = () => page.waitForFunction(() => Number(document.querySelector('#demo')?.dataset.frames) > 0 && document.querySelector('#status')?.dataset.error !== 'true');
  async function pointer(locator) {
    const bounds = JSON.parse(await locator.getAttribute('data-volt-bounds'));
    const r = await page.locator('canvas').boundingBox();
    await page.mouse.click(r.x + bounds[0] + bounds[2] / 2, r.y + bounds[1] + bounds[3] / 2);
  }
  try {
    await page.goto(`${url}/live/?renderer=software`);
    await waitFrame();
    assert.match(await page.locator('#backend').innerText(), /software \/ Canvas2D/);
    await pointer(page.getByRole('button', { name: 'Add one', exact: true }));
    await page.locator('[data-volt-id="count"]').filter({ hasText: 'Count: 1' }).waitFor();
    await page.getByRole('button', { name: 'Add one', exact: true }).press('Enter');
    await page.locator('[data-volt-id="count"]').filter({ hasText: 'Count: 2' }).waitFor();
    await pointer(page.getByRole('checkbox', { name: 'Enable notifications' }));
    await page.waitForFunction(() => document.querySelector('[role=checkbox]')?.getAttribute('aria-checked') === 'true');
    await capture(`${label}-01-components.png`);
    await page.getByRole('tab', { name: 'Components', exact: true }).focus();
    await page.keyboard.press('ArrowRight');
    const name = page.getByRole('textbox', { name: 'Display name' });
    await name.waitFor();
    await name.fill('Zoë 東京');
    await pointer(page.getByRole('button', { name: 'Save profile', exact: true }));
    await page.locator('[data-volt-id="saved"]').filter({ hasText: 'Saved: Zoë 東京' }).waitFor();
    // A native draft paints without becoming committed application text.
    await name.evaluate(el => {
      el.focus();el.select();el.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));
      el.value='未確定';el.dispatchEvent(new CompositionEvent('compositionupdate',{bubbles:true,data:'未確定'}));
      el.dispatchEvent(new InputEvent('input',{bubbles:true,isComposing:true,inputType:'insertCompositionText'}));
    });
    assert.match(await page.locator('[data-volt-id="saved"]').innerText(),/Zoë 東京/);
    await name.evaluate(el=>el.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true,data:'未確定'})));
    await pointer(page.getByRole('button',{name:'Save profile',exact:true}));
    await page.locator('[data-volt-id="saved"]').filter({hasText:'Saved: 未確定'}).waitFor();
    await capture(`${label}-02-form.png`);
    await pointer(page.getByRole('tab', { name: 'Scrolling', exact: true }));
    const list = page.locator('[data-volt-id="scroll-list"]');
    await list.waitFor();
    const bounds = JSON.parse(await list.getAttribute('data-volt-bounds'));
    const r = await page.locator('canvas').boundingBox();
    await page.mouse.move(r.x + bounds[0] + bounds[2] / 2, r.y + bounds[1] + bounds[3] / 2);
    await page.mouse.wheel(0, 3000);
    await page.waitForFunction(() => JSON.parse(document.querySelector('[data-volt-id="scroll-list"]').dataset.voltScroll)[1] > 500);
    await list.focus();
    await page.keyboard.press('Home');
    await page.waitForFunction(() => JSON.parse(document.querySelector('[data-volt-id="scroll-list"]').dataset.voltScroll)[1] === 0);
    await page.keyboard.press('End');
    await page.waitForFunction(() => JSON.parse(document.querySelector('[data-volt-id="scroll-list"]').dataset.voltScroll)[1] > 500);
    await pointer(page.getByRole('button', { name: 'Dark mode', exact: true }));
    await page.getByRole('button', { name: 'Light mode', exact: true }).waitFor();
    await capture(`${label}-03-dark-scroll.png`);
    await page.waitForTimeout(600);
    const before = await page.locator('#demo').getAttribute('data-frames');
    await page.waitForTimeout(1000);
    assert.equal(await page.locator('#demo').getAttribute('data-frames'), before, 'idle view should not continuously repaint');
    await page.setViewportSize({ width: 390, height: 900 });
    await page.waitForTimeout(300);
    await capture(`${label}-04-compact.png`);
    await page.setViewportSize({ width: 1100, height: 1000 });
    await page.locator('#restart').click();
    await waitFrame();
    await page.locator('[data-volt-id="count"]').filter({ hasText: 'Count: 0' }).waitFor();
    // Browser/preferred mode must identify its actual renderer, not pretend a
    // CPU fallback was GPU drawing. Hardware availability varies in CI.
    await page.locator('#renderer').selectOption('gpu');
    await waitFrame();
    const mode = await page.locator('#backend').innerText();
    assert.match(mode, /Vello \/ WebGPU|software \/ Canvas2D/);
    await pointer(page.getByRole('button', { name: 'Add one', exact: true }));
    await page.locator('[data-volt-id="count"]').filter({ hasText: 'Count: 1' }).waitFor();
    await capture(`${label}-05-preferred-renderer.png`);
    // The reference now mounts the original Catalog fields directly in the
    // document, not an iframe containing the unrelated starter playground.
    await page.goto(`${url}/reference/text-field.html`);
    const embedded=page.locator('[data-volt-page="text-field"][data-volt-section="tf-label"]');
    await embedded.scrollIntoViewIfNeeded();
    await embedded.locator('[data-volt-id="tf-email"]').fill('invalid-address');
    await embedded.getByText('Enter an email address with an @.',{exact:true}).waitFor();
    await capture(`${label}-06-embedded-reference.png`);
    assert.equal(errors.length, 0, `browser errors: ${errors.join('\n')}`);
    console.log(`${label}: pointer, keyboard tabs, Unicode form, composition, scroll, themes, compact resize, reset, idle and preferred rendering passed (${mode}).`);
  } catch (error) {
    console.error(label, 'status:', await page.locator('#status').innerText({timeout:1000}).catch(() => 'unavailable'), 'backend:', await page.locator('#backend').innerText({timeout:1000}).catch(() => 'unavailable'), 'errors:', errors);
    await capture(`${label}-failure.png`);
    throw error;
  } finally { await browser.close(); }
}
(async () => {
  if (process.env.VOLT_ONLY_WEBKIT !== '1') await run(chromium, { channel: process.env.PLAYWRIGHT_CHANNEL || undefined, headless: true }, 'chromium');
  if (process.env.VOLT_TEST_WEBKIT === '1') await run(webkit, { headless: true }, 'webkit');
})().catch(error => { console.error(error); process.exitCode = 1; });
