import { t } from './i18n';
import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
  Check,
  ChevronDown,
  Code2,
  FolderOpen,
  Pin,
  Plus,
  RefreshCw,
  SquareTerminal,
} from 'lucide-react';
import Select from './Select';
import {
  ideNames,
  type AppPreference,
  type DevelopmentApp,
  type DevelopmentApps,
  type Project,
} from './types';

export function appName(target: string, apps: DevelopmentApp[]) {
  return (
    apps.find((app) => app.id === target)?.name ||
    ideNames[target] ||
    (target.startsWith('APP:')
      ? target
          .split('/')
          .pop()
          ?.replace(/\.app$/, '')
      : undefined) ||
    t('开发应用')
  );
}

export function AppSelect({
  value,
  preference,
  apps,
  onChange,
  choose,
  refresh,
  busy,
  disabled,
  id,
}: {
  value: string;
  preference?: AppPreference;
  apps: DevelopmentApp[];
  onChange: (target: string) => void;
  choose: () => Promise<DevelopmentApp | null>;
  refresh: () => Promise<void>;
  busy: boolean;
  disabled: boolean;
  id?: string;
}) {
  const installed = apps.filter((app) => app.kind !== 'TERMINAL');
  const resolved =
    installed.find((app) => app.id === value)?.id ||
    (preference?.available && installed.some((app) => app.id === preference.id)
      ? preference.id
      : value);
  const options: { value: string; label: string; disabled?: boolean }[] = installed.map((app) => ({
    value: app.id,
    label:
      app.name +
      (installed.filter((a) => a.name === app.name).length > 1
        ? ` · ${app.version ? `${app.version} · ` : ''}${app.path}`
        : ''),
  }));
  if (!options.some((option) => option.value === resolved)) {
    options.unshift({
      value: resolved,
      label: t('{{0}}（未找到）', [preference?.name || appName(value, apps)]),
      disabled: true,
    });
  }
  return (
    <div className="app-select-field">
      <Select
        id={id}
        aria-label={t('默认开发应用')}
        value={resolved}
        options={options}
        onValueChange={onChange}
        disabled={disabled || busy}
      />
      <div className="app-select-actions">
        <button
          type="button"
          className="text-button"
          disabled={disabled || busy}
          onClick={() =>
            void choose().then((app) => {
              if (app && app.kind !== 'TERMINAL') onChange(app.id);
            })
          }
        >
          <Plus size={14} />
          {t('选择其他应用…')}
        </button>
        <button
          type="button"
          className="text-button"
          disabled={disabled || busy}
          onClick={() => void refresh()}
        >
          <RefreshCw size={14} className={busy ? 'spin' : ''} />
          {t('刷新')}
        </button>
      </div>
    </div>
  );
}

