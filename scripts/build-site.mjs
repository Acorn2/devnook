import { mkdir, readFile, writeFile, cp } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve, join } from 'node:path';
import { renderPage, validateRelease } from '../site/template.mjs';
import { origin, basePath } from '../site/content.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const source = join(root, 'site');
const output = resolve(root, 'dist/site');
const release = validateRelease(JSON.parse(await readFile(join(source, 'release.json'), 'utf8')));
const packageJson = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
if (release.version !== packageJson.version)
  throw new Error('Website and desktop versions must match');
await mkdir(join(output, 'en'), { recursive: true });
for (const filename of ['style.css', 'main.js', 'theme.js'])
  await cp(join(source, filename), join(output, filename));
await cp(join(source, 'assets'), join(output, 'assets'), { recursive: true });
await writeFile(join(output, 'index.html'), renderPage('zh', release));
await writeFile(join(output, 'en/index.html'), renderPage('en', release));
await writeFile(join(output, '.nojekyll'), '');
await writeFile(
  join(output, 'robots.txt'),
  `User-agent: *\nAllow: /\nSitemap: ${origin}${basePath}sitemap.xml\n`,
);
await writeFile(
  join(output, 'sitemap.xml'),
  `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>${origin}${basePath}</loc></url><url><loc>${origin}${basePath}en/</loc></url></urlset>\n`,
);
console.log(`Built Chinese and English GitHub Pages site: ${output}`);
