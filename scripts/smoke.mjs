// Headless smoke test: boots the game in Chrome, plays a bit, saves screenshots, reports console errors.
// Usage: npm run dev (port 5173) or pass URL: node scripts/smoke.mjs http://localhost:5199/
import { chromium } from 'playwright';

const url = process.argv[2] ?? 'http://localhost:5173/';
const out = process.argv[3] ?? '.';
const browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 });
const errors = [];
page.on('pageerror', e => errors.push('pageerror: ' + e.stack));
page.on('console', m => { if (m.type() === 'error' || m.type() === 'warning') errors.push(`${m.type()}: ${m.text()}`); });
const wait = ms => page.waitForTimeout(ms);
const tap = async (k, ms = 60) => { await page.keyboard.down(k); await wait(ms); await page.keyboard.up(k); await wait(60); };

await page.goto(url);
await wait(1500);
await page.screenshot({ path: `${out}/shot-menu.png` });

// 1P vs CPU
await tap('Enter');
await wait(3600);
await page.screenshot({ path: `${out}/shot-fight-start.png` });
for (let i = 0; i < 12; i++) {
  await page.keyboard.down('KeyD'); await wait(250); await page.keyboard.up('KeyD');
  await tap('KeyJ'); await tap('KeyW'); await tap('KeyJ'); await tap('KeyK');
}
await page.screenshot({ path: `${out}/shot-fight-mid.png` });
await tap('KeyH');
await wait(300);
await page.screenshot({ path: `${out}/shot-fight-debug.png` });
await tap('Escape');
await wait(200);
await page.screenshot({ path: `${out}/shot-pause.png` });
await tap('KeyQ');
await wait(500);

// Training: seed settings directly (King vs dummy), then START
await page.evaluate(() => localStorage.setItem('stick-smash.v2', JSON.stringify({ settings: { mode: 'training', c1: 2, c2: 0, dummy: 'stand' } })));
await page.reload();
await wait(1500);
await page.screenshot({ path: `${out}/shot-menu-training.png` });
await tap('KeyW');
await tap('Enter');
await wait(800);
await page.keyboard.down('KeyD'); await wait(400); await page.keyboard.up('KeyD');
await page.keyboard.down('KeyD'); await page.keyboard.down('KeyJ'); await wait(40); await page.keyboard.up('KeyD');
await wait(250);
await page.screenshot({ path: `${out}/shot-training-charge.png` });
await page.keyboard.up('KeyJ');
await wait(350);
await page.screenshot({ path: `${out}/shot-training-hit.png` });
await wait(900);
await tap('KeyK'); await wait(300); await tap('KeyS', 30); await tap('KeyW'); await wait(200);
await page.keyboard.down('KeyW'); await tap('KeyK'); await page.keyboard.up('KeyW');
await wait(700);
await page.screenshot({ path: `${out}/shot-training-after.png` });

console.log(errors.length ? errors.join('\n') : 'NO ERRORS');
await browser.close();
