import { t } from './i18n';
import { useEffect, useRef, useState } from 'react';
import { Globe, LoaderCircle } from 'lucide-react';
import { call } from './api';
import Select from './Select';
import type { BrowserAddress, BrowserTarget, PortBinding } from './types';

export default function ServiceBrowser({
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
  const [opening, setOpening] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const eligible = available && bindings.length > 0 && !disabled;
  const current = result?.signature === signature ? result : null;
  const addresses = current?.addresses || [];
  const http = addresses.filter((address) => address.url.startsWith('http:'));
  const singlePort = new Set(bindings.map((binding) => binding.port)).size === 1;
  const loading = eligible && !current;
  const reason = !available
    ? t('监听状态不可用')
    : !bindings.length
      ? t('尚无实际监听地址，启动服务后可打开')
      : current?.error || (!addresses.length && !loading ? t('暂无可打开的监听地址') : '');

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

  async function open(address: BrowserAddress) {
    if (!eligible || opening) return;
    setOpening(true);
    try {
      await call('open_service_browser', { target, address });
    } catch (error) {
      report(error);
      setReload((value) => value + 1);
    } finally {
      setOpening(false);
    }
  }

  return (
    <div className="service-browser-action" ref={root}>
      <button
        className="button secondary small"
        disabled={!eligible || loading || opening || !http.length}
        title={reason || (singlePort && http.length ? http[0].url : t('选择服务监听地址'))}
        onClick={() => {
          if (singlePort && http.length) void open(http[0]);
          else {
            const trigger = root.current?.querySelector<HTMLButtonElement>('[role="combobox"]');
            trigger?.focus();
            trigger?.click();
          }
        }}
      >
        {loading || opening ? <LoaderCircle size={15} className="spin" /> : <Globe size={15} />}
        {t('打开浏览器')}
      </button>
      <div className="service-browser-choice">
        <Select
          aria-label={t('选择浏览器地址')}
          value=""
          displayLabel={t('地址')}
          placeholder={t('选择地址或 HTTPS')}
          disabled={!eligible || opening || loading}
          options={[
            ...addresses.map((address) => ({ value: address.url, label: address.url })),
            { value: 'REFRESH', label: t('重新读取地址') },
          ]}
          onValueChange={(url) => {
            if (url === 'REFRESH') setReload((value) => value + 1);
            else {
              const address = addresses.find((address) => address.url === url);
              if (address) void open(address);
            }
          }}
        />
      </div>
      {current?.error && (
        <span className="field-hint" role="status">
          {t('地址读取失败，请从地址菜单重试')}
        </span>
      )}
    </div>
  );
}
