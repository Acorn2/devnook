import { Copy, SquareTerminal } from 'lucide-react';
import { t } from './i18n';
import { serviceAgentInstructions, serviceCliCommands } from './ServicePresentation';

export default function ServiceAiGuide({
  serviceId,
  openIntegrations,
  notify,
  report,
}: {
  serviceId: string;
  openIntegrations: () => void;
  notify: (text: string) => void;
  report: (error: unknown) => void;
}) {
  async function copy(text: string) {
    try {
      if (!navigator.clipboard) throw new Error(t('无法访问剪贴板，请手动选择文字复制。'));
      await navigator.clipboard.writeText(text);
      notify(t('已复制 AI 管理指引'));
    } catch (error) {
      report(error);
    }
  }
  return (
    <details className="service-ai-guide">
      <summary>{t('让 AI 继续管理此服务')}</summary>
      <div className="form-stack">
        <p>
          {t(
            'DevNook 维持服务运行，你和 AI 助手可通过同一份配置操作。关闭 AI 会话或桌面窗口不会自动停止服务。',
          )}
        </p>
        <p className="field-hint">
          {t(
            '先将交接说明发给原 Codex 或 IDE 会话，避免它重新拉起旧实例。DevNook 不会自动通知或配置 AI 客户端。',
          )}
        </p>
        <div className="service-guide-actions">
          <button
            type="button"
            className="button secondary small"
            onClick={() => void copy(serviceAgentInstructions(serviceId))}
          >
            <Copy size={14} />
            {t('复制给 AI 助手')}
          </button>
          <button type="button" className="text-button" onClick={openIntegrations}>
            {t('查看 CLI / MCP 接入')}
          </button>
        </div>
        <pre tabIndex={0} aria-label={t('当前服务的 CLI 命令')}>
          {serviceCliCommands(serviceId)}
        </pre>
        <p className="field-hint">
          <SquareTerminal size={13} />{' '}
          {t(
            'CLI 按本机用户权限执行；未加入 PATH 时使用已确认的绝对路径，源码运行可先执行 pnpm build:cli。启停需确认，非交互 --yes 仅用于已授权操作。',
          )}
        </p>
        <p className="field-hint">
          {t(
            'MCP 默认只读。控制服务需在设置中明确创建范围授权并配置客户端；启动和重启仍使用短期确认。此处不会创建授权或执行命令。',
          )}
        </p>
      </div>
    </details>
  );
}
