import * as React from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import Markdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Icon } from './components/ui/icons/Icon';
import type { IconName } from './components/ui/icons/paths';
import type { ResearchAction, ResearchActivity, ResearchPhase, ResearchState, Researcher } from './useResearch';

const REMARK_PLUGINS = [remarkGfm];

function handleLinkClick(e: React.MouseEvent<HTMLAnchorElement>) {
  e.preventDefault();
  const href = e.currentTarget.getAttribute('href');
  if (href && /^https?:\/\//i.test(href)) {
    openUrl(href).catch(() => undefined);
  }
}

function handleLinkAuxClick(e: React.MouseEvent<HTMLAnchorElement>) {
  e.preventDefault();
  const href = e.currentTarget.getAttribute('href');
  if (e.button === 1 && href && /^https?:\/\//i.test(href)) {
    openUrl(href).catch(() => undefined);
  }
}

function ExternalLink({
  href,
  children,
  ...props
}: React.AnchorHTMLAttributes<HTMLAnchorElement>) {
  return (
    <a {...props} href={href} onClick={handleLinkClick} onAuxClick={handleLinkAuxClick}>
      {children}
    </a>
  );
}

const MARKDOWN_COMPONENTS: Components = {
  a: ExternalLink,
};

const HEADER: Record<ResearchPhase, string> = {
  idle: 'Research',
  running: 'Research in progress',
  done: 'Research report',
  failed: 'Research failed',
};

const panelStyle: React.CSSProperties = {
  background: 'var(--menu-bg)',
  backdropFilter: 'blur(var(--blur-material)) saturate(var(--material-saturation))',
  WebkitBackdropFilter: 'blur(var(--blur-material)) saturate(var(--material-saturation))',
  boxShadow: '0 4px 16px rgba(0,0,0,0.28), 0 1px 3px rgba(0,0,0,0.12)',
};

const WHO: Record<Researcher, string> = { lead: 'Lead', subagent: 'Subagent' };

const ACTION: Record<ResearchAction, string> = {
  searching: 'searching',
  reading: 'reading',
  delegating: 'delegating',
};

const MARKDOWN_CLASSES = `text-pn-body text-label leading-relaxed
  [&_h1]:text-[20px] [&_h1]:font-semibold [&_h1]:mt-0 [&_h1]:mb-3
  [&_h2]:text-[16px] [&_h2]:font-semibold [&_h2]:mt-6 [&_h2]:mb-2
  [&_h3]:text-[15px] [&_h3]:font-semibold [&_h3]:mt-5 [&_h3]:mb-2
  [&_p]:my-2 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5
  [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_li]:my-1
  [&_a]:text-link [&_a]:underline [&_strong]:font-semibold
  [&_code]:rounded [&_code]:bg-fill [&_code]:px-1 [&_code]:font-mono [&_code]:text-pn-small
  [&_pre]:my-3 [&_pre]:overflow-x-auto [&_pre]:rounded [&_pre]:bg-fill [&_pre]:p-3
  [&_blockquote]:border-l-2 [&_blockquote]:border-separator [&_blockquote]:pl-3
  [&_blockquote]:text-label-secondary [&_hr]:my-4 [&_hr]:border-separator
  [&_table]:my-3 [&_th]:text-left [&_th]:font-semibold [&_td]:align-top [&_td]:pr-4`;

function Tally({ icon, value, label }: { icon: IconName; value: number; label: string }) {
  return (
    <span className="flex items-center gap-1.5 text-pn-small text-label-secondary">
      <Icon name={icon} size={13} />
      <span className="tabular-nums text-label">{value}</span>
      <span>{label}</span>
    </span>
  );
}

/** What the agents are doing right now; the report itself stays hidden until it lands. */
function Working({ activity }: { activity: ResearchActivity | null }) {
  return (
    <div className="flex flex-col gap-2">
      <p className="flex items-center gap-2 text-pn-body text-label">
        <Icon name="ellipsis" size={16} className="shrink-0 text-accent" />
        <span className="truncate">
          {activity === null
            ? 'Deciding what to look for'
            : `${WHO[activity.by]} ${ACTION[activity.action]} ${activity.detail}`}
        </span>
      </p>
      <p className="text-pn-small text-label-secondary">
        The report appears when the lead agent finishes.
      </p>
    </div>
  );
}

/** The expanded research view: live counts, current activity, then the report. */
export function ResearchPanel({ state, onReset }: { state: ResearchState; onReset: () => void }) {
  const { activity } = state;

  return (
    <div className="flex h-screen w-screen items-stretch justify-center bg-transparent p-6">
      <div className="flex w-full flex-col overflow-hidden rounded-xl" style={panelStyle}>
        <header
          className="flex shrink-0 items-center gap-3 border-b border-separator px-5 py-3.5"
          aria-label={HEADER[state.phase]}
        >
          <Icon name="search" size={16} className="shrink-0 text-label-secondary" />
          <span className="truncate text-pn-body text-label">{state.query}</span>
          <span className="ml-auto flex shrink-0 items-center gap-4">
            <Tally icon="search" value={activity?.searches ?? 0} label="searches" />
            <Tally icon="globe" value={activity?.reads ?? 0} label="pages" />
            <Tally icon="person" value={activity?.subagents ?? 0} label="subagents" />
          </span>
          <button
            type="button"
            onClick={onReset}
            disabled={state.phase === 'running'}
            aria-label="Reset search"
            className="flex size-[22px] shrink-0 items-center justify-center rounded-pn-sm text-label-secondary enabled:hover:bg-fill enabled:hover:text-label enabled:active:bg-fill-strong focus-visible:outline-hidden focus-visible:shadow-pn-focus-ring disabled:opacity-(--opacity-disabled)"
          >
            <Icon name="xmark" size={14} />
          </button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
          {state.phase === 'running' && <Working activity={activity} />}

          {state.phase === 'failed' && (
            <p className="flex items-start gap-2 text-pn-body text-red-text">
              <Icon name="warning" size={16} className="mt-0.5 shrink-0" />
              <span>{state.error}</span>
            </p>
          )}

          {state.phase === 'done' && (
            <article className={MARKDOWN_CLASSES}>
              <Markdown remarkPlugins={REMARK_PLUGINS} components={MARKDOWN_COMPONENTS}>
                {state.report}
              </Markdown>
            </article>
          )}
        </div>
      </div>
    </div>
  );
}
