import { t } from './i18n';
import { useEffect, useRef, useState } from 'react';
import { Check, Copy, Globe, LoaderCircle, RotateCw } from 'lucide-react';
import { call } from './api';
import Select from './Select';
import { browserHttpChoices, browserShortcut, httpsAlternative } from './BrowserPresentation';
import type { BrowserAddress, BrowserTarget, PortBinding } from './types';

export function useServiceBrowser({
  target,
  bindings,
  available,
  disabled,
  report,
}: {
  target: BrowserTarget;
  bindings: PortBinding[];
  available: boolean;
  disabled: boolean;
  report: (error: unknown) => void;
}) {
  const signature = JSON.stringify([
    target,
    available,
    bindings.map((b) => [b.pid, b.port, b.family, b.address]),
  ]);
  const [reload, setReload] = useState(0);
  const [result, setResult] = useState<{
    signature: string;
    addresses: BrowserAddress[];
    error: string;
  } | null>(null);
  const [selectedUrl, setSelectedUrl] = useState('');
  const [opening, setOpening] = useState(false);
  const openingNow = useRef(false);
  const eligible = available && bindings.length > 0 && !disabled;
  const current = result?.signature === signature ? result : null;
  const addresses = eligible ? current?.addresses || [] : [];
  const http = browserHttpChoices(addresses);
  const selected = http.find((address) => address.url === selectedUrl) || http[0];
  const webUi = selected?.web_ui === true;
  const shortcut = browserShortcut(selected);
  const https = httpsAlternative(addresses, selected);
  const loading = eligible && !current;
  const reason = !available
    ? t('监听状态不可用')
    : !bindings.length
      ? t('尚无实际监听地址，启动服务后可打开')
      : current?.error
        ? t('地址读取失败，请从更多菜单重试')
        : loading
          ? t('正在读取访问地址…')
          : !selected
            ? t('暂无可打开的监听地址')
            : '';

  useEffect(() => {
    let cancelled = false;
    if (!eligible) return;
    setResult(null);
    call<BrowserAddress[]>('service_browser_addresses', { target })
      .then((addresses) => {
        if (!cancelled) setResult({ signature, addresses, error: '' });
      })
      .catch((error) => {
        if (!cancelled) setResult({ signature, addresses: [], error: String(error) });
      });
    return () => {
      cancelled = true;
    };
    // target and listener contents are represented by the stable signature.
  }, [signature, eligible, reload]);

  function refresh() {
    if (!eligible || openingNow.current) return;
    setResult(null);
    setReload((value) => value + 1);
  }
  async function open(address = selected) {
    if (!eligible || !address || openingNow.current || !addresses.includes(address)) return;
    openingNow.current = true;
    setOpening(true);
    try {
      await call('open_service_browser', { target, address });
    } catch (error) {
      report(error);
      setResult(null);
      setReload((value) => value + 1);
    } finally {
      openingNow.current = false;
      setOpening(false);
    }
  }
  return {
    http,
    selected,
    webUi,
    shortcut,
    https,
    eligible,
    loading,
    opening,
    reason,
    error: current?.error,
    refresh,
    select: setSelectedUrl,
    open,
    report,
  };
}

type BrowserState = ReturnType<typeof useServiceBrowser>;

export default function ServiceBrowser({ browser }: { browser: BrowserState }) {
  if (browser.shortcut === 'none') return null;
  return (
    <button
      className={`button ${browser.webUi ? 'primary ' : ''}small service-browser-open`}
      disabled={!browser.selected || browser.loading || browser.opening}
      title={
        browser.error ||
        browser.reason ||
        (browser.webUi ? browser.selected?.url : t('尚未确认网页入口，可尝试在浏览器中打开。'))
      }
      onClick={() => void browser.open()}
    >
      {browser.loading || browser.opening ? (
        <LoaderCircle size={15} className="spin" />
      ) : (
        <Globe size={15} />
      )}
      {t('打开浏览器')}
    </button>
  );
}

export function BrowserAddressField({ browser }: { browser: BrowserState }) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    setCopied(false);
  }, [browser.selected?.url]);
  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(timer);
  }, [copied]);
  if (!browser.selected)
    return (
      <span className="field-hint" role="status" title={browser.error}>
        {browser.reason}
      </span>
    );
  return (
    <div className="service-browser-address">
      {browser.http.length > 1 ? (
        <Select
          aria-label={t('选择浏览器地址')}
          value={browser.selected.url}
          options={browser.http.map((address) => ({ value: address.url, label: address.url }))}
          disabled={browser.opening || browser.loading}
          onValueChange={browser.select}
        />
      ) : (
        <code>{browser.selected.url}</code>
      )}
      <button
        className="icon-button"
        aria-label={copied ? t('地址已复制') : t('复制访问地址')}
        title={copied ? t('地址已复制') : t('复制访问地址')}
        onClick={async () => {
          try {
            if (!navigator.clipboard)
              throw new Error(t('当前窗口无法访问剪贴板，请手动选择地址复制。'));
            await navigator.clipboard.writeText(browser.selected!.url);
            setCopied(true);
          } catch (error) {
            browser.report(error);
          }
        }}
      >
        {copied ? <Check size={14} /> : <Copy size={14} />}
      </button>
    </div>
  );
}

export function BrowserMenuActions({ browser }: { browser: BrowserState }) {
  return (
    <>
      {browser.shortcut === 'none' &&
        browser.http.map((address) => (
          <button
            key={address.url}
            className="text-button"
            disabled={browser.loading || browser.opening}
            title={t('未确认页面入口，仅尝试访问 HTTP 根地址。')}
            onClick={() => {
              browser.select(address.url);
              void browser.open(address);
            }}
          >
            <Globe size={15} />
            {browser.http.length === 1
              ? t('尝试用 HTTP 打开')
              : t('尝试用 HTTP 打开 :{{0}}', [new URL(address.url).port || '80'])}
          </button>
        ))}
      <button
        className="text-button"
        disabled={!browser.https || browser.loading || browser.opening}
        title={t('仅在服务已启用 TLS 时使用；不会自动检测或启用 HTTPS。')}
        onClick={() => {
          if (browser.https) void browser.open(browser.https);
        }}
      >
        <Globe size={15} />
        {t('使用 HTTPS 打开')}
      </button>
      <button
        className="text-button"
        disabled={!browser.eligible || browser.loading || browser.opening}
        onClick={browser.refresh}
      >
        <RotateCw size={15} />
        {t('重新读取地址')}
      </button>
    </>
  );
}
