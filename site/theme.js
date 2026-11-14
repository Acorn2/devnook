(() => {
  let theme = 'dark';
  try {
    const saved = localStorage.getItem('devnook.site.appearance');
    if (saved === 'light' || saved === 'dark') theme = saved;
  } catch {}
  document.documentElement.dataset.theme = theme;
})();
