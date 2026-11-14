import { readFile, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve, join, dirname, sep } from 'node:path';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

const root = fileURLToPath(new URL('../dist/site', import.meta.url));
for (const file of ['main.js', 'theme.js']) {
  execFileSync(process.execPath, ['--check', join(root, file)], { stdio: 'inherit' });
}
let count = 0;
for (const relative of ['index.html', 'en/index.html']) {
  const path = join(root, relative);
  const html = await readFile(path, 'utf8');
  assert.match(html, relative.startsWith('en') ? /<html lang="en"/ : /<html lang="zh-CN"/);
  const ids = [...html.matchAll(/\sid="([^"]+)"/g)].map((match) => match[1]);
  assert.equal(new Set(ids).size, ids.length, 'Duplicate element IDs');
  assert.equal((html.match(/<h1\b/g) || []).length, 1);
  for (const [, attribute, value] of html.matchAll(/\s(href|src)="([^"]+)"/g)) {
    if (/^https?:/.test(value)) continue;
    if (value.startsWith('#')) {
      assert.ok(ids.includes(value.slice(1)), `Missing anchor: ${value}`);
      count++;
      continue;
    }
    let asset = resolve(dirname(path), value);
    assert.ok(
      asset.startsWith(resolve(root) + sep) || asset === resolve(root),
      'Asset escapes site',
    );
    if ((await stat(asset)).isDirectory()) asset = join(asset, 'index.html');
    assert.ok((await stat(asset)).isFile(), `Missing ${attribute}: ${value}`);
    count++;
  }
  assert.doesNotMatch(html, /\/Users\/|\.tmp\/|localhost:|127\.0\.0\.1/);
}
console.log(`Checked two static language routes, headings, and ${count} local links / assets.`);
