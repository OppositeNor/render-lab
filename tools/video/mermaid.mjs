// Renders a video's Mermaid diagrams into PNG panels that the lab composites
// into the frame's content area (1840 x 728 at 1080p). A diagram whose labels
// would render below 24 px fails the build.
//
// Usage: node tools/video/mermaid.mjs JOBS.json
// JOBS.json: {"width", "height", "nodeModules", "fontFamily"?, "metrics"?, "jobs": [{"id", "diagram", "focus": [...], "output"}]}
// Paths are absolute. Mermaid and Puppeteer come from `nodeModules` (tools/video/setup.sh installs them).
import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';

const spec = JSON.parse(await fs.readFile(process.argv[2], 'utf8'));
const modules = path.resolve(spec.nodeModules);
const require = createRequire(path.join(modules, 'resolver.cjs'));
const {default: puppeteer} = await import(pathToFileURL(require.resolve('puppeteer')).href);
const {width, height} = spec;
const fontFamily = spec.fontFamily || "'DejaVu Sans'";
const MIN_LABEL_PX = 24;
const BASE_FONT_PX = 27;
// Serves only Mermaid's files, from node_modules, to the page.
const server = http.createServer(async (req, res) => {
  try {
    const pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    if (pathname === '/') { res.writeHead(200, {'Content-Type': 'text/html'}); res.end('<!doctype html><html><body></body></html>'); return; }
    const resolved = path.resolve(modules, '.' + pathname.replace(/^\/node_modules/, ''));
    if (!resolved.startsWith(modules + path.sep)) { res.writeHead(403); res.end(); return; }
    const bytes = await fs.readFile(resolved);
    res.writeHead(200, {'Content-Type': resolved.endsWith('.mjs') || resolved.endsWith('.js') ? 'text/javascript' : 'application/octet-stream'});
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
  await page.setViewport({width, height, deviceScaleFactor: 1});
  await page.goto(address);
  await page.addStyleTag({content: `
    *{box-sizing:border-box}html,body{margin:0;width:${width}px;height:${height}px;overflow:hidden;background:#07121f;font-family:${fontFamily},sans-serif}
    .panel{position:absolute;inset:0;border:1px solid #274155;border-radius:14px;background:#0b1b2d;display:flex;align-items:center;justify-content:center}
    .diagram svg{overflow:visible}
    .nodeLabel{line-height:1.32!important}.edgeLabel{background:#0b1b2d!important;color:#b9d1e4!important}
    .active rect,.active polygon,.active circle,.active path{filter:drop-shadow(0 0 9px #55ddcf55)}
  `});
  await page.evaluate(async ({fontSize, fontFamily}) => {
    const {default: mermaid} = await import('/node_modules/mermaid/dist/mermaid.esm.min.mjs');
    mermaid.initialize({startOnLoad: false, securityLevel: 'strict', theme: 'base', fontFamily,
      themeVariables: {fontFamily, fontSize: fontSize + 'px', primaryColor: '#152d42', primaryTextColor: '#ecf5ff', primaryBorderColor: '#52738f', lineColor: '#7897af', secondaryColor: '#1c344b', tertiaryColor: '#13283c', edgeLabelBackground: '#0b1b2d', clusterBkg: '#102335', clusterBorder: '#375269'},
      flowchart: {htmlLabels: true, useMaxWidth: false, curve: 'basis', nodeSpacing: 30, rankSpacing: 46, padding: 16},
    });
    window.diagramEngine = mermaid;
  }, {fontSize: BASE_FONT_PX, fontFamily});
  for (const job of spec.jobs) {
    const graph = await fs.readFile(job.diagram, 'utf8');
    const focus = job.focus.length ? `\nclassDef active fill:#145a59,stroke:#64f5df,stroke-width:4px,color:#ffffff,font-weight:bold;\nclass ${job.focus.join(',')} active;\n` : '';
    const result = await page.evaluate(async ({definition, id, width, height}) => {
      document.body.innerHTML = '<div class="panel"><div class="diagram"></div></div>';
      const {svg} = await window.diagramEngine.render('d_' + id.replaceAll('-', '_'), definition);
      const container = document.querySelector('.diagram');
      container.innerHTML = svg;
      await document.fonts.ready;
      const element = container.querySelector('svg');
      const box = element.viewBox.baseVal;
      const scale = Math.min((width - 70) / box.width, (height - 70) / box.height, 1.25);
      element.removeAttribute('style');
      element.setAttribute('width', String(box.width * scale));
      element.setAttribute('height', String(box.height * scale));
      const labels = [...element.querySelectorAll('.nodeLabel')].map(n => n.getBoundingClientRect().height);
      return {scale, minLabelHeight: Math.min(...labels)};
    }, {definition: graph + focus, id: job.id, width, height});
    const fontPixels = BASE_FONT_PX * result.scale;
    if (fontPixels < MIN_LABEL_PX) throw new Error(`${job.id}: labels would be ${fontPixels.toFixed(1)} px (< ${MIN_LABEL_PX}); simplify the diagram`);
    await fs.mkdir(path.dirname(job.output), {recursive: true});
    await page.screenshot({path: job.output, omitBackground: false});
    metrics.push({id: job.id, scale: result.scale, fontPixels, minLabelHeight: result.minLabelHeight});
    console.log(`Diagram ${metrics.length}/${spec.jobs.length}: ${job.id} (${fontPixels.toFixed(1)} px labels)`);
  }
  if (spec.metrics) await fs.writeFile(spec.metrics, JSON.stringify(metrics, null, 2) + '\n');
} finally {
  if (browser) await browser.close();
  await new Promise(resolve => server.close(resolve));
}
