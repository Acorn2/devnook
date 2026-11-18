import { useEffect, useMemo, useState } from 'react';
import { call } from './api';
import type { LogPage } from './types';

type LogRequest = { id: string; query: string; limit: number };
type LogResult = { request: LogRequest; page: LogPage | null; error: string };

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
        if (!cancelled) setLatest({ request, page, error: '' });
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
