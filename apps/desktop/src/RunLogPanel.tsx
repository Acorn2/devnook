import { useState, type ReactNode } from 'react';
import { Copy, FileText, Pause, Play, Search, Trash2, X } from 'lucide-react';
import { t } from './i18n';
import ServiceOptions from './ServiceOptions';
import { serviceIsActive } from './ServicePresentation';
import type { ServiceRun } from './types';
import { useRunLog } from './useRunLog';

export default function RunLogPanel({
  runId,
  limitKb,
  disabled,
  picker,
  onClear,
  managedRun,
  notify,
  report,
}: {
  runId?: string;
  limitKb: number;
  disabled: boolean;
  picker?: ReactNode;
  onClear?: (run: ServiceRun, invalidate: () => void) => void;
  managedRun?: ServiceRun | null;
  notify: (text: string) => void;
  report: (e: unknown) => void;
}) {
  const [logQuery, setLogQuery] = useState('');
  const runLog = useRunLog(disabled ? '' : runId || '', logQuery);
  const log = runLog.page;
  async function copyVisibleLog() {
    if (!log?.lines.length) return;
    try {
      if (!navigator.clipboard) throw new Error(t('无法访问剪贴板，请手动选择文字复制。'));
      await navigator.clipboard.writeText(log.lines.join('\n'));
      notify(t('已复制当前显示的日志'));
    } catch (e) {
      report(e);
    }
  }

  return (
    <section className="service-log-section" aria-label={t('运行日志')}>
      <div className="log-toolbar">
        <h3>
          <FileText size={16} />
          {t('运行日志')}
        </h3>
        {picker}
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
        <button
          type="button"
          className="icon-button"
          aria-label={runLog.paused ? t('继续刷新日志') : t('暂停刷新日志')}
          title={runLog.paused ? t('继续刷新日志') : t('暂停刷新日志')}
          disabled={!log}
          onClick={runLog.togglePause}
        >
          {runLog.paused ? <Play size={15} /> : <Pause size={15} />}
        </button>
        <ServiceOptions label={t('更多日志操作')}>
          <button
            type="button"
            className="text-button"
            disabled={!log?.lines.length}
            onClick={copyVisibleLog}
          >
            <Copy size={13} />
            {t('复制当前显示的日志')}
          </button>
          {onClear && (
            <button
              className="text-button"
              disabled={disabled || !runId || !managedRun || serviceIsActive(managedRun.state)}
              onClick={() => managedRun && onClear(managedRun, runLog.invalidate)}
            >
              <Trash2 size={13} />
              {t('清理本次日志…')}
            </button>
          )}
        </ServiceOptions>
      </div>
      <div
        className="service-log"
        role="log"
        aria-label={t('服务输出日志')}
        aria-live="off"
        tabIndex={0}
      >
        {runLog.error ? (
          <span className="log-error">{t(runLog.error)}</span>
        ) : runLog.loading ? (
          <span className="log-empty">{t('正在读取日志…')}</span>
        ) : disabled ? (
          <span className="log-empty">{t('后台连接不可用，暂时无法读取日志。')}</span>
        ) : !runId && !managedRun ? (
          <span className="log-empty">{t('启动后，在这里查看带时间戳的 stdout / stderr。')}</span>
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
          {runLog.paused ? t('日志刷新已暂停') : t('每秒更新 · 最多显示最近 500 行')}
        </span>
        <span>
          {log?.truncated || managedRun?.log_truncated
            ? t('日志已达到限制或仅显示尾部')
            : t('每次运行最多 ') + limitKb + ' KiB'}
        </span>
      </div>
    </section>
  );
}
