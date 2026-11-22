import { t, dateLocale } from './i18n';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import {
  Activity,
  ArrowRightLeft,
  ArrowLeft,
  Code2,
  Check,
  ChevronRight,
  CircleAlert,
  LoaderCircle,
  Pencil,
  Play,
  Plus,
  RotateCw,
  Search,
  ShieldCheck,
  Square,
  SquareTerminal,
  Star,
  Trash2,
  X,
} from 'lucide-react';
import { call } from './api';
import RunLogPanel from './RunLogPanel';
import { Modal } from './components';
import { projectColor } from './ColorLabels';
import ExternalServiceConfirmation from './ExternalServiceConfirmation';
import ServiceConflicts from './ServiceConflicts';
import ServiceBrowser, {
  BrowserAddressField,
  BrowserMenuActions,
  useServiceBrowser,
} from './ServiceBrowser';
import ServiceOptions from './ServiceOptions';
import {
  externalServiceIdentity,
  managedServiceIdentity,
  relativeServiceDirectory,
  serviceIsActive,
  serviceStateLabel,
  externalServiceChoices,
  externalPortConflict,
  groupServiceDrafts,
  pendingServiceDrafts,
} from './ServicePresentation';
import Select from './Select';
import ServiceAiGuide from './ServiceAiGuide';
import { ExistingConfigurationChoice, HandoffChooser } from './ServiceHandoff';
import {
  ExternalProcessDetail,
  ListeningPorts,
  PortSummary,
  ProcessAssignment,
} from './ServicePorts';
import type {
  PortBinding,
  Project,
  ServiceDraft,
  ReadinessKind,
  ServiceActionPreview,
  ServiceDefinition,
  ServiceRun,
  ServiceState,
  ServiceView,
  StopPreview,
  ExternalServiceConfiguration,
  ExternalServiceHandoffPreview,
} from './types';
import type { MainNavigation } from './TrayPresentation';
const active = serviceIsActive;
const isLaunchable = ({ current_run }: ServiceView) =>
  !current_run || current_run.state === 'STOPPED';
const message = (e: unknown) => (e instanceof Error ? e.message : String(e));
const date = (value: number) =>
  new Date(value * 1000).toLocaleString(dateLocale(), { hour12: false });

function fresh(project?: Project): ServiceDefinition {
  return {
    id: '',
    project_id: project?.id || '',
    name: '',
    runtime_type:
      project?.project_type === 'JAVA'
        ? 'JAVA'
        : project?.project_type === 'PYTHON'
          ? 'PYTHON'
          : 'NODE',
    executable: '',
    args: [],
    shell_mode: false,
    working_directory: project?.canonical_path || '',
    environment: {},
    secret_keys: [],
    expected_ports: [],
    dependencies: [],
    readiness: {
      kind: 'PROCESS',
      port: null,
      url: 'http://127.0.0.1:8080/health',
      phrase: '',
      timeout_seconds: 30,
      interval_ms: 500,
      grace_ms: 500,
    },
    stop_grace_seconds: 5,
    log_limit_kb: 1024,
    revision: '',
    created_at: 0,
    updated_at: 0,
  };
}
export function ServiceBadge({ state }: { state?: ServiceState }) {
  return (
    <span className={'service-badge state-' + (state || 'STOPPED').toLowerCase()}>
      <span className="status-dot" />
      {serviceStateLabel(state)}
    </span>
  );
}

