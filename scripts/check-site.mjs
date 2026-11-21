import { readFile, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve, join, dirname, sep } from 'node:path';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { demos } from '../site/content.mjs';

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
  const demoCount = Object.keys(demos).length;
  assert.equal((html.match(/<video\b[^>]*\bpreload="none"/g) || []).length, demoCount);
  assert.doesNotMatch(html, /<video\b[^>]*\bautoplay\b/);
  assert.equal(
    (html.match(/<track\b[^>]*\bsrclang="en"/g) || []).length,
    relative.startsWith('en') ? demoCount : 0,
  );
  for (const id of Object.keys(demos)) {
    const story = html.match(
      new RegExp(`<article class="feature-story" id="${id}"[^>]*>(.*?)</article>`, 's'),
    )?.[1];
    assert.ok(story, `Missing feature: ${id}`);
    assert.doesNotMatch(story, /<(img|details)\b/, `Unexpected screenshot or accordion: ${id}`);
    assert.match(story, /<video\b[^>]*\bcontrols\b/);
  }
  for (const [, attribute, value] of html.matchAll(/\s(href|src|poster)="([^"]+)"/g)) {
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
