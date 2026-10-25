import * as React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type ResearchPhase = 'idle' | 'running' | 'done' | 'failed';
export type Researcher = 'lead' | 'subagent';
export type ResearchAction = 'searching' | 'reading' | 'delegating';

/** One line of live progress, carrying the running totals. */
export interface ResearchActivity {
  by: Researcher;
  action: ResearchAction;
  detail: string;
  searches: number;
  reads: number;
  subagents: number;
}

export interface ResearchState {
  phase: ResearchPhase;
  query: string;
  activity: ResearchActivity | null;
  report: string;
  error: string;
}

const ACTIVITY_EVENT = 'research://activity';

const IDLE: ResearchState = {
  phase: 'idle',
  query: '',
  activity: null,
  report: '',
  error: '',
};

/** The one subscription to a research run: counts while it works, the report
 * only once the agent returns it. */
export function useResearch(): {
  state: ResearchState;
  start: (query: string) => void;
  reset: () => void;
} {
  const [state, setState] = React.useState<ResearchState>(IDLE);
  const runId = React.useRef(0);

  React.useEffect(() => {
    let active = true;
    const unlisten = listen<ResearchActivity>(ACTIVITY_EVENT, (event) => {
      if (active) {
        // Kept after the run settles, so the last tool call's counts still show
        // beside the report. Nothing arrives while idle: no run is going.
        setState((current) =>
          current.phase === 'idle' ? current : { ...current, activity: event.payload },
        );
      }
    });
    return () => {
      active = false;
      unlisten
        .then((dispose) => dispose())
        .catch(() => undefined);
    };
  }, []);

  // A superseded run cannot write over the one that replaced it.
  const start = React.useCallback((query: string) => {
    runId.current += 1;
    const id = runId.current;
    setState({ phase: 'running', query, activity: null, report: '', error: '' });
    invoke<string>('research_ask', { prompt: query })
      .then((report) => {
        if (runId.current === id) {
          setState((current) => ({ ...current, phase: 'done', report }));
        }
      })
      .catch((error: unknown) => {
        if (runId.current === id) {
          setState((current) => ({ ...current, phase: 'failed', error: String(error) }));
        }
      });
  }, []);

  const reset = React.useCallback(() => {
    runId.current += 1;
    setState(IDLE);
  }, []);

  return { state, start, reset };
}
