import { useId, useLayoutEffect, useRef, useState } from 'react';
import { Copy } from 'lucide-react';
import { t } from './i18n';

export default function ProcessCommand({
  command,
  notify,
  report,
}: {
  command: string;
  notify: (message: string) => void;
  report: (error: unknown) => void;
}) {
  const id = useId();
  const previewRef = useRef<HTMLDivElement>(null);
  const codeRef = useRef<HTMLElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [overflows, setOverflows] = useState(false);

  useLayoutEffect(() => {
    const code = codeRef.current;
    if (!code) return;
    const measure = () => {
      const lineHeight = parseFloat(getComputedStyle(code).lineHeight);
      setOverflows(code.scrollHeight > lineHeight * 4 + 1);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(code);
    return () => observer.disconnect();
  }, [command]);

  return (
    <div className="process-command">
      <div
        ref={previewRef}
        id={id}
        className={`process-command-preview${expanded && overflows ? ' expanded' : ''}`}
        tabIndex={expanded && overflows ? 0 : undefined}
        role="region"
        aria-label={t('启动命令')}
      >
        <code ref={codeRef}>{command}</code>
      </div>
      <div className="process-command-actions">
        {overflows && (
          <button
            type="button"
            className="text-button"
            aria-expanded={expanded}
            aria-controls={id}
            onClick={() => {
              if (previewRef.current) previewRef.current.scrollTop = 0;
              setExpanded((value) => !value);
            }}
          >
            {expanded ? t('收起') : t('展开')}
          </button>
        )}
        <button
          type="button"
          className="text-button"
          onClick={async () => {
            try {
              if (!navigator.clipboard) throw new Error(t('无法访问剪贴板，请手动选择文字复制。'));
              await navigator.clipboard.writeText(command);
              notify(t('命令已复制'));
            } catch (error) {
              report(error);
            }
          }}
        >
          <Copy size={13} aria-hidden="true" />
          {t('复制命令')}
        </button>
      </div>
    </div>
  );
}