export default function ServicesPage({
  navigation,
  navigationRequest,
  navigationHandled,
  projects,
  ports,
  portError,
  lastRefresh,
  refreshingPorts,
  refreshPorts,
  disabled,
  projectId,
  back,
  editProject,
  updateProject,
  launchProject,
  inspectPort,
  showAllServices,
  openIntegrations,
  report,
  notify,
}: {
  navigation: ReactNode;
  navigationRequest: MainNavigation | null;
  navigationHandled: (sequence: number) => void;
  projects: Project[];
  ports: PortBinding[];
  portError: string;
  lastRefresh: Date | null;
  refreshingPorts: boolean;
  refreshPorts: () => Promise<void>;
  disabled: boolean;
  projectId: string;
  back: () => void;
  editProject: (project: Project) => void;
  updateProject: (project: Project) => Promise<void>;
  launchProject: (project: Project) => Promise<void>;
  inspectPort: (pid: number) => void;
  showAllServices: () => void;
  openIntegrations: () => void;
  report: (e: unknown) => void;
  notify: (s: string) => void;
}) {
  const [services, setServices] = useState<ServiceView[]>([]);
  const [servicesLoaded, setServicesLoaded] = useState(false);
  const [selected, setSelected] = useState('');
  const [filter, setFilter] = useState(projectId);
  const [query, setQuery] = useState('');
  const [launchableOpen, setLaunchableOpen] = useState<boolean | null>(null);
  const [editing, setEditing] = useState<ServiceDefinition | null>(null);
  const [archive, setArchive] = useState<ServiceDefinition | null>(null);
  const [confirmation, setConfirmation] = useState<ServiceActionPreview | null>(null);
  const [externalStop, setExternalStop] = useState<StopPreview | null>(null);
  const [externalSetup, setExternalSetup] = useState<ExternalServiceConfiguration | null>(null);
  const [newExternalConfig, setNewExternalConfig] = useState(false);
  const [choosingHandoff, setChoosingHandoff] = useState('');
  const [handoff, setHandoff] = useState<ExternalServiceHandoffPreview | null>(null);
  const [history, setHistory] = useState<ServiceRun[]>([]);
  const [clearLog, setClearLog] = useState<{ run: ServiceRun; invalidate: () => void } | null>(
    null,
  );
  const [runChoice, setRunChoice] = useState('latest');
  const [busy, setBusy] = useState(false);
  const [favoriteBusy, setFavoriteBusy] = useState('');
  const favoriteSaving = useRef(false);
  const [fetchError, setFetchError] = useState('');
  const project = projects.find((p) => p.id === projectId);
  const projectName = (id: string) => projects.find((p) => p.id === id)?.name || t('已移除的项目');
  const managedIdentity = (definition: ServiceDefinition) =>
    managedServiceIdentity(
      definition,
      projects.find((p) => p.id === definition.project_id),
      !!project,
    );
  const [drafts, setDrafts] = useState<ServiceDraft[]>([]);
  const [draftError, setDraftError] = useState('');
  const [assignment, setAssignment] = useState<{ pid: number | null } | null>(null);
  const portsAvailable = !!lastRefresh && !portError;
  const externalIdentity = (bindings: PortBinding[]) =>
    externalServiceIdentity(
      bindings,
      projects.find((p) => p.id === bindings[0].project_id),
      !!project,
      portsAvailable,
    );
  const actualPorts = (id: string) =>
    ports.filter((b) => b.source === 'DEVNOOK' && b.service_id === id);
  const external = [
    ...new Set(
      ports
        .filter(
          (b) =>
            b.process_kind === 'SERVICE' &&
            b.source !== 'DEVNOOK' &&
            b.project_id &&
            (!filter || b.project_id === filter),
        )
        .map((b) => b.pid),
    ),
  ]
    .map((pid) => ports.filter((b) => b.pid === pid && b.source !== 'DEVNOOK'))
    .filter((bindings) =>
      [
        projectName(bindings[0].project_id!),
        bindings[0].name,
        bindings[0].command,
        bindings[0].cwd,
        bindings[0].pid,
        ...bindings.map((b) => b.port),
      ]
        .join(' ')
        .toLowerCase()
        .includes(query.toLowerCase()),
    );
  const externalView = external.find((bindings) => selected === `external:${bindings[0].pid}`);
  useEffect(() => {
    setDrafts([]);
    setDraftError('');
    if (!projectId || disabled) return;
    let cancelled = false;
    call<ServiceDraft[]>('service_drafts', { projectId })
      .then((values) => {
        if (!cancelled) setDrafts(values);
      })
      .catch((e) => {
        if (!cancelled) setDraftError(message(e));
      });
    return () => {
      cancelled = true;
    };
  }, [projectId, disabled]);
  const refreshing = useRef(false);
  const serviceVersion = useRef(0);
  const refresh = useCallback(async () => {
    if (disabled || refreshing.current) return;
    refreshing.current = true;
    const version = serviceVersion.current;
    try {
      const values = await call<ServiceView[]>('list_services');
      if (version === serviceVersion.current) setServices(values);
      setFetchError('');
    } catch (e) {
      setFetchError(message(e));
    } finally {
      refreshing.current = false;
      setServicesLoaded(true);
    }
  }, [disabled]);
  const navigationStarted = useRef(0);
  useEffect(() => {
    if (
      !navigationRequest ||
      disabled ||
      busy ||
      editing ||
      confirmation ||
      archive ||
      externalStop ||
      externalSetup ||
      choosingHandoff ||
      handoff ||
      clearLog ||
      assignment ||
      navigationStarted.current >= navigationRequest.sequence
    )
      return;
    const request = navigationRequest;
    navigationStarted.current = request.sequence;
    setBusy(true);
    void (async () => {
      try {
        const values = await call<ServiceView[]>('list_services');
        serviceVersion.current += 1;
        setServices(values);
        setServicesLoaded(true);
        setFetchError('');
        setFilter('');
        setQuery('');
        if (request.destination.kind === 'managed') {
          const destination = request.destination;
          const target = values.find((value) => value.definition.id === destination.id);
          if (!target) throw new Error(t('服务配置已不存在，请重新选择。'));
          if (isLaunchable(target)) setLaunchableOpen(true);
          setSelected(destination.id);
          if (destination.action)
            setConfirmation(
              await call<ServiceActionPreview>('prepare_service_action', {
                id: destination.id,
                action: destination.action,
              }),
            );
        } else if (request.destination.kind === 'external') {
          await refreshPorts();
          setSelected(`external:${request.destination.pid}`);
        }
      } catch (error) {
        report(error);
      } finally {
        setBusy(false);
        navigationHandled(request.sequence);
      }
    })();
  }, [
    navigationRequest,
    navigationHandled,
    disabled,
    busy,
    editing,
    confirmation,
    archive,
    externalStop,
    externalSetup,
    choosingHandoff,
    handoff,
    clearLog,
    assignment,
    refreshPorts,
    report,
  ]);
  useEffect(() => {
    void refresh();
    const interval = setInterval(() => void refresh(), 1000);
    return () => clearInterval(interval);
  }, [refresh]);
  const runStates = services
    .map((v) => `${v.definition.id}:${v.current_run?.id}:${v.current_run?.state}`)
    .sort()
    .join('|');
  useEffect(() => {
    if (!disabled) void refreshPorts();
  }, [runStates, disabled, refreshPorts]);
  useEffect(() => {
    setFilter(projectId);
    if (projectId) setLaunchableOpen(null);
  }, [projectId]);
  const visible = services.filter(
    (v) =>
      (!filter || v.definition.project_id === filter) &&
      [
        projectName(v.definition.project_id),
        v.definition.name,
        v.definition.executable,
        v.definition.working_directory,
        ...actualPorts(v.definition.id).flatMap((b) => [b.port, b.pid]),
      ]
        .join(' ')
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  const running: ServiceView[] = [];
  const launchable: ServiceView[] = [];
  const needsAttention: ServiceView[] = [];
  for (const service of visible) {
    const state = service.current_run?.state;
    if (isLaunchable(service)) launchable.push(service);
    else if (state !== 'UNKNOWN' && active(state)) running.push(service);
    else needsAttention.push(service);
  }
  const showLaunchable = launchableOpen ?? !!query.trim();
  const pending = pendingServiceDrafts(drafts, services, query);
  const { primaryDrafts, otherDrafts } = groupServiceDrafts(
    pending,
    drafts,
    project?.canonical_path,
  );
  const firstDraft = primaryDrafts[0] ?? otherDrafts[0];
  const keyOf = (draft: ServiceDraft) => 'draft:' + draft.definition.working_directory;
  const view = visible.find((v) => v.definition.id === selected);
  const viewExternalChoices = view
    ? externalServiceChoices(ports, view.definition.project_id, portsAvailable)
    : [];
  const viewExternalConflict = !!view && externalPortConflict(view.definition, viewExternalChoices);
  const managedBrowser = useServiceBrowser({
    target: { kind: 'MANAGED', service_id: view?.definition.id || '' },
    bindings: view ? actualPorts(view.definition.id) : [],
    available: portsAvailable,
    disabled:
      disabled ||
      busy ||
      !['STARTING', 'RUNNING', 'READY'].includes(view?.current_run?.state || ''),
    report,
  });
  const draft = pending.find((d) => keyOf(d) === selected);
  const firstService = (projectId ? visible : running)[0]?.definition.id;
  const firstSelection =
    firstService ||
    (external[0] ? `external:${external[0][0].pid}` : '') ||
    needsAttention[0]?.definition.id ||
    (showLaunchable ? launchable[0]?.definition.id : '') ||
    (firstDraft ? keyOf(firstDraft) : '');
  useEffect(() => {
    if (!projectId && !servicesLoaded) return;
    if (!view && !draft && !externalView) setSelected(firstSelection);
  }, [projectId, servicesLoaded, view, draft, externalView, firstSelection]);
  useEffect(() => {
    setRunChoice('latest');
    setHistory([]);
  }, [selected]);
  useEffect(() => {
    if (!view || disabled) return;
    let cancelled = false;
    let inFlight = false;
    const load = async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        const values = await call<ServiceRun[]>('service_history', { id: selected });
        if (!cancelled) setHistory(values);
      } catch (e) {
        if (!cancelled) report(e);
      } finally {
        inFlight = false;
      }
    };
    void load();
    const timer = setInterval(() => void load(), 1500);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [selected, !!view, disabled, report]);
  const chosenRun =
    runChoice === 'latest'
      ? view?.current_run
      : history.find((r) => r.id === runChoice && r.service_id === selected);
  function selectSavedService(id: string) {
    setSelected(id);
    if (services.some((service) => service.definition.id === id && isLaunchable(service)))
      setLaunchableOpen(true);
  }
  async function prepare(action: 'start' | 'restart' | 'force_stop', target = view) {
    if (!target) return;
    setBusy(true);
    try {
      setConfirmation(
        await call<ServiceActionPreview>('prepare_service_action', {
          id: target.definition.id,
          action,
        }),
      );
    } catch (e) {
      report(e);
    } finally {
      setBusy(false);
    }
  }
  async function stop(target: ServiceView) {
    setBusy(true);
    try {
      await call('stop_service', { id: target.definition.id });
      await refresh();
    } catch (e) {
      report(e);
    } finally {
      setBusy(false);
    }
  }
  async function externalAction(pid: number, action: 'stop' | 'configure') {
    setBusy(true);
    try {
      if (action === 'stop') {
        setExternalStop(await call<StopPreview>('prepare_external_service_stop', { pid }));
      } else {
        setNewExternalConfig(false);
        setExternalSetup(
          await call<ExternalServiceConfiguration>('external_service_configuration', { pid }),
        );
      }
    } catch (e) {
      report(e);
    } finally {
      setBusy(false);
    }
  }
  async function prepareHandoff(definition: ServiceDefinition, pid: number) {
    setBusy(true);
    try {
      const current = await call<ExternalServiceConfiguration>('external_service_configuration', {
        pid,
      });
      if (current.handoff_supported === false) {
        throw new Error(t('当前后台不支持切换托管；可先查看已有配置，更新 Core 后再预览。'));
      }
      const preview = await call<ExternalServiceHandoffPreview>(
        'prepare_external_service_handoff',
        {
          id: definition.id,
          revision: definition.revision,
          pid,
          fingerprint: current.process.fingerprint,
        },
      );
      selectSavedService(definition.id);
      setFilter(definition.project_id);
      setQuery('');
      setHandoff(preview);
      setExternalSetup(null);
      setChoosingHandoff('');
    } catch (e) {
      report(e);
    } finally {
      setBusy(false);
    }
  }
  async function saveExternal(
    definition: ServiceDefinition,
    secret_values: Record<string, string>,
  ) {
    if (!externalSetup) return;
    const previous = externalSetup.process;
    const handoffSupported = externalSetup.handoff_supported !== false;
    setBusy(true);
    try {
      const saved = await call<ServiceDefinition>('save_service', {
        input: { definition, secret_values },
      });
      // Saving is complete even if the old process disappears before a handoff preview.
      serviceVersion.current += 1;
      setExternalSetup(null);
      setSelected(saved.id);
      if (!projectId) setLaunchableOpen(true);
      setFilter(saved.project_id);
      setServices((values) => [...values, { definition: saved, current_run: null }]);
      if (!handoffSupported) {
        notify(t('配置已保存，旧服务继续运行；当前后台版本不支持一键切换托管'));
        await refresh();
        return;
      }
      notify(t('配置已保存，旧服务继续运行；是否切换请单独确认'));
      try {
        setHandoff(
          await call<ExternalServiceHandoffPreview>('prepare_external_service_handoff', {
            id: saved.id,
            revision: saved.revision,
            pid: previous.pid,
            fingerprint: previous.fingerprint,
          }),
        );
      } catch (e) {
        report(t('配置已保存，未停止旧实例或启动新服务：') + message(e));
      }
      await refresh();
    } finally {
      setBusy(false);
    }
  }
  async function save(
    definition: ServiceDefinition,
    secret_values: Record<string, string>,
    start = false,
  ) {
    setBusy(true);
    try {
      const saved = await call<ServiceDefinition>('save_service', {
        input: { definition, secret_values },
      });
      serviceVersion.current += 1;
      setServices((previous) => [
        ...previous.filter((v) => v.definition.id !== saved.id),
        {
          definition: saved,
          current_run: previous.find((v) => v.definition.id === saved.id)?.current_run || null,
        },
      ]);
      setEditing(null);
      setSelected(saved.id);
      if (!projectId) setLaunchableOpen(true);
      setFilter(saved.project_id);
      await refresh();
      if (start) await prepare('start', { definition: saved, current_run: null });
      else notify(t('配置已保存'));
    } finally {
      setBusy(false);
    }
  }
  async function startDraft(value: ServiceDraft) {
    setSelected(keyOf(value));
    if (!value.command) return;
    setBusy(true);
    try {
      await save(value.definition, {}, true);
    } catch (e) {
      report(e);
    } finally {
      setBusy(false);
    }
  }
  const relativeDirectory = (directory: string, id: string) =>
    relativeServiceDirectory(
      directory,
      projects.find((p) => p.id === id),
    );
  async function toggleFavorite(target: Project) {
    if (disabled || favoriteSaving.current) return;
    favoriteSaving.current = true;
    setFavoriteBusy(target.id);
    try {
      await updateProject({ ...target, is_favorite: !target.is_favorite });
    } finally {
      favoriteSaving.current = false;
      setFavoriteBusy('');
    }
  }
  function favoriteButton(id: string | null) {
    const target = projects.find((p) => p.id === id);
    if (!target) return null;
    const label = target.is_favorite
      ? t('取消收藏项目 {{0}}', [target.name])
      : t('收藏项目 {{0}}', [target.name]);
    return (
      <button
        type="button"
        className={`icon-button favorite ${target.is_favorite ? 'is-favorite' : ''}`}
        aria-label={label}
        title={label}
        aria-pressed={target.is_favorite}
        aria-busy={favoriteBusy === target.id}
        disabled={disabled || !!favoriteBusy}
        onClick={() => void toggleFavorite(target).catch(() => {})}
      >
        <Star size={16} fill={target.is_favorite ? 'currentColor' : 'none'} />
      </button>
    );
  }
  function serviceRow(service: ServiceView) {
    const { definition, current_run } = service;
    const state = current_run?.state;
    const identity = managedIdentity(definition);
    const isActive = active(state);
    const canSwitch =
      !isActive &&
      externalPortConflict(
        definition,
        externalServiceChoices(ports, definition.project_id, portsAvailable),
      );
    const ActionIcon = isActive ? Square : canSwitch ? ArrowRightLeft : Play;
    return (
      <div
        key={definition.id}
        data-color={projectColor(definition.project_id)}
        className={'service-row ' + (selected === definition.id ? 'selected' : '')}
      >
        <button className="service-row-select" onClick={() => setSelected(definition.id)}>
          <span className="service-row-heading">
            <span className="service-row-title">
              <span className="service-identity-icon" aria-hidden="true">
                <SquareTerminal size={15} />
              </span>
              <strong title={identity.title}>{identity.title}</strong>
            </span>
            <ServiceBadge state={state} />
          </span>
          <span className="service-row-meta">
            <span className="service-project" title={identity.qualifier}>
              {project
                ? relativeDirectory(definition.working_directory, definition.project_id)
                : identity.qualifier}
            </span>
            <PortSummary bindings={actualPorts(definition.id)} available={portsAvailable} compact />
          </span>
        </button>
        <div className="service-row-actions">
          {favoriteButton(definition.project_id)}
          <button
            className="text-button"
            disabled={disabled || busy || state === 'UNKNOWN' || state === 'STOPPING'}
            onClick={() => {
              if (isActive) void stop(service);
              else if (canSwitch) {
                setSelected(definition.id);
                setChoosingHandoff(definition.id);
              } else void prepare('start', service);
            }}
          >
            <ActionIcon size={13} />
            {isActive ? t('停止') : canSwitch ? t('切换…') : t('启动')}
          </button>
        </div>
      </div>
    );
  }
  function draftRow(value: ServiceDraft) {
    return (
      <div
        className={'service-row ' + (selected === keyOf(value) ? 'selected' : '')}
        key={keyOf(value)}
      >
        <button className="service-row-select" onClick={() => setSelected(keyOf(value))}>
          <span className="service-row-heading">
            <span className="service-row-title">
              <SquareTerminal size={17} />
              <strong title={managedIdentity(value.definition).title}>
                {managedIdentity(value.definition).title}
              </strong>
            </span>
            <span className="service-badge state-stopped">{t('未配置')}</span>
          </span>
          <span className="service-project">{value.command || t('填写启动命令后即可运行')}</span>
        </button>
        <div className="service-row-actions">
          {favoriteButton(value.definition.project_id)}
          <button
            className="text-button"
            disabled={disabled || busy}
            onClick={() => void startDraft(value)}
          >
            <Play size={13} />
            {value.command ? t('启动…') : t('填写命令')}
          </button>
        </div>
      </div>
    );
  }
  return (
    <section className="services-page">
      <header className="service-management-header">
        <div className="project-service-context">
          {project && (
            <button
              className="icon-button"
              aria-label={t('返回项目列表')}
              title={t('返回项目列表')}
              onClick={back}
            >
              <ArrowLeft size={15} />
            </button>
          )}
          <h1 className="service-page-title" title={project?.canonical_path}>
            {project?.name || t('服务管理')}
          </h1>
        </div>
        {navigation}
        {project && (
          <div className="project-service-actions">
            <button className="button secondary small" onClick={() => void launchProject(project)}>
              <Code2 size={15} />
              {t('打开 IDE')}
            </button>
            <button
              className="icon-button"
              aria-label={t('编辑项目资料')}
              onClick={() => editProject(project)}
            >
              <Pencil size={15} />
            </button>
          </div>
        )}
      </header>
      <div className="services-toolbar">
        <div className="search-field">
          <Search size={16} />
          <input
            aria-label={t('搜索服务')}
            placeholder={t('搜索服务、进程、端口或工作目录…')}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setLaunchableOpen(null);
            }}
          />
        </div>
        {!project && (
          <Select
            aria-label={t('按项目筛选服务')}
            value={filter}
            onValueChange={setFilter}
            options={[
              { value: '', label: t('所有项目') },
              ...projects.map((project) => ({ value: project.id, label: project.name })),
            ]}
          />
        )}
        <div className="service-port-status" role="status">
          {!portError && (
            <span>
              {lastRefresh
                ? `TCP · ${lastRefresh.toLocaleTimeString(dateLocale(), { hour12: false })}`
                : t('端口状态未知')}
            </span>
          )}
          <button
            className="icon-button"
            aria-label={refreshingPorts ? t('正在刷新端口') : t('刷新端口')}
            title={t('刷新端口')}
            disabled={disabled || refreshingPorts}
            onClick={() => void refreshPorts()}
          >
            <RotateCw size={15} className={refreshingPorts ? 'spin' : ''} />
          </button>
        </div>
        <button
          className="button primary"
          disabled={disabled || !projects.length}
          onClick={() => setEditing(fresh(projects.find((p) => p.id === filter) || projects[0]))}
        >
          <Plus size={16} />
          {t('添加服务')}
        </button>
      </div>
      {fetchError && (
        <div className="error-banner" role="alert">
          <CircleAlert size={15} />
          {t(fetchError)}
          <button className="button small" onClick={() => void refresh()}>
            {t('重试')}
          </button>
        </div>
      )}
      {draftError && (
        <div className="error-banner" role="alert">
          {t(draftError)}
        </div>
      )}
      {portError && (
        <div className="error-banner" role="alert">
          {t('端口读取失败：')}
          {t(portError)}
        </div>
      )}
      <div className="services-workspace">
        <div className="service-list" role="region" aria-label={t('项目服务列表')} tabIndex={0}>
          <div
            className={projectId ? 'port-list-header' : 'port-list-header service-section-heading'}
          >
            <span>{projectId ? t('服务配置') : t('运行中的托管服务')}</span>
            <span>{projectId ? visible.length : running.length}</span>
          </div>
          {servicesLoaded &&
            !fetchError &&
            !visible.length &&
            !pending.length &&
            (projectId || !external.length) && (
              <div className="service-empty">
                <SquareTerminal size={29} strokeWidth={1.2} />
                {!projectId && query.trim() ? (
                  <p>{t('没有匹配的服务')}</p>
                ) : (
                  <>
                    <p>{!projects.length ? t('先扫描并导入一个项目') : t('暂无服务')}</p>
                    <span>{t('可添加服务并填写启动命令。')}</span>
                  </>
                )}
              </div>
            )}
          {(projectId ? visible : running).map(serviceRow)}
          {primaryDrafts.length > 0 && (
            <div className="port-list-header">
              <span>{t('启动建议')}</span>
              <span>{primaryDrafts.length}</span>
            </div>
          )}
          {primaryDrafts.map(draftRow)}
          {otherDrafts.length > 0 && (
            <details className="other-launches">
              <summary>{t('其他启动方式')}</summary>
              <p>{t('根目录脚本可能同时启动多个服务，请避免重复启动。')}</p>
              {otherDrafts.map(draftRow)}
            </details>
          )}
          <div className="external-list-header">
            <span>
              {t('外部服务')} <small>{portsAvailable ? external.length : t('未知')}</small>
            </span>
            <button
              className="text-button"
              disabled={disabled || !portsAvailable || !projects.length}
              onClick={() => setAssignment({ pid: null })}
            >
              {t('关联服务…')}
            </button>
          </div>
          {external.map((bindings) => (
            <div
              className={
                'service-row ' + (selected === `external:${bindings[0].pid}` ? 'selected' : '')
              }
              key={bindings[0].pid}
              data-color={projectColor(bindings[0].project_id)}
            >
              <button
                className="service-row-select"
                onClick={() => setSelected(`external:${bindings[0].pid}`)}
              >
                <span className="service-row-heading">
                  <span className="service-row-title">
                    <span className="service-identity-icon" aria-hidden="true">
                      <Activity size={15} />
                    </span>
                    <strong
                      className={project ? 'service-component-title' : undefined}
                      title={externalIdentity(bindings).title}
                    >
                      {externalIdentity(bindings).title}
                    </strong>
                  </span>
                  <span
                    className={'service-badge state-' + (portsAvailable ? 'listening' : 'unknown')}
                  >
                    {portsAvailable ? t('正在监听') : t('端口状态未知')}
                  </span>
                </span>
                <span className="service-row-meta">
                  <span
                    className="service-project external-qualifier"
                    title={externalIdentity(bindings).qualifier}
                  >
                    {project ? bindings[0].name : externalIdentity(bindings).qualifier}
                  </span>
                  {!project && <small className="service-runtime">{bindings[0].name}</small>}
                  {project && <small className="service-runtime">{t('外部')}</small>}
                </span>
              </button>
              <div className="service-row-actions">{favoriteButton(bindings[0].project_id)}</div>
            </div>
          ))}
          {!external.length && (!portsAvailable || (query && (projectId || !visible.length))) && (
            <p className="external-empty">
              {portsAvailable
                ? query
                  ? t('没有匹配的外部服务')
                  : t('暂未发现外部启动的项目服务。其他监听可在端口管理查看。')
                : t('端口状态未知，暂无法列出外部服务')}
            </p>
          )}
          {!projectId && needsAttention.length > 0 && (
            <>
              <div className="port-list-header service-section-heading">
                <span>{t('需要关注')}</span>
                <span>{needsAttention.length}</span>
              </div>
              {needsAttention.map(serviceRow)}
            </>
          )}
          {!projectId && launchable.length > 0 && (
            <>
              <button
                type="button"
                className="port-list-header service-section-heading service-disclosure"
                aria-expanded={showLaunchable}
                aria-controls="launchable-services"
                onClick={() => setLaunchableOpen(!showLaunchable)}
              >
                <ChevronRight size={15} aria-hidden="true" />
                <span>{t('可启动')}</span>
                <span>{launchable.length}</span>
              </button>
              <div id="launchable-services" hidden={!showLaunchable}>
                {showLaunchable && launchable.map(serviceRow)}
              </div>
            </>
          )}
        </div>
        <div
          className={
            'service-detail' + (view && !externalView && !draft ? ' service-detail-managed' : '')
          }
          data-color={projectColor(externalView?.[0].project_id || view?.definition.project_id)}
        >
          {externalView ? (
            <ExternalProcessDetail
              bindings={externalView}
              identity={externalIdentity(externalView)}
              directory={relativeDirectory(
                externalView[0].cwd || '',
                externalView[0].project_id || '',
              )}
              associate={() => setAssignment({ pid: externalView[0].pid })}
              disabled={disabled || busy || !portsAvailable}
              stop={() => void externalAction(externalView[0].pid, 'stop')}
              configure={() => void externalAction(externalView[0].pid, 'configure')}
              report={report}
              portsAvailable={portsAvailable}
            />
          ) : draft ? (
            <div className="draft-service">
              <h2>{managedIdentity(draft.definition).title}</h2>
              <p className="field-hint">
                {t('配置启动方式')} · {t(draft.source)}
              </p>
              <ServiceEditor
                key={keyOf(draft)}
                initial={draft.definition}
                services={services}
                projects={projects}
                inline
                close={() => {}}
                saved={(definition, values) => save(definition, values, true)}
              />
            </div>
          ) : !view ? (
            <div className="process-placeholder">
              <SquareTerminal size={35} strokeWidth={1.2} />
              <h3>{t('选择左侧服务')}</h3>
              <p>
                {t('查看启动命令、运行状态和日志，')}
                <br />
                {t('也可以直接启动或停止。')}
              </p>
              <span className="read-only-note">
                <ShieldCheck size={13} />
                {t('仅管理你明确登记的服务')}
              </span>
            </div>
          ) : (
            <>
              <div className="service-overview">
                <div className="service-detail-heading">
                  <div className="service-detail-title">
                    <h2>{managedIdentity(view.definition).title}</h2>
                    <ServiceBadge state={view.current_run?.state} />
                  </div>
                  <div className="service-actions">
                    {active(view.current_run?.state) && <ServiceBrowser browser={managedBrowser} />}
                    {!active(view.current_run?.state) && (
                      <button
                        className="button primary small"
                        disabled={disabled || busy || active(view.current_run?.state)}
                        onClick={() =>
                          viewExternalConflict
                            ? setChoosingHandoff(view.definition.id)
                            : void prepare('start')
                        }
                      >
                        {viewExternalConflict ? <ArrowRightLeft size={15} /> : <Play size={15} />}
                        {viewExternalConflict ? t('切换为托管…') : t('启动')}
                      </button>
                    )}
                    <ServiceOptions label={t('更多服务操作')}>
                      <BrowserMenuActions browser={managedBrowser} />
                      <hr />
                      {active(view.current_run?.state) && (
                        <button
                          className="text-button"
                          disabled={
                            disabled ||
                            busy ||
                            !active(view.current_run?.state) ||
                            view.current_run?.state === 'UNKNOWN'
                          }
                          onClick={async () => {
                            setBusy(true);
                            try {
                              await call('stop_service', { id: view.definition.id });
                              notify(t('已请求正常停止，请以进程组实际退出状态为准'));
                              await refresh();
                            } catch (e) {
                              report(e);
                            } finally {
                              setBusy(false);
                            }
                          }}
                        >
                          <Square size={14} />
                          {t('停止')}
                        </button>
                      )}
                      {active(view.current_run?.state) && (
                        <button
                          className="text-button"
                          disabled={disabled || busy || view.current_run?.state === 'UNKNOWN'}
                          onClick={() => void prepare('restart')}
                        >
                          <RotateCw size={15} />
                          {t('重启')}
                        </button>
                      )}

                      {!active(view.current_run?.state) && (
                        <button
                          className="text-button"
                          disabled={disabled || busy || !viewExternalChoices.length}
                          onClick={() => setChoosingHandoff(view.definition.id)}
                        >
                          <ArrowRightLeft size={14} />
                          {t('切换为托管…')}
                        </button>
                      )}
                      {!active(view.current_run?.state) && viewExternalConflict && (
                        <button
                          className="text-button"
                          disabled={disabled || busy}
                          onClick={() => void prepare('start')}
                        >
                          <Play size={14} />
                          {t('普通启动…')}
                        </button>
                      )}
                      <button
                        className="text-button"
                        aria-label={t('编辑服务配置')}
                        disabled={busy || active(view.current_run?.state)}
                        onClick={() => setEditing(view.definition)}
                      >
                        <Pencil size={16} />
                        {t('编辑服务配置')}
                      </button>
                      <button
                        className="text-button danger-text"
                        aria-label={t('移除服务配置')}
                        disabled={busy || active(view.current_run?.state)}
                        onClick={() => setArchive(view.definition)}
                      >
                        <Trash2 size={16} />
                        {t('移除服务配置')}
                      </button>
                    </ServiceOptions>
                  </div>
                </div>
                {!project && (
                  <p className="service-identity">{managedIdentity(view.definition).qualifier}</p>
                )}
                <dl className="service-detail-facts">
                  <div>
                    <dt>{managedBrowser.webUi ? t('访问地址') : t('实际监听')}</dt>
                    <dd>
                      {managedBrowser.webUi ? (
                        <BrowserAddressField browser={managedBrowser} />
                      ) : (
                        <PortSummary
                          bindings={actualPorts(view.definition.id)}
                          available={portsAvailable}
                        />
                      )}
                    </dd>
                  </div>
                  <div>
                    <dt>{t('启动命令')}</dt>
                    <dd>
                      <code>
                        {view.definition.shell_mode ? '/bin/zsh -lc ' : ''}
                        {view.definition.executable}{' '}
                        {view.definition.args.map((a) => JSON.stringify(a)).join(' ')}
                      </code>
                    </dd>
                  </div>
                  <div>
                    <dt>{t('工作目录')}</dt>
                    <dd>
                      <code>{view.definition.working_directory}</code>
                    </dd>
                  </div>
                </dl>
                {!active(view.current_run?.state) && viewExternalConflict && (
                  <p className="service-handoff-notice" role="status">
                    {t(
                      '预期端口由本项目的外部实例占用；此配置尚未接管该实例。普通启动不会关闭外部服务。',
                    )}
                  </p>
                )}
                {choosingHandoff === view.definition.id && !active(view.current_run?.state) && (
                  <HandoffChooser
                    key={view.definition.id}
                    service={view.definition}
                    choices={externalServiceChoices(
                      ports,
                      view.definition.project_id,
                      portsAvailable,
                    )}
                    disabled={disabled || busy || !portsAvailable}
                    preview={prepareHandoff}
                    close={() => setChoosingHandoff('')}
                  />
                )}
                {managedBrowser.error && (
                  <p className="field-hint" role="status" title={managedBrowser.error}>
                    {t('地址读取失败，请从更多菜单重试')}
                  </p>
                )}
                {portsAvailable &&
                  !actualPorts(view.definition.id).length &&
                  active(view.current_run?.state) && (
                    <p className="field-hint">
                      {t('进程存活或“已就绪”不一定代表已监听 TCP 端口。')}
                    </p>
                  )}
                {view.current_run?.error && (
                  <div className="service-warning" role="status">
                    <CircleAlert size={16} />
                    <span>{t(view.current_run.error)}</span>
                  </div>
                )}
                {!active(view.current_run?.state) && (
                  <ServiceConflicts
                    service={view}
                    services={services}
                    ports={ports}
                    projects={projects}
                    available={portsAvailable}
                    disabled={disabled || busy}
                    inspect={inspectPort}
                    selectService={(id) => {
                      setFilter('');
                      setQuery('');
                      selectSavedService(id);
                      showAllServices();
                    }}
                  />
                )}
                {view.current_run?.state === 'STOPPING' && (
                  <div className="force-stop-row">
                    <span>
                      {t('宽限')}
                      {view.definition.stop_grace_seconds} {t('秒；不会自动强杀。')}
                    </span>
                    <button
                      className="text-button danger-text"
                      disabled={busy}
                      onClick={() => void prepare('force_stop')}
                    >
                      {t('强制停止…')}
                    </button>
                  </div>
                )}
                <details className="service-runtime-details">
                  <summary>{t('运行详情')}</summary>
                  {portsAvailable && <ListeningPorts bindings={actualPorts(view.definition.id)} />}
                  <p className="field-hint">
                    {view.definition.shell_mode ? t('Shell 模式') : t('直接执行')} ·{' '}
                    {view.definition.readiness.kind} {t('就绪检查')}
                  </p>
                  <dl className="service-facts">
                    <div>
                      <dt>{t('PID / 进程组')}</dt>
                      <dd>
                        {view.current_run?.pid || '—'} / {view.current_run?.pgid || '—'}
                      </dd>
                    </div>
                    <div>
                      <dt>{t('预期端口')}</dt>
                      <dd>{view.definition.expected_ports.join(', ') || t('未指定')}</dd>
                    </div>
                    <div>
                      <dt>{t('最近启动')}</dt>
                      <dd>
                        {view.current_run ? date(view.current_run.started_at) : t('尚未运行')}
                      </dd>
                    </div>
                    <div>
                      <dt>{t('最近退出码')}</dt>
                      <dd>{view.current_run?.exit_code ?? '—'}</dd>
                    </div>
                  </dl>
                </details>
                <ServiceAiGuide
                  serviceId={view.definition.id}
                  openIntegrations={openIntegrations}
                  notify={notify}
                  report={report}
                />
              </div>
              <RunLogPanel
                key={selected}
                runId={chosenRun?.log_path ? chosenRun.id : undefined}
                managedRun={chosenRun}
                onClear={(run, invalidate) => setClearLog({ run, invalidate })}
                limitKb={view.definition.log_limit_kb}
                disabled={disabled}
                report={report}
                notify={notify}
                picker={
                  <Select
                    aria-label={t('选择运行记录')}
                    value={runChoice}
                    onValueChange={setRunChoice}
                    options={[
                      { value: 'latest', label: t('最近运行') },
                      ...history.map((run) => ({
                        value: run.id,
                        label: `${date(run.started_at)} · ${serviceStateLabel(run.state)}`,
                      })),
                    ]}
                  />
                }
              />
            </>
          )}
        </div>
      </div>
      {assignment && (
        <ProcessAssignment
          initialPid={assignment.pid}
          projectId={filter}
          projects={projects}
          ports={ports}
          close={() => setAssignment(null)}
          assigned={async () => {
            await refreshPorts();
            notify(t('已确认本次进程的项目归属，外部进程仍独立运行'));
          }}
        />
      )}
      {editing && (
        <ServiceEditor
          initial={editing}
          projectLocked={!!project}
          services={services}
          projects={projects}
          close={() => setEditing(null)}
          saved={(definition, values) => save(definition, values)}
        />
      )}
      {externalSetup &&
      !newExternalConfig &&
      services.some((v) => v.definition.project_id === externalSetup.process.project_id) ? (
        <ExistingConfigurationChoice
          setup={externalSetup}
          services={services.filter(
            (v) => v.definition.project_id === externalSetup.process.project_id,
          )}
          busy={busy}
          disabled={disabled || !portsAvailable || !!fetchError}
          preview={prepareHandoff}
          create={() => setNewExternalConfig(true)}
          select={(id) => {
            selectSavedService(id);
            setQuery('');
            setFilter(externalSetup.process.project_id || '');
            setExternalSetup(null);
          }}
          close={() => setExternalSetup(null)}
        />
      ) : (
        externalSetup && (
          <ServiceEditor
            initial={externalSetup.draft.definition}
            projectLocked
            services={services}
            projects={projects}
            close={() => setExternalSetup(null)}
            saved={saveExternal}
            title={t('配置为 DevNook 服务')}
            description={
              t(externalSetup.draft.source) +
              (externalSetup.handoff_supported === false
                ? t(
                    '。当前后台版本支持保存独立配置，但不支持一键切换托管。请自行核对环境变量，原服务继续运行。',
                  )
                : t(
                    '。保存只创建新配置，旧服务继续运行；随后可选择是否切换。环境变量需自行核对，IDE 调试会话不会迁移。',
                  ))
            }
          />
        )
      )}
      {externalStop && (
        <ExternalServiceConfirmation
          key={externalStop.token}
          preview={externalStop}
          close={() => setExternalStop(null)}
          confirm={async () => {
            setBusy(true);
            try {
              await call('confirm_stop', {
                token: externalStop.token,
                confirmedPid: externalStop.process.pid,
              });
              notify(t('已发送正常停止信号，请核对退出结果；不会自动强杀'));
            } catch (e) {
              report(e);
            } finally {
              setExternalStop(null);
              setBusy(false);
              await refreshPorts();
            }
          }}
        />
      )}
      {handoff && (
        <ExternalServiceConfirmation
          key={handoff.token}
          preview={handoff}
          close={() => setHandoff(null)}
          confirm={async () => {
            setBusy(true);
            try {
              const run = await call<ServiceRun>('confirm_external_service_handoff', {
                token: handoff.token,
                confirmedPid: handoff.process.pid,
              });
              notify(
                run.state === 'BLOCKED' || run.state === 'CRASHED'
                  ? t('旧实例已停止，新服务未能启动；请查看错误原因，配置已保留')
                  : t('旧实例已退出，DevNook 已提交启动；请核对就绪状态'),
              );
              setRunChoice('latest');
            } catch (e) {
              report(t('切换未完成，配置已保留，请核对旧进程与服务状态：') + message(e));
            } finally {
              setHandoff(null);
              setBusy(false);
              await Promise.all([refresh(), refreshPorts()]);
            }
          }}
        />
      )}
      {confirmation && (
        <ActionConfirmation
          preview={confirmation}
          close={() => setConfirmation(null)}
          confirm={async () => {
            setBusy(true);
            try {
              const run = await call<ServiceRun>('confirm_service_action', {
                token: confirmation.token,
              });
              setConfirmation(null);
              notify(
                run.state === 'BLOCKED'
                  ? t('启动被端口冲突阻断')
                  : run.state === 'CRASHED'
                    ? t('本次启动失败，请查看错误原因')
                    : t('操作已提交，状态按实际进程更新'),
              );
              setRunChoice('latest');
              await refresh();
            } catch (e) {
              setConfirmation(null);
              report(e);
            } finally {
              setBusy(false);
            }
          }}
        />
      )}
      {archive && (
        <Modal
          title={t('移除服务配置？')}
          subtitle={t('配置会归档，历史运行和日志仍保留在本机。')}
          close={() => setArchive(null)}
        >
          <div className="modal-body">
            <strong>{archive.name}</strong>
            <p className="field-hint">{t('正在运行的服务必须先停止。')}</p>
          </div>
          <div className="modal-actions">
            <button className="button secondary" onClick={() => setArchive(null)}>
              {t('取消')}
            </button>
            <button
              className="button danger"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                try {
                  await call('archive_service', { id: archive.id });
                  setArchive(null);
                  setSelected('');
                  await refresh();
                  notify(t('服务配置已归档'));
                } catch (e) {
                  report(e);
                } finally {
                  setBusy(false);
                }
              }}
            >
              {t('移除配置')}
            </button>
          </div>
        </Modal>
      )}
      {clearLog && (
        <Modal
          title={t('清理本次运行日志？')}
          subtitle={t('只清理所选运行的日志文件，保留配置与运行记录。')}
          close={() => setClearLog(null)}
        >
          <div className="modal-body">
            <p>{date(clearLog.run.started_at)}</p>
            <code className="path-block">{clearLog.run.log_path}</code>
          </div>
          <div className="modal-actions">
            <button className="button secondary" onClick={() => setClearLog(null)}>
              {t('取消')}
            </button>
            <button
              className="button danger"
              onClick={async () => {
                try {
                  await call('clear_service_log', { id: clearLog.run.id });
                  clearLog.invalidate();
                  setClearLog(null);
                  notify(t('本次历史日志已清理'));
                } catch (e) {
                  report(e);
                }
              }}
            >
              {t('确认清理')}
            </button>
          </div>
        </Modal>
      )}
    </section>
  );
}

