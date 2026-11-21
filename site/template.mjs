import { content, demos, repo, origin, basePath } from './content.mjs';
import { validateRelease, architectures } from './release.mjs';

export function escapeHtml(value) {
  return String(value).replace(
    /[&<>"']/g,
    (character) =>
      ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character],
  );
}

const arrow = '<span aria-hidden="true">↗</span>';
const play = '<span class="demo-play" aria-hidden="true">▶</span>';
const downloadIcon =
  '<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 4v11m0 0 4.5-4.5M12 15l-4.5-4.5M5 19h14"/></svg>';
const moon =
  '<svg class="moon-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M20.4 14.3A8.4 8.4 0 0 1 9.7 3.6 8.5 8.5 0 1 0 20.4 14.3Z"/></svg><svg class="sun-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><circle cx="12" cy="12" r="3.8"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M4.9 4.9l1.4 1.4m11.4 11.4 1.4 1.4m0-14.2-1.4 1.4M6.3 17.7l-1.4 1.4"/></svg>';
const boundaryIcons = [
  '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 8V6a2 2 0 0 1 2-2h2M16 4h2a2 2 0 0 1 2 2v2M20 16v2a2 2 0 0 1-2 2h-2M8 20H6a2 2 0 0 1-2-2v-2"/><circle cx="12" cy="12" r="3"/></svg>',
  '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><ellipse cx="12" cy="6" rx="7" ry="3"/><path d="M5 6v6c0 1.7 3.1 3 7 3s7-1.3 7-3V6M5 12v6c0 1.7 3.1 3 7 3s7-1.3 7-3v-6"/></svg>',
  '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3 5 6v5c0 4.4 3 8.4 7 10 4-1.6 7-5.6 7-10V6l-7-3Z"/><path d="m9 12 2 2 4-4"/></svg>',
  '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="M3 9h18M8 4v5"/></svg>',
];
const windowMarks = '<span class="window-marks" aria-hidden="true"><i></i><i></i><i></i></span>';
const config = JSON.stringify(
  { mcpServers: { devnook: { command: '/absolute/path/to/devnook', args: ['mcp', 'serve'] } } },
  null,
  2,
);
const cliLines = [
  'pnpm build:cli',
  './target/debug/devnook project list',
  './target/debug/devnook service list',
  './target/debug/devnook ports',
];

