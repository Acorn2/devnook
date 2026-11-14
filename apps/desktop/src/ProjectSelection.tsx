import { t } from './i18n';
import { useEffect, useRef, useState } from 'react';
import CategoryPicker from './CategoryPicker';
import { setProjectCategories } from './ProjectBatch';
import type { Project } from './types';

export function ProjectCheckbox({
  label,
  checked,
  mixed = false,
  disabled,
  change,
}: {
  label: string;
  checked: boolean;
  mixed?: boolean;
  disabled: boolean;
  change: () => void;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = mixed;
  }, [mixed]);
  return (
    <label className="project-checkbox" title={label}>
      <input
        ref={ref}
        type="checkbox"
        aria-label={label}
        checked={checked}
        disabled={disabled}
        onChange={change}
      />
    </label>
  );
}

export function ProjectBatchBar({
  ids,
  categories,
  busy,
  setBusy,
  updated,
  clear,
  notify,
}: {
  ids: string[];
  categories: string[];
  busy: boolean;
  setBusy: (busy: boolean) => void;
  updated: (projects: Project[] | null, completed: Set<string>) => void;
  clear: () => void;
  notify: (message: string) => void;
}) {
  const [category, setCategory] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState('');
  const submitting = useRef(false);
  return (
    <form
      className="project-batch-bar"
      aria-label={t('批量设置分类')}
      aria-busy={busy}
      onSubmit={async (event) => {
        event.preventDefault();
        if (submitting.current || category === null || !ids.length) return;
        submitting.current = true;
        setBusy(true);
        setError('');
        setProgress(0);
        try {
          const result = await setProjectCategories(ids, category, setProgress);
          updated(result.projects, result.completed);
          setError(result.error);
          const remaining = ids.length - result.completed.size;
          notify(
            result.error
              ? t('已完成 {{0}} 项，{{1}} 项未完成。', [result.completed.size, remaining])
              : t('已将 {{0}} 个项目设为{{1}}。', [
                  result.completed.size,
                  category.trim() ? `“${category.trim()}”` : t('未分类'),
                ]),
          );
        } finally {
          submitting.current = false;
          setBusy(false);
        }
      }}
    >
      <span className="project-batch-count" role="status">
        {busy
          ? t('正在保存 {{0}} / {{1}}', [progress, ids.length])
          : t('已选 {{0}} 项', [ids.length])}
      </span>
      <CategoryPicker
        id="batch-category"
        value={category}
        categories={categories}
        change={setCategory}
        disabled={busy}
      />
      <button
        type="submit"
        className="button primary small"
        disabled={busy || !ids.length || category === null}
      >
        {busy ? t('正在保存…') : t('应用分类')}
      </button>
      <button
        type="button"
        className="text-button"
        disabled={busy || !ids.length}
        onClick={() => {
          clear();
          setError('');
        }}
      >
        {t('清空勾选')}
      </button>
      {error && (
        <p className="project-batch-error" role="alert">
          {t(error)} {t('已完成的更改会保留，请核对未完成项后重试。')}
        </p>
      )}
    </form>
  );
}
