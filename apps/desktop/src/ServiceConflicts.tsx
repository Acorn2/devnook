import { t } from './i18n';
import { CircleAlert } from 'lucide-react';
import { conflictingBindings, portRows } from './PortPresentation';
import type { PortBinding, Project, ServiceView } from './types';

export default function ServiceConflicts({
  service,
  services,
  ports,
  projects,
  available,
  disabled,
  inspect,
  selectService,
}: {
  service: ServiceView;
  services: ServiceView[];
  ports: PortBinding[];
  projects: Project[];
  available: boolean;
  disabled: boolean;
  inspect: (pid: number) => void;
  selectService: (id: string) => void;
}) {
  const expected = service.definition.expected_ports;
  const bindings = available ? conflictingBindings(ports, expected, service.definition.id) : [];
  const rows = portRows(bindings);
  const blocked =
    service.current_run?.state === 'BLOCKED' &&
    /^(端口 \d+|预期端口)/.test(service.current_run.error || '');
  const reservations = services.filter(
    (v) =>
      blocked &&
      v.definition.id !== service.definition.id &&
      v.current_run &&
      ['STARTING', 'RUNNING', 'READY', 'STOPPING', 'UNKNOWN'].includes(v.current_run.state) &&
      v.definition.expected_ports.some((p) => expected.includes(p)) &&
      !rows.some((p) => p.source === 'DEVNOOK' && p.service_id === v.definition.id),
  );
  if (!rows.length && !blocked) return null;
  return (
    <section className="service-conflicts" aria-label={t('端口冲突详情')}>
      <div className="service-conflicts-heading">
        <strong>
          <CircleAlert size={15} />
          {t('端口占用')}
        </strong>
        <span>{t('查看占用后处理，重试仍需确认')}</span>
      </div>
      {!available ? (
        <p>{t('监听快照不可用，暂无法核对当前占用。上方错误为本次启动记录。')}</p>
      ) : !rows.length && !reservations.length ? (
        <p>{t('当前快照未发现相同端口的占用。上方错误为上次启动记录，重试时会重新检查。')}</p>
      ) : null}
      <div className="service-conflicts-list">
        {rows.map((p) => {
          const managed =
            p.source === 'DEVNOOK' && p.service_id
              ? services.find((v) => v.definition.id === p.service_id)
              : undefined;
          return (
            <article className="service-conflict-row" key={`${p.pid}:${p.port}`}>
              <div className="service-conflict-meta">
                <strong>:{p.port}</strong>
                <span>
                  PID {p.pid} · {p.name}
                </span>
                <span>{p.source === 'DEVNOOK' ? t('DevNook 托管') : t('外部启动')}</span>
                <span>{projects.find((v) => v.id === p.project_id)?.name || t('未关联项目')}</span>
              </div>
              <code>{p.command || t('命令无法读取')}</code>
              <code className="service-conflict-cwd">{p.cwd || t('工作目录无法读取')}</code>
              <div className="service-conflict-actions">
                <button className="text-button" disabled={disabled} onClick={() => inspect(p.pid)}>
                  {t('查看进程…')}
                </button>
                {managed && (
                  <button
                    className="text-button"
                    disabled={disabled}
                    onClick={() => selectService(managed.definition.id)}
                  >
                    {t('查看托管服务：')}
                    {managed.definition.name}
                  </button>
                )}
              </div>
            </article>
          );
        })}
        {reservations.map((v) => (
          <div className="service-conflict-row" key={v.definition.id}>
            <span>
              {t('活动服务“')}
              {v.definition.name}
              {t('”配置了相同预期端口：')}
              {v.definition.expected_ports.filter((p) => expected.includes(p)).join('、')}
              {t('。预期端口不代表已监听。')}
            </span>
            <button
              className="text-button"
              disabled={disabled}
              onClick={() => selectService(v.definition.id)}
            >
              {t('查看托管服务')}
            </button>
          </div>
        ))}
      </div>
      <p className="field-hint">
        {t('停止在服务或进程详情中操作，沿用身份核验与确认；不会自动停止占用者。')}
      </p>
    </section>
  );
}
