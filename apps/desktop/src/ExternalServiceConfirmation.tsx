import { t } from './i18n';
import { useEffect, useState } from 'react';
import { Check, LoaderCircle } from 'lucide-react';
import { Modal } from './components';
import type { ExternalServiceHandoffPreview, StopPreview } from './types';

export default function ExternalServiceConfirmation({
  preview,
  close,
  confirm,
}: {
  preview: StopPreview | ExternalServiceHandoffPreview;
  close: () => void;
  confirm: () => Promise<void>;
}) {
  const handoff = 'service' in preview ? preview : null;
  const [pid, setPid] = useState('');
  const [remaining, setRemaining] = useState(preview.expires_in_seconds);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    const timer = setInterval(() => setRemaining((v) => Math.max(0, v - 1)), 1000);
    return () => clearInterval(timer);
  }, []);
  return (
    <Modal
      title={handoff ? t('切换为 DevNook 托管？') : t('停止外部服务？')}
      subtitle={
        handoff
          ? t('可仅保留配置；确认切换后，先停止旧实例，再由 DevNook 创建新的托管实例。')
          : t('重新核验身份后，仅向下面的进程发送正常停止信号，不自动强杀。')
      }
      className="service-confirm-dialog"
      close={() => !busy && close()}
    >
      <div className="modal-body form-stack">
        <div>
          <strong>
            {t('外部实例：')}
            {preview.process.name} · PID {preview.process.pid}
          </strong>
          <div className="service-command">
            <code>{preview.process.command}</code>
            <span>{preview.process.cwd}</span>
          </div>
          <p className="field-hint">
            {t('启动时间：')}
            {preview.process.start_time} {t('· 监听端口：')}
            {[...new Set(preview.process.bindings.map((b) => b.port))].join(', ')}
          </p>
        </div>
        {handoff && (
          <div>
            <strong>
              {t('新托管服务：')}
              {handoff.service.name}
            </strong>
            <div className="service-command">
              <code>{handoff.command}</code>
              <span>{handoff.service.working_directory}</span>
            </div>
            <p className="field-hint">
              {t('预期端口：')}
              {handoff.service.expected_ports.join(', ') || t('未指定')} ·{' '}
              {handoff.service.readiness.kind} {t('就绪检测')}
              <br />
              {t('环境变量名：')}
              {handoff.environment_keys.join(', ') || t('无额外配置')}
              <br />
              {t('依赖服务：')}
              {handoff.dependencies.map((v) => v.definition.name).join('、') || t('无')}
            </p>
          </div>
        )}
        <p className="field-hint">
          {handoff
            ? t(
                '切换会短暂中断服务。旧实例须在 {{0}} 秒内退出且相关端口释放，否则中止。新启动失败时配置仍保留，不自动恢复旧实例。',
                [handoff.service.stop_grace_seconds],
              )
            : t(
                '可能中断正在处理的任务。发送信号后仍需核对实际退出结果；不会停止 Codex、IDEA 或其他外部进程组。',
              )}
        </p>
        {handoff && (
          <p className="field-hint">
            {t(
              '若旧实例由 Codex 或 IDE 维护，请先告知原会话改用 DevNook CLI / MCP，避免重复拉起。切换后仍可由已接入的 AI 助手管理。',
            )}
          </p>
        )}
        {busy ? (
          <p role="status">
            {handoff
              ? t('正在正常停止旧实例并等待退出，随后启动新服务…')
              : t('正在核验身份并发送停止信号…')}
          </p>
        ) : (
          <span className="field-hint">
            {t('确认剩余')}
            {remaining} {t('秒，过期后需重新预览。')}
          </span>
        )}
      </div>
      <div className="external-confirm-identity">
        {handoff ? (
          <span className="field-hint">
            {t('将停止的外部进程：')} <strong>PID {preview.process.pid}</strong>
          </span>
        ) : (
          <label>
            {t('输入 PID {{0}} 以确认', [preview.process.pid])}
            <input
              autoComplete="off"
              inputMode="numeric"
              disabled={busy}
              value={pid}
              onChange={(e) => setPid(e.target.value)}
              placeholder={String(preview.process.pid)}
            />
          </label>
        )}
      </div>
      <div className="modal-actions">
        <button className="button secondary" disabled={busy} onClick={close}>
          {handoff ? t('暂不切换') : t('取消')}
        </button>
        <button
          className={'button ' + (handoff ? 'primary' : 'danger')}
          disabled={busy || !remaining || (!handoff && pid !== String(preview.process.pid))}
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
          {handoff ? t('停止旧实例并以托管方式重启') : t('确认正常停止')}
        </button>
      </div>
    </Modal>
  );
}
