import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve, join, extname, sep } from 'node:path';
import { basePath } from '../site/content.mjs';

const root = resolve(fileURLToPath(new URL('../dist/site', import.meta.url)));
const port = Number(process.env.PORT || 4310);
if (!Number.isInteger(port) || port < 1024 || port > 65535) throw new Error('Invalid preview port');
const types = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.mp4': 'video/mp4',
  '.vtt': 'text/vtt; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.xml': 'application/xml',
  '.txt': 'text/plain; charset=utf-8',
};
const server = createServer(async (request, response) => {
  try {
    if (!['GET', 'HEAD'].includes(request.method)) {
      response.writeHead(405);
      response.end();
      return;
    }
    const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    if (pathname === '/') {
      response.writeHead(302, { Location: basePath });
      response.end();
      return;
    }
    if (!pathname.startsWith(basePath)) {
      response.writeHead(404);
      response.end('Not found');
      return;
    }
    let path = resolve(root, pathname.slice(basePath.length));
    if (path !== root && !path.startsWith(root + sep)) {
      response.writeHead(403);
      response.end();
      return;
    }
    if ((await stat(path)).isDirectory()) path = join(path, 'index.html');
    let body = await readFile(path);
    const extension = extname(path);
    const headers = {
      'Content-Type': types[extension] || 'application/octet-stream',
      'Cache-Control': 'no-store',
      'X-Content-Type-Options': 'nosniff',
    };
    let status = 200;
    if (extension === '.mp4') {
      headers['Accept-Ranges'] = 'bytes';
      const range = /^bytes=(\d*)-(\d*)$/.exec(request.headers.range || '');
      if (range && (range[1] || range[2])) {
        const size = body.length;
        const start = range[1] ? Number(range[1]) : Math.max(0, size - Number(range[2]));
        const end = range[1] && range[2] ? Math.min(Number(range[2]), size - 1) : size - 1;
        if (start > end || start >= size) {
          response.writeHead(416, { ...headers, 'Content-Range': `bytes */${size}` });
          response.end();
          return;
        }
        status = 206;
        headers['Content-Range'] = `bytes ${start}-${end}/${size}`;
        body = body.subarray(start, end + 1);
      }
    }
    response.writeHead(status, { ...headers, 'Content-Length': body.length });
    response.end(request.method === 'HEAD' ? undefined : body);
  } catch {
    response.writeHead(404);
    response.end('Not found');
  }
});
server.listen(port, '127.0.0.1', () =>
  console.log(`DevNook website preview: http://127.0.0.1:${port}${basePath}`),
);
