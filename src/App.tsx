import { ResearchBar } from './ResearchBar';
import { ResearchPanel } from './ResearchPanel';
import { useResearch } from './useResearch';
import * as React from 'react';
import { invoke } from '@tauri-apps/api/core';

export default function App() {
  const { state, start, reset } = useResearch();
  const expanded = state.phase !== 'idle';

  const collapse = React.useCallback(() => {
    reset();
    invoke('set_research_view', { expanded: false }).catch(() => undefined);
  }, [reset]);

  const submit = React.useCallback(
    (query: string) => {
      invoke('set_research_view', { expanded: true }).catch(() => undefined);
      start(query);
    },
    [start],
  );

  React.useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') {
        return;
      }
      // A run in flight cannot be cancelled, so Escape only hides the window;
      // collapsing would put a new question up against the slot it still holds.
      if (expanded && state.phase !== 'running') {
        collapse();
      } else {
        invoke('hide_window').catch(() => undefined);
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [expanded, state.phase, collapse]);

  return expanded ? (
    <ResearchPanel state={state} onReset={collapse} />
  ) : (
    <ResearchBar onSubmit={submit} />
  );
}