function ActionConfirmation({
  preview,
  close,
  confirm,
}: {
  preview: ServiceActionPreview;
  close: () => void;
  confirm: () => Promise<void>;
}) {
  const [pid, setPid] = useState('');
  const [remaining, setRemaining] = useState(preview.expires_in_seconds);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    const timer = setInterval(() => setRemaining((v) => Math.max(0, v - 1)), 1000);
    return () => clearInterval(timer);
  }, []);
  const force = preview.action === 'force_stop';
  const action = force ? t('强制停止') : preview.action === 'restart' ? t('重启') : t('启动');
  return (
    <Modal
      title={t('确认') + action + ' ' + preview.service.name}
      subtitle={
        force
          ? t('SIGKILL 会立即中断托管进程组，请再次核对 PID 与归属。')
          : t('确认后执行以下命令。')
      }
      className="service-confirm-dialog"
      close={close}
    >
      <div className="modal-body form-stack">
        <div className="service-command">
          <code>{preview.command}</code>
          <span>{preview.service.working_directory}</span>
        </div>
        <details className="service-advanced">
          <summary>{t('执行详情')}</summary>
          <div>
            <span className="field-label">{t('环境变量差异')}</span>
            <div className="environment-preview">
              {Object.entries(preview.service.environment).map(([k, v]) => (
                <code key={k}>
                  {k}={v}
                </code>
              ))}
              {preview.service.secret_keys.map((k) => (
                <code key={k}>
                  {k}
                  {t('=[Keychain · 值不显示]')}
                </code>
              ))}
              {!preview.environment_keys.length && (
                <span>{t('无额外环境变量，仅继承必要的用户与工具路径。')}</span>
              )}
            </div>
          </div>
          <p className="field-hint">
            {t('预期端口：')}
            {preview.service.expected_ports.join(', ') || t('未指定')} ·{' '}
            {preview.service.readiness.kind} {t('就绪检测 · 超时')}{' '}
            {preview.service.readiness.timeout_seconds} {t('秒')}
            <br />
            {preview.action === 'restart'
              ? t('重启会先正常停止原进程组；原进程不退出时会阻止新实例启动。')
              : t('端口冲突会阻止执行，不会自动停止其他进程。')}
            <br />
            {t('确认剩余')}
            {remaining} {t('秒。')}
          </p>
        </details>
        <span className="field-hint">
          {t('确认剩余')}
          {remaining} {t('秒。')}
        </span>
        {force && (
          <label>
            {t('输入托管进程组 ID')}
            {preview.run?.pgid} {t('确认')}
            <input
              autoComplete="off"
              inputMode="numeric"
              value={pid}
              onChange={(e) => setPid(e.target.value)}
            />
          </label>
        )}
      </div>
      <div className="modal-actions">
        <button className="button secondary" onClick={close}>
          {t('取消')}
        </button>
        <button
          className={'button ' + (force ? 'danger' : 'primary')}
          disabled={busy || !remaining || (force && pid !== String(preview.run?.pgid))}
          onClick={async () => {
            setBusy(true);
            try {
              await confirm();
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? <LoaderCircle size={15} className="spin" /> : <Check size={15} />}
          {t('确认')}
          {action}
        </button>
      </div>
    </Modal>
  );
}

function ServiceEditor({
  initial,
  services,
  projects,
  close,
  saved,
  inline = false,
  projectLocked = false,
  title,
  description,
}: {
  title?: string;
  description?: string;
  projectLocked?: boolean;
  inline?: boolean;
  initial: ServiceDefinition;
  services: ServiceView[];
  projects: Project[];
  close: () => void;
  saved: (definition: ServiceDefinition, secrets: Record<string, string>) => Promise<void>;
}) {
  const [edit, setEdit] = useState(initial);
  const [command, setCommand] = useState(
    !initial.executable
      ? ''
      : initial.shell_mode
        ? initial.executable
        : [initial.executable, ...initial.args]
            .map((word) =>
              /^[\w./:@%+=,-]+$/.test(word) ? word : "'" + word.replaceAll("'", "'\\''") + "'",
            )
            .join(' '),
  );
  const [ports, setPorts] = useState(initial.expected_ports.join(', '));
  const [env, setEnv] = useState(
    Object.entries(initial.environment)
      .map(([k, v]) => k + '=' + v)
      .join('\n'),
  );
  const [secrets, setSecrets] = useState(initial.secret_keys.map((key) => ({ key, value: '' })));
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const form = (
    <form
      onInvalidCapture={(event) => {
        const details = (event.target as HTMLElement).closest('details');
        if (details) details.open = true;
      }}
      onSubmit={async (e) => {
        e.preventDefault();
        setBusy(true);
        setError('');
        try {
          const environment: Record<string, string> = {};
          for (const line of env.split('\n').filter((l) => l.trim())) {
            const split = line.indexOf('=');
            if (split < 1) throw new Error(t('环境变量格式应为 KEY=value，每行一项'));
            const key = line.slice(0, split).trim();
            if (Object.hasOwn(environment, key)) throw new Error(t('环境变量名称重复'));
            environment[key] = line.slice(split + 1);
          }
          const expected_ports = ports
            .split(/[,，\s]+/)
            .filter(Boolean)
            .map((v) => {
              if (!/^\d+$/.test(v) || Number(v) < 1 || Number(v) > 65535)
                throw new Error(t('端口应为 1–65535 的整数'));
              return Number(v);
            });
          const secret_values: Record<string, string> = {};
          for (const row of secrets) {
            if (!row.key.trim()) throw new Error(t('敏感变量名称不能为空'));
            if (row.value) secret_values[row.key.trim()] = row.value;
          }
          const parsed = edit.shell_mode
            ? { executable: command, args: [] }
            : await call<{ executable: string; args: string[] }>('parse_service_command', {
                command,
              });
          await saved(
            {
              ...edit,
              ...parsed,
              environment,
              secret_keys: secrets.map((v) => v.key.trim()),
              expected_ports,
            },
            secret_values,
          );
        } catch (e) {
          setError(message(e));
        } finally {
          setBusy(false);
        }
      }}
    >
      <div className="modal-body form-stack service-editor">
        {error && (
          <div className="error-banner" role="alert">
            {t(error)}
          </div>
        )}
        <label>
          {t('服务名称')}
          <input
            required
            maxLength={160}
            placeholder={t('如：frontend、api、worker')}
            value={edit.name}
            onChange={(e) => setEdit({ ...edit, name: e.target.value })}
          />
        </label>
        <label>
          {t('启动命令')}
          <input
            aria-label={t('启动命令')}
            required
            autoComplete="off"
            spellCheck={false}
            placeholder={t('如：pnpm dev、python3 app.py')}
            value={command}
            onChange={(e) => setCommand(e.target.value)}
          />
        </label>
        <p className="field-hint command-directory">
          {t('运行目录：')}
          {edit.working_directory}
        </p>
        <details className="service-advanced">
          <summary>{t('高级设置')}</summary>
          <div className="form-stack">
            <div className="form-row">
              <div className="form-field">
                <label htmlFor="service-project">{t('所属项目')}</label>
                <Select
                  id="service-project"
                  aria-label={t('所属项目')}
                  value={edit.project_id}
                  disabled={inline || projectLocked || !!initial.id}
                  required
                  placeholder={t('选择所属项目')}
                  options={projects.map((project) => ({ value: project.id, label: project.name }))}
                  onValueChange={(projectId) => {
                    const p = projects.find((p) => p.id === projectId);
                    setEdit({
                      ...edit,
                      project_id: projectId,
                      working_directory: p?.canonical_path || '',
                    });
                  }}
                />
              </div>
            </div>
            <div className="form-row">
              <label>
                {t('运行类型')}
                <Select
                  aria-label={t('运行类型')}
                  value={edit.runtime_type}
                  onValueChange={(runtime_type) => setEdit({ ...edit, runtime_type })}
                  options={[
                    { value: 'NODE', label: 'Node.js' },
                    { value: 'PYTHON', label: 'Python' },
                    { value: 'JAVA', label: 'Java / Maven / Gradle' },
                    { value: 'OTHER', label: t('其他命令') },
                  ]}
                />
              </label>
              <label>
                {t('预期端口')}
                <input
                  placeholder={t('如：5173, 8080')}
                  value={ports}
                  onChange={(e) => setPorts(e.target.value)}
                />
              </label>
            </div>
            <fieldset className="dependency-picker">
              <legend>{t('服务依赖 · 全部 READY 后才能启动')}</legend>
              {services
                .filter((s) => s.definition.id !== edit.id)
                .map(({ definition: d }) => (
                  <label className="checkbox-label" key={d.id}>
                    <input
                      type="checkbox"
                      checked={edit.dependencies.includes(d.id)}
                      onChange={(e) =>
                        setEdit({
                          ...edit,
                          dependencies: e.target.checked
                            ? [...edit.dependencies, d.id]
                            : edit.dependencies.filter((id) => id !== d.id),
                        })
                      }
                    />
                    {d.name}
                  </label>
                ))}
              {services.filter((s) => s.definition.id !== edit.id).length === 0 && (
                <p className="muted">{t('添加其他服务后可设置依赖。')}</p>
              )}
            </fieldset>
            <label>
              {t('工作目录')}
              <input
                required
                value={edit.working_directory}
                onChange={(e) => setEdit({ ...edit, working_directory: e.target.value })}
              />
            </label>
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={edit.shell_mode}
                onChange={(e) => setEdit({ ...edit, shell_mode: e.target.checked })}
              />
              {t('显式使用 Shell Mode（/bin/zsh -lc）')}
            </label>
            <div className="editor-divider">
              <ShieldCheck size={15} />
              {t('环境变量 · 敏感值进入 Keychain，不保存到数据库或日志')}
            </div>
            <label>
              {t('普通环境变量（每行 KEY=value）')}
              <textarea
                rows={3}
                placeholder={'NODE_ENV=development\nSPRING_PROFILES_ACTIVE=dev'}
                value={env}
                onChange={(e) => setEnv(e.target.value)}
              />
            </label>
            <div className="secret-heading">
              <span>{t('Keychain 敏感环境变量')}</span>
              <button
                type="button"
                className="text-button"
                onClick={() => setSecrets([...secrets, { key: '', value: '' }])}
              >
                <Plus size={13} />
                {t('添加变量')}
              </button>
            </div>
            {secrets.map((row, i) => (
              <div className="secret-row" key={i}>
                <input
                  aria-label={t('敏感变量名称 ') + (i + 1)}
                  placeholder="API_TOKEN"
                  autoComplete="off"
                  value={row.key}
                  onChange={(e) =>
                    setSecrets(secrets.map((v, j) => (i === j ? { ...v, key: e.target.value } : v)))
                  }
                />
                <input
                  type="password"
                  aria-label={t('敏感变量值 ') + (i + 1)}
                  autoComplete="new-password"
                  placeholder={
                    initial.secret_keys.includes(row.key)
                      ? t('留空保留已有 Keychain 值')
                      : t('输入要存入 Keychain 的值')
                  }
                  value={row.value}
                  onChange={(e) =>
                    setSecrets(
                      secrets.map((v, j) => (i === j ? { ...v, value: e.target.value } : v)),
                    )
                  }
                />
                <button
                  type="button"
                  className="icon-button"
                  aria-label={t('移除敏感变量 ') + (i + 1)}
                  onClick={() => setSecrets(secrets.filter((_, j) => j !== i))}
                >
                  <X size={15} />
                </button>
              </div>
            ))}
            <div className="editor-divider">
              <Activity size={15} />
              {t('就绪检测')}
            </div>
            <div className="form-row">
              <label>
                {t('检测方式')}
                <Select
                  aria-label={t('检测方式')}
                  value={edit.readiness.kind}
                  onValueChange={(kind) =>
                    setEdit({
                      ...edit,
                      readiness: { ...edit.readiness, kind: kind as ReadinessKind },
                    })
                  }
                  options={[
                    { value: 'PROCESS', label: t('PROCESS · 存活并通过宽限期') },
                    { value: 'PORT', label: t('PORT · 本次进程组监听端口') },
                    { value: 'HTTP', label: t('HTTP · 本机健康检查 2xx') },
                    { value: 'LOG_PHRASE', label: t('LOG_PHRASE · 输出包含关键字') },
                  ]}
                />
              </label>
              <label>
                {t('就绪超时（秒）')}
                <input
                  type="number"
                  min={1}
                  max={300}
                  required
                  value={edit.readiness.timeout_seconds}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      readiness: { ...edit.readiness, timeout_seconds: Number(e.target.value) },
                    })
                  }
                />
              </label>
            </div>
            {edit.readiness.kind === 'PORT' && (
              <label>
                {t('检测端口')}
                <input
                  type="number"
                  min={1}
                  max={65535}
                  required
                  value={edit.readiness.port ?? ''}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      readiness: { ...edit.readiness, port: Number(e.target.value) },
                    })
                  }
                />
              </label>
            )}
            {edit.readiness.kind === 'HTTP' && (
              <label>
                {t('健康检查 URL')}
                <input
                  required
                  placeholder="http://127.0.0.1:8080/health"
                  value={edit.readiness.url}
                  onChange={(e) =>
                    setEdit({ ...edit, readiness: { ...edit.readiness, url: e.target.value } })
                  }
                />
                <span className="field-hint">
                  {t('仅访问 loopback HTTP，要求本次进程组持有目标端口；不跟随重定向。')}
                </span>
              </label>
            )}
            {edit.readiness.kind === 'LOG_PHRASE' && (
              <label>
                {t('日志关键字')}
                <input
                  required
                  maxLength={4096}
                  value={edit.readiness.phrase}
                  onChange={(e) =>
                    setEdit({ ...edit, readiness: { ...edit.readiness, phrase: e.target.value } })
                  }
                />
              </label>
            )}
            <div className="form-row">
              <label>
                {t('检测间隔（毫秒）')}
                <input
                  type="number"
                  min={100}
                  max={5000}
                  required
                  value={edit.readiness.interval_ms}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      readiness: { ...edit.readiness, interval_ms: Number(e.target.value) },
                    })
                  }
                />
              </label>
              <label>
                {t('进程启动宽限（毫秒）')}
                <input
                  type="number"
                  min={100}
                  max={10000}
                  required
                  value={edit.readiness.grace_ms}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      readiness: { ...edit.readiness, grace_ms: Number(e.target.value) },
                    })
                  }
                />
              </label>
            </div>
            <div className="form-row">
              <label>
                {t('正常停止宽限（秒）')}
                <input
                  type="number"
                  min={1}
                  max={30}
                  required
                  value={edit.stop_grace_seconds}
                  onChange={(e) => setEdit({ ...edit, stop_grace_seconds: Number(e.target.value) })}
                />
              </label>
              <label>
                {t('每次日志上限（KiB）')}
                <input
                  type="number"
                  min={64}
                  max={10240}
                  required
                  value={edit.log_limit_kb}
                  onChange={(e) => setEdit({ ...edit, log_limit_kb: Number(e.target.value) })}
                />
              </label>
            </div>
          </div>
        </details>
      </div>
      <div className="modal-actions">
        {!inline && (
          <span className="modal-hint">
            <ShieldCheck size={13} />
            {t('不会修改项目源码或安装依赖')}
          </span>
        )}
        {!inline && (
          <button type="button" className="button secondary" disabled={busy} onClick={close}>
            {t('取消')}
          </button>
        )}
        <button type="submit" className="button primary" disabled={busy}>
          {busy ? <LoaderCircle size={15} className="spin" /> : <Play size={15} />}
          {inline ? t('启动…') : t('保存配置')}
        </button>
      </div>
    </form>
  );
  return inline ? (
    form
  ) : (
    <Modal
      title={title || (initial.id ? t('编辑启动方式') : t('添加服务'))}
      subtitle={description || t('填写启动命令，其余选项按需展开。')}
      className="service-editor-dialog"
      close={() => !busy && close()}
    >
      {form}
    </Modal>
  );
}
