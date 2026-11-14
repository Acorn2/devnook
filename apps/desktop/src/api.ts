import { t } from './i18n';
import { invoke, isTauri } from '@tauri-apps/api/core';

export const native = isTauri();
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!native) throw new Error(t('请在 DevNook 桌面应用中使用本地功能；浏览器仅提供界面预览。'));
  return invoke<T>(command, args);
}
