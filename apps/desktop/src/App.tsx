import { t, dateLocale } from './i18n';
import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent } from 'react';
import {
  Activity,
  ArrowDownUp,
  Check,
  ChevronRight,
  CircleHelp,
  Code2,
  Folder,
  FolderOpen,
  FolderSearch,
  Grid2X2,
  LayoutList,
  LoaderCircle,
  Monitor,
  MoreHorizontal,
  Network,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  ShieldCheck,
  SquareTerminal,
  Star,
  Table2,
  Trash2,
  Unplug,
  Layers3,
  X,
} from 'lucide-react';
import { call, native } from './api';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useWindowVisible } from './WindowVisibility';
import type { MainNavigation } from './TrayPresentation';
import { LanguageSettings, useLanguage } from './Language';
import { AppearanceSettings, AppearanceToggle, useAppearance, type Appearance } from './Appearance';
import ServicesPage from './ServicesPage';
import WorkspacesPage from './WorkspacesPage';
import AdvancedIntegrations from './AgentsPage';
import Select from './Select';
import { filterPortBindings, portRows, runtimeOptions } from './PortPresentation';
import { formatCpuTime, formatMemoryBytes, serviceIsActive } from './ServicePresentation';
import ProcessCommand from './ProcessCommand';
import CategoryPicker from './CategoryPicker';
import { ProjectBatchBar, ProjectCheckbox } from './ProjectSelection';
import { useProjectPresentation } from './ProjectPresentation';
import ScanModal from './ScanModal';
import OpenWith, { AppSelect, TerminalSelect, appName } from './OpenWith';
import { open as chooseApplication } from '@tauri-apps/plugin-dialog';
import { CopyButton, EmptyState, Modal, ProjectGlyph, Tags } from './components';
import { projectColor } from './ColorLabels';
import {
  defaults,
  typeNames,
  type DevelopmentApp,
  type DevelopmentApps,
  type PortBinding,
  type ProcessDetail,
  type Project,
  type ServiceView,
  type Settings,
  type StopPreview,
} from './types';

type Page = 'all' | 'favorites' | 'ports' | 'services' | 'settings';
type PortStatus = 'ALL' | 'LISTENING' | 'NO_LISTENER';
const pageNames: Record<Page, string> = {
  all: '全部项目',
  favorites: '已收藏',
  ports: '端口管理',
  services: '服务管理',
  settings: '设置',
};
const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));
const pathShort = (path: string) => path.replace(/^\/Users\/[^/]+/, '~');
const time = (value: number) =>
  new Date(value * 1000).toLocaleString(dateLocale(), { hour12: false });

