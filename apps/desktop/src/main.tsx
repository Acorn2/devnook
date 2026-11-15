import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import TrayPanel from './TrayPanel';
import { call, native } from './api';
import { initializeLanguage, syncLanguage } from './i18n';
import './styles.css';
import './theme.css';

async function bootstrap() {
  let systemLanguage = navigator.language;
  if (native) {
    try {
      systemLanguage = await call<string>('system_language');
    } catch {
      systemLanguage = 'en';
    }
  }
  initializeLanguage(systemLanguage);
  window.addEventListener('storage', (event) => {
    if (event.key === 'devnook.language' || event.key === null) syncLanguage(systemLanguage);
  });
  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      {new URLSearchParams(location.search).get('panel') === 'tray' ? <TrayPanel /> : <App />}
    </React.StrictMode>,
  );
}

void bootstrap();
