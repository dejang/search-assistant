import * as React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type DownloadPhase = 'absent' | 'downloading' | 'present' | 'failed';

export interface DownloadStatus {
  phase: DownloadPhase;
  downloadedBytes: number;
  totalBytes: number;
  message: string | null;
}

/** Event carrying every download state change. */
const DOWNLOAD_EVENT = 'model://download';

/** Fraction complete in 0–1; zero while the total size is still unknown. */
function fractionOf(status: DownloadStatus): number {
  if (status.totalBytes <= 0) {
    return 0;
  }
  return Math.min(1, status.downloadedBytes / status.totalBytes);
}

/** The one subscription to the model downloader, shared by every view that
 * cares whether the model is on disk, so they cannot disagree. */
export function useModelDownload(): {
  status: DownloadStatus | null;
  shown: number;
  retry: () => void;
} {
  const [status, setStatus] = React.useState<DownloadStatus | null>(null);
  const [shown, setShown] = React.useState(0);

  // Progress never moves backwards within one download, and resets after it.
  const apply = React.useCallback((next: DownloadStatus) => {
    setStatus(next);
    setShown((previous) =>
      next.phase === 'downloading' ? Math.max(previous, fractionOf(next)) : 0,
    );
  }, []);

  const retry = React.useCallback(() => {
    invoke('model_download').catch(() => undefined);
  }, []);

  // The listener registers first, and the snapshot is dropped if an event
  // arrived while it was in flight, so a stale snapshot cannot revive a bar.
  React.useEffect(() => {
    let active = true;
    let eventsSeen = 0;
    const unlisten = listen<DownloadStatus>(DOWNLOAD_EVENT, (event) => {
      eventsSeen += 1;
      if (active) {
        apply(event.payload);
      }
    });
    unlisten
      .then(() => {
        const seen = eventsSeen;
        return invoke<DownloadStatus>('model_download_status').then((snapshot) => {
          if (active && eventsSeen === seen) {
            apply(snapshot);
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
  }, [apply]);

  return { status, shown, retry };
}
