import { t } from './i18n';
import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Check, Moon, Sun } from 'lucide-react';
import { native } from './api';

export type Appearance = 'light' | 'dark';

function applyAppearance(appearance: Appearance) {
  const root = document.documentElement;
  const changed = root.dataset.theme !== appearance;
  if (changed) root.classList.add('switching-appearance');
  root.dataset.theme = appearance;
  root.style.colorScheme = appearance;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute('content', appearance === 'dark' ? '#101318' : '#f2f5fa');
  if (changed) {
    requestAnimationFrame(() =>
      requestAnimationFrame(() => root.classList.remove('switching-appearance')),
    );
  }
}

export function useAppearance(report: (error: unknown) => void) {
  const [appearance, setAppearance] = useState<Appearance>(
    document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light',
  );
  const [storageError, setStorageError] = useState(false);
  useEffect(() => {
    const synchronize = (event: StorageEvent) => {
      if (event.key !== 'devnook.appearance' && event.key !== null) return;
      const value = event.newValue;
      const next =
        value === 'light' || value === 'dark'
          ? value
          : matchMedia('(prefers-color-scheme: dark)').matches
            ? 'dark'
            : 'light';
      applyAppearance(next);
      setAppearance(next);
    };
    window.addEventListener('storage', synchronize);
    return () => window.removeEventListener('storage', synchronize);
  }, []);
  useEffect(() => {
    applyAppearance(appearance);
    if (native) {
      // Window chrome follows the explicit app choice, independently of the OS setting.
      void getCurrentWindow().setTheme(appearance).catch(report);
    }
  }, [appearance, report]);

  function changeAppearance(next: Appearance) {
    applyAppearance(next);
    setAppearance(next);
    try {
      localStorage.setItem('devnook.appearance', next);
      setStorageError(false);
    } catch {
      setStorageError(true);
    }
  }
  return { appearance, changeAppearance, storageError };
}

export function AppearanceToggle({
  appearance,
  change,
}: {
  appearance: Appearance;
  change: (appearance: Appearance) => void;
}) {
  const label = appearance === 'dark' ? t('切换到浅色模式') : t('切换到暗色模式');
  return (
    <button
      type="button"
      className="appearance-toggle"
      aria-label={label}
      title={label}
      onClick={() => change(appearance === 'dark' ? 'light' : 'dark')}
    >
      {appearance === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
      <span>{appearance === 'dark' ? t('浅色') : t('暗色')}</span>
    </button>
  );
}

export function AppearanceSettings({
  appearance,
  change,
  storageError,
}: {
  appearance: Appearance;
  change: (appearance: Appearance) => void;
  storageError: boolean;
}) {
  return (
    <section className="settings-section appearance-settings" aria-labelledby="appearance-title">
      <div className="settings-section-title">
        <Sun size={20} />
        <div>
          <h2 id="appearance-title">{t('界面外观')}</h2>
          <p>{t('选择适合当前光线的工作环境。')}</p>
        </div>
      </div>
      <div className="appearance-options" role="group" aria-label={t('配色模式')}>
        {(['light', 'dark'] as const).map((value) => (
          <button
            type="button"
            className="appearance-option"
            data-preview={value}
            aria-pressed={appearance === value}
            key={value}
            onClick={() => change(value)}
          >
            <span className="appearance-swatch" aria-hidden="true">
              <span className="swatch-sidebar">
                <i />
                <i />
                <i />
              </span>
              <span className="swatch-content">
                <i />
                <span>
                  <i />
                  <i />
                </span>
              </span>
            </span>
            <span className="appearance-option-label">
              {value === 'light' ? <Sun size={16} /> : <Moon size={16} />}
              <span>{value === 'light' ? t('浅色') : t('暗色')}</span>
              {appearance === value && <Check size={16} />}
            </span>
          </button>
        ))}
      </div>
      <p className="appearance-hint" role="status">
        {storageError
          ? t('已切换外观，但当前无法保存选择；重新打开后可能恢复默认。')
          : t('即时生效，自动记住选择。首次打开时使用系统配色。')}
      </p>
    </section>
  );
}
