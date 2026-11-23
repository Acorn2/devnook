import { useEffect, useMemo, useState } from 'react';
import { call } from './api';
import type { LogPage } from './types';

type LogRequest = { id: string; query: string; limit: number };
type LogResult = { request: LogRequest; page: LogPage | null; error: string };

function arePagesEqual(a: LogPage | null, b: LogPage | null): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  if (a.run_id !== b.run_id || a.truncated !== b.truncated || a.lines.length !== b.lines.length) {
    return false;
  }
  if (a.lines.length === 0) return true;
  if (a.lines[0] !== b.lines[0] || a.lines[a.lines.length - 1] !== b.lines[b.lines.length - 1]) {
    return false;
  }
  for (let i = 0; i < a.lines.length; i++) {
    if (a.lines[i] !== b.lines[i]) return false;
  }
  return true;
}

export function useRunLog(runId: string, query: string) {
  const [revision, setRevision] = useState(0);
  const request = useMemo(() => ({ id: runId, query, limit: 500 }), [runId, query, revision]);
  const [latest, setLatest] = useState<LogResult | null>(null);
  const [frozen, setFrozen] = useState<LogResult | null>(null);
  const paused = frozen?.request === request;
  const result = paused ? frozen : latest?.request === request ? latest : null;

  useEffect(() => {
    if (!request.id || paused) return;
    let cancelled = false;
    let inFlight = false;
    const load = async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        const page = await call<LogPage>('service_logs', request);
        if (!cancelled) {
          setLatest((prev) => {
            if (prev && prev.request === request && !prev.error && arePagesEqual(prev.page, page)) {
              return prev;
            }
            return { request, page, error: '' };
          });
        }
      } catch (e) {
        if (!cancelled)
          setLatest({ request, page: null, error: e instanceof Error ? e.message : String(e) });
      } finally {
        inFlight = false;
      }
    };
    void load();
    const timer = setInterval(load, 1000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [request, paused]);

  return {
    page: result?.page,
    error: result?.error,
    loading: !!runId && !result,
    paused,
    togglePause: () => setFrozen(paused ? null : result),
    invalidate: () => setRevision((value) => value + 1),
  };
}
