import { t } from './i18n';
import { useEffect, useRef, useState } from 'react';
import { CircleAlert, Network, Search, ShieldCheck, Square, SquareTerminal } from 'lucide-react';
import { call } from './api';
import { Modal } from './components';
import Select from './Select';
import ServiceBrowser from './ServiceBrowser';
import type { PortBinding, ProcessDetail, Project } from './types';

export function portNumbers(bindings: PortBinding[]) {
  return [...new Set(bindings.map((b) => b.port))].sort((a, b) => a - b);
}

export function PortSummary({
  bindings,
  available,
  compact = false,
}: {
  bindings: PortBinding[];
  available: boolean;
  compact?: boolean;
}) {
  const summary = available
    ? portNumbers(bindings)
        .map((p) => `:${p}`)
        .join(' · ') || (compact ? t('无 TCP 监听') : t('未检测到 TCP 监听'))
    : t('端口状态未知');
  return (
    <span className="service-port-summary" title={summary}>
      <Network size={13} />
      <span>{summary}</span>
    </span>
  );
}

export function ListeningPorts({
  bindings,
  compact = false,
}: {
  bindings: PortBinding[];
  compact?: boolean;
}) {
  return (
    <div
      className={'listening-bindings' + (compact ? ' listening-bindings-compact' : '')}
      aria-label={t('实际 TCP 监听地址')}
    >
      {bindings.map((b) => (
        <span key={`${b.pid}:${b.family}:${b.address}:${b.port}`}>
          <code>
            {b.family === 'IPv6'
              ? `[${b.address.replace(/^\[|\]$/g, '')}]:${b.port}`
              : `${b.address}:${b.port}`}
          </code>
          <small>
            {b.family}
            {!compact && ` · PID ${b.pid}`}
          </small>
        </span>
      ))}
    </div>
  );
}

export function ExternalProcessDetail({
  bindings,
  projects,
  associate,
  disabled,
  stop,
  configure,
  report,
  portsAvailable,
}: {
  bindings: PortBinding[];
  projects: Project[];
  associate: () => void;
  disabled: boolean;
  stop: () => void;
  configure: () => void;
  report: (error: unknown) => void;
  portsAvailable: boolean;
}) {
  const process = bindings[0];
  return (
    <section className="external-service-detail" aria-label={t('外部服务详情')}>
      <div className="service-detail-heading">
        <div className="service-detail-title">
          <h2>{process.name}</h2>
          <span className="service-badge state-ready">{t('正在监听')}</span>
        </div>
        <button
          className="button secondary small"
          aria-label={t('确认或更改项目归属')}
          onClick={associate}
        >
          {t('项目归属…')}
        </button>
      </div>
      <div className="external-service-meta">
        <span className="external-service-readonly">
          <ShieldCheck size={13} />
          {t('外部启动 · 尚未托管')}
        </span>
        <span>{projects.find((p) => p.id === process.project_id)?.name || t('未关联项目')}</span>
        <span>PID {process.pid}</span>
      </div>
      <dl className="external-service-facts">
        <div>
          <dt>{t('监听地址')}</dt>
          <dd>
            <ListeningPorts bindings={bindings} compact />
          </dd>
        </div>
        <div>
          <dt>{t('启动命令')}</dt>
          <dd>
            <code>{process.command || process.name}</code>
          </dd>
        </div>
        <div>
          <dt>{t('工作目录')}</dt>
          <dd>
            <code>{process.cwd || t('工作目录无法读取')}</code>
          </dd>
        </div>
      </dl>
      <div className="service-actions">
        <ServiceBrowser
          key={process.pid}
          target={{ kind: 'EXTERNAL', pid: process.pid }}
          bindings={bindings}
          available={portsAvailable}
          disabled={disabled}
          report={report}
        />
        <button className="button danger" disabled={disabled} onClick={stop}>
          <Square size={14} />
          {t('停止外部服务')}
        </button>
        <button className="button secondary" disabled={disabled} onClick={configure}>
          <SquareTerminal size={15} />
          {t('配置为 DevNook 服务')}
        </button>
      </div>
      <p className="field-hint">
        {t('停止前核验身份并要求确认。保存配置不会停止旧服务，是否切换将单独确认。')}
      </p>
    </section>
  );
}

