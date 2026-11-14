import { useSyncExternalStore } from 'react';
import { Languages } from 'lucide-react';
import Select from './Select';
import { changeLanguage, languageSnapshot, subscribeLanguage, t } from './i18n';

export const useLanguage = () =>
  useSyncExternalStore(subscribeLanguage, languageSnapshot, languageSnapshot);

export function LanguageSettings() {
  const { language, storageError } = useLanguage();
  return (
    <section className="settings-section" aria-labelledby="language-title">
      <div className="settings-section-title">
        <Languages size={20} />
        <div>
          <h2 id="language-title">{t('界面语言')}</h2>
          <p>{t('首次按系统首选语言选择：中文系统使用中文，其他系统使用英文。')}</p>
        </div>
      </div>
      <Select
        aria-label={t('语言')}
        value={language}
        options={[
          { value: 'zh', label: '简体中文' },
          { value: 'en', label: 'English' },
        ]}
        onValueChange={(value) => {
          if (value === 'zh' || value === 'en') changeLanguage(value);
        }}
      />
      <p className="appearance-hint" role="status">
        {t(
          storageError
            ? '语言已切换，但无法保存选择；重新打开后可能恢复默认。'
            : '立即切换，自动记住；无需保存设置。',
        )}
      </p>
    </section>
  );
}
