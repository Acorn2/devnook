import { t, dateLocale } from './i18n';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import {
  Activity,
  ArrowLeft,
  Code2,
  Check,
  CircleAlert,
  FileText,
  LoaderCircle,
  MoreHorizontal,
  Pencil,
  Play,
  Plus,
  RotateCw,
  Search,
  ShieldCheck,
  Square,
  SquareTerminal,
  Trash2,
  X,
} from 'lucide-react';
import { call } from './api';
import { Modal } from './components';
import ExternalServiceConfirmation from './ExternalServiceConfirmation';
import ServiceConflicts from './ServiceConflicts';
import ServiceBrowser from './ServiceBrowser';
import Select from './Select';
import {
  ExternalProcessDetail,
  ListeningPorts,
  PortSummary,
  ProcessAssignment,
} from './ServicePorts';
import type {
  LogPage,
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

const labels: Record<ServiceState, string> = {
  STOPPED: '已停止',
  STARTING: '正在启动',
  RUNNING: '运行中 · 未就绪',
  READY: '已就绪',
  STOPPING: '正在停止',
  BLOCKED: '启动被阻断',
  CRASHED: '运行失败',
  UNKNOWN: '身份待核验',
};
const active = (state?: ServiceState) =>
  !!state && ['STARTING', 'RUNNING', 'READY', 'STOPPING', 'UNKNOWN'].includes(state);
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
export function ServiceBadge({ state }: { state: ServiceState }) {
  return (
    <span className={'service-badge state-' + state.toLowerCase()}>
      <span className="status-dot" />
      {t(labels[state])}
    </span>
  );
}

export default function ServicesPage({
  navigation,
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
  launchProject,
  inspectPort,
  showAllServices,
  report,
  notify,
}: {
  navigation: ReactNode;
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
  launchProject: (project: Project) => Promise<void>;
  inspectPort: (pid: number) => void;
  showAllServices: () => void;
  report: (e: unknown) => void;
  notify: (s: string) => void;
}) {
  const [services, setServices] = useState<ServiceView[]>([]);
  const [selected, setSelected] = useState('');
  const [filter, setFilter] = useState(projectId);
  const [query, setQuery] = useState('');
  const [editing, setEditing] = useState<ServiceDefinition | null>(null);
  const [archive, setArchive] = useState<ServiceDefinition | null>(null);
  const [confirmation, setConfirmation] = useState<ServiceActionPreview | null>(null);
  const [externalStop, setExternalStop] = useState<StopPreview | null>(null);
  const [externalSetup, setExternalSetup] = useState<ExternalServiceConfiguration | null>(null);
  const [handoff, setHandoff] = useState<ExternalServiceHandoffPreview | null>(null);
  const [history, setHistory] = useState<ServiceRun[]>([]);
  const [runChoice, setRunChoice] = useState('latest');
  const [log, setLog] = useState<LogPage | null>(null);
  const [logQuery, setLogQuery] = useState('');
  const [logError, setLogError] = useState('');
  const [clearLog, setClearLog] = useState(false);
  const [busy, setBusy] = useState(false);
  const [fetchError, setFetchError] = useState('');
  const project = projects.find((p) => p.id === projectId);
  const [drafts, setDrafts] = useState<ServiceDraft[]>([]);
  const [draftError, setDraftError] = useState('');
  const [assignment, setAssignment] = useState<{ pid: number | null } | null>(null);
  const portsAvailable = !!lastRefresh && !portError;
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
    }
  }, [disabled]);
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
  useEffect(() => setFilter(projectId), [projectId]);
  const visible = services.filter(
    (v) =>
      (!filter || v.definition.project_id === filter) &&
      [
        v.definition.name,
        v.definition.executable,
        v.definition.working_directory,
        ...actualPorts(v.definition.id).flatMap((b) => [b.port, b.pid]),
      ]
        .join(' ')
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  const pending = drafts.filter(
    (draft) =>
      !services.some(
        (v) =>
          v.definition.project_id === projectId &&
          v.definition.working_directory === draft.definition.working_directory,
      ) &&
      [draft.definition.name, draft.command, draft.definition.working_directory]
        .join(' ')
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  const keyOf = (draft: ServiceDraft) => 'draft:' + draft.definition.working_directory;
  const view = visible.find((v) => v.definition.id === selected);
  const draft = pending.find((d) => keyOf(d) === selected);
  useEffect(() => {
    if (!view && !draft && !externalView)
      setSelected(
        visible[0]?.definition.id ||
          (external[0] ? `external:${external[0][0].pid}` : pending[0] ? keyOf(pending[0]) : ''),
      );
  }, [view, draft, visible, pending, externalView, external]);
  useEffect(() => {
    setRunChoice('latest');
    setLog(null);
    setLogQuery('');
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
    runChoice === 'latest' ? view?.current_run : history.find((r) => r.id === runChoice);
  useEffect(() => {
    const id = chosenRun?.id;
    if (!id || !chosenRun.log_path || disabled) {
      setLog(null);
      return;
    }
    let cancelled = false;
    let inFlight = false;
    setLog(null);
    setLogError('');
    const load = async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        const page = await call<LogPage>('service_logs', { id, limit: 500, query: logQuery });
        if (!cancelled) {
          setLog(page);
          setLogError('');
        }
      } catch (e) {
        if (!cancelled) setLogError(message(e));
      } finally {
        inFlight = false;
      }
    };
    void load();
    const timer = setInterval(() => void load(), 1000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [chosenRun?.id, chosenRun?.log_path, logQuery, disabled]);
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
  async function saveExternal(
    definition: ServiceDefinition,
    secret_values: Record<string, string>,
  ) {
    if (!externalSetup) return;
    const previous = externalSetup.process;
    setBusy(true);
    try {
      const saved = await call<ServiceDefinition>('save_service', {
        input: { definition, secret_values },
      });
      // Saving is complete even if the old process disappears before a handoff preview.
      serviceVersion.current += 1;
      setExternalSetup(null);
      setSelected(saved.id);
      setFilter(saved.project_id);
      setServices((values) => [...values, { definition: saved, current_run: null }]);
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
  const projectName = (id: string) => projects.find((p) => p.id === id)?.name || t('已移除的项目');
  const relativeDirectory = (directory: string, id: string) => {
    const root = projects.find((p) => p.id === id)?.canonical_path;
    if (directory === root) return '.';
    return root && directory.startsWith(root + '/')
      ? '.' + directory.slice(root.length)
      : directory;
  };
  const hasChildDraft = drafts.some(
    (d) => d.definition.working_directory !== project?.canonical_path,
  );
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
              <strong title={value.definition.name}>{value.definition.name}</strong>
            </span>
            <span className="service-badge state-stopped">{t('未配置')}</span>
          </span>
          <span className="service-project">{value.command || t('填写启动命令后即可运行')}</span>
        </button>
        <button
          className="text-button"
          disabled={disabled || busy}
          onClick={() => void startDraft(value)}
        >
          <Play size={13} />
          {value.command ? t('启动…') : t('填写命令')}
        </button>
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
            onChange={(e) => setQuery(e.target.value)}
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
          <div className="port-list-header">
            <span>{t('托管 / 待配置')}</span>
            <span>{visible.length + pending.length}</span>
          </div>
          {!visible.length && !pending.length && (
            <div className="service-empty">
              <SquareTerminal size={29} strokeWidth={1.2} />
              <p>{!projects.length ? t('先扫描并导入一个项目') : t('暂无服务')}</p>
              <span>{t('可添加服务并填写启动命令。')}</span>
            </div>
          )}
          {visible.map((v) => (
            <div
              key={v.definition.id}
              className={'service-row ' + (selected === v.definition.id ? 'selected' : '')}
            >
              <button className="service-row-select" onClick={() => setSelected(v.definition.id)}>
                <span className="service-row-heading">
                  <span className="service-row-title">
                    <SquareTerminal size={17} />
                    <strong title={v.definition.name}>{v.definition.name}</strong>
                  </span>
                  <ServiceBadge state={v.current_run?.state || 'STOPPED'} />
                </span>
                <span className="service-row-meta">
                  <PortSummary
                    bindings={actualPorts(v.definition.id)}
                    available={portsAvailable}
                    compact
                  />
                  <span className="service-project">
                    {project
                      ? relativeDirectory(v.definition.working_directory, v.definition.project_id)
                      : projectName(v.definition.project_id)}
                  </span>
                </span>
              </button>
              <div className="service-row-actions">
                <button
                  className="text-button"
                  disabled={
                    disabled ||
                    busy ||
                    v.current_run?.state === 'UNKNOWN' ||
                    v.current_run?.state === 'STOPPING'
                  }
                  onClick={() =>
                    active(v.current_run?.state) ? void stop(v) : void prepare('start', v)
                  }
                >
                  {active(v.current_run?.state) ? <Square size={13} /> : <Play size={13} />}
                  {active(v.current_run?.state) ? t('停止') : t('启动')}
                </button>
              </div>
            </div>
          ))}
          {pending
            .filter(
              (d) => !hasChildDraft || d.definition.working_directory !== project?.canonical_path,
            )
            .map(draftRow)}
          {hasChildDraft &&
            pending.some((d) => d.definition.working_directory === project?.canonical_path) && (
              <details className="other-launches">
                <summary>{t('其他启动方式')}</summary>
                <p>{t('根目录脚本可能同时启动多个服务，请避免重复启动。')}</p>
                {pending
                  .filter((d) => d.definition.working_directory === project?.canonical_path)
                  .map(draftRow)}
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
            >
              <button
                className="service-row-select"
                onClick={() => setSelected(`external:${bindings[0].pid}`)}
              >
                <span className="service-row-title">
                  <Activity size={17} />
                  <strong>{bindings[0].name}</strong>
                  <small>{t('外部')}</small>
                </span>
                <span className="service-project">
                  {projectName(bindings[0].project_id!)} · PID {bindings[0].pid}
                </span>
                <PortSummary bindings={bindings} available={portsAvailable} />
              </button>
            </div>
          ))}
          {!external.length && (!portsAvailable || query) && (
            <p className="external-empty">
              {portsAvailable
                ? query
                  ? t('没有匹配的外部服务')
                  : t('暂未发现外部启动的项目服务。其他监听可在端口管理查看。')
                : t('端口状态未知，暂无法列出外部服务')}
            </p>
          )}
        </div>
        <div
          className={
            'service-detail' + (view && !externalView && !draft ? ' service-detail-managed' : '')
          }
        >
          {externalView ? (
            <ExternalProcessDetail
              bindings={externalView}
              projects={projects}
              associate={() => setAssignment({ pid: externalView[0].pid })}
              disabled={disabled || busy || !portsAvailable}
              stop={() => void externalAction(externalView[0].pid, 'stop')}
              configure={() => void externalAction(externalView[0].pid, 'configure')}
              report={report}
              portsAvailable={portsAvailable}
            />
          ) : draft ? (
            <div className="draft-service">
              <h2>{t('配置启动方式')}</h2>
              <p className="field-hint">{t(draft.source)}</p>
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
                    <h2>{view.definition.name}</h2>
                    <ServiceBadge state={view.current_run?.state || 'STOPPED'} />
                  </div>
                  <div className="service-actions">
                    <ServiceBrowser
                      key={view.definition.id}
                      target={{ kind: 'MANAGED', service_id: view.definition.id }}
                      bindings={actualPorts(view.definition.id)}
                      available={portsAvailable}
                      disabled={
                        disabled ||
                        busy ||
                        !['STARTING', 'RUNNING', 'READY'].includes(view.current_run?.state || '')
                      }
                      report={report}
                    />
                    {!active(view.current_run?.state) && (
                      <button
                        className="button primary small"
                        disabled={disabled || busy || active(view.current_run?.state)}
                        onClick={() => void prepare('start')}
                      >
                        <Play size={15} />
                        {t('启动')}
                      </button>
                    )}
                    {active(view.current_run?.state) && (
                      <button
                        className="button secondary small"
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
                        className="button secondary small"
                        disabled={disabled || busy || view.current_run?.state === 'UNKNOWN'}
                        onClick={() => void prepare('restart')}
                      >
                        <RotateCw size={15} />
                        {t('重启')}
                      </button>
                    )}
                    <ServiceOptions label={t('更多服务操作')}>
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
                <div className="service-command service-command-inline">
                  <code>
                    {view.definition.shell_mode ? '/bin/zsh -lc ' : ''}
                    {view.definition.executable}{' '}
                    {view.definition.args.map((a) => JSON.stringify(a)).join(' ')}
                  </code>
                  <span title={view.definition.working_directory}>
                    {!project && projectName(view.definition.project_id) + ' · '}
                    {relativeDirectory(
                      view.definition.working_directory,
                      view.definition.project_id,
                    )}
                  </span>
                </div>
                <div className="service-actual-ports">
                  <span>{t('实际监听')}</span>
                  <PortSummary
                    bindings={actualPorts(view.definition.id)}
                    available={portsAvailable}
                  />
                </div>
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
                      setSelected(id);
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
                  <p className="field-hint command-directory">
                    {t('工作目录：')}
                    {view.definition.working_directory}
                  </p>
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
              </div>
              <section className="service-log-section" aria-label={t('运行日志')}>
                <div className="log-toolbar">
                  <h3>
                    <FileText size={16} />
                    {t('运行日志')}
                  </h3>
                  <Select
                    aria-label={t('选择运行记录')}
                    value={runChoice}
                    onValueChange={setRunChoice}
                    options={[
                      { value: 'latest', label: t('最近运行') },
                      ...history.map((run) => ({
                        value: run.id,
                        label: `${date(run.started_at)} · ${t(labels[run.state])}`,
                      })),
                    ]}
                  />
                  <div className="search-field">
                    <Search size={14} />
                    <input
                      aria-label={t('搜索服务日志')}
                      placeholder={t('筛选日志关键字…')}
                      value={logQuery}
                      onChange={(e) => setLogQuery(e.target.value)}
                    />
                    {logQuery && (
                      <button
                        className="icon-button"
                        aria-label={t('清空日志搜索')}
                        onClick={() => setLogQuery('')}
                      >
                        <X size={14} />
                      </button>
                    )}
                  </div>
                  <ServiceOptions label={t('更多日志操作')}>
                    <button
                      className="text-button"
                      disabled={!chosenRun?.log_path || active(chosenRun?.state)}
                      onClick={() => setClearLog(true)}
                    >
                      <Trash2 size={13} />
                      {t('清理本次日志…')}
                    </button>
                  </ServiceOptions>
                </div>
                <div
                  className="service-log"
                  role="log"
                  aria-label={t('服务输出日志')}
                  aria-live="off"
                  tabIndex={0}
                >
                  {logError ? (
                    <span className="log-error">{t(logError)}</span>
                  ) : !chosenRun ? (
                    <span className="log-empty">
                      {t('启动后，在这里查看带时间戳的 stdout / stderr。')}
                    </span>
                  ) : !log?.lines.length ? (
                    <span className="log-empty">
                      {logQuery ? t('没有匹配的日志行。') : t('本次运行暂时没有可显示的输出。')}
                    </span>
                  ) : (
                    log.lines.map((line, i) => (
                      <div
                        key={i}
                        className={
                          line.includes('[stderr]')
                            ? 'stderr-line'
                            : line.includes('[system]')
                              ? 'system-line'
                              : ''
                        }
                      >
                        {line}
                      </div>
                    ))
                  )}
                </div>
                <div className="log-footer">
                  <span>
                    <span className="status-dot" />
                    {t('每秒更新 · 最多显示最近 500 行')}
                  </span>
                  <span>
                    {log?.truncated || chosenRun?.log_truncated
                      ? t('日志已达到限制或仅显示尾部')
                      : t('每次运行最多 ') + view.definition.log_limit_kb + ' KiB'}
                  </span>
                </div>
              </section>
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
      {externalSetup && (
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
            t(
              '。保存只创建新配置，旧服务继续运行；随后可选择是否切换。环境变量需自行核对，IDE 调试会话不会迁移。',
            )
          }
        />
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
      {clearLog && chosenRun && (
        <Modal
          title={t('清理本次运行日志？')}
          subtitle={t('只清理所选运行的日志文件，保留配置与运行记录。')}
          close={() => setClearLog(false)}
        >
          <div className="modal-body">
            <p>{date(chosenRun.started_at)}</p>
            <code className="path-block">{chosenRun.log_path}</code>
          </div>
          <div className="modal-actions">
            <button className="button secondary" onClick={() => setClearLog(false)}>
              {t('取消')}
            </button>
            <button
              className="button danger"
              onClick={async () => {
                try {
                  await call('clear_service_log', { id: chosenRun.id });
                  setLog(null);
                  setClearLog(false);
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

function ServiceOptions({ label, children }: { label: string; children: ReactNode }) {
  const root = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target) && root.current)
        root.current.open = false;
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, []);
  return (
    <details
      ref={root}
      className="service-options"
      onBlur={(event) => {
        if (
          event.relatedTarget instanceof Node &&
          !event.currentTarget.contains(event.relatedTarget)
        )
          event.currentTarget.open = false;
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape' && event.currentTarget.open) {
          event.preventDefault();
          event.stopPropagation();
          event.currentTarget.open = false;
          event.currentTarget.querySelector('summary')?.focus();
        }
      }}
    >
      <summary className="icon-button" aria-label={label} title={label}>
        <MoreHorizontal size={17} />
      </summary>
      <div
        className="service-options-content"
        role="group"
        aria-label={label}
        onClick={(event) => {
          if ((event.target as HTMLElement).closest('button') && root.current)
            root.current.open = false;
        }}
      >
        {children}
      </div>
    </details>
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
