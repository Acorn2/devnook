import { t } from './i18n';
import { useEffect, useRef, type ReactNode } from 'react';
import { ArrowUpRight, Check, Copy, Folder, X } from 'lucide-react';
import { typeNames, type Candidate } from './types';

export function Modal({
  title,
  subtitle,
  children,
  close,
  wide = false,
  className = '',
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
  close: () => void;
  wide?: boolean;
  className?: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => dialog?.close();
  }, []);
  return (
    <dialog
      ref={ref}
      className={`modal ${wide ? 'modal-wide' : ''} ${className}`}
      onCancel={(e) => {
        e.preventDefault();
        close();
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <header className="modal-head">
        <div>
          <h2>{title}</h2>
          {subtitle && <p>{subtitle}</p>}
        </div>
        <button className="icon-button" aria-label={t('关闭对话框')} onClick={close}>
          <X size={20} />
        </button>
      </header>
      {children}
    </dialog>
  );
}

export function ProjectGlyph({ type, small = false }: { type: string; small?: boolean }) {
  return (
    <span
      className={`project-glyph type-${type.toLowerCase()} ${small ? 'small' : ''}`}
      aria-hidden="true"
    >
      {(
        {
          NODE: 'JS',
          JAVA: 'J',
          PYTHON: 'Py',
          RUST: 'Rs',
          GO: 'Go',
          DOTNET: 'C#',
          PHP: 'PHP',
          RUBY: 'Rb',
          DART: 'Dt',
          SWIFT: 'Sw',
          CPP: 'C++',
          MIXED: '{ }',
        } as Record<string, string>
      )[type] || <Folder size={22} />}
    </span>
  );
}

export function Tags({ project }: { project: Candidate }) {
  return (
    <div className="tags">
      <span>{t(typeNames[project.project_type]) || project.project_type}</span>
      {project.frameworks.slice(0, 3).map((v) => (
        <span key={v}>{v}</span>
      ))}
      {project.is_monorepo && <span className="tag-green">Monorepo</span>}
    </div>
  );
}

export function EmptyState({
  title,
  description,
  filtered,
  scan,
  disabled,
}: {
  title?: string;
  description?: string;
  filtered: boolean;
  scan: () => void;
  disabled: boolean;
}) {
  return (
    <section className="empty-state">
      <div className="empty-art" aria-hidden="true">
        <div className="art-grid" />
        <div className="art-folder">
          <Folder size={52} strokeWidth={1.2} />
          <span className="art-code">{'{ }'}</span>
        </div>
        <div className="art-spark">
          <ArrowUpRight size={19} />
        </div>
        <span className="art-dot dot-one" />
        <span className="art-dot dot-two" />
      </div>
      <span className="eyebrow">YOUR NEXT IDEA STARTS HERE</span>
      <h2>{title || (filtered ? t('没有找到匹配的项目') : t('给你的项目，一个安静的角落。'))}</h2>
      <p>
        {description ||
          (filtered
            ? t('试试其他关键词，或调整项目筛选条件。')
            : t('扫描本地目录，把分散的项目整理到一起。\n下一次开始，只需打开 DevNook。'))}
      </p>
      {!filtered && (
        <button className="button primary" onClick={scan} disabled={disabled}>
          <Folder size={17} />
          {t('扫描第一个项目')}
          <ArrowUpRight size={16} />
        </button>
      )}
      {!filtered && (
        <div className="empty-notes">
          <span>
            <Check size={13} />
            {t('只读扫描')}
          </span>
          <span>
            <Check size={13} />
            {t('数据留在本机')}
          </span>
          <span>
            <Check size={13} />
            {t('不改动项目')}
          </span>
        </div>
      )}
    </section>
  );
}

export function CopyButton({
  text,
  done,
  failed,
}: {
  text: string;
  done: () => void;
  failed: (e: unknown) => void;
}) {
  return (
    <button
      type="button"
      className="icon-button"
      aria-label={t('复制路径')}
      onClick={async () => {
        try {
          if (!navigator.clipboard)
            throw new Error(t('当前窗口无法访问剪贴板，请手动选择路径复制。'));
          await navigator.clipboard.writeText(text);
          done();
        } catch (e) {
          failed(e);
        }
      }}
    >
      <Copy size={15} />
    </button>
  );
}
