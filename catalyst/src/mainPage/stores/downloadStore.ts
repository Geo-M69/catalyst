import type { SteamDownloadProgressPayload } from "../../shared/ipc/contracts";

export interface DownloadEtaSnapshot {
  lastBytesDownloaded: number;
  lastSampleAtMs: number;
  smoothedBytesPerSecond: number;
}

export const downloadStore = {
  downloadPollTimer: null as number | null,
  isDownloadPollInFlight: false,
  activeDownloads: [] as SteamDownloadProgressPayload[],
  previousActiveDownloadsByKey: new Map<string, SteamDownloadProgressPayload>(),
  downloadCompletionRefreshTimer: null as number | null,
  downloadEtaByKey: new Map<string, DownloadEtaSnapshot>(),
};
