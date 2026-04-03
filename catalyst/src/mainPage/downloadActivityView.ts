import type { SteamDownloadProgressPayload } from "../shared/ipc/contracts";
import { formatBytes, isFiniteNonNegativeNumber } from "../shared/utils/format";
import type { DownloadEtaSnapshot } from "./stores";

interface DownloadActivityState {
  activeDownloads: SteamDownloadProgressPayload[];
  downloadEtaByKey: Map<string, DownloadEtaSnapshot>;
}

interface DownloadActivityViewOptions {
  downloadActivityElement: HTMLElement;
  downloadActivityCountElement: HTMLElement;
  downloadActivityListElement: HTMLElement;
  state: DownloadActivityState;
  isSteamLinked: () => boolean;
  downloadEtaSmoothingFactor: number;
  downloadEtaSampleMinSeconds: number;
  downloadEtaStaleMs: number;
}

interface DownloadActivityView {
  getDownloadEtaKey: (download: SteamDownloadProgressPayload) => string;
  updateDownloadEtaSnapshots: (downloads: SteamDownloadProgressPayload[]) => void;
  normalizeDownloadPercent: (download: SteamDownloadProgressPayload) => number | null;
  renderDownloadActivity: () => void;
}

export const createDownloadActivityView = ({
  downloadActivityElement,
  downloadActivityCountElement,
  downloadActivityListElement,
  state,
  isSteamLinked,
  downloadEtaSmoothingFactor,
  downloadEtaSampleMinSeconds,
  downloadEtaStaleMs,
}: DownloadActivityViewOptions): DownloadActivityView => {
  const getDownloadEtaKey = (download: SteamDownloadProgressPayload): string => {
    return `${download.provider}:${download.externalId}`;
  };

  const updateDownloadEtaSnapshots = (downloads: SteamDownloadProgressPayload[]): void => {
    const nowMs = Date.now();
    const activeKeys = new Set<string>();

    for (const download of downloads) {
      const key = getDownloadEtaKey(download);
      activeKeys.add(key);

      if (!isFiniteNonNegativeNumber(download.bytesDownloaded)) {
        state.downloadEtaByKey.delete(key);
        continue;
      }

      const currentBytesDownloaded = download.bytesDownloaded;
      const previousSnapshot = state.downloadEtaByKey.get(key);
      if (
        !previousSnapshot
        || currentBytesDownloaded < previousSnapshot.lastBytesDownloaded
        || nowMs <= previousSnapshot.lastSampleAtMs
      ) {
        state.downloadEtaByKey.set(key, {
          lastBytesDownloaded: currentBytesDownloaded,
          lastSampleAtMs: nowMs,
          smoothedBytesPerSecond: previousSnapshot?.smoothedBytesPerSecond ?? 0,
        });
        continue;
      }

      const elapsedSeconds = (nowMs - previousSnapshot.lastSampleAtMs) / 1000;
      let smoothedBytesPerSecond = previousSnapshot.smoothedBytesPerSecond;
      if (elapsedSeconds >= downloadEtaSampleMinSeconds) {
        const deltaBytes = currentBytesDownloaded - previousSnapshot.lastBytesDownloaded;
        if (deltaBytes > 0) {
          const instantaneousBytesPerSecond = deltaBytes / elapsedSeconds;
          if (Number.isFinite(instantaneousBytesPerSecond) && instantaneousBytesPerSecond > 0) {
            smoothedBytesPerSecond = smoothedBytesPerSecond > 0
              ? (
                  smoothedBytesPerSecond * (1 - downloadEtaSmoothingFactor)
                  + instantaneousBytesPerSecond * downloadEtaSmoothingFactor
                )
              : instantaneousBytesPerSecond;
          }
        }
      }

      state.downloadEtaByKey.set(key, {
        lastBytesDownloaded: currentBytesDownloaded,
        lastSampleAtMs: nowMs,
        smoothedBytesPerSecond,
      });
    }

    for (const key of [...state.downloadEtaByKey.keys()]) {
      if (!activeKeys.has(key)) {
        state.downloadEtaByKey.delete(key);
      }
    }
  };

  const normalizeDownloadPercent = (download: SteamDownloadProgressPayload): number | null => {
    if (
      typeof download.progressPercent === "number"
      && Number.isFinite(download.progressPercent)
      && download.progressPercent >= 0
    ) {
      return Math.min(100, Math.max(0, download.progressPercent));
    }

    if (
      typeof download.bytesDownloaded === "number"
      && Number.isFinite(download.bytesDownloaded)
      && download.bytesDownloaded >= 0
      && typeof download.bytesTotal === "number"
      && Number.isFinite(download.bytesTotal)
      && download.bytesTotal > 0
    ) {
      return Math.min(100, Math.max(0, (download.bytesDownloaded / download.bytesTotal) * 100));
    }

    return null;
  };

  const getDownloadTransferRateLabel = (download: SteamDownloadProgressPayload): string | null => {
    if (download.progressSource === "directory-estimate") {
      return null;
    }
    const stateLabel = download.state.trim().toLocaleLowerCase();
    if (!(stateLabel.includes("download") || stateLabel === "updating")) {
      return null;
    }

    const etaSnapshot = state.downloadEtaByKey.get(getDownloadEtaKey(download));
    if (!etaSnapshot || etaSnapshot.smoothedBytesPerSecond <= 0) {
      return null;
    }

    if (Date.now() - etaSnapshot.lastSampleAtMs > downloadEtaStaleMs) {
      return null;
    }

    const speedLabel = formatBytes(etaSnapshot.smoothedBytesPerSecond);
    if (!speedLabel) {
      return null;
    }

    return `${speedLabel}/s`;
  };

  const renderDownloadActivity = (): void => {
    const activeCount = state.activeDownloads.length;
    downloadActivityCountElement.hidden = activeCount <= 0;
    downloadActivityCountElement.textContent = `${activeCount}`;
    downloadActivityElement.setAttribute(
      "aria-label",
      activeCount > 0 ? `${activeCount} active download${activeCount === 1 ? "" : "s"}` : "Downloads"
    );

    downloadActivityListElement.replaceChildren();
    if (activeCount === 0) {
      const emptyMessage = document.createElement("p");
      emptyMessage.className = "download-activity-empty";
      emptyMessage.textContent = isSteamLinked()
        ? "No active downloads"
        : "Connect Steam to view download activity";
      downloadActivityListElement.append(emptyMessage);
      return;
    }

    for (const download of state.activeDownloads) {
      const row = document.createElement("article");
      row.className = "download-activity-item";

      const header = document.createElement("div");
      header.className = "download-activity-item-header";

      const name = document.createElement("p");
      name.className = "download-activity-item-name";
      name.textContent = download.name;

      const stateLabel = document.createElement("p");
      stateLabel.className = "download-activity-item-state";
      stateLabel.textContent = download.state;

      header.append(name, stateLabel);
      row.append(header);

      const normalizedPercent = normalizeDownloadPercent(download);
      if (normalizedPercent !== null) {
        const track = document.createElement("div");
        track.className = "download-activity-progress-track";
        track.setAttribute("role", "progressbar");
        track.setAttribute("aria-valuemin", "0");
        track.setAttribute("aria-valuemax", "100");
        track.setAttribute("aria-valuenow", `${Math.round(normalizedPercent)}`);
        track.setAttribute(
          "aria-label",
          `${download.name}: ${Math.round(normalizedPercent)} percent`
        );

        const fill = document.createElement("div");
        fill.className = "download-activity-progress-fill";
        fill.style.width = `${normalizedPercent}%`;
        track.append(fill);
        row.append(track);
      }

      const meta = document.createElement("p");
      meta.className = "download-activity-item-meta";
      const displayDownloadedBytes = isFiniteNonNegativeNumber(download.bytesDownloaded)
        && isFiniteNonNegativeNumber(download.bytesTotal)
        ? Math.min(download.bytesDownloaded, download.bytesTotal)
        : download.bytesDownloaded;
      const downloadedLabel = formatBytes(displayDownloadedBytes);
      const totalLabel = formatBytes(download.bytesTotal);
      let metadataLabel: string;
      if (downloadedLabel && totalLabel) {
        metadataLabel = normalizedPercent !== null
          ? `${downloadedLabel} / ${totalLabel} (${Math.round(normalizedPercent)}%)`
          : `${downloadedLabel} / ${totalLabel}`;
      } else if (totalLabel) {
        metadataLabel = `Total ${totalLabel}`;
      } else if (normalizedPercent !== null) {
        metadataLabel = `${Math.round(normalizedPercent)}%`;
      } else {
        metadataLabel = download.state;
      }

      const transferRateLabel = getDownloadTransferRateLabel(download);
      if (transferRateLabel) {
        metadataLabel = `${metadataLabel} | ${transferRateLabel}`;
      }
      meta.textContent = metadataLabel;

      row.append(meta);
      downloadActivityListElement.append(row);
    }
  };

  return {
    getDownloadEtaKey,
    updateDownloadEtaSnapshots,
    normalizeDownloadPercent,
    renderDownloadActivity,
  };
};
