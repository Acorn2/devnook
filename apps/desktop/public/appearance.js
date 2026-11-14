// Apply the saved appearance before the application stylesheet can paint.
(() => {
  let appearance;
  try {
    appearance = localStorage.getItem('devnook.appearance');
  } catch {
    // Restricted storage must not prevent the workspace from opening.
  }
  if (appearance !== 'light' && appearance !== 'dark') {
    appearance = matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  document.documentElement.dataset.theme = appearance;
  document.documentElement.style.colorScheme = appearance;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute('content', appearance === 'dark' ? '#101318' : '#f2f5fa');
})();
