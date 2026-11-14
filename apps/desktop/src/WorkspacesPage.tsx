import { t, dateLocale } from './i18n';
import { useCallback, useEffect, useState } from 'react';
import { ArrowDown, ArrowUp, Layers, Pencil, Play, Plus, Square, Trash2 } from 'lucide-react';
import { call } from './api';
import { Modal } from './components';
import { ServiceBadge } from './ServicesPage';
import type { ServiceView, WorkspaceGroup, WorkspacePreview, WorkspaceRun } from './types';

const initial: WorkspaceGroup = {
  id: '',
  name: '',
  description: '',
  service_ids: [],
  revision: '',
  created_at: 0,
  updated_at: 0,
};
const busyState = (state?: string) =>
  ['STARTING', 'STOPPING', 'ROLLING_BACK'].includes(state || '');
const states: Record<string, string> = {
  STARTING: '顺序启动中',
  READY: '编排已完成',
  STOPPING: '停止中',
  ROLLING_BACK: '失败回滚中',
  FAILED: '启动失败 · 已回滚',
  ROLLBACK_FAILED: '停止未完成 · 需检查',
  STOPPED: '已停止',
};
export default function WorkspacesPage({
  disabled,
  report,
  notify,
}: {
  disabled: boolean;
  report: (e: unknown) => void;
  notify: (text: string) => void;
}) {
  const [groups, setGroups] = useState<WorkspaceGroup[]>([]);
  const [services, setServices] = useState<ServiceView[]>([]);
  const [selected, setSelected] = useState('');
  const [history, setHistory] = useState<WorkspaceRun[]>([]);
  const [editing, setEditing] = useState<WorkspaceGroup | null>(null);
  const [preview, setPreview] = useState<WorkspacePreview | null>(null);
  const [removing, setRemoving] = useState(false);
  const [busy, setBusy] = useState(false);
  const refresh = useCallback(async () => {
    if (disabled) return;
    const [groups, services, runs] = await Promise.all([
      call<WorkspaceGroup[]>('list_workspaces'),
      call<ServiceView[]>('list_services'),
      selected ? call<WorkspaceRun[]>('workspace_history', { id: selected }) : Promise.resolve([]),
    ]);
    setGroups(groups);
    setServices(services);
    setHistory(runs);
    setSelected((id) => (groups.some((g) => g.id === id) ? id : groups[0]?.id || ''));
  }, [disabled, selected]);
  useEffect(() => {
    let inFlight = false;
    let done = false;
    const load = async () => {
      if (inFlight || done) return;
      inFlight = true;
      try {
        await refresh();
      } catch (e) {
        if (!done) report(e);
      } finally {
        inFlight = false;
      }
    };
    void load();
    const timer = setInterval(() => void load(), 1200);
    return () => {
      done = true;
      clearInterval(timer);
    };
  }, [refresh, report]);
  const group = groups.find((g) => g.id === selected);
  const current = history[0];
  const active =
    current && (busyState(current.state) || ['READY', 'ROLLBACK_FAILED'].includes(current.state));
  const operate = async (action: string) => {
    setBusy(true);
    try {
      setPreview(
        await call<WorkspacePreview>('prepare_workspace_action', { id: selected, action }),
      );
    } catch (e) {
      report(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className="phase-three-page">
      <div className="services-summary">
        <div>
          <span className="eyebrow">STARTUP PLANS</span>
          <h2>{t('把多个服务组合，一次启动')}</h2>
          <p>{t('依赖先就绪，下游再启动。失败时只回滚本次新增的运行。')}</p>
        </div>
        <button
          className="button primary"
          disabled={disabled}
          onClick={() => setEditing({ ...initial })}
        >
          <Plus size={16} />
          {t('新建启动方案')}
        </button>
      </div>
      {!groups.length ? (
        <div className="phase-empty">
          <Layers size={36} />
          <h3>{t('把项目服务组合起来')}</h3>
          <p>{t('先在“服务列表”中登记服务，再组合成启动方案。')}</p>
        </div>
      ) : (
        <div className="services-workspace">
          <div className="service-list">
            {groups.map((g) => (
              <button
                key={g.id}
                className={'service-row ' + (g.id === selected ? 'selected' : '')}
                onClick={() => {
                  setSelected(g.id);
                  setHistory([]);
                }}
              >
                <div className="service-row-title">
                  <Layers size={17} />
                  <strong>{g.name}</strong>
                </div>
                <span className="service-project">
                  {g.service_ids.length} {t('个入口服务 · 自动展开依赖')}
                </span>
              </button>
            ))}
          </div>
          {group && (
            <article className="service-detail">
              <div className="service-detail-heading">
                <div>
                  <h2>{group.name}</h2>
                  <p>{group.description || t('按依赖顺序管理这组服务')}</p>
                </div>
                <span className="service-badge">
                  {current ? t(states[current.state]) || current.state : t('尚未启动')}
                </span>
              </div>
              <div className="service-actions">
                <button
                  className="button primary"
                  disabled={busy || disabled || !!active}
                  onClick={() => void operate('start')}
                >
                  <Play size={15} />
                  {t('启动方案…')}
                </button>
                <button
                  className="button secondary"
                  disabled={busy || disabled || !active || !!current?.cancel_requested}
                  onClick={() => void operate('stop')}
                >
                  <Square size={15} />
                  {busyState(current?.state) ? t('取消并回滚') : t('停止本次运行')}
                </button>
                <button
                  className="button secondary"
                  disabled={busy || !!active}
                  onClick={() => setEditing({ ...group, service_ids: [...group.service_ids] })}
                >
                  <Pencil size={15} />
                  {t('编辑')}
                </button>
                <button
                  className="button danger"
                  disabled={busy || !!active}
                  onClick={() => setRemoving(true)}
                >
                  <Trash2 size={15} />
                  {t('移除')}
                </button>
              </div>
              {current?.error && (
                <div className="service-warning" role="alert">
                  {t(current.error)}
                </div>
              )}
              {current?.cancel_requested && busyState(current.state) && (
                <p className="muted" role="status">
                  {t('已请求取消，正在停止本次新启动的服务。')}
                </p>
              )}
              <h3>{t('服务与依赖')}</h3>
              <ol className="workspace-chain">
                {(
                  current?.plan ||
                  group.service_ids
                    .map((id) => services.find((s) => s.definition.id === id))
                    .filter((s): s is ServiceView => !!s)
                ).map((saved, index) => {
                  const live = services.find((s) => s.definition.id === saved.definition.id);
                  const reused = current?.reused_service_ids.includes(saved.definition.id);
                  return (
                    <li key={saved.definition.id}>
                      <span className="chain-number">{index + 1}</span>
                      <div>
                        <strong>{saved.definition.name}</strong>
                        <p>
                          {saved.definition.dependencies.length} {t('个依赖 ·')}{' '}
                          {saved.definition.readiness.kind} ·{' '}
                          {reused
                            ? t('复用既有运行，停止时保留')
                            : t('本次新增运行可随启动方案停止')}
                        </p>
                        <code>
                          {saved.definition.executable} {saved.definition.args.join(' ')}
                        </code>
                      </div>
                      {live?.current_run && <ServiceBadge state={live.current_run.state} />}
                    </li>
                  );
                })}
              </ol>
              <h3>
                {t('编排记录')}
                <span className="muted">{history.length}</span>
              </h3>
              <div className="workspace-history">
                {history.length ? (
                  history.map((r) => (
                    <details key={r.id}>
                      <summary>
                        <span>{t(states[r.state]) || r.state}</span>
                        <time>{new Date(r.started_at * 1000).toLocaleString(dateLocale())}</time>
                      </summary>
                      <p>
                        {t('本次新增')}
                        {r.owned_run_ids.length} {t('个运行，复用')}
                        {r.reused_service_ids.length} {t('个服务。')}
                      </p>
                      {r.error && <p className="error-text">{t(r.error)}</p>}
                      <code>{r.id}</code>
                      <ul>
                        {r.plan.map((s) => (
                          <li key={s.definition.id}>
                            {s.definition.name} ·{' '}
                            {r.reused_service_ids.includes(s.definition.id)
                              ? t('复用')
                              : t('计划启动')}
                          </li>
                        ))}
                      </ul>
                    </details>
                  ))
                ) : (
                  <p className="muted">{t('启动后会记录依赖展开、运行归属与回滚结果。')}</p>
                )}
              </div>
            </article>
          )}
        </div>
      )}
      {editing && (
        <WorkspaceEditor
          initial={editing}
          services={services}
          close={() => setEditing(null)}
          saved={async (group) => {
            const saved = await call<WorkspaceGroup>('save_workspace', { group });
            setSelected(saved.id);
            setEditing(null);
            notify(t('启动方案已保存'));
            await refresh();
          }}
        />
      )}
      {preview && (
        <Modal
          title={preview.action === 'start' ? t('确认启动方案') : t('确认停止方案')}
          subtitle={t('确认有效期 60 秒；配置或运行改变后需要重新预览。')}
          close={() => !busy && setPreview(null)}
          wide
        >
          <div className="modal-body">
            <p>
              {preview.action === 'start'
                ? t('按以下顺序启动，等待每个服务 READY。失败时逆序回滚本次新增实例。')
                : t('只停止本次启动方案创建的实例；既有运行和随后手动重启的实例会保留。')}
            </p>
            <ol className="workspace-chain">
              {preview.plan.map((s) => (
                <li key={s.definition.id}>
                  <div>
                    <strong>{s.definition.name}</strong>
                    <p>
                      {preview.action === 'stop'
                        ? s.current_run && preview.owned_run_ids.includes(s.current_run.id)
                          ? t('停止本次创建的运行')
                          : t('保留：既有运行或后续重启')
                        : s.current_run?.state === 'READY'
                          ? t('当前已就绪，可复用')
                          : t('检查后启动或等待就绪')}{' '}
                      · {s.definition.readiness.kind} / {s.definition.readiness.timeout_seconds}s
                    </p>
                    <pre>
                      {s.definition.shell_mode ? '/bin/zsh -lc ' : ''}
                      {JSON.stringify(s.definition.executable)}{' '}
                      {s.definition.args.map((a) => JSON.stringify(a)).join(' ')}
                    </pre>
                    <p>
                      {t('目录：')}
                      {s.definition.working_directory}
                    </p>
                    <p>
                      {t('端口：')}
                      {s.definition.expected_ports.join(', ') || t('未指定')} {t('· 环境变量：')}
                      {[
                        ...Object.keys(s.definition.environment),
                        ...s.definition.secret_keys.map((k) => k + ' [Keychain]'),
                      ].join(', ') || t('无')}
                    </p>
                  </div>
                </li>
              ))}
            </ol>
          </div>
          <footer className="modal-footer">
            <button className="button secondary" disabled={busy} onClick={() => setPreview(null)}>
              {t('取消')}
            </button>
            <button
              className="button primary"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                try {
                  await call('confirm_workspace_action', { token: preview.token });
                  setPreview(null);
                  notify(t('操作已提交，正在执行'));
                  await refresh();
                } catch (e) {
                  report(e);
                } finally {
                  setBusy(false);
                }
              }}
            >
              {t('确认')}
              {preview.action === 'start' ? t('启动') : t('停止')}
            </button>
          </footer>
        </Modal>
      )}
      {removing && group && (
        <Modal
          title={t('移除启动方案配置')}
          subtitle={t('服务配置、运行记录和日志仍会保留。')}
          close={() => !busy && setRemoving(false)}
        >
          <div className="modal-body">
            {t('确认移除「')}
            {group.name}」？
          </div>
          <footer className="modal-footer">
            <button className="button secondary" onClick={() => setRemoving(false)}>
              {t('取消')}
            </button>
            <button
              className="button danger"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                try {
                  await call('remove_workspace', { id: group.id });
                  setRemoving(false);
                  await refresh();
                } catch (e) {
                  report(e);
                } finally {
                  setBusy(false);
                }
              }}
            >
              {t('移除配置')}
            </button>
          </footer>
        </Modal>
      )}
    </section>
  );
}
function WorkspaceEditor({
  initial,
  services,
  close,
  saved,
}: {
  initial: WorkspaceGroup;
  services: ServiceView[];
  close: () => void;
  saved: (group: WorkspaceGroup) => Promise<void>;
}) {
  const [edit, setEdit] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const move = (index: number, delta: number) => {
    const ids = [...edit.service_ids];
    [ids[index], ids[index + delta]] = [ids[index + delta], ids[index]];
    setEdit({ ...edit, service_ids: ids });
  };
  return (
    <Modal
      title={edit.id ? t('编辑启动方案') : t('新建启动方案')}
      subtitle={t('入口顺序用于无依赖服务的排序；依赖始终先启动。')}
      close={() => !busy && close()}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError('');
          try {
            await saved(edit);
          } catch (e) {
            setError(String(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        <div className="modal-body form-stack">
          <label>
            {t('启动方案名称')}
            <input
              required
              maxLength={160}
              value={edit.name}
              onChange={(e) => setEdit({ ...edit, name: e.target.value })}
            />
          </label>
          <label>
            {t('说明')}
            <textarea
              maxLength={2048}
              value={edit.description}
              onChange={(e) => setEdit({ ...edit, description: e.target.value })}
            />
          </label>
          <fieldset className="dependency-picker">
            <legend>{t('选择入口服务')}</legend>
            {services.map(({ definition: s }) => (
              <label className="checkbox-label" key={s.id}>
                <input
                  type="checkbox"
                  checked={edit.service_ids.includes(s.id)}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      service_ids: e.target.checked
                        ? [...edit.service_ids, s.id]
                        : edit.service_ids.filter((id) => id !== s.id),
                    })
                  }
                />
                {s.name}
              </label>
            ))}
            {!services.length && <p>{t('请先在服务管理中添加服务。')}</p>}
          </fieldset>
          <ol className="workspace-order">
            {edit.service_ids.map((id, i) => (
              <li key={id}>
                <span>{services.find((s) => s.definition.id === id)?.definition.name}</span>
                <button
                  type="button"
                  className="icon-button"
                  aria-label={t('上移服务')}
                  disabled={i === 0}
                  onClick={() => move(i, -1)}
                >
                  <ArrowUp size={16} />
                </button>
                <button
                  type="button"
                  className="icon-button"
                  aria-label={t('下移服务')}
                  disabled={i === edit.service_ids.length - 1}
                  onClick={() => move(i, 1)}
                >
                  <ArrowDown size={16} />
                </button>
              </li>
            ))}
          </ol>
          {error && (
            <p className="error-text" role="alert">
              {t(error)}
            </p>
          )}
        </div>
        <footer className="modal-footer">
          <button type="button" className="button secondary" disabled={busy} onClick={close}>
            {t('取消')}
          </button>
          <button className="button primary" disabled={busy || !edit.service_ids.length}>
            {t('保存启动方案')}
          </button>
        </footer>
      </form>
    </Modal>
  );
}
