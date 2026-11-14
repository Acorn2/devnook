import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { call, native } from './api';
import { initializeLanguage } from './i18n';
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
  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void bootstrap();
