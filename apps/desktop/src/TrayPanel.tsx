import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Check,
  CircleAlert,
  Globe,
  LoaderCircle,
  Network,
  Play,
  Square,
  RefreshCw,
  RotateCw,
  Search,
  SquareTerminal,
  X,
} from 'lucide-react';
import { call, native } from './api';
import { useAppearance } from './Appearance';
import { useLanguage } from './Language';
import { dateLocale, t } from './i18n';
import {
  filterTrayGroups,
  reconcileTrayGroups,
  trayBrowserShortcut,
  trayCopyableEndpoint,
  trayEntryPorts,
  trayEntryStatus,
  trayGroups,
  traySummary,
  type MainDestination,
  type TrayEntry,
  type TrayGroup,
} from './TrayPresentation';
import { serviceStateLabel } from './ServicePresentation';
import { projectColor } from './ColorLabels';
import { useWindowVisible } from './WindowVisibility';
import type { BrowserAddress, BrowserShortcut, PortBinding, Project, ServiceView } from './types';
import './tray.css';

type Snapshot = { portsAvailable: boolean; shortcuts: BrowserShortcut[]; time: Date };
type Session = { groups: TrayGroup[]; autoSearch: boolean };
const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

export default function TrayPanel() {
  useLanguage();
  const visible = useWindowVisible();
  const header = useRef<HTMLElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const scroll = useRef<HTMLDivElement>(null);
  const footer = useRef<HTMLElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const toolbar = useRef<HTMLDivElement>(null);
  const heightLocked = useRef(false);
  const requestedHeight = useRef(0);
  const [session, setSession] = useState<Session | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [dataReady, setDataReady] = useState(false);
  const [query, setQuery] = useState('');
  const [searchRequested, setSearchRequested] = useState(false);
  const [scrollHints, setScrollHints] = useState({ above: false, below: false });
  const [error, setError] = useState('');
  const [portError, setPortError] = useState('');
  const [actionError, setActionError] = useState('');
  const [copiedKey, setCopiedKey] = useState('');
  const [refreshing, setRefreshing] = useState(false);
  const [reload, setReload] = useState(0);
  const [busy, setBusy] = useState('');
  const actionPending = useRef(false);
  const report = useCallback((error: unknown) => setActionError(message(error)), []);
  useAppearance(report);
  const showSearch = !!session?.autoSearch || searchRequested;

  const updateScrollHints = useCallback(() => {
    const element = scroll.current;
    if (!element) return;
    const above = element.scrollTop > 1;
    const below = element.scrollHeight - element.clientHeight - element.scrollTop > 1;
    setScrollHints((previous) =>
      previous.above === above && previous.below === below ? previous : { above, below },
    );
  }, []);

  useEffect(() => {
    document.documentElement.classList.add('tray-document');
    return () => document.documentElement.classList.remove('tray-document');
  }, []);
  useEffect(() => {
    if (!visible) return;
    heightLocked.current = false;
    requestedHeight.current = 0;
    setSession(null);
    setSnapshot(null);
    setQuery('');
    setSearchRequested(false);
    if (scroll.current) scroll.current.scrollTop = 0;
  }, [visible]);
  useEffect(() => {
    if (!visible || !native) return;
    const keyboard = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        void call('hide_tray').catch(report);
      } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f') {
        event.preventDefault();
        setSearchRequested(true);
        requestAnimationFrame(() => search.current?.focus());
      } else if (
        (event.key === 'ArrowDown' || event.key === 'ArrowUp') &&
        !(event.target instanceof HTMLInputElement)
      ) {
        const rows = [
          ...(content.current?.querySelectorAll<HTMLButtonElement>('.tray-service-name') || []),
        ];
        if (!rows.length) return;
        event.preventDefault();
        const current = (document.activeElement as HTMLElement | null)
          ?.closest('.tray-service')
          ?.querySelector('.tray-service-name');
        const index = rows.findIndex((row) => row === current);
        const next =
          index < 0
            ? event.key === 'ArrowDown'
              ? 0
              : rows.length - 1
            : (index + (event.key === 'ArrowDown' ? 1 : -1) + rows.length) % rows.length;
        rows[next].focus({ preventScroll: true });
        rows[next].scrollIntoView({ block: 'nearest' });
      }
    };
    window.addEventListener('keydown', keyboard);
    return () => window.removeEventListener('keydown', keyboard);
  }, [visible, report]);

  useEffect(() => {
    if (!visible || !native) return;
    let cancelled = false;
    let inFlight = false;
    setDataReady(false);
    setError('');
    setActionError('');
    const refresh = async () => {
      if (inFlight) return;
      inFlight = true;
      setRefreshing(true);
      try {
        const [projects, services, ports, shortcuts] = await Promise.all([
          call<Project[]>('list_projects'),
          call<ServiceView[]>('list_services'),
          call<PortBinding[]>('list_ports').then(
            (value) => ({ value, error: '' }),
            (error: unknown) => ({ value: [] as PortBinding[], error: message(error) }),
          ),
          call<BrowserShortcut[]>('tray_browser_shortcuts').catch(() => [] as BrowserShortcut[]),
        ]);
        if (!cancelled) {
          const current = trayGroups(projects, services, ports.value);
          setSession((previous) => ({
            groups: reconcileTrayGroups(previous?.groups || null, current),
            autoSearch:
              previous?.autoSearch ??
              current.reduce((count, group) => count + group.entries.length, 0) > 12,
          }));
          setSnapshot({ portsAvailable: !ports.error, shortcuts, time: new Date() });
          setPortError(ports.error);
          setError('');
          setDataReady(true);
        }
      } catch (error) {
        if (!cancelled) {
          setDataReady(false);
          setError(message(error));
        }
      } finally {
        inFlight = false;
        if (!cancelled) setRefreshing(false);
      }
    };
    void refresh();
    const timer = setInterval(() => void refresh(), 2000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [visible, reload]);

  useEffect(() => {
    if (!visible || !native || !content.current || !footer.current) return;
    let frame = 0;
    const measure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        updateScrollHints();
        if (heightLocked.current) return;
        const height = Math.ceil(
          (header.current?.getBoundingClientRect().height || 0) +
            (content.current?.getBoundingClientRect().height || 0) +
            (footer.current?.getBoundingClientRect().height || 0) +
            (toolbar.current?.getBoundingClientRect().height || 0),
        );
        if (!height) return;
        if (height !== requestedHeight.current) {
          requestedHeight.current = height;
          void call('resize_tray', { height }).catch(report);
        }
        if (dataReady) heightLocked.current = true;
      });
    };
    const observer = new ResizeObserver(measure);
    [header.current, content.current, footer.current, scroll.current, toolbar.current].forEach(
      (element) => element && observer.observe(element),
    );
    measure();
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [visible, dataReady, showSearch, report, updateScrollHints]);

  async function navigate(destination?: MainDestination) {
    if (actionPending.current) return;
    actionPending.current = true;
    setBusy('navigate');
    try {
      await call('open_main_window', { destination: destination || null });
    } catch (error) {
      report(error);
    } finally {
      actionPending.current = false;
      setBusy('');
    }
  }
  async function openBrowser(entry: TrayEntry, address: BrowserAddress) {
    if (actionPending.current || !dataReady || !snapshot?.portsAvailable || entry.unavailable)
      return;
    actionPending.current = true;
    setBusy('browser:' + entry.key);
    setActionError('');
    try {
      // No discovery or HTTP probe here. Core independently rechecks this receipt.
      await call('open_service_browser', { target: entry.target, address });
    } catch (error) {
      report(error);
    } finally {
      actionPending.current = false;
      setBusy('');
    }
  }
  async function stop(entry: TrayEntry) {
    if (
      actionPending.current ||
      !dataReady ||
      entry.unavailable ||
      entry.destination.kind !== 'managed' ||
      !['STARTING', 'RUNNING', 'READY'].includes(entry.state || '')
    )
      return;
    actionPending.current = true;
    setBusy('stop:' + entry.key);
    setActionError('');
    try {
      await call('stop_service', { id: entry.destination.id });
      setReload((value) => value + 1);
    } catch (error) {
      report(error);
    } finally {
      actionPending.current = false;
      setBusy('');
    }
  }
  async function copyPort(entryKey: string, endpoint: string) {
    try {
      if (!navigator.clipboard) throw new Error(t('无法访问剪贴板，请手动选择文字复制。'));
      await navigator.clipboard.writeText(endpoint);
      setCopiedKey(entryKey);
      setTimeout(() => {
        setCopiedKey((current) => (current === entryKey ? '' : current));
      }, 1500);
    } catch (error) {
      report(error);
    }
  }
  const allGroups = session?.groups || [];
  const summary = traySummary(allGroups);
  const groups = filterTrayGroups(allGroups, query);
  const disabled = !!busy || !dataReady || !visible;
  const retry = () => setReload((value) => value + 1);

  return (
    <main className="tray-panel" aria-label={t('DevNook 快捷面板')}>
      <header className="tray-header" ref={header}>
        <div className="tray-brand">
          <img
            src="/devnook.svg"
            width={16}
            height={16}
            alt=""
            draggable={false}
            className="tray-brand-mark"
          />
          <strong>DevNook</strong>
          <div className="tray-summary-pills" role="status">
            {summary.attention > 0 && (
              <span className="tray-pill tray-pill-danger">
                {t('{{0}} 需关注', [summary.attention])}
              </span>
            )}
            {summary.running > 0 && (
              <span className="tray-pill tray-pill-success">
                {t('{{0}} 运行中', [summary.running])}
              </span>
            )}
            {summary.external > 0 && (
              <span className="tray-pill tray-pill-cyan">
                {t('{{0}} 外部', [summary.external])}
              </span>
            )}
          </div>
        </div>
        <div className="tray-header-actions">
          <button
            type="button"
            className={`tray-icon ${showSearch ? 'active' : ''}`}
            disabled={!native || !dataReady}
            aria-label={t('搜索项目、服务或端口')}
            aria-pressed={showSearch}
            title={t('搜索项目、服务或端口') + ' (⌘F)'}
            onClick={() => {
              if (showSearch && !session?.autoSearch) {
                setQuery('');
                setSearchRequested(false);
              } else {
                setSearchRequested(true);
                requestAnimationFrame(() => search.current?.focus());
              }
            }}
          >
            <Search size={13} />
          </button>
          <button
            type="button"
            className="tray-icon"
            disabled={!native || refreshing || !!busy}
            onClick={retry}
            aria-label={t('刷新')}
            title={
              snapshot
                ? t('更新于 {{0}}', [
                    snapshot.time.toLocaleTimeString(dateLocale(), {
                      hour: '2-digit',
                      minute: '2-digit',
                      hour12: false,
                    }),
                  ])
                : t('刷新')
            }
          >
            <RefreshCw size={13} className={refreshing ? 'spin' : ''} />
          </button>
        </div>
      </header>
      {showSearch && (
        <div className="tray-search" ref={toolbar}>
          <Search size={12} aria-hidden="true" />
          <input
            ref={search}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t('搜索项目、服务或端口')}
            aria-label={t('搜索项目、服务或端口')}
            spellCheck={false}
          />
          {query ? (
            <button
              className="tray-icon"
              aria-label={t('清除搜索')}
              onClick={() => {
                setQuery('');
                search.current?.focus();
              }}
            >
              <X size={12} />
            </button>
          ) : (
            <kbd>⌘F</kbd>
          )}
        </div>
      )}
      <div
        className="tray-scroll-region"
        data-scroll-top={scrollHints.above || undefined}
        data-scroll-bottom={scrollHints.below || undefined}
      >
        <div className="tray-content" ref={scroll} onScroll={updateScrollHints}>
          <div className="tray-list" ref={content}>
            {!native ? (
              <p className="tray-empty">
                {t('请在 DevNook 桌面应用中使用本地功能；浏览器仅提供界面预览。')}
              </p>
            ) : error ? (
              <div className="tray-empty" role="alert">
                <strong>{t('后台未连接')}</strong>
                <span title={t(error)}>{t(error)}</span>
                <button className="tray-inline-button" onClick={retry}>
                  {t('重新连接')}
                </button>
              </div>
            ) : !snapshot ? (
              <div className="tray-loading" role="status">
                <LoaderCircle size={13} className="spin" />
                {t('正在读取服务…')}
              </div>
            ) : (
              <>
                {groups.map((group) => (
                  <section
                    className={`tray-group ${group.compact ? 'tray-compact-group' : ''}`}
                    key={group.id}
                    data-color={projectColor(group.id)}
                    aria-label={group.name}
                  >
                    {!group.compact && (
                      <h2 title={group.name}>
                        <span className="tray-group-dot" aria-hidden="true" />
                        <span>{group.name}</span>
                      </h2>
                    )}
                    {group.entries.map((entry, index) => {
                      const unavailable = !!entry.unavailable;
                      const canRestart =
                        !unavailable &&
                        !entry.external &&
                        ['RUNNING', 'READY'].includes(entry.state || '');
                      const canStart =
                        !unavailable &&
                        !entry.external &&
                        (!entry.state || ['STOPPED', 'BLOCKED', 'CRASHED'].includes(entry.state));
                      const canStop =
                        !unavailable &&
                        !entry.external &&
                        ['STARTING', 'RUNNING', 'READY'].includes(entry.state || '');
                      const state =
                        !snapshot.portsAvailable && entry.external
                          ? 'unknown'
                          : unavailable
                            ? 'unavailable'
                            : entry.external
                              ? 'external'
                              : (entry.state || 'STOPPED').toLowerCase();
                      const ports = trayEntryPorts(entry);
                      const copied = copiedKey === entry.key;
                      const name = group.compact && index === 0 ? group.name : entry.name;
                      const label = [group.name, entry.name, ports].filter(Boolean).join(' · ');
                      const fullStatus =
                        !snapshot.portsAvailable && entry.external
                          ? t('端口状态未知')
                          : unavailable
                            ? t('已不可用')
                            : entry.external
                              ? t('外部启动 · 检测到监听')
                              : serviceStateLabel(entry.state);
                      const browser = snapshot.portsAvailable
                        ? trayBrowserShortcut(entry, snapshot.shortcuts)
                        : undefined;
                      const copyable = snapshot.portsAvailable
                        ? trayCopyableEndpoint(entry, browser)
                        : null;
                      const status =
                        !snapshot.portsAvailable && entry.external
                          ? t('待核验')
                          : entry.external && group.compact && !unavailable
                            ? t('外部')
                            : trayEntryStatus(entry);
                      return (
                        <div
                          className={`tray-service state-${state}`}
                          key={entry.key}
                          data-entry-key={entry.key}
                        >
                          <span className="tray-indicator" aria-hidden="true" />
                          <button
                            className="tray-service-name"
                            aria-disabled={disabled || unavailable}
                            onClick={() => {
                              if (!disabled && !unavailable) void navigate(entry.destination);
                            }}
                            aria-label={t('查看 {{0}} 的详情与启停操作', [label])}
                            title={[
                              label,
                              fullStatus,
                              !snapshot.portsAvailable && t('端口状态未知'),
                            ]
                              .filter(Boolean)
                              .join(' · ')}
                          >
                            <span className="tray-name-text">{name}</span>
                            {status && <span className="tray-state">{status}</span>}
                            {!snapshot.portsAvailable && <span className="tray-state">?</span>}
                          </button>
                          {ports && (
                            <button
                              type="button"
                              className={`tray-port ${copied ? 'copied' : ''}`}
                              disabled={!copyable || unavailable}
                              title={
                                copied
                                  ? t('地址已复制')
                                  : copyable
                                    ? t('复制 {{0}}', [copyable])
                                    : ports
                              }
                              aria-label={copyable ? t('复制 {{0}}', [copyable]) : ports}
                              onClick={() => {
                                if (copyable) void copyPort(entry.key, copyable);
                              }}
                            >
                              {copied ? <Check size={10} /> : null}
                              <span>{ports}</span>
                            </button>
                          )}
                          <div className="tray-service-actions">
                            {browser && (
                              <button
                                className="tray-icon tray-browser"
                                disabled={disabled}
                                title={t('打开浏览器')}
                                aria-label={t('打开 {{0}} 的浏览器入口', [label])}
                                onClick={() => void openBrowser(entry, browser)}
                              >
                                {busy === 'browser:' + entry.key ? (
                                  <LoaderCircle size={13} className="spin" />
                                ) : (
                                  <Globe size={13} />
                                )}
                              </button>
                            )}
                            {(canStart || canRestart) && entry.destination.kind === 'managed' && (
                              <button
                                className={`tray-icon ${canRestart ? 'tray-restart' : 'tray-start'}`}
                                disabled={disabled}
                                title={t(canRestart ? '重启…' : '启动…')}
                                aria-label={t(canRestart ? '确认重启 {{0}}' : '确认启动 {{0}}', [
                                  label,
                                ])}
                                onClick={() =>
                                  entry.destination.kind === 'managed' &&
                                  void navigate({
                                    ...entry.destination,
                                    action: canRestart ? 'restart' : 'start',
                                  })
                                }
                              >
                                {canRestart ? <RotateCw size={13} /> : <Play size={13} />}
                              </button>
                            )}
                            {canStop && (
                              <button
                                className="tray-icon tray-stop"
                                disabled={disabled}
                                title={t('正常停止')}
                                aria-label={t('正常停止 {{0}}', [label])}
                                onClick={() => void stop(entry)}
                              >
                                {busy === 'stop:' + entry.key ? (
                                  <LoaderCircle size={13} className="spin" />
                                ) : (
                                  <Square size={10} fill="currentColor" strokeWidth={0} />
                                )}
                              </button>
                            )}
                          </div>
                        </div>
                      );
                    })}
                  </section>
                ))}
                {!groups.length && (
                  <p className="tray-empty">
                    {query.trim() ? t('没有匹配的服务') : t('暂无快捷服务')}
                    {!query.trim() && (
                      <span>{t('运行中、异常及收藏项目的服务会显示在这里。')}</span>
                    )}
                  </p>
                )}
              </>
            )}
          </div>
        </div>
      </div>
      <footer className="tray-footer" ref={footer}>
        {(actionError || portError) && (
          <div className="tray-footer-alert" role="alert" title={t(actionError || portError)}>
            <CircleAlert size={12} aria-hidden="true" />
            <span>{actionError ? t(actionError) : t('监听状态不可用')}</span>
          </div>
        )}
        <div className="tray-footer-nav">
          <button
            className="tray-menu-item"
            disabled={!native || !!busy}
            onClick={() => void navigate({ kind: 'services' })}
          >
            <span className="tray-menu-label">
              <SquareTerminal size={13} aria-hidden="true" />
              {t('打开 DevNook')}
            </span>
          </button>
          <button
            className="tray-menu-item"
            disabled={!native || !!busy}
            onClick={() => void navigate({ kind: 'ports' })}
          >
            <span className="tray-menu-label">
              <Network size={13} aria-hidden="true" />
              {t('端口管理')}
            </span>
          </button>
        </div>
        <button
          className="tray-menu-item tray-quit-item"
          disabled={!native || !!busy}
          title={t('退出应用后，已运行的服务将继续运行。')}
          onClick={() => void call('quit_desktop').catch(report)}
        >
          <span>{t('退出 DevNook')}</span>
          <kbd>⌘Q</kbd>
        </button>
      </footer>
    </main>
  );
}
