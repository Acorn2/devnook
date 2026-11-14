import { t } from './i18n';
import { useEffect, useRef, useState } from 'react';
import Select from './Select';

export default function CategoryPicker({
  id,
  value,
  categories,
  change,
  disabled,
}: {
  id: string;
  value: string | null;
  categories: string[];
  change: (value: string) => void;
  disabled: boolean;
}) {
  const [creating, setCreating] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (creating) input.current?.focus();
  }, [creating]);
  return (
    <div className="category-picker">
      <Select
        id={id}
        aria-label={t('项目分类')}
        disabled={disabled}
        value={
          creating ? 'new' : value === null ? 'placeholder' : value ? `category:${value}` : 'none'
        }
        options={[
          ...(value === null
            ? [{ value: 'placeholder', label: t('选择分类…'), disabled: true }]
            : []),
          { value: 'none', label: t('未分类') },
          ...categories.map((category) => ({ value: `category:${category}`, label: category })),
          { value: 'new', label: t('新建分类…') },
        ]}
        onValueChange={(next) => {
          setCreating(next === 'new');
          change(next === 'none' || next === 'new' ? '' : next.slice('category:'.length));
        }}
      />
      {creating && (
        <label htmlFor={`${id}-new`}>
          <span className="category-name-label">{t('新分类名称')}</span>
          <input
            id={`${id}-new`}
            ref={input}
            aria-label={t('新分类名称')}
            required
            disabled={disabled}
            maxLength={80}
            autoComplete="off"
            placeholder={t('如：产品项目、工具脚本')}
            value={value || ''}
            onChange={(event) => {
              const next = event.target.value;
              event.target.setCustomValidity(
                !next.trim()
                  ? t('请输入分类名称，或选择“未分类”。')
                  : new TextEncoder().encode(next.trim()).length > 80
                    ? t('分类名称过长，请缩短后重试。')
                    : '',
              );
              change(next);
            }}
          />
        </label>
      )}
    </div>
  );
}
