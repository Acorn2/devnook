const root = document.documentElement;
const themeButton = document.querySelector('#theme-toggle');
function syncThemeLabel() {
  const dark = root.dataset.theme === 'dark';
  themeButton.setAttribute(
    'aria-label',
    dark ? themeButton.dataset.labelDark : themeButton.dataset.labelLight,
  );
  themeButton.setAttribute('aria-pressed', String(dark));
}
syncThemeLabel();
themeButton.addEventListener('click', () => {
  const next = root.dataset.theme === 'dark' ? 'light' : 'dark';
  root.dataset.theme = next;
  try {
    localStorage.setItem('devnook.site.appearance', next);
  } catch {}
  syncThemeLabel();
  updateScreenshots(next);
});

const menuButton = document.querySelector('#menu-toggle');
const header = document.querySelector('.site-header');
function closeMenu() {
  header.classList.remove('nav-open');
  menuButton.setAttribute('aria-expanded', 'false');
}
menuButton.addEventListener('click', () => {
  const open = menuButton.getAttribute('aria-expanded') !== 'true';
  header.classList.toggle('nav-open', open);
  menuButton.setAttribute('aria-expanded', String(open));
});
document.querySelector('#navigation').addEventListener('click', (event) => {
  if (event.target.closest('a')) closeMenu();
});
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && menuButton.getAttribute('aria-expanded') === 'true') {
    closeMenu();
    menuButton.focus();
  }
});

function updateScreenshots(appearance) {
  document.querySelectorAll('[data-theme-image]').forEach((preview) => {
    preview.src = `${preview.dataset.prefix}assets/${preview.dataset.themeImage}-${appearance}-${root.dataset.locale}.png`;
  });
  document.querySelectorAll('[data-image-link]').forEach((link) => {
    const prefix = document.querySelector('#app-preview').dataset.prefix;
    link.href = `${prefix}assets/${link.dataset.scene}-${appearance}-${root.dataset.locale}.png`;
  });
  document.querySelectorAll('[data-preview]').forEach((button) => {
    button.setAttribute('aria-pressed', String(button.dataset.preview === appearance));
  });
}
updateScreenshots(root.dataset.theme);
document.querySelectorAll('[data-preview]').forEach((button) => {
  button.addEventListener('click', () => {
    updateScreenshots(button.dataset.preview);
  });
});

const featureLinks = new Map(
  [...document.querySelectorAll('[data-feature-link]')].map((link) => [
    link.dataset.featureLink,
    link,
  ]),
);
if (featureLinks.size) {
  const setActive = (id) => {
    featureLinks.forEach((link, key) => link.classList.toggle('is-active', key === id));
  };
  featureLinks.forEach((link, id) => link.addEventListener('click', () => setActive(id)));
  const stories = [...document.querySelectorAll('.feature-story')];
  let featureFrame = 0;
  const syncActiveFeature = () => {
    featureFrame = 0;
    const threshold = window.innerHeight * 0.35;
    const current = stories.find((story) => {
      const { top, bottom } = story.getBoundingClientRect();
      return top <= threshold && bottom > threshold;
    });
    setActive(current ? current.id : null);
  };
  const queueActiveFeature = () => {
    if (!featureFrame) featureFrame = requestAnimationFrame(syncActiveFeature);
  };
  window.addEventListener('scroll', queueActiveFeature, { passive: true });
  window.addEventListener('resize', queueActiveFeature);
  syncActiveFeature();
}

const copyButton = document.querySelector('#copy-config');
copyButton.addEventListener('click', async () => {
  const code = document.querySelector('#mcp-config');
  const status = document.querySelector('#copy-status');
  try {
    await navigator.clipboard.writeText(code.textContent);
    copyButton.textContent = copyButton.dataset.copied;
    status.textContent = copyButton.dataset.copied;
  } catch {
    const selection = window.getSelection();
    const range = document.createRange();
    range.selectNodeContents(code);
    selection.removeAllRanges();
    selection.addRange(range);
    copyButton.textContent = copyButton.dataset.error;
    status.textContent = copyButton.dataset.error;
  }
});
