import { content, repo, origin, basePath } from './content.mjs';
import { validateRelease, architectures } from './release.mjs';

export function escapeHtml(value) {
  return String(value).replace(
    /[&<>"']/g,
    (character) =>
      ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character],
  );
}

const arrow = '<span aria-hidden="true">↗</span>';
const moon =
  '<svg class="moon-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M20.4 14.3A8.4 8.4 0 0 1 9.7 3.6 8.5 8.5 0 1 0 20.4 14.3Z"/></svg><svg class="sun-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><circle cx="12" cy="12" r="3.8"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M4.9 4.9l1.4 1.4m11.4 11.4 1.4 1.4m0-14.2-1.4 1.4M6.3 17.7l-1.4 1.4"/></svg>';
const config = JSON.stringify(
  { mcpServers: { devnook: { command: '/absolute/path/to/devnook', args: ['mcp', 'serve'] } } },
  null,
  2,
);

export function renderPage(locale, release) {
  const t = content[locale];
  if (!t) throw new Error('Unknown website language');
  validateRelease(release);
  const e = escapeHtml;
  const prefix = locale === 'zh' ? './' : '../';
  const pageUrl = `${origin}${basePath}${locale === 'en' ? 'en/' : ''}`;
  const readme = `${repo}/blob/main/${locale === 'zh' ? 'README.md' : 'README.en.md'}`;
  const sourceUrl = `${readme}#${locale === 'zh' ? '快速开始' : 'quick-start'}`;
  const screenshot = (feature) =>
    `<figure class="feature-preview">${feature.shots ? `<div class="preview-switch feature-switch" role="group" aria-label="${e(t.screenshotViews)}" hidden>${feature.shots.map((shot, index) => `<button type="button" data-shot-choice="${e(shot.scene)}" data-shot-alt="${e(shot.alt)}" data-shot-caption="${e(shot.caption)}" aria-pressed="${index === 0}">${e(shot.label)}</button>`).join('')}</div>` : ''}<a data-image-link data-scene="${feature.scene}" href="${prefix}assets/${feature.scene}-dark-${locale}.png" target="_blank" rel="noopener" aria-label="${e(t.previewZoom)}"><img data-theme-image="${feature.scene}" data-prefix="${prefix}" src="${prefix}assets/${feature.scene}-dark-${locale}.png" width="1280" height="800" loading="lazy" alt="${e(feature.alt)}"></a><figcaption><span data-shot-caption>${e(feature.caption)}</span><a class="preview-full" data-image-link data-scene="${feature.scene}" href="${prefix}assets/${feature.scene}-dark-${locale}.png" target="_blank" rel="noopener" aria-label="${e(t.previewZoom)}">${e(t.previewFull)} ${arrow}</a></figcaption></figure>`;
  const mb = (bytes) => `${(bytes / 1_000_000).toFixed(2)} MB`;
  const renderBuild = (build) => {
    const hasArtifact = build.dmgBytes !== null;
    const label = architectures[build.arch].label;
    const status = build.published
      ? t.releasePublished
      : hasArtifact
        ? t.releaseLocal
        : t.releasePending;
    return `<div class="release-build" data-arch="${build.arch}"><div class="release-build-heading"><h3>${label}</h3><p class="release-state">${e(status)}</p></div>${hasArtifact ? `<dl class="release-sizes"><div><dt>${e(t.appSize)}</dt><dd>${mb(build.appBytes)}</dd></div><div><dt>${e(t.dmgSize)}</dt><dd>${mb(build.dmgBytes)}</dd></div></dl>` : ''}<a class="button primary release-action" href="${e(build.published ? build.url : sourceUrl)}"${build.published ? ' download' : ''}>${e(build.published ? t.download.replace('macOS', label) : t.buildSource)} <span aria-hidden="true">↓</span></a>${!build.published ? `<p class="release-note">${e(t.pendingNote)}</p>` : ''}<p class="release-note">${e(build.signing === 'developer-id' && build.notarized ? t.signedNote : t.unsignedNote)}</p>${hasArtifact ? `<details class="checksum"><summary>${e(t.hash)}</summary><code>${e(build.sha256)}</code></details>` : ''}</div>`;
  };
  return `<!doctype html>
<html lang="${t.lang}" data-locale="${locale}" data-theme="dark">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="light dark">
  <meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'none'">
  <title>${e(t.title)}</title>
  <meta name="description" content="${e(t.description)}">
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="DevNook">
  <meta property="og:title" content="${e(t.title)}">
  <meta property="og:description" content="${e(t.description)}">
  <meta property="og:url" content="${pageUrl}">
  <meta property="og:image" content="${origin}${basePath}assets/projects-dark-${locale}.png">
  <meta name="twitter:card" content="summary_large_image">
  <link rel="canonical" href="${pageUrl}">
  <link rel="alternate" hreflang="zh-CN" href="${origin}${basePath}">
  <link rel="alternate" hreflang="en" href="${origin}${basePath}en/">
  <link rel="alternate" hreflang="x-default" href="${origin}${basePath}">
  <link rel="icon" type="image/svg+xml" href="${prefix}assets/devnook.svg">
  <script src="${prefix}theme.js"></script>
  <link rel="stylesheet" href="${prefix}style.css">
  <script src="${prefix}main.js" defer></script>
</head>
<body>
  <a class="skip-link" href="#content">${e(t.skip)}</a>
  <header class="site-header container">
    <a class="brand" href="${prefix}" aria-label="DevNook"><img src="${prefix}assets/devnook.svg" width="36" height="36" alt=""><span>DevNook<span class="brand-dot">.</span></span></a>
    <nav id="navigation" aria-label="${e(t.menu)}"><a href="#features">${e(t.nav[0])}</a><a href="#workflow">${e(t.nav[1])}</a><a href="#questions">${e(t.nav[2])}</a><a href="${repo}">GitHub ${arrow}</a></nav>
    <div class="header-actions">
      <a class="language-link" href="${locale === 'zh' ? 'en/' : '../'}" lang="${locale === 'zh' ? 'en' : 'zh-CN'}" hreflang="${locale === 'zh' ? 'en' : 'zh-CN'}" aria-label="${e(t.language)}">${e(t.languageText)}</a>
      <button class="icon-button" id="theme-toggle" type="button" aria-label="${e(t.theme[0])}" data-label-light="${e(t.theme[0])}" data-label-dark="${e(t.theme[1])}">${moon}</button>
      <a class="header-start" href="#download">${e(t.start)} <span aria-hidden="true">↓</span></a>
      <button class="icon-button menu-toggle" id="menu-toggle" type="button" aria-expanded="false" aria-controls="navigation" aria-label="${e(t.menu)}"><span></span><span></span></button>
    </div>
  </header>
  <main id="content">
    <div class="hero-stage">
      <section class="hero container" aria-labelledby="hero-title">
        <div class="hero-copy"><p class="eyebrow"><span class="eyebrow-mark" aria-hidden="true">⌘</span>${e(t.eyebrow)}</p><h1 id="hero-title">${t.heading.map((line) => `<span class="heading-line">${e(line)}</span>`).join('')}</h1><div class="hero-intro"><p>${e(t.intro)}</p><div class="hero-actions"><a class="button primary" href="#download">${e(t.start)} <span aria-hidden="true">↓</span></a><a class="text-link" href="${repo}">${e(t.source)} ${arrow}</a></div><p class="hero-meta">${e(t.meta)}</p></div></div>
        <figure class="product-preview" aria-label="${e(t.preview)}">
      <div class="preview-bar"><span class="preview-label"><span class="window-marks" aria-hidden="true"><i></i><i></i><i></i></span>DEVNOOK / ${locale === 'zh' ? '工作台' : 'WORKSPACE'}</span><div class="preview-switch" role="group" aria-label="${e(t.preview)}"><button type="button" data-preview="light" aria-pressed="false">${e(t.previewThemes[0])}</button><button type="button" data-preview="dark" aria-pressed="true">${e(t.previewThemes[1])}</button></div></div>
      <div class="preview-screen"><a id="app-preview-link" data-image-link data-scene="projects" href="${prefix}assets/projects-dark-${locale}.png" target="_blank" rel="noopener" aria-label="${e(t.previewZoom)}"><img id="app-preview" data-theme-image="projects" src="${prefix}assets/projects-dark-${locale}.png" width="1280" height="800" alt="${e(t.previewAlt)}" fetchpriority="high" data-prefix="${prefix}"></a></div>
      <figcaption><span>${e(t.previewNote)}</span><span class="preview-links"><span class="preview-spec">1280 × 800</span><a class="preview-full" data-image-link data-scene="projects" href="${prefix}assets/projects-dark-${locale}.png" target="_blank" rel="noopener" aria-label="${e(t.previewZoom)}">${e(t.previewFull)} ${arrow}</a></span></figcaption>
        </figure>
      </section>
      <nav class="task-strip container" aria-label="${e(t.featuresEyebrow)}">${t.taskLinks.map(([href, title, body]) => `<a href="${href}"><strong>${e(title)} <span aria-hidden="true">↗</span></strong><span>${e(body)}</span></a>`).join('')}</nav>
    </div>
    <section class="features-section container section" id="features" aria-labelledby="features-title">
      <div class="section-heading"><p class="eyebrow">${e(t.featuresEyebrow)}</p><h2 id="features-title">${e(t.featuresTitle)}</h2><p>${e(t.featuresIntro)}</p></div>
      <div class="feature-stories">${t.features.map((feature) => `<article class="feature-story" id="${feature.id}" aria-labelledby="${feature.id}-title"><div class="feature-copy"><p class="eyebrow">${e(feature.label)}</p><h3 id="${feature.id}-title">${e(feature.title).replaceAll('\n', '<br>')}</h3><p>${e(feature.body)}</p><ul>${feature.bullets.map((item) => `<li>${e(item)}</li>`).join('')}</ul><p class="feature-note">${e(feature.note)}</p></div>${screenshot(feature)}</article>`).join('')}</div>
    </section>
    <section class="workflow container section" id="workflow" aria-labelledby="workflow-title">
      <div class="section-heading"><p class="eyebrow">${e(t.workflowEyebrow)}</p><h2 id="workflow-title">${e(t.workflowTitle)}</h2><p>${e(t.workflowIntro)}</p></div>
      <div class="workflow-grid">${t.steps.map((step) => `<article class="workflow-step"><div class="step-top"><span>${step.number}</span><span>${step.label}</span></div><h3>${e(step.title)}</h3><p>${e(step.body)}</p><p class="step-flow">${e(step.detail)}</p><ul class="tags">${step.tags.map((tag) => `<li>${e(tag)}</li>`).join('')}</ul></article>`).join('')}</div>
    </section>
    <section class="local-section container" aria-labelledby="local-title"><h2 id="local-title">${e(t.localTitle)}</h2><div class="local-grid">${t.localItems.map(([title, body]) => `<article><h3><span class="small-cross" aria-hidden="true">+</span>${e(title)}</h3><p>${e(body)}</p></article>`).join('')}</div></section>
    <section class="agents-section" id="agents" aria-labelledby="agents-title"><div class="container agents-grid"><div><p class="eyebrow">${e(t.agentsEyebrow)}</p><h2 id="agents-title">${t.agentsTitle.map((line) => `<span class="heading-line">${e(line)}</span>`).join(' ')}</h2><p class="agents-body">${e(t.agentsBody)}</p><p class="agents-note">${e(t.agentsNote)}</p><a class="text-link" href="${repo}/blob/main/docs/PHASE-3.md#mcp-接入">${e(t.agentsLink)} ${arrow}</a></div><div class="integration-examples"><div class="cli-example"><p class="eyebrow">${e(t.cliLabel)}</p><pre><code>pnpm build:cli
./target/debug/devnook project list
./target/debug/devnook service list
./target/debug/devnook ports</code></pre><p>${e(t.cliNote)}</p></div><details class="mcp-example"><summary>${e(t.mcpSummary)}</summary><div class="code-panel"><div class="code-header"><span>${e(t.codeLabel)}</span><button id="copy-config" type="button" data-copy="${e(t.copy)}" data-copied="${e(t.copied)}" data-error="${e(t.copyFailed)}">${e(t.copy)}</button></div><pre><code id="mcp-config">${e(config)}</code></pre><p>${e(t.codeNote)}</p><span id="copy-status" class="sr-only" role="status" aria-live="polite"></span></div></details></div></div></section>
    <section class="questions container section" id="questions" aria-labelledby="questions-title"><div class="section-heading"><p class="eyebrow">${e(t.faqEyebrow)}</p><h2 id="questions-title">${e(t.faqTitle)}</h2></div><div class="faq-list">${t.faqs.map(([question, answer]) => `<details><summary>${e(question)}<span aria-hidden="true">+</span></summary><p>${e(answer)}</p></details>`).join('')}</div></section>
    <section class="download-section container" id="download" aria-labelledby="download-title"><div class="download-copy"><p class="eyebrow">${e(t.downloadEyebrow)}</p><h2 id="download-title">${e(t.downloadTitle).replace('\n', '<br>')}</h2><p>${e(t.downloadIntro)}</p><p class="requirements">${e(t.requirements)}</p></div><div class="release-panel"><p class="release-state">macOS <span class="version">v${e(release.version)}</span></p>${release.builds.map(renderBuild).join('')}<a class="release-history" href="${repo}/releases">${e(t.releases)} ${arrow}</a></div></section>
    <noscript><p class="container no-script">${e(t.noScript)}</p></noscript>
  </main>
  <footer class="site-footer container"><div><a class="brand" href="${prefix}"><img src="${prefix}assets/devnook.svg" width="32" height="32" alt=""><span>DevNook<span class="brand-dot">.</span></span></a><p>${e(t.footer)}</p></div><nav aria-label="Footer"><a href="${readme}">${e(t.docs)}</a><a href="${repo}/issues">${e(t.issues)}</a><a href="${repo}/blob/main/LICENSE">${e(t.license)}</a><a href="https://hreshhao.com/">${e(t.author)} ${arrow}</a></nav><p class="copyright">© 2026 Hresh · DevNook</p></footer>
</body>
</html>\n`;
}
