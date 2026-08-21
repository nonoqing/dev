import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const [html, css, ui] = await Promise.all([
  readFile(join(root, 'source/index.html'), 'utf8'),
  readFile(join(root, 'source/style.css'), 'utf8'),
  readFile(join(root, 'source/ui.js'), 'utf8'),
]);

const port = Number(process.env.CAT_MINIAPP_PREVIEW_PORT || 4178);

function previewDocument(url) {
  const locale = url.searchParams.get('locale') === 'en-US' ? 'en-US' : 'zh-CN';
  const theme = url.searchParams.get('theme') === 'dark' ? 'dark' : 'light';
  const mock = `
    <script>
      document.documentElement.setAttribute('data-bf-appearance-mode', ${JSON.stringify(theme)});
      document.documentElement.lang = ${JSON.stringify(locale)};
      window.app = {
        locale: ${JSON.stringify(locale)},
        storage: {
          async get(key) { return JSON.parse(localStorage.getItem(key) || 'null'); },
          async set(key, value) { localStorage.setItem(key, JSON.stringify(value)); }
        },
        onLocaleChange(callback) { window.__previewLocaleCallback = callback; }
      };
    </script>`;
  return html
    .replace('</head>', `<style>${css}</style>${mock}</head>`)
    .replace('</body>', `<script type="module">${ui}</script></body>`);
}

createServer((request, response) => {
  const url = new URL(request.url || '/', `http://${request.headers.host || `127.0.0.1:${port}`}`);
  if (url.pathname === '/' || url.pathname === '/index.html') {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' });
    response.end(previewDocument(url));
    return;
  }
  response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
  response.end('Not found');
}).listen(port, '127.0.0.1', () => {
  process.stdout.write(`Cat MiniApp preview: http://127.0.0.1:${port}/\n`);
});
