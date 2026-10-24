import * as React from 'react';
import { Progress } from '@base-ui/react/progress';
import type { DownloadStatus } from './useModelDownload';

interface ModelProgressProps {
  status: DownloadStatus | null;
  /** Fraction complete in 0–1, monotonic within one download. */
  shown: number;
  onRetry: () => void;
}

/** Download state for the local model, shown under the search bar. */
export function ModelProgress({ status, shown, onRetry }: ModelProgressProps) {
  const value = status !== null && status.totalBytes > 0 ? Math.round(shown * 100) : null;
  const fillStyle = React.useMemo(
    () => (value === null ? undefined : { width: `${value}%` }),
    [value],
  );

  if (status === null || status.phase === 'absent' || status.phase === 'present') {
    return null;
  }

  if (status.phase === 'failed') {
    return (
      <div className="pointer-events-none absolute inset-x-0 top-full mt-4 flex items-center gap-2 px-6">
        <span
          className="pointer-events-auto truncate text-pn-small text-red-text"
          title={status.message ?? undefined}
        >
          {status.message ?? 'The model could not be downloaded.'}
        </span>
        <button
          type="button"
          onClick={onRetry}
          className="pointer-events-auto shrink-0 rounded-pn-xs px-1.5 py-0.5 text-pn-small text-link hover:bg-fill"
        >
          Retry
        </button>
      </div>
    );
  }

  return (
    <Progress.Root
      value={value}
      min={0}
      max={100}
      className="pointer-events-none absolute inset-x-0 top-full mt-4 flex items-center gap-2 px-6"
    >
      <Progress.Label className="shrink-0 text-pn-small text-label-secondary">
        Downloading the model…
      </Progress.Label>
      <Progress.Track className="relative h-1 flex-1 overflow-hidden rounded-full bg-fill-strong">
        <Progress.Indicator
          className={
            value === null
              ? 'pn-progress-indeterminate absolute inset-y-0 left-0 w-[35%] rounded-full bg-accent'
              : 'pn-progress-fill h-full rounded-full bg-accent'
          }
          style={fillStyle}
        />
      </Progress.Track>
      <Progress.Value className="shrink-0 text-pn-small text-label-secondary tabular-nums" />
    </Progress.Root>
  );
}
