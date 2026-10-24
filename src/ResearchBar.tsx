import * as React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Icon } from './components/ui/icons/Icon';
import { TextField } from './components/ui/TextField';

const pillStyle: React.CSSProperties = {
  height: '64px',
  background: 'var(--menu-bg)',
  backdropFilter: 'blur(var(--blur-material)) saturate(var(--material-saturation))',
  WebkitBackdropFilter: 'blur(var(--blur-material)) saturate(var(--material-saturation))',
  boxShadow: '0 4px 24px rgba(0,0,0,0.28), 0 1px 3px rgba(0,0,0,0.12)',
};

/**
 * Floating research command bar rendered inside the undecorated Tauri window.
 * A translucent pill with a search icon and large input, using the Pane TextField.
 */
export function ResearchBar() {
  const inputRef = React.useRef<HTMLInputElement>(null);
  const [value, setValue] = React.useState('');

  const handleChange = React.useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => setValue(e.target.value),
    [],
  );

  // Focus input when the window is shown via global shortcut
  React.useEffect(() => {
    const unlisten = listen('window-open', () => {
      setValue('');
      requestAnimationFrame(() => {
        inputRef.current?.focus();
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Hide on Escape
  React.useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        invoke('hide_window');
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  // Auto-focus on mount
  React.useEffect(() => {
    inputRef.current?.focus();
  }, []);

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-transparent select-none px-10 py-14">
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
          placeholder="What would you like to research?"
          spellCheck={false}
          autoComplete="off"
          aria-label="Research query"
          className="flex-1 min-w-0"
        />
      </div>
    </div>
  );
}