export function renderPage(locale, release) {
  const t = content[locale];
  if (!t) throw new Error('Unknown website language');
  validateRelease(release);
  const e = escapeHtml;
  const prefix = locale === 'zh' ? './' : '../';
  const pageUrl = `${origin}${basePath}${locale === 'en' ? 'en/' : ''}`;
  const readme = `${repo}/blob/main/${locale === 'zh' ? 'README.md' : 'README.en.md'}`;
  const sourceUrl = `${readme}#${locale === 'zh' ? '快速开始' : 'quick-start'}`;
  const headingLine = (line) => {
    const [, text, dot] = /^(.*?)([。.]?)$/.exec(line);
    return `<span class="heading-line">${e(text)}${dot ? `<span class="heading-dot">${e(dot)}</span>` : ''}</span>`;
  };
  const screenshot = (feature) =>
    `<figure class="feature-preview"><a data-image-link data-scene="${feature.scene}" href="${prefix}assets/${feature.scene}-dark-${locale}.png" target="_blank" rel="noopener" aria-label="${e(t.previewZoom)}"><img data-theme-image="${feature.scene}" data-prefix="${prefix}" src="${prefix}assets/${feature.scene}-dark-${locale}.png" width="${feature.imageWidth || 1280}" height="${feature.imageHeight || 800}" loading="lazy" alt="${e(feature.alt)}"></a><figcaption><span>${e(feature.caption)}</span><span class="preview-links"><a class="preview-full" data-image-link data-scene="${feature.scene}" href="${prefix}assets/${feature.scene}-dark-${locale}.png" target="_blank" rel="noopener" aria-label="${e(t.previewZoom)}">${e(t.previewFull)} ${arrow}</a></span></figcaption></figure>`;
  const demoDuration = (id) => e(t.demoDuration.replace('{seconds}', demos[id].seconds));
  const demoLinks = Object.keys(demos)
    .map(
      (id) =>
        `<li><a href="#demo-${id}"><span>${e(t.demoLabels[id])}</span><span class="demo-duration">${demoDuration(id)}</span></a></li>`,
    )
    .join('');
  const featureMedia = (feature) => {
    const demo = demos[feature.id];
    if (!demo) return screenshot(feature);
    const asset = `${prefix}assets/demos/${demo.file}`;
    const label = e(t.demoLabels[feature.id]);
    return `<figure class="feature-demo" id="demo-${feature.id}"><figcaption>${play}<span class="demo-title">${label}</span><span class="demo-duration">${demoDuration(feature.id)}</span></figcaption><video controls playsinline preload="none" width="1600" height="1000" poster="${asset}.jpg" aria-label="${label}"><source src="${asset}.mp4" type="video/mp4">${
      locale === 'en'
        ? `<track kind="captions" src="${asset}.en.vtt" srclang="en" label="English" default>`
        : ''
    }</video><a class="text-link" href="${asset}.mp4">${e(t.demoFile)} ${arrow}</a></figure>`;
  };
  const mb = (bytes) => `${(bytes / 1_000_000).toFixed(2)} MB`;
  const renderBuild = (build) => {
    const hasArtifact = build.dmgBytes !== null;
    const label = architectures[build.arch].label;
    const status = build.published ? t.releasePublished : hasArtifact ? t.releaseLocal : t.releasePending;
    return `<article class="release-build" data-arch="${build.arch}"><div class="release-build-heading"><h3>${label}</h3><p class="release-state">${e(status)}</p></div>${
      hasArtifact
        ? `<dl class="release-sizes"><div><dt>${e(t.appSize)}</dt><dd>${mb(build.appBytes)}</dd></div><div><dt>${e(t.dmgSize)}</dt><dd>${mb(build.dmgBytes)}</dd></div></dl>`
        : ''
    }<a class="button primary release-action" href="${e(build.published ? build.url : sourceUrl)}"${build.published ? ' download' : ''}><span>${e(build.published ? t.download.replace('macOS', label) : t.buildSource)}</span><span class="button-icon">${downloadIcon}</span></a>${
      !build.published ? `<p class="release-note">${e(t.pendingNote)}</p>` : ''
    }<p class="release-note">${e(build.signing === 'developer-id' && build.notarized ? t.signedNote : t.unsignedNote)}</p>${
      hasArtifact
        ? `<details class="checksum"><summary>${e(t.hash)}<span class="summary-plus" aria-hidden="true">+</span></summary><code>${e(build.sha256)}</code></details>`
        : ''
    }</article>`;
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
  <header class="site-header">
    <div class="container header-inner">
      <a class="brand" href="${prefix}" aria-label="DevNook"><img src="${prefix}assets/devnook.svg" width="36" height="36" alt=""><span>DevNook<span class="brand-dot">.</span></span></a>
      <span class="version-tag" aria-hidden="true">v${e(release.version)}</span>
      <nav id="navigation" aria-label="${e(t.menu)}"><a href="#features">${e(t.nav[0])}</a><a href="#workflow">${e(t.nav[1])}</a><a href="#questions">${e(t.nav[2])}</a><a href="${repo}">GitHub ${arrow}</a></nav>
      <div class="header-actions">
        <a class="language-link" href="${locale === 'zh' ? 'en/' : '../'}" lang="${locale === 'zh' ? 'en' : 'zh-CN'}" hreflang="${locale === 'zh' ? 'en' : 'zh-CN'}" aria-label="${e(t.language)}">${e(t.languageText)}</a>
        <button class="icon-button" id="theme-toggle" type="button" aria-label="${e(t.theme[0])}" data-label-light="${e(t.theme[0])}" data-label-dark="${e(t.theme[1])}">${moon}</button>
        <a class="header-start" href="#download">${e(t.start)} ${downloadIcon}</a>
        <button class="icon-button menu-toggle" id="menu-toggle" type="button" aria-expanded="false" aria-controls="navigation" aria-label="${e(t.menu)}"><span></span><span></span></button>
      </div>
    </div>
  </header>
  <main id="content">
    <div class="hero-stage">
      <section class="hero container" aria-labelledby="hero-title">
        <div class="hero-copy">
          <p class="hero-kicker"><span class="pulse-dot" aria-hidden="true"></span><span>${e(t.eyebrow)}</span><a class="kicker-version" href="#download">v${e(release.version)} <span aria-hidden="true">↓</span></a></p>
          <h1 id="hero-title">${t.heading.map(headingLine).join('')}</h1>
          <p class="hero-lead">${e(t.intro)}</p>
          <div class="hero-actions"><a class="button primary" href="#download"><span>${e(t.start)}</span><span class="button-icon">${downloadIcon}</span></a><a class="text-link" href="#demos">${play}${e(t.demoHeading)}</a><a class="text-link" href="${repo}">${e(t.source)} ${arrow}</a></div>
          <ul class="hero-chips">${t.heroChips.map((chip) => `<li>${e(chip)}</li>`).join('')}</ul>
          <p class="hero-meta">${e(t.meta)}</p>
        </div>
        <figure class="product-preview" aria-label="${e(t.preview)}">
          <div class="preview-frame">
            <div class="preview-bar"><span class="preview-label">${windowMarks}<span class="preview-app">DEVNOOK</span><span class="preview-divider" aria-hidden="true">/</span><span>${locale === 'zh' ? '工作台' : 'WORKSPACE'}</span></span><div class="preview-switch" role="group" aria-label="${e(t.preview)}"><button type="button" data-preview="light" aria-pressed="false">${e(t.previewThemes[0])}</button><button type="button" data-preview="dark" aria-pressed="true">${e(t.previewThemes[1])}</button></div></div>
            <div class="preview-screen"><a id="app-preview-link" data-image-link data-scene="projects" href="${prefix}assets/projects-dark-${locale}.png" target="_blank" rel="noopener" aria-label="${e(t.previewZoom)}"><img id="app-preview" data-theme-image="projects" src="${prefix}assets/projects-dark-${locale}.png" width="1280" height="800" alt="${e(t.previewAlt)}" fetchpriority="high" data-prefix="${prefix}"></a></div>
            <figcaption><span>${e(t.previewNote)}</span><span class="preview-links"><span class="preview-spec">1280 × 800</span><a class="preview-full" data-image-link data-scene="projects" href="${prefix}assets/projects-dark-${locale}.png" target="_blank" rel="noopener" aria-label="${e(t.previewZoom)}">${e(t.previewFull)} ${arrow}</a></span></figcaption>
          </div>
          <div class="preview-float" aria-hidden="true"><span class="float-icon">${boundaryIcons[1]}</span><div><strong>${e(t.floatTitle)}</strong><span>${e(t.floatBody)}</span></div></div>
        </figure>
      </section>
      <nav class="task-strip container" aria-label="${e(t.featuresEyebrow)}">${t.taskLinks.map(([href, title, body], index) => `<a class="task-card" href="${href}"><span class="task-index">0${index + 1}</span><span class="task-copy"><strong>${e(title)}</strong><span>${e(body)}</span></span><span class="task-arrow" aria-hidden="true">↗</span></a>`).join('')}</nav>
    </div>
    <section class="features-section container section" id="features" aria-labelledby="features-title">
      <div class="section-heading split"><div><p class="eyebrow">${e(t.featuresEyebrow)}</p><h2 id="features-title">${e(t.featuresTitle)}</h2></div><p class="section-lead">${e(t.featuresIntro)}</p></div>
      <nav class="demo-index" id="demos" aria-labelledby="demos-title"><div><h3 id="demos-title">${e(t.demoHeading)}</h3><p>${e(t.demoIntro)}</p></div><ul>${demoLinks}</ul></nav>
      <nav class="feature-index" aria-label="${e(t.featuresEyebrow)}">${t.features.map((feature, index) => `<a href="#${feature.id}" data-feature-link="${feature.id}"><span aria-hidden="true">0${index + 1}</span>${e(t.featureNav[index])}</a>`).join('')}</nav>
      <div class="feature-stories">${t.features.map((feature) => `<article class="feature-story" id="${feature.id}" aria-labelledby="${feature.id}-title"><div class="feature-copy"><p class="eyebrow"><span class="eyebrow-dot" aria-hidden="true"></span>${e(feature.label)}</p><h3 id="${feature.id}-title">${e(feature.title).replaceAll('\n', '<br>')}</h3><p>${e(feature.body)}</p><ul class="feature-points">${feature.bullets.map((item) => `<li>${e(item)}</li>`).join('')}</ul><p class="feature-note">${e(feature.note)}</p></div>${featureMedia(feature)}</article>`).join('')}</div>
    </section>
    <section class="workflow container section" id="workflow" aria-labelledby="workflow-title">
      <div class="section-heading split"><div><p class="eyebrow">${e(t.workflowEyebrow)}</p><h2 id="workflow-title">${e(t.workflowTitle)}</h2></div><p class="section-lead">${e(t.workflowIntro)}</p></div>
      <div class="workflow-grid">${t.steps.map((step) => `<article class="workflow-step"><div class="step-top"><span class="step-number">${step.number}</span><span class="step-label">${e(step.label)}</span></div><h3>${e(step.title)}</h3><p>${e(step.body)}</p><p class="step-flow">${e(step.detail)}</p><ul class="tags">${step.tags.map((tag) => `<li>${e(tag)}</li>`).join('')}</ul></article>`).join('')}</div>
    </section>
    <section class="boundary-section" aria-labelledby="boundary-title"><div class="container"><div class="boundary-heading"><p class="eyebrow">${e(t.boundaryEyebrow)}</p><h2 id="boundary-title">${e(t.localTitle)}</h2></div><div class="boundary-grid">${t.localItems.map(([title, body], index) => `<article class="boundary-card"><span class="boundary-icon">${boundaryIcons[index]}</span><h3>${e(title)}</h3><p>${e(body)}</p></article>`).join('')}</div></div></section>
    <section class="agents-section" id="agents" aria-labelledby="agents-title"><div class="container agents-grid"><div class="agents-copy"><p class="eyebrow">${e(t.agentsEyebrow)}</p><h2 id="agents-title">${t.agentsTitle.map(headingLine).join(' ')}</h2><p class="agents-body">${e(t.agentsBody)}</p><p class="agents-note">${e(t.agentsNote)}</p><p class="agents-badge"><span class="pulse-dot" aria-hidden="true"></span>${e(t.readonlyBadge)}</p><a class="text-link" href="${repo}/blob/main/docs/PHASE-3.md#mcp-接入">${e(t.agentsLink)} ${arrow}</a></div><div class="integration-examples"><div class="terminal"><div class="terminal-bar">${windowMarks}<span class="terminal-title">CLI / ${locale === 'zh' ? '终端' : 'TERMINAL'}</span><span class="terminal-badge">${e(t.readonlyBadge)}</span></div><pre><code>${cliLines.map((line) => `<span class="terminal-line"><span class="prompt" aria-hidden="true">$</span> ${e(line)}</span>`).join('\n')}</code></pre><p class="terminal-note">${e(t.cliNote)}</p></div><details class="mcp-example"><summary><span>${e(t.mcpSummary)}</span><span class="summary-plus" aria-hidden="true">+</span></summary><div class="code-panel"><div class="code-header"><span>${e(t.codeLabel)}</span><button id="copy-config" type="button" data-copy="${e(t.copy)}" data-copied="${e(t.copied)}" data-error="${e(t.copyFailed)}">${e(t.copy)}</button></div><pre><code id="mcp-config">${e(config)}</code></pre><p>${e(t.codeNote)}</p><span id="copy-status" class="sr-only" role="status" aria-live="polite"></span></div></details></div></div></section>
    <section class="questions container section" id="questions" aria-labelledby="questions-title"><div class="faq-heading"><p class="eyebrow">${e(t.faqEyebrow)}</p><h2 id="questions-title">${e(t.faqTitle)}</h2><p class="faq-more">${e(t.faqMore)}<a href="${repo}/issues">${e(t.faqMoreLink)} ${arrow}</a></p></div><div class="faq-list">${t.faqs.map(([question, answer]) => `<details><summary><span>${e(question)}</span><span class="summary-plus" aria-hidden="true">+</span></summary><p>${e(answer)}</p></details>`).join('')}</div></section>
    <section class="download-section" id="download" aria-labelledby="download-title"><div class="container"><div class="download-panel"><div class="download-copy"><p class="eyebrow">${e(t.downloadEyebrow)}</p><h2 id="download-title">${e(t.downloadTitle).replace('\n', '<br>')}</h2><p>${e(t.downloadIntro)}</p><ul class="download-chips"><li>${e(t.localItems[1][0])}</li><li>${e(t.localItems[0][0])}</li><li>MIT License</li></ul><p class="requirements">${e(t.requirements)}</p></div><div class="release-panel"><div class="release-head"><span class="release-platform">macOS</span><span class="version">v${e(release.version)}</span></div><div class="release-builds">${release.builds.map(renderBuild).join('')}</div><a class="release-history" href="${repo}/releases">${e(t.releases)} ${arrow}</a></div></div></div></section>
    <noscript><p class="container no-script">${e(t.noScript)}</p></noscript>
  </main>
  <footer class="site-footer">
    <div class="container footer-top"><div><a class="brand" href="${prefix}"><img src="${prefix}assets/devnook.svg" width="32" height="32" alt=""><span>DevNook<span class="brand-dot">.</span></span></a><p>${e(t.footer)}</p></div><nav aria-label="Footer"><a href="${readme}">${e(t.docs)}</a><a href="${repo}/issues">${e(t.issues)}</a><a href="${repo}/blob/main/LICENSE">${e(t.license)}</a><a href="https://hreshhao.com/">${e(t.author)} ${arrow}</a></nav></div>
    <div class="container footer-bottom"><p>© 2026 Hresh · DevNook</p><p>${e(t.meta)}</p></div>
  </footer>
</body>
</html>\n`;
}