export default function OpenWith({
  project,
  catalog,
  loading,
  error,
  disabled,
  launch,
  setDefault,
  choose,
  refresh,
}: {
  project: Project;
  catalog: DevelopmentApps | null;
  loading: boolean;
  error: string;
  disabled: boolean;
  launch: (project: Project, target?: string) => Promise<void>;
  setDefault: (project: Project, app: DevelopmentApp) => Promise<void>;
  choose: () => Promise<DevelopmentApp | null>;
  refresh: () => Promise<void>;
}) {
  const apps = catalog?.apps || [];
  const selected = apps.find((app) => app.id === project.default_ide);
  const preference = selected
    ? { id: selected.id, name: selected.name, available: true }
    : catalog?.project_defaults[project.id];
  const name = preference?.name || appName(project.default_ide, apps);
  const [open, setOpen] = useState(false);
  const [opensAbove, setOpensAbove] = useState(false);
  const [position, setPosition] = useState<{
    left: number;
    top: number;
    width: number;
    maxHeight: number;
  } | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const focused = useRef(false);
  const menuId = useId();
  function close(restore = false) {
    setOpen(false);
    if (restore) trigger.current?.focus();
  }
  function show() {
    focused.current = false;
    setPosition(null);
    setOpen(true);
  }

  useLayoutEffect(() => {
    if (!open || !trigger.current || !menu.current) return;
    const bounds = trigger.current.getBoundingClientRect();
    const fontSize = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
    const width = Math.min(18 * fontSize, innerWidth - 16);
    menu.current.style.width = `${width}px`;
    const options = menu.current.querySelector<HTMLElement>('.open-with-options');
    const heading = menu.current.querySelector<HTMLElement>('.open-with-heading');
    const footer = menu.current.querySelector<HTMLElement>('.open-with-footer');
    const height = Math.min(
      (options?.scrollHeight || 0) +
        (heading?.offsetHeight || 0) +
        (footer?.offsetHeight || 0) +
        fontSize,
      22 * fontSize,
    );
    const above = bounds.top - 14;
    const below = innerHeight - bounds.bottom - 14;
    // Prefer a usable dropdown with internal scrolling. Only flip when even a
    // short list cannot fit below, keeping the popup anchored to the button.
    const upwards = below < Math.min(height, 10 * fontSize) && above > below;
    const maxHeight = Math.max(0, Math.min(height, upwards ? above : below));
    setOpensAbove(upwards);
    setPosition({
      width,
      maxHeight,
      left: Math.max(8, Math.min(bounds.right - width, innerWidth - width - 8)),
      top: upwards ? bounds.top - 6 - maxHeight : bounds.bottom + 6,
    });
  }, [open, catalog, loading, error]);
  useLayoutEffect(() => {
    if (loading) focused.current = false;
    if (open && position && !loading && !focused.current) {
      menu.current?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
      focused.current = true;
    }
  }, [open, position, loading]);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        !menu.current?.contains(event.target) &&
        !trigger.current?.contains(event.target)
      )
        close();
    };
    const scroll = (event: Event) => {
      if (!(event.target instanceof Node) || !menu.current?.contains(event.target)) close();
    };
    const resize = () => close();
    document.addEventListener('pointerdown', outside, true);
    window.addEventListener('scroll', scroll, true);
    window.addEventListener('resize', resize);
    return () => {
      document.removeEventListener('pointerdown', outside, true);
      window.removeEventListener('scroll', scroll, true);
      window.removeEventListener('resize', resize);
    };
  }, [open]);
  useEffect(() => {
    if (disabled) close();
  }, [disabled]);

  const editors = apps.filter((app) => app.kind !== 'TERMINAL');
  const recommended = editors.filter((app) => app.recommended_for.includes(project.id));
  const others = editors.filter((app) => !app.recommended_for.includes(project.id));
  const ordered = [
    ...editors.filter((app) => app.id === preference?.id),
    ...recommended.filter((app) => app.id !== preference?.id),
    ...others.filter((app) => app.id !== preference?.id),
  ];
  const terminal = apps.find((app) => app.kind === 'TERMINAL');
  function rows(items: DevelopmentApp[]) {
    return items.map((app) => (
      <div className="open-with-row" key={app.id} data-default={preference?.id === app.id}>
        <button
          type="button"
          className="open-with-app"
          title={`${app.name}${app.version ? ` ${app.version}` : ''}\n${app.path}`}
          disabled={disabled || loading}
          onClick={() => {
            close(true);
            void launch(project, app.id);
          }}
        >
          <span>
            <strong>{app.name}</strong>
            {apps.filter((item) => item.name === app.name).length > 1 && (
              <small>
                {app.version ? `${app.version} · ` : ''}
                {app.path}
              </small>
            )}
          </span>
          {preference?.id === app.id && (
            <span className="open-with-current">
              <Check size={13} />
              {t('默认')}
            </span>
          )}
        </button>
        {preference?.id !== app.id && (
          <button
            type="button"
            className="icon-button open-with-pin"
            aria-label={t('将 {{0}} 设为此项目默认应用', [app.name])}
            title={t('设为此项目默认应用')}
            disabled={disabled || loading}
            onClick={() => {
              close(true);
              void setDefault(project, app);
            }}
          >
            <Pin size={15} />
          </button>
        )}
      </div>
    ));
  }
  return (
    <div className="open-with-control">
      <button
        type="button"
        className="open-ide"
        disabled={disabled || loading}
        title={
          preference?.available
            ? t('用 {{0}} 打开项目', [name])
            : t('默认应用未找到，请选择其他应用')
        }
        onClick={() => {
          if (preference?.available) void launch(project);
          else show();
        }}
      >
        <Code2 size={15} />
        <span>{preference?.available ? t('打开 {{0}}', [name]) : t('选择打开应用')}</span>
      </button>
      <button
        ref={trigger}
        type="button"
        className="icon-button open-with-trigger"
        aria-label={t('选择 {{0}} 的打开应用', [project.name])}
        aria-haspopup="dialog"
        aria-expanded={open}
        data-placement={open && opensAbove ? 'top' : 'bottom'}
        aria-controls={open ? menuId : undefined}
        disabled={disabled}
        onClick={() => (open ? close() : show())}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown') {
            event.preventDefault();
            show();
          }
        }}
      >
        <ChevronDown size={15} />
      </button>
      {open &&
        createPortal(
          <div
            ref={menu}
            id={menuId}
            className="select-menu open-with-menu"
            role="dialog"
            aria-label={t('{{0}} 的打开方式', [project.name])}
            style={position || { visibility: 'hidden' }}
            onBlur={(event) => {
              if (
                event.relatedTarget instanceof Node &&
                !event.currentTarget.contains(event.relatedTarget)
              )
                close();
            }}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                event.preventDefault();
                event.stopPropagation();
                close(true);
              }
              if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
                event.preventDefault();
                const buttons = Array.from(
                  menu.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') || [],
                );
                const current = buttons.indexOf(document.activeElement as HTMLButtonElement);
                const next =
                  event.key === 'Home'
                    ? 0
                    : event.key === 'End'
                      ? buttons.length - 1
                      : (current + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) %
                        buttons.length;
                buttons[next]?.focus();
              }
            }}
          >
            <div className="open-with-heading">{t('打开方式')}</div>
            <div className="open-with-options">
              {!preference?.available && (
                <p className="open-with-notice">
                  {t('未找到默认应用')}
                  {name}
                  {t('，请选择其他应用。')}
                </p>
              )}
              {loading && (
                <p className="open-with-notice" role="status">
                  {t('正在识别已安装应用…')}
                </p>
              )}
              {error && (
                <p className="open-with-notice" role="alert">
                  {t(error)}
                </p>
              )}
              {ordered.length > 0 && (
                <>
                  <div className="open-with-label">{t('开发应用')}</div>
                  {rows(ordered)}
                </>
              )}
              {!editors.length && !loading && !error && (
                <p className="open-with-notice">{t('未发现支持的开发应用，可手动选择 .app。')}</p>
              )}
              {catalog?.warnings.map((warning) => (
                <p key={t(warning)} className="open-with-notice">
                  {t(warning)}
                </p>
              ))}
              <div className="open-with-divider" />
              <button
                type="button"
                className="open-with-action"
                disabled={disabled}
                onClick={() => {
                  close(true);
                  void launch(project, 'FINDER');
                }}
              >
                <FolderOpen size={16} />
                {t('在 Finder 中显示')}
              </button>
              {terminal && (
                <button
                  type="button"
                  className="open-with-action"
                  disabled={disabled}
                  onClick={() => {
                    close(true);
                    void launch(project, terminal.id);
                  }}
                >
                  <SquareTerminal size={16} />
                  {t('在终端打开')}
                </button>
              )}
            </div>
            <div className="open-with-footer">
              <button
                type="button"
                className="open-with-action"
                disabled={disabled || loading}
                onClick={() => {
                  close(true);
                  void choose().then((app) => {
                    if (app) void launch(project, app.id);
                  });
                }}
              >
                <Plus size={16} />
                {t('选择其他应用…')}
              </button>
              <button
                type="button"
                className="icon-button open-with-refresh"
                aria-label={t('刷新应用列表')}
                title={t('刷新应用列表')}
                disabled={disabled || loading}
                onClick={() => void refresh()}
              >
                <RefreshCw size={15} className={loading ? 'spin' : ''} />
              </button>
            </div>
          </div>,
          document.body,
        )}
    </div>
  );
}
