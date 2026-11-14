import { t, dateLocale } from './i18n';
import { useCallback, useEffect, useState } from 'react';
import { KeyRound, Plus } from 'lucide-react';
import { call } from './api';
import { CopyButton, Modal } from './components';
import type {
  AccessAudit,
  AgentGrant,
  GrantInput,
  Project,
  ServiceView,
  WorkspaceGroup,
} from './types';

type IntegrationProps = {
  disabled: boolean;
  report: (e: unknown) => void;
  notify: (text: string) => void;
  projects: Project[];
};

export default function AdvancedIntegrations(props: IntegrationProps) {
  const [mcpOpen, setMcpOpen] = useState(false);
  return (
    <div className="advanced-integrations">
      <section aria-labelledby="cli-heading">
        <h3 id="cli-heading">{t('终端命令（CLI）')}</h3>
        <p>
          {t(
            '在终端或脚本中查看服务、读取日志、启动已保存的启动方案。CLI 与桌面共用项目、配置和运行记录。',
          )}
        </p>
        <pre>
          devnook service list{'\n'}
          devnook service logs &lt;service-id&gt; --limit 100{'\n'}
          devnook workspace start &lt;workspace-id&gt; --yes
        </pre>
        <p className="field-hint">
          {t(
            '未加入 PATH 时，将示例中的 devnook 替换为 CLI 可执行文件的绝对路径。源码运行可先执行',
          )}{' '}
          <code>pnpm build:cli</code> {t('构建。')}
        </p>
      </section>
      <details className="mcp-settings" onToggle={(e) => setMcpOpen(e.currentTarget.open)}>
        <summary>{t('AI 助手接入（MCP，可选）')}</summary>
        {mcpOpen && (
          <>
            <p>
              {t(
                '希望 AI 编程助手复用 DevNook 的服务配置、查看日志，并让它启动的服务也能在桌面管理时，再配置此接入。 日常使用桌面和 CLI 无需接入 MCP。',
              )}
            </p>
            <section className="mcp-setup" aria-labelledby="mcp-setup-heading">
              <h4 id="mcp-setup-heading">{t('先连接 AI 客户端')}</h4>
              <p>
                {t(
                  '在支持 MCP 的 AI 工具中添加本地连接：程序填写 CLI 可执行文件的绝对路径，启动参数填写',
                )}{' '}
                <code>mcp serve</code>
                {t('，使用本机 stdio 通信。')}
              </p>
              <p>
                {t(
                  '连接后默认可以查询项目、服务、端口和脱敏日志，无需创建控制授权。客户端配置需手动完成。',
                )}
              </p>
            </section>
            <McpAccess {...props} />
          </>
        )}
      </details>
    </div>
  );
}