export function ProcessAssignment({
  initialPid,
  projectId,
  projects,
  ports,
  close,
  assigned,
}: {
  initialPid: number | null;
  projectId: string;
  projects: Project[];
  ports: PortBinding[];
  close: () => void;
  assigned: () => Promise<void>;
}) {
  const [pid, setPid] = useState(initialPid);
  const [query, setQuery] = useState('');
  const [detail, setDetail] = useState<ProcessDetail | null>(null);
  const [target, setTarget] = useState(projectId);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const request = useRef(0);
  useEffect(() => {
    const version = ++request.current;
    setDetail(null);
    setError('');
    if (pid === null) return;
    setLoading(true);
    call<ProcessDetail>('inspect_process', { pid })
      .then((value) => {
        if (version !== request.current) return;
        setDetail(value);
        setTarget(projectId || value.project_id || value.suggested_project_id || '');
      })
      .catch((e) => {
        if (version === request.current) setError(String(e));
      })
      .finally(() => {
        if (version === request.current) setLoading(false);
      });
    return () => {
      request.current++;
    };
  }, [pid, projectId]);
  const candidates = [
    ...new Set(
      ports.filter((b) => b.source !== 'DEVNOOK' && b.process_kind === 'SERVICE').map((b) => b.pid),
    ),
  ]
    .map((pid) => ports.filter((b) => b.pid === pid))
    .filter((bindings) =>
      [
        bindings[0].name,
        bindings[0].command,
        bindings[0].cwd,
        bindings[0].pid,
        ...portNumbers(bindings),
      ]
        .join(' ')
        .toLowerCase()
        .includes(query.toLowerCase()),
    )
    .sort((a, b) => Number(!!a[0].project_id) - Number(!!b[0].project_id) || a[0].pid - b[0].pid);
  return (
    <Modal
      title={t('确认进程的项目归属')}
      subtitle={t('只列出已识别的服务入口。其他监听可在端口管理查看；确认归属不会创建或接管服务。')}
      close={() => !busy && close()}
      className="process-assignment-dialog"
    >
      <div className="modal-body">
        {initialPid === null && (
          <>
            <div className="search-field">
              <Search size={15} />
              <input
                aria-label={t('查找外部服务')}
                placeholder={t('搜索端口、PID、命令或目录…')}
                value={query}
                disabled={busy}
                onChange={(e) => setQuery(e.target.value)}
              />
            </div>
            <div className="assignment-candidates" role="group" aria-label={t('外部服务监听')}>
              {candidates.map((bindings) => (
                <button
                  key={bindings[0].pid}
                  aria-pressed={pid === bindings[0].pid}
                  disabled={busy}
                  onClick={() => setPid(bindings[0].pid)}
                >
                  <strong>
                    {bindings[0].name} · PID {bindings[0].pid}
                  </strong>
                  <PortSummary bindings={bindings} available />
                  <small>
                    {projects.find((p) => p.id === bindings[0].project_id)?.name || t('未关联项目')}
                    {' · '}
                    {bindings[0].cwd || t('工作目录无法读取')}
                  </small>
                </button>
              ))}
              {!candidates.length && <p className="field-hint">{t('没有匹配的外部服务监听。')}</p>}
            </div>
          </>
        )}
        {loading && <p role="status">{t('正在核验进程身份…')}</p>}
        {error && (
          <p className="error-banner" role="alert">
            <CircleAlert size={15} />
            {t(error)}
          </p>
        )}
        {detail && (
          <div className="assignment-detail">
            <strong>
              {detail.name} · PID {detail.pid}
            </strong>
            <code>{detail.command}</code>
            <span>{detail.cwd || t('工作目录无法读取')}</span>
            <ListeningPorts bindings={detail.bindings} />
            {detail.source === 'DEVNOOK' ? (
              <p className="field-hint">{t('此进程现已属于托管服务，项目归属由服务配置决定。')}</p>
            ) : (
              <>
                <label htmlFor="assignment-project">{t('所属项目')}</label>
                <Select
                  id="assignment-project"
                  aria-label={t('确认所属项目')}
                  value={target}
                  onValueChange={setTarget}
                  disabled={busy}
                  options={[
                    { value: '', label: t('选择项目…') },
                    ...projects.map((p) => ({ value: p.id, label: p.name })),
                  ]}
                />
                {detail.suggested_project_id && !detail.project_id && (
                  <p className="field-hint">{t('命令中的项目路径仅作为建议，请核对后确认。')}</p>
                )}
                <span className="field-hint">
                  {t('启动时间：')}
                  {detail.start_time} {t('· 本次确认不会套用到以后复用此 PID 的进程。')}
                </span>
              </>
            )}
          </div>
        )}
      </div>
      <div className="modal-actions">
        <span className="modal-hint">
          <ShieldCheck size={13} />
          {t('外部进程保持独立运行')}
        </span>
        <button className="button secondary" disabled={busy} onClick={close}>
          {t('取消')}
        </button>
        <button
          className="button primary"
          disabled={
            busy ||
            loading ||
            !detail ||
            !target ||
            detail.source === 'DEVNOOK' ||
            !detail.bindings.length
          }
          onClick={async () => {
            if (!detail) return;
            setBusy(true);
            setError('');
            try {
              await call('assign_process', {
                pid: detail.pid,
                fingerprint: detail.fingerprint,
                projectId: target,
              });
              await assigned();
              close();
            } catch (e) {
              setDetail(null);
              setError(t('{{0}}。请关闭后重新选择进程。', [String(e)]));
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? t('正在确认…') : t('确认关联')}
        </button>
      </div>
    </Modal>
  );
}
