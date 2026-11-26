import { useMemo, useState } from 'react';
import { Activity, LoaderCircle, Play, Plus, ShieldCheck, X } from 'lucide-react';
import { call } from './api';
import { Modal } from './components';
import { t } from './i18n';
import Select from './Select';
import { splitSensitiveEnvLines } from './ServicePresentation';
import type { Project, ReadinessKind, ServiceDefinition, ServiceView } from './types';

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export default function ServiceEditor({
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
  const sensitiveEnv = useMemo(() => splitSensitiveEnvLines(env), [env]);
  function moveSensitiveToKeychain() {
    if (!sensitiveEnv.sensitiveEntries.length) return;
    setEnv(sensitiveEnv.plainLines.join('\n'));
    setSecrets((current) => {
      const next = current.filter((row) => row.key.trim() || row.value);
      for (const entry of sensitiveEnv.sensitiveEntries) {
        const existing = next.findIndex((row) => row.key.trim() === entry.key);
        if (existing >= 0) {
          next[existing] = { key: entry.key, value: entry.value };
        } else {
          next.push({ key: entry.key, value: entry.value });
        }
      }
      return next;
    });
  }
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
            {sensitiveEnv.sensitiveEntries.length > 0 && (
              <div className="sensitive-env-notice" role="status">
                <span>
                  {t('检测到 {{0}} 个敏感变量（{{1}}），请移入 Keychain 保存。', [
                    sensitiveEnv.sensitiveEntries.length,
                    sensitiveEnv.sensitiveEntries.map((entry) => entry.key).join(', '),
                  ])}
                </span>
                <button
                  type="button"
                  className="button secondary small"
                  onClick={moveSensitiveToKeychain}
                >
                  <ShieldCheck size={13} />
                  {t('移至 Keychain')}
                </button>
              </div>
            )}
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
                    Boolean(initial.id) && initial.secret_keys.includes(row.key)
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
          {inline ? t('保存并继续…') : t('保存配置')}
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
