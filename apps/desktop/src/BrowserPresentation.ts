import type { BrowserAddress } from './types';

// Display only Core-provided choices. Prefer IPv4 and one HTTP choice per port;
// these URLs are not evidence that HTTP/TLS is enabled on the TCP listener.
export function preferredHttpAddresses(addresses: BrowserAddress[]) {
  const ports = new Set<string>();
  return addresses
    .filter((address) => address.url.startsWith('http://'))
    .sort((a, b) => Number(a.url.includes('[')) - Number(b.url.includes('[')))
    .filter((address) => {
      try {
        const port = new URL(address.url).port || '80';
        if (ports.has(port)) return false;
        ports.add(port);
        return true;
      } catch {
        return false;
      }
    });
}

export function httpsAlternative(addresses: BrowserAddress[], selected?: BrowserAddress) {
  if (!selected) return undefined;
  try {
    const http = new URL(selected.url);
    return addresses.find((address) => {
      if (address.pid !== selected.pid || address.fingerprint !== selected.fingerprint)
        return false;
      try {
        const https = new URL(address.url);
        return (
          https.protocol === 'https:' &&
          https.hostname === http.hostname &&
          (https.port || '443') === (http.port || '80') &&
          https.pathname === http.pathname &&
          https.search === http.search
        );
      } catch {
        return false;
      }
    });
  } catch {
    return undefined;
  }
}

// Prefer confirmed page endpoints, then unconfirmed endpoints. Only a confirmed
// non-page response hides the normal shortcut; missing evidence never does.
export function browserHttpChoices(addresses: BrowserAddress[]) {
  const pages = preferredHttpAddresses(addresses.filter((address) => address.web_ui === true));
  if (pages.length) return pages;
  const unknown = preferredHttpAddresses(addresses.filter((address) => address.web_ui !== false));
  return unknown.length ? unknown : preferredHttpAddresses(addresses);
}

export function browserShortcut(address?: BrowserAddress) {
  if (!address || address.web_ui === false) return 'none';
  return address.web_ui === true ? 'page' : 'try';
}
