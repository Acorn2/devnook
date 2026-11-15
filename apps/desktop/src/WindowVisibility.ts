import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { native } from './api';

// Hidden WebViews stay mounted to preserve drafts, but need not keep polling Core.
export function useWindowVisible() {
  const [visible, setVisible] = useState(!native);
  useEffect(() => {
    if (!native) return;
    let cancelled = false;
    let version = 0;
    const window = getCurrentWindow();
    const update = () => {
      const request = ++version;
      void window
        .isVisible()
        .then((value) => {
          if (!cancelled && request === version) setVisible(value);
        })
        .catch(() => {
          if (!cancelled && request === version) setVisible(false);
        });
    };
    const changed = window.listen('tauri://focus', update);
    const blurred = window.listen('tauri://blur', update);
    const opened = window.listen('devnook:tray-opened', update);
    const navigated = window.listen('devnook:navigate', update);
    const shown = window.listen('devnook:window-shown', update);
    const hidden = window.listen('devnook:window-hidden', () => {
      ++version;
      setVisible(false);
    });
    update();
    return () => {
      cancelled = true;
      for (const pending of [changed, blurred, opened, navigated, shown, hidden])
        void pending.then((unlisten) => unlisten());
    };
  }, []);
  return visible;
}
