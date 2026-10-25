import * as React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Icon } from './components/ui/icons/Icon';
import { ModelProgress } from './ModelProgress';
import { useModelDownload } from './useModelDownload';
import { TextField } from './components/ui/TextField';

type EngineStatus = 'online' | 'offline';

const pillStyle: React.CSSProperties = {
  height: '64px',
  background: 'var(--menu-bg)',
  backdropFilter: 'blur(var(--blur-material)) saturate(var(--material-saturation))',
  WebkitBackdropFilter: 'blur(var(--blur-material)) saturate(var(--material-saturation))',
  boxShadow: '0 4px 24px rgba(0,0,0,0.28), 0 1px 3px rgba(0,0,0,0.12)',
};

interface ResearchBarProps {
  /** Hands the trimmed query to the research run. */
  onSubmit: (query: string) => void;
}

/** Floating research command bar: translucent pill with search icon + TextField. */
export function ResearchBar({ onSubmit }: ResearchBarProps) {
  const inputRef = React.useRef<HTMLInputElement>(null);
  const [value, setValue] = React.useState('');
  const [engineStatus, setEngineStatus] = React.useState<EngineStatus | null>(null);
  const { status, shown, retry } = useModelDownload();

  const downloading = status?.phase === 'downloading';
  const disabled = downloading || engineStatus !== 'online';
  const showEngineError =
    engineStatus === 'offline' && !downloading && status?.phase !== 'failed';

  const handleChange = React.useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => setValue(e.target.value),
    [],
  );

  const submit = React.useCallback(() => {
    const query = value.trim();
    if (query !== '') {
      onSubmit(query);
    }
  }, [value, onSubmit]);

  const handleKeyDown = React.useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      // Enter that commits an IME candidate is not a submit.
      if (e.key === 'Enter' && !e.nativeEvent.isComposing) {
        e.preventDefault();
        submit();
      }
    },
    [submit],
  );

  // Subscribe to engine status changes and read initial snapshot.
  React.useEffect(() => {
    let active = true;
    let eventsSeen = 0;
    const unlisten = listen<EngineStatus>('engine://status', (event) => {
      eventsSeen += 1;
      if (active) {
        setEngineStatus(event.payload);
      }
    });
    unlisten
      .then(() => {
        const seen = eventsSeen;
        return invoke<EngineStatus | null>('engine_status').then((snapshot) => {
          if (active && eventsSeen === seen) {
            setEngineStatus(snapshot);
          }
        });
      })
      .catch(() => undefined);
    return () => {
      active = false;
      unlisten
        .then((dispose) => dispose())
        .catch(() => undefined);
    };
  }, []);

  // Focus input when the window is shown via global shortcut
  React.useEffect(() => {
    const unlisten = listen('window-open', () => {
      setValue('');
      requestAnimationFrame(() => {
        inputRef.current?.focus();
      });
    });
    return () => {
      unlisten
        .then((dispose) => dispose())
        .catch(() => undefined);
    };
  }, []);

  // A disabled field cannot take focus, so focus when the engine comes online.
  React.useEffect(() => {
    if (!disabled) {
      inputRef.current?.focus();
    }
  }, [disabled]);

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-transparent px-10 py-14">
      {/* Positions the model progress row against the pill's bottom edge */}
      <div className="relative w-full">
        {/* The pill — shadow applied directly here, sized to fit within window padding */}
        <div
          className="flex w-full items-center gap-3 rounded-full px-5
          [&_input]:bg-transparent!
          [&_input]:shadow-none!
          [&_input]:focus:shadow-none!
          [&_input]:border-0!
          [&_input]:outline-none!
          [&_input]:p-0!
          [&_input]:h-auto!
          [&_input]:rounded-none!
          [&_input]:text-[22px]
          [&_input]:leading-7!
          [&_input]:font-normal
          [&_input]:text-label
          [&_input]:placeholder:text-label-secondary
        "
          style={pillStyle}
        >
          {/* Search icon */}
          <Icon
            name="search"
            size={22}
            className="shrink-0 text-label-secondary"
          />

          {/* TextField — styled to blend into the pill via parent descendant overrides */}
          <TextField
            ref={inputRef}
            value={value}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            disabled={disabled}
            placeholder="What would you like to research?"
            spellCheck={false}
            autoComplete="off"
            aria-label={
              disabled ? 'Research query, unavailable while the engine is offline' : 'Research query'
            }
            className="flex-1 min-w-0"
          />
        </div>

        <ModelProgress status={status} shown={shown} onRetry={retry} />

        {showEngineError && (
          <div className="pointer-events-none absolute inset-x-0 top-full mt-4 flex items-center gap-2 px-6">
            <span className="pointer-events-auto truncate text-pn-small text-red-text">
              no agent can be used at this point
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
