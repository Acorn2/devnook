import { useEffect, useState } from 'react';
import { Check, LoaderCircle } from 'lucide-react';
import { Modal } from './components';
import { t } from './i18n';
import type { ServiceActionPreview } from './types';

export default function ActionConfirmation({
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
