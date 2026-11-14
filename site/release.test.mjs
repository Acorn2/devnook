import test from 'node:test';
import assert from 'node:assert/strict';
import { renderPage } from './template.mjs';

const artifact = {
  version: '0.1.0',
  arch: 'aarch64',
  minMacOS: '12.0',
  published: false,
  url: null,
  dmgBytes: 5_000_000,
  appBytes: 10_000_000,
  sha256: 'a'.repeat(64),
  signing: 'ad-hoc',
  notarized: false,
};

test('unpublished installers offer source builds in both languages', () => {
  for (const locale of ['zh', 'en']) {
    const html = renderPage(locale, artifact);
    assert.doesNotMatch(html, /href="[^"]*releases\/download\//);
    assert.doesNotMatch(html, / download>/);
    assert.match(html, /href="https:\/\/github.com\/Acorn2\/devnook\/blob\/main\/README/);
    assert.match(html, /5\.00 MB/);
    assert.match(html, locale === 'zh' ? /尚未公开发布/ : /not published yet/);
    assert.match(
      html,
      locale === 'zh' ? /尚未完成 Developer ID/ : /has not completed Developer ID/,
    );
  }
});

test('published installers use exact release assets and retain their signing status', () => {
  const url =
    'https://github.com/Acorn2/devnook/releases/download/v0.1.0/DevNook_0.1.0_aarch64.dmg';
  const html = renderPage('en', { ...artifact, published: true, url });
  assert.ok(html.includes(`href="${url}" download`));
  assert.match(html, /has not completed Developer ID/);
  assert.doesNotMatch(html, /Developer ID signed and Apple notarized/);
});

test('incomplete, cross-repository, and contradictory publication metadata blocks a build', () => {
  for (const changes of [
    { published: true },
    { published: true, url: 'https://example.com/app.dmg' },
    { published: true, url: 'https://github.com/Acorn2/other/releases/download/v0.1.0/app.dmg' },
    { sha256: null },
    { dmgBytes: -1 },
    { notarized: true },
    { url: 'https://github.com/Acorn2/devnook/releases' },
  ])
    assert.throws(() => renderPage('en', { ...artifact, ...changes }));
});
