import { english } from './locales/en';

export type Language = 'zh' | 'en';
export const languageStorageKey = 'devnook.language';
const listeners = new Set<() => void>();
let state: { language: Language; storageError: boolean } = {
  language: 'en',
  storageError: false,
};

// Only the first system language is considered, never a secondary preference.
export function resolveLanguage(
  systemLanguage: string | null | undefined,
  saved?: unknown,
): Language {
  if (saved === 'zh' || saved === 'en') return saved;
  return /^zh(?:[-_]|$)/i.test(systemLanguage?.trim() || '') ? 'zh' : 'en';
}

function applyLanguage() {
  if (typeof document === 'undefined') return;
  document.documentElement.lang = state.language === 'zh' ? 'zh-CN' : 'en';
  document.title = t('DevNook · 本地开发工作台');
}

export function initializeLanguage(systemLanguage?: string) {
  let saved: unknown;
  try {
    saved = localStorage.getItem(languageStorageKey);
  } catch {
    // Unavailable storage must not block startup or system-language detection.
  }
  state = { language: resolveLanguage(systemLanguage, saved), storageError: false };
  applyLanguage();
}

export function changeLanguage(language: Language) {
  let storageError = false;
  try {
    localStorage.setItem(languageStorageKey, language);
  } catch {
    storageError = true;
  }
  state = { language, storageError };
  applyLanguage();
  listeners.forEach((listener) => listener());
}

export const languageSnapshot = () => state;
export function subscribeLanguage(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
export const dateLocale = () => (state.language === 'zh' ? 'zh-CN' : 'en-US');

const interpolate = (text: string, values: readonly unknown[]) =>
  text.replace(/\{\{(\d+)\}\}/g, (match, index: string) =>
    Number(index) < values.length ? String(values[Number(index)] ?? '') : match,
  );

const chinese = new Map(
  Object.entries(english).map(([source, translated]) => [translated, source]),
);
const escapePattern = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const messagePatterns = Object.entries(english)
  .filter(([source]) => /\{\{\d+\}\}/.test(source))
  .flatMap(([source, translated]) =>
    [source, translated].map((template) => ({
      source,
      translated,
      pattern: new RegExp(
        '^' +
          template
            .split(/\{\{\d+\}\}/)
            .map(escapePattern)
            .join('([\\s\\S]*?)') +
          '$',
      ),
    })),
  );

// Call only for application copy and diagnostics, never names, paths, commands,
// category values, tokens, or raw logs. Parameters are inserted literally.
export function t(source: string | null | undefined, values: readonly unknown[] = []): string {
  if (!source) return '';
  if (Object.prototype.hasOwnProperty.call(english, source))
    return interpolate(state.language === 'zh' ? source : english[source], values);
  const original = chinese.get(source);
  if (original !== undefined) return state.language === 'zh' ? original : source;
  // Toasts/validation errors already in state can change language without remounting.
  for (const message of messagePatterns) {
    const match = message.pattern.exec(source);
    if (match)
      return interpolate(
        state.language === 'zh' ? message.source : message.translated,
        match.slice(1),
      );
  }
  return source;
}
