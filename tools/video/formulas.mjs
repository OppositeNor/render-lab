// Renders an investigation's LaTeX formulas with KaTeX into transparent PNGs
// that the lab draws at the top of its readout panel, one CSS pixel per pixel
// at 1080p. A formula whose smallest glyph would be under `minFontPx`, or that
// is wider than `maxWidth`, fails the build.
//
// Usage: node tools/video/formulas.mjs JOBS.json
// JOBS.json: {"nodeModules", "fontPx", "minFontPx", "maxWidth", "colors": {"text", "dim"}, "metrics",
//             "jobs": [{"key", "latex", "dim", "output"}]}
// `dim` draws the formula in the dim colour, so that its lit term stands out.
// Paths are absolute. KaTeX and Puppeteer come from `nodeModules` (tools/video/setup.sh installs them).
import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';

const spec = JSON.parse(await fs.readFile(process.argv[2], 'utf8'));
const modules = path.resolve(spec.nodeModules);
const require = createRequire(path.join(modules, 'resolver.cjs'));
const {default: puppeteer} = await import(pathToFileURL(require.resolve('puppeteer')).href);
const PAD = 4;
const types = {'.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.woff2': 'font/woff2', '.woff': 'font/woff', '.ttf': 'font/ttf'};
// Serves only KaTeX's files, from node_modules, to the page.
const server = http.createServer(async (req, res) => {
  try {
    const pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    if (pathname === '/') { res.writeHead(200, {'Content-Type': 'text/html'}); res.end('<!doctype html><html><head><link rel="stylesheet" href="/node_modules/katex/dist/katex.min.css"></head><body></body></html>'); return; }
    const resolved = path.resolve(modules, '.' + pathname.replace(/^\/node_modules/, ''));
    if (!resolved.startsWith(modules + path.sep)) { res.writeHead(403); res.end(); return; }
    const bytes = await fs.readFile(resolved);
    res.writeHead(200, {'Content-Type': types[path.extname(resolved)] || 'application/octet-stream'});
    res.end(bytes);
  } catch { res.writeHead(404); res.end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const address = `http://127.0.0.1:${server.address().port}`;
let browser;
const metrics = [];
try {
  browser = await puppeteer.launch({headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu'], ...(process.env.CHROME_PATH ? {executablePath: process.env.CHROME_PATH} : {})});
  const page = await browser.newPage();
  await page.setViewport({width: 1600, height: 900, deviceScaleFactor: 1});
  await page.goto(address, {waitUntil: 'networkidle0'});
  await page.addStyleTag({content: `html,body{margin:0;background:transparent}.formula{position:absolute;left:0;top:0;padding:${PAD}px;font-size:${spec.fontPx}px;white-space:nowrap}.katex-display{margin:0}`});
  await page.evaluate(async () => {
    const {default: katex} = await import('/node_modules/katex/dist/katex.mjs');
    window.formulaEngine = katex;
  });
  for (const job of spec.jobs) {
    const result = await page.evaluate(async ({latex, color}) => {
      document.body.innerHTML = '<div class="formula"></div>';
      const element = document.querySelector('.formula');
      element.style.color = color;
      window.formulaEngine.render(latex, element, {displayMode: true, throwOnError: true, strict: 'error', output: 'html'});
      await document.fonts.ready;
      // Every glyph's size: the computed font size of each element holding text
      // (KaTeX's zero-width spaces, which keep its vertical lists apart, draw nothing).
      const sizes = [...element.querySelectorAll('.katex-html *')]
        .filter(e => [...e.childNodes].some(n => n.nodeType === Node.TEXT_NODE && n.textContent.replace(/[\s\u200b]/g, '')))
        .map(e => parseFloat(getComputedStyle(e).fontSize));
      const inner = element.querySelector('.katex-html').getBoundingClientRect();
      const box = element.getBoundingClientRect();
      return {minFontPx: Math.min(...sizes), contentWidth: inner.width, x: box.x, y: box.y, width: box.width, height: box.height};
    }, {latex: job.latex, color: job.dim ? spec.colors.dim : spec.colors.text});
    if (result.minFontPx < spec.minFontPx - 0.05) throw new Error(`${job.key}: its smallest glyph would be ${result.minFontPx.toFixed(1)} px (< ${spec.minFontPx}); avoid nested scripts`);
    const width = Math.ceil(result.contentWidth) + 2 * PAD;
    if (width > spec.maxWidth) throw new Error(`${job.key}: ${width} px wide (> ${spec.maxWidth}); break it over more lines`);
    const height = Math.ceil(result.height);
    await fs.mkdir(path.dirname(job.output), {recursive: true});
    await page.screenshot({path: job.output, omitBackground: true, clip: {x: 0, y: 0, width, height}});
    metrics.push({key: job.key, width, height, minFontPx: result.minFontPx});
    console.log(`Formula ${metrics.length}/${spec.jobs.length}: ${job.key} (${width} x ${height}, smallest glyph ${result.minFontPx.toFixed(1)} px)`);
  }
  await fs.writeFile(spec.metrics, JSON.stringify(metrics, null, 2) + '\n');
} finally {
  if (browser) await browser.close();
  await new Promise(resolve => server.close(resolve));
}