export default function App() {
  const { language } = useLanguage();
  const windowVisible = useWindowVisible();
  const [mainNavigation, setMainNavigation] = useState<MainNavigation | null>(null);
  const [page, setPage] = useState<Page>('all');
  const [showIntegrations, setShowIntegrations] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [ports, setPorts] = useState<PortBinding[]>([]);
  const [serviceSummary, setServiceSummary] = useState<{
    running: number;
    attention: number;
  } | null>(null);
  const [settings, setSettings] = useState<Settings>(defaults);
  const [appCatalog, setAppCatalog] = useState<DevelopmentApps | null>(null);
  const [appsLoading, setAppsLoading] = useState(false);
  const [appsError, setAppsError] = useState('');
  const appRequest = useRef(0);
  const [coreReady, setCoreReady] = useState(false);
  const [connectingCore, setConnectingCore] = useState(native);
  const [connectionError, setConnectionError] = useState('');
  const [connectionAttempt, setConnectionAttempt] = useState(0);
  const [error, setError] = useState('');
  const [portError, setPortError] = useState('');
  const [toast, setToast] = useState('');
  const [query, setQuery] = useState('');
  const [searchCollapsedCategories, setSearchCollapsedCategories] = useState<string[]>([]);
  const [filter, setFilter] = useState('ALL');
  const [category, setCategory] = useState<string | null>(null);
  const [portStatus, setPortStatus] = useState<PortStatus>('ALL');
  const [servicesView, setServicesView] = useState<'services' | 'plans'>('services');
  const {
    presentation,
    setPresentation,
    storageError: presentationStorageError,
  } = useProjectPresentation();
  const { view, groupBy, sort, collapsedCategories } = presentation;
  const [scanOpen, setScanOpen] = useState(false);
  const [editing, setEditing] = useState<Project | null>(null);
  const [batchMode, setBatchMode] = useState(false);
  const [selectedProjectIds, setSelectedProjectIds] = useState<string[]>([]);
  const [batchBusy, setBatchBusy] = useState(false);
  const [serviceProjectId, setServiceProjectId] = useState('');
  const [portFocus, setPortFocus] = useState<number | null>(null);
  const [removing, setRemoving] = useState<Project | null>(null);
  const [lastRefresh, setLastRefresh] = useState<Date | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const portRefresh = useRef<Promise<void> | null>(null);
  const refreshAgain = useRef(false);

  useEffect(() => {
    if (!native) return;
    let cancelled = false;
    let lastSequence = 0;
    const take = async () => {
      try {
        const request = await call<MainNavigation | null>('take_main_navigation');
        if (!cancelled && request && request.sequence > lastSequence) {
          lastSequence = request.sequence;
          setMainNavigation((previous) =>
            !previous || request.sequence > previous.sequence ? request : previous,
          );
        }
      } catch (error) {
        if (!cancelled) setError(errorText(error));
      }
    };
    const subscription = getCurrentWindow().listen('devnook:navigate', () => void take());
    void subscription.then(() => {
      if (!cancelled) void take();
    });
    return () => {
      cancelled = true;
      void subscription.then((unlisten) => unlisten());
    };
  }, []);
  const navigationHandled = useCallback((sequence: number) => {
    setMainNavigation((request) => (request?.sequence === sequence ? null : request));
  }, []);
  useEffect(() => {
    if (!mainNavigation || batchBusy || actionBusy || scanOpen || editing || removing) return;
    endBatch();
    if (mainNavigation.destination.kind === 'ports') {
      setPage('ports');
      setPortFocus(null);
      navigationHandled(mainNavigation.sequence);
      return;
    }
    setPage('services');
    setServicesView('services');
    setServiceProjectId('');
  }, [mainNavigation, navigationHandled, batchBusy, actionBusy, scanOpen, editing, removing]);

  useEffect(() => setSearchCollapsedCategories([]), [query]);

  const notify = useCallback((message: string) => setToast(message), []);
  useEffect(() => {
    if (toast) {
      const timer = setTimeout(() => setToast(''), 3500);
      return () => clearTimeout(timer);
    }
  }, [toast]);
  const report = useCallback((e: unknown) => setError(errorText(e)), []);
  const { appearance, changeAppearance, storageError } = useAppearance(report);
  useEffect(() => {
    if (!native) return;
    let cancelled = false;
    setConnectingCore(true);
    setConnectionError('');
    Promise.all([call<Project[]>('list_projects'), call<Settings>('get_settings')])
      .then(([p, s]) => {
        if (!cancelled) {
          setProjects(p);
          setSettings(s);
          setCoreReady(true);
        }
      })
      .catch((e) => {
        if (!cancelled) setConnectionError(errorText(e));
      })
      .finally(() => {
        if (!cancelled) setConnectingCore(false);
      });
    return () => {
      cancelled = true;
    };
  }, [connectionAttempt]);
  const refreshApps = useCallback(async () => {
    if (!native) return;
    const request = ++appRequest.current;
    setAppsLoading(true);
    try {
      const catalog = await call<DevelopmentApps>('list_development_apps');
      if (request === appRequest.current) {
        setAppCatalog(catalog);
        setAppsError('');
      }
    } catch (e) {
      if (request === appRequest.current) {
        setAppsError(errorText(e));
        setAppCatalog(null);
      }
    } finally {
      if (request === appRequest.current) setAppsLoading(false);
    }
  }, []);
  const appDiscoveryKey = useMemo(
    () =>
      JSON.stringify({
        projects: projects.map((p) => [p.id, p.canonical_path, p.project_type, p.default_ide]),
        settings: [
          settings.default_ide,
          settings.default_terminal,
          settings.idea_path,
          settings.vscode_path,
          settings.cursor_path,
        ],
      }),
    [projects, settings],
  );
  useEffect(() => {
    if (coreReady) void refreshApps();
  }, [coreReady, appDiscoveryKey, refreshApps]);
  async function chooseApp(): Promise<DevelopmentApp | null> {
    try {
      const path = await chooseApplication({
        multiple: false,
        directory: false,
        defaultPath: '/Applications',
        filters: [{ name: t('macOS 应用'), extensions: ['app'] }],
      });
      if (!path || Array.isArray(path)) return null;
      const app = await call<DevelopmentApp>('remember_development_app', { path });
      await refreshApps();
      return app;
    } catch (e) {
      report(e);
      return null;
    }
  }
  async function setProjectApp(project: Project, app: DevelopmentApp) {
    setActionBusy(true);
    try {
      setProjects(await call<Project[]>('set_project_app', { id: project.id, target: app.id }));
      notify(t('已将 {{0}} 设为此项目默认应用', [app.name]));
    } catch (e) {
      report(e);
    } finally {
      setActionBusy(false);
    }
  }
  const refresh = useCallback((): Promise<void> => {
    if (!native) return Promise.resolve();
    if (portRefresh.current) {
      refreshAgain.current = true;
      return portRefresh.current;
    }
    setRefreshing(true);
    portRefresh.current = (async () => {
      try {
        do {
          refreshAgain.current = false;
          try {
            const [values, serviceViews] = await Promise.all([
              call<PortBinding[]>('list_ports'),
              call<ServiceView[]>('list_services').catch(() => null),
            ]);
            // A post-import / assignment request must not be satisfied by an older snapshot.
            if (!refreshAgain.current) {
              setPorts(values);
              setLastRefresh(new Date());
              setPortError('');
              if (serviceViews) {
                let running = 0;
                let attention = 0;
                for (const item of serviceViews) {
                  const state = item.current_run?.state;
                  if (state && ['BLOCKED', 'CRASHED', 'UNKNOWN'].includes(state)) attention++;
                  else if (serviceIsActive(state)) running++;
                }
                setServiceSummary({ running, attention });
              }
            }
          } catch (e) {
            setPortError(errorText(e));
            setPorts([]);
            setLastRefresh(null);
          }
        } while (refreshAgain.current);
      } finally {
        portRefresh.current = null;
        setRefreshing(false);
      }
    })();
    return portRefresh.current;
  }, []);
  useEffect(() => {
    if (!coreReady || !windowVisible) return;
    void refresh();
    const timer = setInterval(() => void refresh(), settings.refresh_seconds * 1000);
    return () => clearInterval(timer);
  }, [coreReady, windowVisible, refresh, settings.refresh_seconds]);
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        searchRef.current?.focus();
      }
      if ((e.metaKey || e.ctrlKey) && e.key === ',' && !batchBusy) {
        e.preventDefault();
        setPage('settings');
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [batchBusy]);

  const servicePorts = useMemo(() => ports.filter((p) => p.process_kind === 'SERVICE'), [ports]);
  const projectServicePortCount = useMemo(
    () => new Set(servicePorts.filter((p) => p.project_id).map((p) => `${p.pid}:${p.port}`)).size,
    [servicePorts],
  );
  const activeIds = useMemo(
    () => new Set(servicePorts.map((p) => p.project_id).filter(Boolean)),
    [servicePorts],
  );
  const favorites = projects.filter((p) => p.is_favorite).length;
  const categories = useMemo(
    () =>
      [...new Set(projects.map((p) => p.category).filter(Boolean))].sort((a, b) =>
        a.localeCompare(b, dateLocale()),
      ),
    [projects, language],
  );
  const filtered = projects
    .filter((p) => {
      if (page === 'favorites' && !p.is_favorite) return false;
      if (
        portStatus !== 'ALL' &&
        (!lastRefresh || activeIds.has(p.id) !== (portStatus === 'LISTENING'))
      )
        return false;
      if (filter !== 'ALL' && p.project_type !== filter) return false;
      if (category !== null && p.category !== category) return false;
      return [p.name, p.canonical_path, p.category, ...p.frameworks, ...p.build_tools]
        .join(' ')
        .toLowerCase()
        .includes(query.toLowerCase());
    })
    .sort((a, b) =>
      sort === 'name'
        ? a.name.localeCompare(b.name, dateLocale())
        : b.created_at - a.created_at || a.name.localeCompare(b.name, dateLocale()),
    );
  const groupedProjects = new Map<string, Project[]>();
  for (const project of filtered) {
    const items = groupedProjects.get(project.category) || [];
    items.push(project);
    groupedProjects.set(project.category, items);
  }
  const projectGroups = [...groupedProjects.entries()].sort(([a], [b]) =>
    a === b ? 0 : !a ? 1 : !b ? -1 : a.localeCompare(b, dateLocale()),
  );
  const isGrouped = groupBy === 'category' && categories.length > 0;
  useEffect(() => {
    if (coreReady && category !== null && !categories.includes(category)) setCategory(null);
  }, [coreReady, category, categories]);
  const isFiltered =
    !!query || filter !== 'ALL' || category !== null || portStatus !== 'ALL' || page !== 'all';
  useEffect(() => {
    if (batchBusy) return;
    const visible = new Set(filtered.map((project) => project.id));
    setSelectedProjectIds((current) => {
      const next = current.filter((id) => visible.has(id));
      return next.length === current.length ? current : next;
    });
  }, [projects, query, filter, category, portStatus, page, activeIds, lastRefresh, batchBusy]);

  function toggleProjects(items: Project[]) {
    const ids = items.map((item) => item.id);
    setSelectedProjectIds((current) =>
      ids.every((id) => current.includes(id))
        ? current.filter((id) => !ids.includes(id))
        : [...new Set([...current, ...ids])],
    );
  }
  function endBatch() {
    setBatchMode(false);
    setSelectedProjectIds([]);
  }

  async function update(project: Project) {
    try {
      setProjects(
        await call<Project[]>('edit_project', {
          edit: {
            id: project.id,
            name: project.name,
            default_ide: project.default_ide,
            category: project.category,
            is_favorite: project.is_favorite,
          },
        }),
      );
    } catch (e) {
      report(e);
      throw e;
    }
  }
  async function launch(project: Project, target = project.default_ide) {
    setActionBusy(true);
    try {
      await call('open_project', { id: project.id, target });
      notify(
        t('已请求打开 {{0}}', [
          target === 'FINDER'
            ? 'Finder'
            : target === 'TERMINAL'
              ? appCatalog?.terminal_default?.name || 'Terminal'
              : appName(target, appCatalog?.apps || []),
        ]),
      );
    } catch (e) {
      report(e);
    } finally {
      setActionBusy(false);
    }
  }
  function navigate(next: Page) {
    if (batchBusy) return;
    endBatch();
    setPage(next);
    setShowIntegrations(false);
    setServiceProjectId('');
    setPortFocus(null);
    setServicesView('services');
    setPortStatus('ALL');
    setQuery('');
    setFilter('ALL');
    setCategory(null);
  }

  function serviceNavigation() {
    return (
      <nav className="service-section-switch" aria-label={t('服务管理视图')}>
        <button
          type="button"
          aria-pressed={servicesView === 'services'}
          onClick={() => setServicesView('services')}
        >
          <SquareTerminal size={16} />
          {t('服务列表')}
        </button>
        <button
          type="button"
          aria-pressed={servicesView === 'plans'}
          onClick={() => {
            setServiceProjectId('');
            setServicesView('plans');
          }}
        >
          <Layers3 size={16} />
          {t('启动方案')}
        </button>
      </nav>
    );
  }

  const nav = [
    { id: 'all' as const, icon: Folder, count: projects.length },
    { id: 'favorites' as const, icon: Star, count: favorites },
  ];
  const isWorkbenchPage = ['all', 'favorites', 'ports', 'services'].includes(page);

  function renderProjects(items: Project[]) {
    return (
      <div className={`projects projects-${view} ${batchMode ? 'projects-selection' : ''}`}>
        {view === 'table' && (
          <div className="table-head">
            <span>{t('项目')}</span>
            <span>{t('技术栈')}</span>
            <span>{t('监听状态')}</span>
            <span>{t('快捷操作')}</span>
          </div>
        )}
        {items.map((project) => (
          <article
            className={`project-card ${batchMode && selectedProjectIds.includes(project.id) ? 'project-selected' : ''}`}
            key={project.id}
            data-color={projectColor(project.id)}
          >
            <div className="project-card-top">
              {batchMode && (
                <ProjectCheckbox
                  label={t('选择 {{0}}', [project.name])}
                  checked={selectedProjectIds.includes(project.id)}
                  disabled={batchBusy}
                  change={() => toggleProjects([project])}
                />
              )}
              <button
                className="project-title-button"
                disabled={batchBusy}
                onClick={() => {
                  endBatch();
                  setServiceProjectId(project.id);
                  setServicesView('services');
                  setPage('services');
                }}
              >
                <ProjectGlyph type={project.project_type} />
                <span>
                  <strong>{project.name}</strong>
                  <span className="project-subtitle">
                    {(!isGrouped && project.category) ||
                      (project.components?.some((c) => c.runnable)
                        ? t('{{0}} 个服务', [
                            project.components.filter(
                              (c) => c.runnable && c.canonical_path !== project.canonical_path,
                            ).length || 1,
                          ])
                        : t('本地项目'))}
                  </span>
                </span>
              </button>
              <button
                className={`icon-button favorite ${project.is_favorite ? 'is-favorite' : ''}`}
                aria-label={`${project.is_favorite ? t('取消收藏') : t('收藏')} ${project.name}`}
                disabled={batchBusy}
                onClick={() =>
                  void update({
                    ...project,
                    is_favorite: !project.is_favorite,
                  }).catch(() => {})
                }
              >
                <Star size={17} fill={project.is_favorite ? 'currentColor' : 'none'} />
              </button>
            </div>
            <button
              className="project-path"
              title={project.canonical_path}
              disabled={batchBusy}
              onClick={() => void launch(project, 'FINDER')}
            >
              <Folder size={13} />
              <span>{pathShort(project.canonical_path)}</span>
            </button>
            <Tags project={project} />
            <div className="project-status">
              <span className={`status-dot ${activeIds.has(project.id) ? '' : 'inactive'}`} />
              {lastRefresh
                ? activeIds.has(project.id)
                  ? t('发现 {{0}} 个服务监听进程', [
                      new Set(
                        servicePorts.filter((p) => p.project_id === project.id).map((p) => p.pid),
                      ).size,
                    ])
                  : t('未检测到服务监听')
                : t('监听状态未获取')}
            </div>
            <div className="project-card-bottom">
              <button
                className="text-button"
                disabled={batchBusy}
                onClick={() => {
                  endBatch();
                  setServiceProjectId(project.id);
                  setServicesView('services');
                  setPage('services');
                }}
              >
                <SquareTerminal size={14} />
                {t('服务')}
              </button>
              <OpenWith
                project={project}
                catalog={appCatalog}
                loading={appsLoading}
                error={t(appsError)}
                disabled={!coreReady || actionBusy || batchBusy}
                launch={launch}
                setDefault={setProjectApp}
                choose={chooseApp}
                refresh={refreshApps}
              />
              <button
                className="icon-button"
                aria-label={t('管理 {{0}}', [project.name])}
                disabled={batchBusy}
                onClick={() => setEditing(project)}
              >
                <MoreHorizontal size={18} />
              </button>
            </div>
          </article>
        ))}
      </div>
    );
  }

  return (
    <div className={`app-shell ${isWorkbenchPage ? 'app-shell-workbench' : ''}`}>
      <aside className="sidebar">
        <a
          className="brand"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            navigate('all');
          }}
          aria-label={t('DevNook 首页')}
        >
          <img
            className="brand-mark"
            src="/devnook.svg"
            width={32}
            height={32}
            alt=""
            draggable={false}
          />
          <span>DevNook</span>
        </a>
        <nav aria-label={t('主导航')}>
          {nav.map(({ id, icon: Icon, count }) => (
            <button
              key={id}
              disabled={batchBusy}
              onClick={() => navigate(id)}
              className={`nav-item ${page === id ? 'selected' : ''}`}
            >
              <Icon size={18} />
              <span>{t(pageNames[id])}</span>
              <span className="nav-count">{coreReady ? count : '—'}</span>
            </button>
          ))}
          <button
            className={'nav-item ' + (page === 'services' ? 'selected' : '')}
            disabled={batchBusy}
            onClick={() => navigate('services')}
          >
            <SquareTerminal size={18} />
            <span>{t('服务管理')}</span>
            {coreReady && serviceSummary && serviceSummary.attention > 0 ? (
              <span
                className="nav-badge nav-badge-danger"
                title={t('{{0}} 个服务需要关注', [serviceSummary.attention])}
              >
                {serviceSummary.attention}
              </span>
            ) : coreReady && serviceSummary && serviceSummary.running > 0 ? (
              <span
                className="nav-badge nav-badge-success"
                title={t('{{0}} 个托管服务运行中', [serviceSummary.running])}
              >
                <span className="nav-badge-dot" aria-hidden="true" />
                {serviceSummary.running}
              </span>
            ) : null}
          </button>
          <button
            className={`nav-item ${page === 'ports' ? 'selected' : ''}`}
            disabled={batchBusy}
            onClick={() => navigate('ports')}
          >
            <Network size={18} />
            <span>{t('端口管理')}</span>
            {lastRefresh && projectServicePortCount > 0 && (
              <span className="nav-count">{projectServicePortCount}</span>
            )}
          </button>
        </nav>
        <div className="sidebar-bottom">
          <nav aria-label={t('工具与设置')}>
            <button
              className={`nav-item ${page === 'settings' ? 'selected' : ''}`}
              onClick={() => navigate('settings')}
            >
              <Settings2 size={18} />
              <span>{t('设置')}</span>
              <span className="key-hint">⌘ ,</span>
            </button>
          </nav>
          <div className="local-status" role="status" aria-label={t('后台连接状态')}>
            <span className={`status-dot ${coreReady ? '' : 'inactive'}`} />
            <span>
              {native
                ? coreReady
                  ? t('后台已连接')
                  : connectingCore
                    ? t('正在连接后台')
                    : t('后台未连接')
                : t('浏览器界面预览')}
            </span>
          </div>
        </div>
      </aside>

      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            <span>{t('工作台')}</span>
            <ChevronRight size={14} />
            <strong>{t(pageNames[page])}</strong>
            {page === 'services' && servicesView === 'plans' && (
              <>
                <ChevronRight size={14} />
                <strong>{t('启动方案')}</strong>
              </>
            )}
          </div>
          <div className="topbar-right">
            <AppearanceToggle appearance={appearance} change={changeAppearance} />
            <button
              className="icon-button"
              aria-label={t('关于 DevNook')}
              onClick={() => navigate('settings')}
            >
              <CircleHelp size={18} />
            </button>
          </div>
        </header>
        <main
          className={
            isWorkbenchPage
              ? `workbench-page ${page === 'ports' ? 'ports-page' : page === 'services' ? (servicesView === 'plans' ? 'plans-workbench' : 'services-workbench') : 'projects-page'}`
              : undefined
          }
        >
          {!native && (
            <div className="preview-banner">
              <Monitor size={15} />
              <span>{t('浏览器预览 · 请启动桌面应用以扫描项目和读取本机进程。')}</span>
              <code>pnpm tauri dev</code>
            </div>
          )}
          {native && !coreReady && (
            <section
              className="connection-banner"
              role={connectionError ? 'alert' : 'status'}
              aria-label={t('Core 连接状态')}
            >
              <div>
                <strong>
                  {connectingCore ? t('正在连接本地 Core…') : t('暂时无法连接本地 Core')}
                </strong>
                {connectionError && <p>{t(connectionError)}</p>}
                {connectionError.includes('协议版本不一致') && (
                  <p>
                    {t(
                      '请在旧版中正常停止托管服务，再退出旧应用及 CLI/MCP 客户端。 等后台正常退出后，点击“重新连接”。项目和服务配置会保留。',
                    )}
                  </p>
                )}
              </div>
              <button
                className="button secondary small"
                type="button"
                disabled={connectingCore}
                onClick={() => setConnectionAttempt((attempt) => attempt + 1)}
              >
                <RefreshCw size={14} className={connectingCore ? 'spin' : ''} />
                {connectingCore ? t('正在连接…') : t('重新连接')}
              </button>
            </section>
          )}
          {error && (
            <div className="error-banner" role="alert">
              <span>{t(error)}</span>
              <button
                className="icon-button"
                aria-label={t('关闭错误提示')}
                onClick={() => setError('')}
              >
                <X size={16} />
              </button>
            </div>
          )}
          {page !== 'services' && page !== 'ports' && (
            <div className={`page-heading ${page !== 'settings' ? 'page-heading-compact' : ''}`}>
              <div className="page-heading-main">
                <h1>{t(pageNames[page])}</h1>
                {page === 'settings' ? (
                  <p>{t('让 DevNook 适应你的开发习惯。')}</p>
                ) : (
                  !batchMode && (
                    <div className="heading-stats" role="group" aria-label={t('工作台')}>
                      <button
                        type="button"
                        className={`heading-stat-pill ${portStatus === 'ALL' ? 'active' : ''}`}
                        title={t('显示全部项目')}
                        onClick={() => setPortStatus('ALL')}
                      >
                        <Folder size={13} aria-hidden="true" />
                        <span>{t('本地项目')}</span>
                        <strong>{coreReady ? projects.length : '—'}</strong>
                      </button>
                      <button
                        type="button"
                        className={`heading-stat-pill ${portStatus === 'LISTENING' ? 'active' : ''}`}
                        disabled={!lastRefresh}
                        title={t('筛选正在监听的项目')}
                        onClick={() =>
                          setPortStatus((current) =>
                            current === 'LISTENING' ? 'ALL' : 'LISTENING',
                          )
                        }
                      >
                        <Activity size={13} aria-hidden="true" />
                        <span>{t('监听中')}</span>
                        <strong>{lastRefresh ? activeIds.size : '—'}</strong>
                      </button>
                      <button
                        type="button"
                        className="heading-stat-pill"
                        disabled={!coreReady}
                        title={t('查看全部监听端口')}
                        onClick={() => navigate('ports')}
                      >
                        <Network size={13} aria-hidden="true" />
                        <span>{t('TCP 绑定')}</span>
                        <strong>{lastRefresh ? ports.length : '—'}</strong>
                      </button>
                    </div>
                  )
                )}
              </div>
              {page !== 'settings' && (
                <div className="heading-actions">
                  <button
                    className={`button secondary small ${refreshing ? 'loading' : ''}`}
                    onClick={() => void refresh()}
                    disabled={!coreReady || refreshing}
                  >
                    <RefreshCw size={14} className={refreshing ? 'spin' : ''} />
                    {t('刷新')}
                  </button>
                  <button
                    className="button primary small"
                    onClick={() => setScanOpen(true)}
                    disabled={!coreReady || batchBusy}
                  >
                    <Plus size={15} />
                    {t('扫描项目')}
                  </button>
                </div>
              )}
            </div>
          )}

          {page === 'settings' ? (
            <SettingsPage
              showIntegrations={showIntegrations}
              appearance={appearance}
              changeAppearance={changeAppearance}
              appearanceStorageError={storageError}
              settings={settings}
              catalog={appCatalog}
              appsLoading={appsLoading}
              appsError={t(appsError)}
              chooseApp={chooseApp}
              refreshApps={refreshApps}
              disabled={!coreReady}
              projects={projects}
              notify={notify}
              onSave={async (s) => {
                await call('save_settings', { settings: s });
                setSettings(s);
                notify(t('设置已保存'));
              }}
              report={report}
            />
          ) : page === 'services' ? (
            <section className="service-management">
              {servicesView === 'plans' && (
                <header className="service-management-header">
                  <h1 className="service-page-title">{t('服务管理')}</h1>
                  {serviceNavigation()}
                </header>
              )}
              {servicesView === 'plans' ? (
                <WorkspacesPage
                  disabled={!coreReady || !windowVisible}
                  report={report}
                  notify={notify}
                />
              ) : (
                <ServicesPage
                  navigation={serviceNavigation()}
                  navigationRequest={mainNavigation}
                  navigationHandled={navigationHandled}
                  projects={projects}
                  ports={ports}
                  portError={t(portError)}
                  lastRefresh={lastRefresh}
                  refreshingPorts={refreshing}
                  refreshPorts={refresh}
                  disabled={!coreReady || !windowVisible}
                  projectId={serviceProjectId}
                  back={() => navigate('all')}
                  editProject={setEditing}
                  updateProject={update}
                  launchProject={launch}
                  inspectPort={(pid) => {
                    navigate('ports');
                    setPortFocus(pid);
                  }}
                  showAllServices={() => setServiceProjectId('')}
                  openIntegrations={() => {
                    navigate('settings');
                    setShowIntegrations(true);
                  }}
                  report={report}
                  notify={notify}
                />
              )}
            </section>
          ) : (
            <>
              {portError && (
                <div className="error-banner" role="alert">
                  <span>
                    {t('监听状态不可用：')}
                    {t(portError)}
                  </span>
                  <button className="button small" onClick={() => void refresh()}>
                    {t('重试')}
                  </button>
                </div>
              )}
              {page === 'ports' ? (
                <PortsPage
                  initialPid={portFocus}
                  ports={ports}
                  projects={projects}
                  query={query}
                  setQuery={setQuery}
                  searchRef={searchRef}
                  refresh={refresh}
                  lastRefresh={lastRefresh}
                  refreshing={refreshing}
                  snapshotError={t(portError)}
                  report={report}
                  notify={notify}
                  disabled={!coreReady}
                />
              ) : (
                <section className="projects-section">
                  <div className="project-toolbar">
                    <div className="search-field">
                      <Search size={17} />
                      <input
                        ref={searchRef}
                        aria-label={t('搜索项目')}
                        placeholder={t('搜索项目名称、路径或技术栈…')}
                        value={query}
                        disabled={batchBusy}
                        onChange={(e) => {
                          setSelectedProjectIds([]);
                          setQuery(e.target.value);
                        }}
                      />
                      <kbd>⌘ K</kbd>
                    </div>
                    <div className="toolbar-controls">
                      <Select
                        aria-label={t('按技术栈筛选')}
                        value={filter}
                        disabled={batchBusy}
                        onValueChange={(value) => {
                          setSelectedProjectIds([]);
                          setFilter(value);
                        }}
                        options={[
                          { value: 'ALL', label: t('所有技术栈') },
                          ...Object.entries(typeNames).map(([value, label]) => ({ value, label })),
                        ]}
                      />
                      <div className="project-port-filter">
                        <label htmlFor="project-port-status">{t('端口状态')}</label>
                        <Select
                          id="project-port-status"
                          aria-label={t('按端口状态筛选')}
                          aria-describedby="project-port-hint"
                          value={portStatus}
                          disabled={batchBusy}
                          onValueChange={(value) => {
                            setSelectedProjectIds([]);
                            setPortStatus(value as PortStatus);
                          }}
                          options={[
                            { value: 'ALL', label: t('全部') },
                            {
                              value: 'LISTENING',
                              label: t('检测到服务监听'),
                              disabled: !lastRefresh,
                            },
                            {
                              value: 'NO_LISTENER',
                              label: t('未检测到服务监听'),
                              disabled: !lastRefresh,
                            },
                          ]}
                        />
                      </div>
                      {categories.length > 0 && (
                        <Select
                          aria-label={t('按分类筛选')}
                          value={category === null ? 'all' : `category:${category}`}
                          disabled={batchBusy}
                          onValueChange={(value) => {
                            setSelectedProjectIds([]);
                            setCategory(value === 'all' ? null : value.slice('category:'.length));
                          }}
                          options={[
                            { value: 'all', label: t('全部分类') },
                            ...categories.map((value) => ({
                              value: `category:${value}`,
                              label: value,
                            })),
                          ]}
                        />
                      )}
                      <div className="view-toggle" aria-label={t('项目视图')}>
                        {(
                          [
                            { id: 'grid', icon: Grid2X2, label: t('卡片视图') },
                            { id: 'compact', icon: LayoutList, label: t('紧凑视图') },
                            { id: 'table', icon: Table2, label: t('表格视图') },
                          ] as const
                        ).map(({ id, icon: Icon, label }) => (
                          <button
                            key={id}
                            className={view === id ? 'active' : ''}
                            aria-label={label}
                            aria-pressed={view === id}
                            onClick={() => setPresentation((current) => ({ ...current, view: id }))}
                          >
                            <Icon size={17} />
                          </button>
                        ))}
                      </div>
                    </div>
                  </div>
                  <p id="project-port-hint" className="project-filter-hint">
                    {lastRefresh
                      ? t('仅统计已识别的服务监听，未检测到不代表项目已停止；全部监听见端口管理。')
                      : t('端口状态尚未获取，获取成功后可按监听状态筛选。')}
                  </p>
                  <div className="section-line">
                    {batchMode ? (
                      <div className="project-select-all">
                        <ProjectCheckbox
                          label={t('全选当前结果')}
                          checked={
                            filtered.length > 0 &&
                            filtered.every((item) => selectedProjectIds.includes(item.id))
                          }
                          mixed={
                            filtered.some((item) => selectedProjectIds.includes(item.id)) &&
                            !filtered.every((item) => selectedProjectIds.includes(item.id))
                          }
                          disabled={batchBusy || !filtered.length}
                          change={() => toggleProjects(filtered)}
                        />
                        <span>
                          {t('全选当前结果（')}
                          {filtered.length}）
                        </span>
                      </div>
                    ) : (
                      <span>
                        {query ? t('搜索结果') : t(pageNames[page])}
                        <span className="subtle-count">{coreReady ? filtered.length : '—'}</span>
                      </span>
                    )}
                    <div className="project-presentation-controls">
                      <button
                        type="button"
                        className="button secondary small"
                        disabled={!coreReady || batchBusy || (!batchMode && !filtered.length)}
                        aria-pressed={batchMode}
                        onClick={() => (batchMode ? endBatch() : setBatchMode(true))}
                      >
                        {batchMode ? t('退出多选') : t('批量分类')}
                      </button>
                      <Select
                        aria-label={t('项目分组方式')}
                        value={groupBy}
                        onValueChange={(value) =>
                          setPresentation((current) => ({
                            ...current,
                            groupBy: value as 'none' | 'category',
                          }))
                        }
                        options={[
                          { value: 'none', label: t('不分组') },
                          { value: 'category', label: t('按分类分组') },
                        ]}
                      />
                      <button
                        className="text-button"
                        onClick={() =>
                          setPresentation((current) => ({
                            ...current,
                            sort: current.sort === 'recent' ? 'name' : 'recent',
                          }))
                        }
                      >
                        <ArrowDownUp size={13} />
                        {sort === 'recent' ? t('最近添加') : t('名称排序')}
                      </button>
                    </div>
                  </div>
                  {batchMode && (
                    <>
                      <ProjectBatchBar
                        ids={selectedProjectIds}
                        categories={categories}
                        busy={batchBusy}
                        setBusy={setBatchBusy}
                        clear={() => setSelectedProjectIds([])}
                        notify={notify}
                        updated={(values, completed) => {
                          if (values) setProjects(values);
                          setSelectedProjectIds((current) =>
                            current.filter(
                              (id) =>
                                !completed.has(id) &&
                                (!values || values.some((item) => item.id === id)),
                            ),
                          );
                        }}
                      />
                    </>
                  )}
                  <div
                    className="project-results"
                    role="region"
                    aria-label={t('项目列表')}
                    tabIndex={0}
                    data-empty={filtered.length === 0}
                  >
                    {filtered.length === 0 ? (
                      <EmptyState
                        title={
                          native && !coreReady
                            ? connectingCore
                              ? t('正在读取项目库')
                              : t('项目库暂不可用')
                            : portStatus !== 'ALL' && !lastRefresh
                              ? t('端口状态暂不可用')
                              : undefined
                        }
                        description={
                          native && !coreReady
                            ? t('连接成功后会显示已保存的项目；已有档案不会被清空。')
                            : portStatus !== 'ALL' && !lastRefresh
                              ? t(
                                  '无法判断项目是否存在监听端口，请重试读取，或将端口状态切回“全部”。',
                                )
                              : undefined
                        }
                        filtered={isFiltered || (native && !coreReady)}
                        scan={() => setScanOpen(true)}
                        disabled={!coreReady}
                      />
                    ) : isGrouped ? (
                      <div className="project-groups">
                        {projectGroups.map(([name, items]) => {
                          const collapsed = (
                            query ? searchCollapsedCategories : collapsedCategories
                          ).includes(name);
                          const id = `project-group-${encodeURIComponent(name)}`;
                          return (
                            <section className="project-group" key={name}>
                              <h2 className="project-group-heading">
                                {batchMode && (
                                  <ProjectCheckbox
                                    label={t('选择{{0}}组', [name || t('未分类')])}
                                    checked={items.every((item) =>
                                      selectedProjectIds.includes(item.id),
                                    )}
                                    mixed={
                                      items.some((item) => selectedProjectIds.includes(item.id)) &&
                                      !items.every((item) => selectedProjectIds.includes(item.id))
                                    }
                                    disabled={batchBusy}
                                    change={() => toggleProjects(items)}
                                  />
                                )}
                                <button
                                  type="button"
                                  aria-expanded={!collapsed}
                                  aria-controls={id}
                                  onClick={() => {
                                    const toggle = (items: string[]) =>
                                      items.includes(name)
                                        ? items.filter((item) => item !== name)
                                        : [...items, name];
                                    if (query) setSearchCollapsedCategories(toggle);
                                    else
                                      setPresentation((current) => ({
                                        ...current,
                                        collapsedCategories: toggle(current.collapsedCategories),
                                      }));
                                  }}
                                >
                                  <ChevronRight
                                    size={15}
                                    className={collapsed ? '' : 'group-expanded'}
                                  />
                                  <span>{name || t('未分类')}</span>
                                  <span className="subtle-count">{items.length}</span>
                                </button>
                              </h2>
                              <div id={id} hidden={collapsed}>
                                {renderProjects(items)}
                              </div>
                            </section>
                          );
                        })}
                      </div>
                    ) : (
                      renderProjects(filtered)
                    )}
                  </div>
                  {presentationStorageError && (
                    <footer className="workspace-footer">
                      <span role="status">
                        <span className="status-dot inactive" />
                        {t('展示选择已生效，但无法记住；重开后可能恢复默认。')}
                      </span>
                    </footer>
                  )}
                </section>
              )}
            </>
          )}
        </main>
      </div>
      {toast && (
        <div className="toast" role="status">
          <Check size={17} />
          {t(toast)}
        </div>
      )}
      {scanOpen && (
        <ScanModal
          settings={settings}
          close={() => setScanOpen(false)}
          report={report}
          imported={(p) => {
            setProjects(p);
            setScanOpen(false);
            setPorts([]);
            setLastRefresh(null);
            void refresh();
            notify(t('项目已导入本地工作台'));
          }}
        />
      )}
      {editing && (
        <ProjectModal
          project={editing}
          categories={categories}
          catalog={appCatalog}
          appsLoading={appsLoading}
          appsError={t(appsError)}
          chooseApp={chooseApp}
          refreshApps={refreshApps}
          close={() => setEditing(null)}
          launch={launch}
          save={async (p) => {
            await update(p);
            setEditing(null);
            notify(t('项目档案已保存'));
          }}
          remove={() => {
            setRemoving(editing);
            setEditing(null);
          }}
          report={report}
          notify={notify}
        />
      )}
      {removing && (
        <Modal
          title={t('移除项目档案？')}
          subtitle={t('仅移除 DevNook 中的记录，项目目录和源码会保留。')}
          close={() => setRemoving(null)}
        >
          <div className="modal-body">
            <strong>{removing.name}</strong>
            <code className="path-block">{removing.canonical_path}</code>
          </div>
          <div className="modal-actions">
            <button className="button secondary" onClick={() => setRemoving(null)}>
              {t('取消')}
            </button>
            <button
              className="button danger"
              disabled={actionBusy}
              onClick={async () => {
                setActionBusy(true);
                try {
                  await call('remove_project', { id: removing.id });
                  setProjects((p) => p.filter((v) => v.id !== removing.id));
                  setRemoving(null);
                  notify(t('项目档案已移除'));
                } catch (e) {
                  report(e);
                } finally {
                  setActionBusy(false);
                }
              }}
            >
              {t('移除档案')}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}

function ProjectModal({
  project,
  categories,
  catalog,
  appsLoading,
  appsError,
  chooseApp,
  refreshApps,
  close,
  save,
  launch,
  remove,
  report,
  notify,
}: {
  project: Project;
  categories: string[];
  catalog: DevelopmentApps | null;
  appsLoading: boolean;
  appsError: string;
  chooseApp: () => Promise<DevelopmentApp | null>;
  refreshApps: () => Promise<void>;
  close: () => void;
  save: (p: Project) => Promise<void>;
  launch: (p: Project, target?: string) => Promise<void>;
  remove: () => void;
  report: (e: unknown) => void;
  notify: (s: string) => void;
}) {
  const [edit, setEdit] = useState(project);
  const [busy, setBusy] = useState(false);
  return (
    <Modal title={t('编辑项目')} subtitle={t('整理项目偏好，源目录保持原样。')} close={close}>
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          try {
            await save({ ...edit, category: edit.category.trim() });
          } catch (e) {
            report(e);
          } finally {
            setBusy(false);
          }
        }}
      >
        <div className="modal-body form-stack">
          <label>
            {t('项目名称')}
            <input
              required
              maxLength={160}
              value={edit.name}
              onChange={(e) => setEdit({ ...edit, name: e.target.value })}
            />
          </label>
          <label>
            {t('项目路径')}
            <div className="path-with-copy">
              <code>{project.canonical_path}</code>
              <CopyButton
                text={project.canonical_path}
                done={() => notify(t('路径已复制'))}
                failed={report}
              />
            </div>
          </label>
          <div className="form-row">
            <label>
              {t('默认开发应用')}
              <AppSelect
                value={edit.default_ide}
                preference={catalog?.project_defaults[project.id]}
                apps={catalog?.apps || []}
                onChange={(default_ide) => setEdit({ ...edit, default_ide })}
                choose={chooseApp}
                refresh={refreshApps}
                busy={appsLoading}
                disabled={busy}
              />
              {appsError && (
                <span className="field-hint" role="alert">
                  {t(appsError)}
                </span>
              )}
            </label>
            <div className="form-field">
              <label htmlFor="project-category">{t('分类')}</label>
              <CategoryPicker
                id="project-category"
                value={edit.category}
                categories={categories}
                change={(category) => setEdit({ ...edit, category })}
                disabled={busy}
              />
            </div>
          </div>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={edit.is_favorite}
              onChange={(e) => setEdit({ ...edit, is_favorite: e.target.checked })}
            />
            {t('收藏这个项目')}
          </label>
          <div className="project-shortcuts">
            <button type="button" className="button secondary" onClick={() => void launch(edit)}>
              <Code2 size={16} />
              {t('打开开发应用')}
            </button>
            <button
              type="button"
              className="button secondary"
              onClick={() => void launch(project, 'FINDER')}
            >
              <FolderOpen size={16} />
              Finder
            </button>
            <button
              type="button"
              className="button secondary"
              onClick={() => void launch(project, 'TERMINAL')}
            >
              <SquareTerminal size={16} />
              {t('终端')}
            </button>
          </div>
        </div>
        <div className="modal-actions">
          <button type="button" className="text-button danger-text" onClick={remove}>
            <Trash2 size={14} />
            {t('移除档案')}
          </button>
          <span className="flex-spacer" />
          <button type="button" className="button secondary" onClick={close}>
            {t('取消')}
          </button>
          <button type="submit" className="button primary" disabled={busy}>
            {t('保存更改')}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function PortsPage({
  initialPid,
  ports,
  projects,
  query,
  setQuery,
  searchRef,
  refresh,
  lastRefresh,
  refreshing,
  snapshotError,
  report,
  notify,
  disabled,
}: {
  initialPid: number | null;
  ports: PortBinding[];
  projects: Project[];
  query: string;
  setQuery: (s: string) => void;
  searchRef: React.RefObject<HTMLInputElement | null>;
  refresh: () => Promise<void>;
  lastRefresh: Date | null;
  refreshing: boolean;
  snapshotError: string;
  report: (e: unknown) => void;
  notify: (s: string) => void;
  disabled: boolean;
}) {
  const [selected, setSelected] = useState<number | null>(null);
  const [detail, setDetail] = useState<ProcessDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [detailError, setDetailError] = useState('');
  const [stop, setStop] = useState<StopPreview | null>(null);
  const [pidInput, setPidInput] = useState('');
  const [busy, setBusy] = useState(false);
  const [scopeFilter, setScopeFilter] = useState<'ALL' | 'MANAGED' | 'PROJECT' | 'SYSTEM'>('ALL');
  const [projectFilter, setProjectFilter] = useState('ALL');
  const [family, setFamily] = useState('ALL');
  const [runtime, setRuntime] = useState('ALL');
  const [kind, setKind] = useState('ALL');
  const [source, setSource] = useState('ALL');
  const [moreFilters, setMoreFilters] = useState(false);
  const requestRef = useRef(0);
  const choose = useCallback(async (pid: number) => {
    const request = ++requestRef.current;
    setSelected(pid);
    setDetail(null);
    setLoading(true);
    setDetailError('');
    try {
      const value = await call<ProcessDetail>('inspect_process', { pid });
      if (request === requestRef.current) setDetail(value);
    } catch (e) {
      if (request === requestRef.current) setDetailError(errorText(e));
    } finally {
      if (request === requestRef.current) setLoading(false);
    }
  }, []);
  useEffect(() => {
    if (initialPid !== null && !disabled) void choose(initialPid);
  }, [initialPid, disabled, choose]);
  useEffect(() => {
    if (selected === null) return;
    if (!ports.some((p) => p.pid === selected)) {
      requestRef.current++;
      setDetail(null);
      setDetailError(t('进程已退出或不再监听 TCP 端口。'));
      setLoading(false);
    }
  }, [ports, selected]);
  const nameOf = (id: string | null) => projects.find((p) => p.id === id)?.name || t('未关联项目');
  const allRows = portRows(ports);
  const managedCount = allRows.filter((p) => p.source === 'DEVNOOK').length;
  const projectCount = allRows.filter(
    (p) => p.source !== 'DEVNOOK' && p.project_id !== null,
  ).length;
  const systemCount = allRows.filter((p) => p.source !== 'DEVNOOK' && p.project_id === null).length;

  const scopedPorts = ports.filter((p) => {
    if (scopeFilter === 'MANAGED') return p.source === 'DEVNOOK';
    if (scopeFilter === 'PROJECT') return p.source !== 'DEVNOOK' && p.project_id !== null;
    if (scopeFilter === 'SYSTEM') return p.source !== 'DEVNOOK' && p.project_id === null;
    return true;
  });

  const visible = filterPortBindings(scopedPorts, projects, {
    query,
    project: projectFilter,
    family,
    runtime,
    kind,
    source,
  });
  const rows = portRows(visible);
  const total = allRows.length;
  const advancedCount = [runtime, kind, source].filter((v) => v !== 'ALL').length;
  const hasSnapshot = !disabled && !snapshotError && lastRefresh !== null;
  function selectScope(next: 'ALL' | 'MANAGED' | 'PROJECT' | 'SYSTEM') {
    setScopeFilter(next);
    if (next === 'MANAGED') {
      if (source === 'OBSERVED') setSource('ALL');
      if (projectFilter === 'EXTERNAL') setProjectFilter('ALL');
    } else if (next === 'PROJECT') {
      if (source === 'DEVNOOK') setSource('ALL');
      if (projectFilter === 'EXTERNAL') setProjectFilter('ALL');
    } else if (next === 'SYSTEM') {
      if (source === 'DEVNOOK') setSource('ALL');
      if (projectFilter !== 'ALL' && projectFilter !== 'EXTERNAL') setProjectFilter('ALL');
    }
  }
  return (
    <section className="ports-section">
      <header className="ports-header">
        <div className="ports-heading">
          <h1>{t('端口管理')}</h1>
          <p className="ports-result-count" role="status">
            {t('当前结果')} <strong>{hasSnapshot ? rows.length : '—'}</strong> {t('/ 总数')}{' '}
            <strong>{hasSnapshot ? total : '—'}</strong>
          </p>
        </div>
        <div className="ports-header-actions">
          <span className="ports-updated">
            {disabled
              ? t('等待后台连接')
              : refreshing
                ? hasSnapshot
                  ? t('正在刷新 · 上次更新于 ')
                  : t('正在读取快照…')
                : hasSnapshot
                  ? t('更新于 ')
                  : snapshotError
                    ? t('读取失败，更新时间不可用')
                    : t('尚未读取系统快照')}
            {hasSnapshot && (
              <time dateTime={lastRefresh.toISOString()}>
                {lastRefresh.toLocaleTimeString(dateLocale(), { hour12: false })}
              </time>
            )}
          </span>
          <button
            type="button"
            className={`button secondary small ${refreshing ? 'loading' : ''}`}
            onClick={() => void refresh()}
            disabled={disabled || refreshing}
          >
            <RefreshCw size={14} className={refreshing ? 'spin' : ''} />
            {t('刷新')}
          </button>
        </div>
      </header>
      <div className="port-scope-tabs" role="tablist" aria-label={t('快捷分类筛选')}>
        <button
          type="button"
          role="tab"
          aria-selected={scopeFilter === 'ALL'}
          className={`port-scope-tab ${scopeFilter === 'ALL' ? 'active' : ''}`}
          onClick={() => selectScope('ALL')}
        >
          {t('全部')} <span className="tab-count">{hasSnapshot ? total : '—'}</span>
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={scopeFilter === 'MANAGED'}
          className={`port-scope-tab ${scopeFilter === 'MANAGED' ? 'active' : ''}`}
          onClick={() => selectScope('MANAGED')}
        >
          {t('DevNook 托管')} <span className="tab-count">{hasSnapshot ? managedCount : '—'}</span>
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={scopeFilter === 'PROJECT'}
          className={`port-scope-tab ${scopeFilter === 'PROJECT' ? 'active' : ''}`}
          onClick={() => selectScope('PROJECT')}
        >
          {t('项目关联')} <span className="tab-count">{hasSnapshot ? projectCount : '—'}</span>
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={scopeFilter === 'SYSTEM'}
          className={`port-scope-tab ${scopeFilter === 'SYSTEM' ? 'active' : ''}`}
          onClick={() => selectScope('SYSTEM')}
        >
          {t('系统与未关联')} <span className="tab-count">{hasSnapshot ? systemCount : '—'}</span>
        </button>
      </div>
      <div className="ports-toolbar">
        <div className="search-field">
          <Search size={16} />
          <input
            ref={searchRef}
            placeholder={t('搜索端口、PID、命令或项目路径…')}
            aria-label={t('搜索端口')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <Select
          aria-label={t('按项目筛选端口')}
          value={projectFilter}
          onValueChange={setProjectFilter}
          options={[
            { value: 'ALL', label: t('所有项目') },
            { value: 'EXTERNAL', label: t('未关联项目') },
            ...projects.map((project) => ({ value: project.id, label: project.name })),
          ]}
        />
        <Select
          aria-label={t('按地址族筛选')}
          value={family}
          onValueChange={setFamily}
          options={[
            { value: 'ALL', label: 'IPv4 / IPv6' },
            { value: 'IPv4', label: 'IPv4' },
            { value: 'IPv6', label: 'IPv6' },
          ]}
        />
        <button
          className="button secondary small ports-filter-toggle"
          aria-expanded={moreFilters}
          aria-controls="ports-advanced-filters"
          onClick={() => setMoreFilters((value) => !value)}
        >
          <Settings2 size={14} />
          {t('更多筛选')}
          {advancedCount ? ` · ${advancedCount}` : ''}
        </button>
      </div>
      {moreFilters && (
        <div className="ports-advanced-filters" id="ports-advanced-filters">
          <Select
            aria-label={t('按运行时筛选')}
            value={runtime}
            onValueChange={setRuntime}
            options={runtimeOptions.map((option) => ({ ...option, label: t(option.label) }))}
          />
          <Select
            aria-label={t('按进程类型筛选')}
            value={kind}
            onValueChange={setKind}
            options={[
              { value: 'ALL', label: t('全部进程类型') },
              { value: 'SERVICE', label: t('服务') },
              { value: 'DEVELOPMENT_TOOL', label: t('开发工具') },
              { value: 'UNKNOWN', label: t('用途未知') },
            ]}
          />
          <Select
            aria-label={t('按启动来源筛选')}
            value={source}
            onValueChange={setSource}
            options={[
              { value: 'ALL', label: t('全部启动来源') },
              { value: 'DEVNOOK', label: t('DevNook 托管') },
              { value: 'OBSERVED', label: t('外部发现') },
            ]}
          />
          <button
            className="text-button"
            onClick={() => {
              setQuery('');
              setScopeFilter('ALL');
              setProjectFilter('ALL');
              setFamily('ALL');
              setRuntime('ALL');
              setKind('ALL');
              setSource('ALL');
            }}
          >
            {t('清除筛选')}
          </button>
          <span className="field-hint">
            {t('运行时按进程名分组；未核验的启动工具统一归为外部。')}
          </span>
        </div>
      )}
      <p className="ports-count-hint">
        {t('按 PID + 端口计数，IPv4 / IPv6 与多地址合并；含系统和开发工具。')}
      </p>
      <div className="ports-workspace">
        <div className="port-list" role="region" aria-label={t('TCP 监听端口列表')} tabIndex={0}>
          <div className="port-list-header">
            <span>{t('TCP 监听项')}</span>
          </div>
          {rows.length === 0 ? (
            <div className="port-list-empty">
              <Network size={27} strokeWidth={1.3} />
              <p>
                {disabled
                  ? t('等待桌面 Core 连接')
                  : snapshotError
                    ? t('监听快照读取失败，请重试')
                    : !lastRefresh
                      ? t('等待监听快照')
                      : t('没有匹配的监听端口')}
              </p>
            </div>
          ) : (
            rows.map((p) => (
              <button
                key={`${p.pid}:${p.port}`}
                className={`port-row ${selected === p.pid ? 'selected' : ''}`}
                data-color={projectColor(p.project_id)}
                onClick={() => void choose(p.pid)}
              >
                <div className="port-number">
                  <span className="status-dot" aria-hidden="true" />
                  <strong className="port-chip">:{p.port}</strong>
                  <span>{p.name}</span>
                </div>
                <div className="port-meta">
                  <span>{nameOf(p.project_id)}</span>
                  <span>PID {p.pid}</span>
                </div>
                <span className="port-source">
                  <span
                    className={`source-label ${p.source === 'DEVNOOK' ? 'managed' : 'external'}`}
                  >
                    {p.source === 'DEVNOOK' ? t('DevNook 托管') : t('外部发现')}
                  </span>
                  <span>
                    {p.family}
                    {ports.some(
                      (b) => b.pid === p.pid && b.port === p.port && b.family !== p.family,
                    )
                      ? t(' + 双栈')
                      : ''}
                  </span>
                </span>
              </button>
            ))
          )}
        </div>
        <div className="process-panel" role="region" aria-label={t('进程详情')} tabIndex={0}>
          {loading ? (
            <div className="process-placeholder">
              <LoaderCircle className="spin" size={26} />
              <p>{t('正在读取进程详情…')}</p>
            </div>
          ) : detailError ? (
            <div className="process-placeholder">
              <Unplug size={29} />
              <p>{t(detailError)}</p>
              {selected && (
                <button className="button secondary" onClick={() => void choose(selected)}>
                  {t('重新读取')}
                </button>
              )}
            </div>
          ) : !detail ? (
            <div className="process-placeholder">
              <div className="process-placeholder-icon">
                <SquareTerminal size={32} strokeWidth={1.2} />
              </div>
              <h3>{t('每个端口，都有来处。')}</h3>
              <p>
                {t('选择左侧的监听端口，')}
                <br />
                {t('查看进程、工作目录与项目归属。')}
              </p>
              <span className="read-only-note">
                <ShieldCheck size={13} />
                {t('未知进程默认只读')}
              </span>
            </div>
          ) : (
            <>
              <div className="process-panel-heading">
                <span className="process-avatar">
                  <SquareTerminal size={25} />
                </span>
                <div>
                  <span className="eyebrow">PROCESS INSPECTOR</span>
                  <h2>
                    {detail.project_id && nameOf(detail.project_id) !== t('未关联项目')
                      ? `${nameOf(detail.project_id)} · ${detail.name}`
                      : detail.name}
                  </h2>
                  <span className="field-hint">
                    PID {detail.pid} ·{' '}
                    {detail.source === 'DEVNOOK' ? t('DevNook 托管') : t('外部发现')}
                  </span>
                </div>
                <button
                  className="icon-button"
                  aria-label={t('刷新进程详情')}
                  onClick={() => void choose(detail.pid)}
                >
                  <RefreshCw size={16} />
                </button>
              </div>
              <div className="process-binding-tags" data-color={projectColor(detail.project_id)}>
                {detail.bindings.map((b, i) => (
                  <span key={`${b.family}:${b.address}:${b.port}:${i}`}>
                    <span className="status-dot" />
                    <code>
                      {b.address}:{b.port}
                    </code>
                    <small>{b.family}</small>
                  </span>
                ))}
              </div>
              <dl className="process-fields">
                <div>
                  <dt>{t('启动命令')}</dt>
                  <dd>
                    <ProcessCommand
                      key={detail.fingerprint}
                      command={detail.command}
                      notify={notify}
                      report={report}
                    />
                  </dd>
                </div>
                <div>
                  <dt>Executable</dt>
                  <dd>
                    <code>{detail.executable || t('不可访问')}</code>
                  </dd>
                </div>
                <div className="process-numbers">
                  <div>
                    <dt>PID</dt>
                    <dd>{detail.pid}</dd>
                  </div>
                  <div>
                    <dt>PPID</dt>
                    <dd>{detail.ppid}</dd>
                  </div>
                  <div>
                    <dt>PGID</dt>
                    <dd>{detail.pgid}</dd>
                  </div>
                  <div>
                    <dt>{t('运行时长')}</dt>
                    <dd>{detail.elapsed}</dd>
                  </div>
                  {detail.memory_rss_bytes ? (
                    <div>
                      <dt>{t('物理内存')}</dt>
                      <dd>{formatMemoryBytes(detail.memory_rss_bytes)}</dd>
                    </div>
                  ) : null}
                  {detail.cpu_time_ms ? (
                    <div>
                      <dt>{t('CPU 耗时')}</dt>
                      <dd>{formatCpuTime(detail.cpu_time_ms)}</dd>
                    </div>
                  ) : null}
                </div>
                <div>
                  <dt>{t('工作目录')}</dt>
                  <dd>
                    <code>{detail.cwd || t('不可访问或权限不足')}</code>
                  </dd>
                </div>
                <div>
                  <dt>{t('启动时间')}</dt>
                  <dd>{detail.start_time}</dd>
                </div>
                <div>
                  <dt>
                    {t('首次发现监听')}
                    <span className="field-hint">{t('本次 Core 会话')}</span>
                  </dt>
                  <dd>
                    {detail.bindings.length
                      ? time(Math.min(...detail.bindings.map((b) => b.first_seen)))
                      : t('当前无监听绑定')}
                  </dd>
                </div>
                <div>
                  <dt>
                    {t('项目归属')}
                    <span
                      className={`confidence ${detail.confidence === 'HIGH' || detail.confidence === 'MANUAL' ? 'high' : ''}`}
                    >
                      {
                        (
                          {
                            HIGH: t('高置信度 · 工作目录'),
                            MEDIUM: t('待确认 · 命令匹配'),
                            NONE: t('未确认'),
                            MANUAL: t('人工确认'),
                          } as Record<string, string>
                        )[detail.confidence]
                      }
                    </span>
                  </dt>
                  <dd>
                    <Select
                      aria-label={t('确认进程所属项目')}
                      disabled={busy || detail.source === 'DEVNOOK'}
                      value={detail.project_id || ''}
                      placeholder={
                        projects.length ? t('选择项目以确认归属') : t('暂无项目，请先导入项目')
                      }
                      options={projects.map((project) => ({
                        value: project.id,
                        label: project.name,
                      }))}
                      onValueChange={async (projectId) => {
                        if (!projectId) return;
                        setBusy(true);
                        try {
                          setDetail(
                            await call<ProcessDetail>('assign_process', {
                              pid: detail.pid,
                              fingerprint: detail.fingerprint,
                              projectId,
                            }),
                          );
                          notify(t('已按本次进程身份确认项目归属'));
                          await refresh();
                        } catch (e) {
                          report(e);
                        } finally {
                          setBusy(false);
                        }
                      }}
                    />
                    {detail.suggested_project_id && (
                      <p className="field-hint">
                        {t('建议：')}
                        {nameOf(detail.suggested_project_id)}
                        {t('，需要人工确认。')}
                      </p>
                    )}
                  </dd>
                </div>
                <div>
                  <dt>
                    {t('父进程链')}
                    <span className="field-hint">{t('仅作来源线索')}</span>
                  </dt>
                  <dd className="parent-chain">
                    {detail.parents.length
                      ? detail.parents.map((p) => (
                          <span key={p.pid}>
                            <code>{p.pid}</code> {p.name}
                          </span>
                        ))
                      : t('无可访问的父链')}
                  </dd>
                </div>
              </dl>
              <div className="process-safety">
                <ShieldCheck size={17} />
                <span>{t(detail.stop_reason)}</span>
              </div>
              <div className="process-actions">
                <button
                  className="button danger"
                  disabled={!detail.can_stop || busy}
                  onClick={async () => {
                    setBusy(true);
                    try {
                      setStop(await call<StopPreview>('prepare_stop', { pid: detail.pid }));
                      setPidInput('');
                    } catch (e) {
                      report(e);
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  {t('停止进程…')}
                </button>
                <span>{t('SIGTERM · 保留确认步骤')}</span>
              </div>
            </>
          )}
        </div>
      </div>
      {stop && (
        <Modal
          title={t('确认停止外部进程')}
          subtitle={t('DevNook 会再次核验身份，然后向此 PID 发送 SIGTERM。')}
          close={() => setStop(null)}
        >
          <div className="modal-body form-stack">
            <div className="stop-summary">
              <strong>
                {stop.process.name} · PID {stop.process.pid}
              </strong>
              <code>{stop.process.command}</code>
              <span>
                {t('项目：')}
                {nameOf(stop.process.project_id)}
              </span>
              <code>{stop.process.cwd}</code>
            </div>
            <p className="field-hint">
              {t('可能中断该进程正在处理的任务。确认在')}
              {stop.expires_in_seconds}{' '}
              {t('秒后过期；若进程不退出，可刷新查看，DevNook 不会自动强杀。')}
            </p>
            <label>
              {t('输入 PID')}
              {stop.process.pid} {t('确认')}
              <input
                inputMode="numeric"
                autoComplete="off"
                value={pidInput}
                onChange={(e) => setPidInput(e.target.value)}
                placeholder={String(stop.process.pid)}
              />
            </label>
          </div>
          <div className="modal-actions">
            <button className="button secondary" onClick={() => setStop(null)}>
              {t('取消')}
            </button>
            <button
              className="button danger"
              disabled={pidInput !== String(stop.process.pid) || busy}
              onClick={async () => {
                setBusy(true);
                try {
                  await call('confirm_stop', { token: stop.token, confirmedPid: stop.process.pid });
                  setStop(null);
                  notify(t('已发送 SIGTERM，请刷新核对退出结果'));
                  await refresh();
                  if (selected) await choose(selected);
                } catch (e) {
                  report(e);
                  setStop(null);
                } finally {
                  setBusy(false);
                }
              }}
            >
              {t('确认发送 SIGTERM')}
            </button>
          </div>
        </Modal>
      )}
    </section>
  );
}

function SettingsPage({
  showIntegrations,
  appearance,
  changeAppearance,
  appearanceStorageError,
  settings,
  catalog,
  appsLoading,
  appsError,
  chooseApp,
  refreshApps,
  disabled,
  projects,
  notify,
  onSave,
  report,
}: {
  showIntegrations: boolean;
  appearance: Appearance;
  changeAppearance: (appearance: Appearance) => void;
  appearanceStorageError: boolean;
  settings: Settings;
  catalog: DevelopmentApps | null;
  appsLoading: boolean;
  appsError: string;
  chooseApp: () => Promise<DevelopmentApp | null>;
  refreshApps: () => Promise<void>;
  disabled: boolean;
  projects: Project[];
  notify: (text: string) => void;
  onSave: (s: Settings) => Promise<void>;
  report: (e: unknown) => void;
}) {
  const [edit, setEdit] = useState(settings);
  const [busy, setBusy] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(showIntegrations);
  const integrations = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    if (showIntegrations) {
      setAdvancedOpen(true);
      integrations.current?.scrollIntoView({ block: 'start' });
      integrations.current?.querySelector('summary')?.focus();
    }
  }, [showIntegrations]);
  useEffect(() => setEdit(settings), [settings]);
  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await onSave(edit);
    } catch (e) {
      report(e);
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="settings-page">
      <form onSubmit={(e) => void submit(e)}>
        <LanguageSettings />
        <AppearanceSettings
          appearance={appearance}
          change={changeAppearance}
          storageError={appearanceStorageError}
        />
        <section className="settings-section">
          <div className="settings-section-title">
            <FolderSearch size={20} />
            <div>
              <h2>{t('发现与刷新')}</h2>
              <p>{t('控制扫描范围与系统快照的更新节奏。')}</p>
            </div>
          </div>
          <details className="settings-advanced">
            <summary>{t('扫描高级设置')}</summary>
            <div className="setting-row">
              <label htmlFor="depth-setting">
                {t('深度补扫的默认层数')}
                <span>{t('默认按项目特征扫描；仅在扫描高级设置启用补扫后使用。')}</span>
              </label>
              <input
                id="depth-setting"
                type="number"
                min={1}
                max={12}
                required
                value={edit.scan_depth}
                onChange={(e) => setEdit({ ...edit, scan_depth: Number(e.target.value) })}
              />
            </div>
          </details>
          <div className="setting-row">
            <label htmlFor="refresh-setting">
              {t('自动刷新间隔')}
              <span>{t('监听端口刷新间隔，单位为秒。')}</span>
            </label>
            <input
              id="refresh-setting"
              type="number"
              min={2}
              max={60}
              required
              value={edit.refresh_seconds}
              onChange={(e) => setEdit({ ...edit, refresh_seconds: Number(e.target.value) })}
            />
          </div>
        </section>
        <section className="settings-section">
          <div className="settings-section-title">
            <Code2 size={20} />
            <div>
              <h2>{t('开发应用偏好')}</h2>
              <p>{t('直接打开项目，沿用你习惯的开发方式。')}</p>
            </div>
          </div>
          <div className="setting-row">
            <label htmlFor="default-ide">
              {t('默认开发应用')}
              <span>{t('新导入项目统一沿用此设置；项目自己的选择优先，Java 项目也一样。')}</span>
            </label>
            <AppSelect
              id="default-ide"
              value={edit.default_ide}
              preference={catalog?.global_default}
              apps={catalog?.apps || []}
              onChange={(default_ide) => setEdit({ ...edit, default_ide })}
              choose={chooseApp}
              refresh={refreshApps}
              busy={appsLoading}
              disabled={disabled || busy}
            />
          </div>
          <div className="setting-row">
            <label htmlFor="default-terminal">
              {t('默认终端应用')}
              <span>
                {t(
                  '项目卡片与编辑弹窗中的“在终端打开”将优先使用此终端（支持 Ghostty、Warp、iTerm2 等）。',
                )}
              </span>
            </label>
            <TerminalSelect
              id="default-terminal"
              value={edit.default_terminal || 'TERMINAL'}
              preference={catalog?.terminal_default}
              apps={catalog?.apps || []}
              onChange={(default_terminal) => setEdit({ ...edit, default_terminal })}
              choose={chooseApp}
              refresh={refreshApps}
              busy={appsLoading}
              disabled={disabled || busy}
            />
          </div>
          {appsError && (
            <p className="field-hint" role="alert">
              {t(appsError)}
            </p>
          )}
          {catalog?.warnings.map((warning) => (
            <p className="field-hint" key={t(warning)}>
              {t(warning)}
            </p>
          ))}
          <details className="settings-advanced">
            <summary>{t('兼容已有应用路径设置')}</summary>
            {(
              [
                { key: 'idea_path', name: 'IntelliJ IDEA' },
                { key: 'vscode_path', name: 'VS Code' },
                { key: 'cursor_path', name: 'Cursor' },
              ] as const
            ).map(({ key, name }) => (
              <div className="setting-row path-setting" key={key}>
                <label htmlFor={key}>
                  {name} {t('应用路径')}
                  <span>{t('留空使用系统已注册的应用。')}</span>
                </label>
                <input
                  id={key}
                  placeholder={`/Applications/${name === 'VS Code' ? 'Visual Studio Code' : name}.app`}
                  value={edit[key]}
                  onChange={(e) => setEdit({ ...edit, [key]: e.target.value })}
                />
              </div>
            ))}
          </details>
        </section>
        <div className="settings-save">
          <span>
            <ShieldCheck size={14} />
            {t('设置仅保存在当前设备')}
          </span>
          <button className="button primary" disabled={disabled || busy} type="submit">
            {busy ? t('正在保存…') : t('保存设置')}
          </button>
        </div>
      </form>
      <details
        className="settings-section settings-integrations"
        ref={integrations}
        open={advancedOpen}
        onToggle={(e) => setAdvancedOpen(e.currentTarget.open)}
      >
        <summary>
          <span>{t('高级选项')}</span>
          <span className="muted">{t('终端命令与 AI 助手接入')}</span>
        </summary>
        {advancedOpen && (
          <AdvancedIntegrations
            disabled={disabled}
            projects={projects}
            report={report}
            notify={notify}
          />
        )}
      </details>
      <section className="about-block">
        <img
          className="brand-mark"
          src="/devnook.svg"
          width={32}
          height={32}
          alt=""
          draggable={false}
        />
        <div>
          <strong>
            DevNook <span>v0.1.7</span>
          </strong>
          <p>Your Local Development Workspace</p>
          <p>{t('管理本地项目、服务和端口，支持终端命令与可选的 AI 助手接入。')}</p>
        </div>
      </section>
    </div>
  );
}