function McpAccess({ disabled, report, notify, projects }: IntegrationProps) {
  const [grants, setGrants] = useState<AgentGrant[]>([]);
  const [audit, setAudit] = useState<AccessAudit[]>([]);
  const [services, setServices] = useState<ServiceView[]>([]);
  const [groups, setGroups] = useState<WorkspaceGroup[]>([]);
  const [creating, setCreating] = useState(false);
  const [token, setToken] = useState('');
  const [revoking, setRevoking] = useState<AgentGrant | null>(null);
  const [busy, setBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [loadError, setLoadError] = useState('');
  const refresh = useCallback(async () => {
    if (disabled) return;
    const [grants, audit, services, groups] = await Promise.all([
      call<AgentGrant[]>('agent_grants'),
      call<AccessAudit[]>('access_audits'),
      call<ServiceView[]>('list_services'),
      call<WorkspaceGroup[]>('list_workspaces'),
    ]);
    setGrants(grants);
    setAudit(audit);
    setServices(services);
    setGroups(groups);
    setLoaded(true);
    setLoadError('');
  }, [disabled]);
  useEffect(() => {
    let done = false;
    let inFlight = false;
    const load = async () => {
      if (done || inFlight) return;
      inFlight = true;
      try {
        await refresh();
      } catch (e) {
        if (!done) setLoadError(String(e));
      } finally {
        inFlight = false;
      }
    };
    void load();
    const timer = setInterval(() => void load(), 2500);
    return () => {
      done = true;
      clearInterval(timer);
    };
  }, [refresh]);
  return (
    <section className="mcp-access" aria-label={t('MCP 控制授权与操作记录')}>
      <div className="integration-actions">
        <div>
          <h4>{t('需要 AI 启停服务时，再创建授权')}</h4>
          <p>{t('只允许操作你选定的服务、启动方案或项目，可随时撤销。')}</p>
        </div>
        <button
          type="button"
          className="button secondary"
          disabled={disabled || !loaded || !!loadError}
          onClick={() => setCreating(true)}
        >
          <Plus size={16} />
          {t('新建控制授权')}
        </button>
      </div>
      <p>
        {t('创建后，将令牌填入客户端环境变量')}
        <code>DEVNOOK_MCP_TOKEN</code>
        {t('，并将启动参数改为')} <code>mcp serve --allow-write</code>
        {t('。创建授权不会自动连接客户端或启动服务。')}
      </p>
      {disabled ? (
        <p className="field-hint" role="status">
          {t('后台未连接，连接后可查看和管理授权。')}
        </p>
      ) : loadError ? (
        <p className="error-text" role="alert">
          {t('授权与操作记录读取失败：')}
          {loadError}
        </p>
      ) : !loaded ? (
        <p className="field-hint" role="status">
          {t('正在读取授权与操作记录…')}
        </p>
      ) : null}
      {loaded && (
        <>
          <h4 className="section-title">
            {t('控制授权')}{' '}
            <span className="muted">
              {grants.filter((g) => !g.revoked).length} {t('个未撤销')}
            </span>
          </h4>
          <div className="agent-grants">
            {grants.length ? (
              grants.map((g) => (
                <article key={g.id}>
                  <div>
                    <KeyRound size={18} />
                    <strong>{g.name}</strong>
                    <span className="service-badge">{g.revoked ? t('已撤销') : t('范围授权')}</span>
                  </div>
                  <p>
                    {Object.keys(g.services).length} {t('个服务（含依赖） ·')}{' '}
                    {Object.keys(g.workspaces).length} {t('个启动方案 ·')}
                    {g.project_ids.length} {t('个项目打开权限 ·')}
                    {g.scan_roots.length} {t('个扫描根目录')}
                  </p>
                  <p className="muted">
                    {t('配置修改后需要重新授权。')}
                    {new Date(g.created_at * 1000).toLocaleString(dateLocale())}
                  </p>
                  <button
                    className="button danger small"
                    disabled={g.revoked || busy || disabled}
                    onClick={() => setRevoking(g)}
                  >
                    {t('撤销授权')}
                  </button>
                </article>
              ))
            ) : (
              <p className="field-hint">
                {t('尚未创建控制授权。仅查询状态和日志时，可以跳过这一步。')}
              </p>
            )}
          </div>
          <h4 className="section-title">
            {t('最近操作')}
            <span className="muted">{t('最多 100 条')}</span>
          </h4>
          <div className="agent-audit">
            {audit.length ? (
              <table>
                <thead>
                  <tr>
                    <th>{t('时间')}</th>
                    <th>{t('来源')}</th>
                    <th>{t('操作')}</th>
                    <th>{t('对象')}</th>
                    <th>{t('结果')}</th>
                  </tr>
                </thead>
                <tbody>
                  {audit.map((a, i) => (
                    <tr key={i}>
                      <td>{new Date(a.created_at * 1000).toLocaleTimeString(dateLocale())}</td>
                      <td title={a.client_id}>{a.origin}</td>
                      <td>{a.action}</td>
                      <td title={a.subject}>{a.subject.slice(0, 12) || '—'}</td>
                      <td>{a.result === 'OK' ? t('成功') : t('拒绝 / 失败')}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <p className="muted">{t('尚无操作记录。')}</p>
            )}
          </div>
        </>
      )}
      {creating && (
        <GrantEditor
          services={services}
          groups={groups}
          projects={projects}
          close={() => setCreating(false)}
          saved={async (input) => {
            const result = await call<{ grant: AgentGrant; token: string }>('create_agent_grant', {
              input,
            });
            setCreating(false);
            setToken(result.token);
            await refresh();
          }}
        />
      )}
      {token && (
        <Modal
          title={t('授权令牌只显示这一次')}
          subtitle={t('关闭后无法再次查看。请保存到 MCP 客户端的环境变量中。')}
          close={() => setToken('')}
        >
          <div className="modal-body form-stack">
            <label>
              DEVNOOK_MCP_TOKEN
              <textarea readOnly value={token} spellCheck={false} />
            </label>
            <CopyButton text={token} done={() => notify(t('令牌已复制'))} failed={report} />
            <p className="muted">
              {t('不要将令牌放进源码、命令参数或日志。撤销后，未执行的确认令牌也会失效。')}
            </p>
          </div>
          <footer className="modal-footer">
            <button className="button primary" onClick={() => setToken('')}>
              {t('已保存，关闭')}
            </button>
          </footer>
        </Modal>
      )}
      {revoking && (
        <Modal
          title={t('撤销 AI 助手控制权限')}
          subtitle={t('后续控制请求将被拒绝，已运行的服务继续运行。')}
          close={() => !busy && setRevoking(null)}
        >
          <div className="modal-body">
            {t('撤销「')}
            {revoking.name}
            {t('」的授权？')}
          </div>
          <footer className="modal-footer">
            <button className="button secondary" disabled={busy} onClick={() => setRevoking(null)}>
              {t('取消')}
            </button>
            <button
              className="button danger"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                try {
                  await call('revoke_agent_grant', { id: revoking.id });
                  setRevoking(null);
                  notify(t('授权已撤销'));
                  await refresh();
                } catch (e) {
                  report(e);
                } finally {
                  setBusy(false);
                }
              }}
            >
              {t('撤销授权')}
            </button>
          </footer>
        </Modal>
      )}
    </section>
  );
}
function GrantEditor({
  services,
  groups,
  projects,
  close,
  saved,
}: {
  services: ServiceView[];
  groups: WorkspaceGroup[];
  projects: Project[];
  close: () => void;
  saved: (input: GrantInput) => Promise<void>;
}) {
  const [input, setInput] = useState<GrantInput>({
    name: '',
    service_ids: [],
    workspace_ids: [],
    project_ids: [],
    scan_roots: [],
  });
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const toggle = (
    key: 'service_ids' | 'workspace_ids' | 'project_ids',
    id: string,
    checked: boolean,
  ) =>
    setInput({
      ...input,
      [key]: checked ? [...input[key], id] : input[key].filter((v) => v !== id),
    });
  const expanded = new Set(input.service_ids);
  input.workspace_ids.forEach((id) =>
    groups.find((g) => g.id === id)?.service_ids.forEach((id) => expanded.add(id)),
  );
  const expand = (id: string) => {
    services
      .find((s) => s.definition.id === id)
      ?.definition.dependencies.forEach((dep) => {
        if (!expanded.has(dep)) {
          expanded.add(dep);
          expand(dep);
        }
      });
  };
  [...expanded].forEach(expand);
  return (
    <Modal
      title={t('新建 AI 助手控制授权')}
      subtitle={t('允许启动、重启和正常停止选定服务；启动方案授权会包含全部依赖。')}
      close={() => !busy && close()}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setError('');
          setBusy(true);
          try {
            await saved({
              ...input,
              scan_roots: input.scan_roots.map((root) => root.trim()).filter(Boolean),
            });
          } catch (e) {
            setError(String(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        <div className="modal-body form-stack">
          <label>
            {t('授权名称')}
            <input
              required
              maxLength={80}
              placeholder={t('如：日常开发助手')}
              value={input.name}
              onChange={(e) => setInput({ ...input, name: e.target.value })}
            />
          </label>
          {(
            [
              ['workspace_ids', t('启动方案'), groups.map((g) => ({ id: g.id, name: g.name }))],
              [
                'service_ids',
                t('服务'),
                services.map((s) => ({ id: s.definition.id, name: s.definition.name })),
              ],
              ['project_ids', t('打开项目'), projects.map((p) => ({ id: p.id, name: p.name }))],
            ] as const
          ).map(([key, label, items]) => (
            <fieldset className="dependency-picker" key={key}>
              <legend>{label}</legend>
              {items.map((item) => (
                <label className="checkbox-label" key={item.id}>
                  <input
                    type="checkbox"
                    checked={input[key].includes(item.id)}
                    onChange={(e) => toggle(key, item.id, e.target.checked)}
                  />
                  {item.name}
                </label>
              ))}
              {!items.length && <p className="muted">{t('暂无可选条目。')}</p>}
            </fieldset>
          ))}
          <label>
            {t('允许扫描和导入的根目录（可选，每行一项）')}
            <textarea
              placeholder={t('如：/Users/you/projects')}
              value={input.scan_roots.join('\n')}
              onChange={(e) => setInput({ ...input, scan_roots: e.target.value.split('\n') })}
            />
          </label>
          <div className="service-warning">
            <strong>{t('本次实际服务范围（含依赖）')}</strong>
            <p>
              {[...expanded]
                .map((id) => services.find((s) => s.definition.id === id)?.definition.name || id)
                .join('、') || t('未选择服务')}
            </p>
            <p>
              {t(
                '配置改变后权限自动失效；不提供任意 Shell、配置修改、外部进程终止或强制停止权限。',
              )}
            </p>
          </div>
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
          <button
            className="button primary"
            disabled={
              busy ||
              !(
                expanded.size +
                input.project_ids.length +
                input.scan_roots.filter((root) => root.trim()).length
              )
            }
          >
            {t('确认创建授权')}
          </button>
        </footer>
      </form>
    </Modal>
  );
}
