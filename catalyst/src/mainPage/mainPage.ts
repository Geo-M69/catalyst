import { type CollectionGridItem, renderCollectionGrid } from "./components/collectionGrid";
import { createConfirmationDialog } from "./components/confirmationDialog";
import { createFilterPanel } from "./components/filterPanel";
import { createCollectionNameDialog } from "./components/collectionNameDialog";
import { createGameContextMenu } from "./components/gameContextMenu";
import { type GameGridSection, renderGameGrid } from "./components/gameGrid";
import { createInstallDialog } from "./components/installDialog";
import {
  type GameCompatibilityToolOption,
  createGamePropertiesPanel,
  type GameBetaAccessCodeValidationResult,
  type GamePropertiesPersistedSettings,
  type GamePrivacySettings,
} from "./components/gamePropertiesPanel";
import { applyLibraryFilters } from "./filtering";
import {
  HIDDEN_GAMES_COLLECTION_NAME,
  type CollectionResponse,
  type GameResponse,
} from "./types";
import { ipcService } from "../shared/ipc/client";
import { normalizeAppError } from "../shared/ipc/errors";
import type {
  GameCustomizationArtworkPayload,
  GameInstallLocationPayload,
  GameInstallationDetailsPayload,
  GamePrivacySettingsPayload,
  GameVersionBetasPayload,
  SteamDownloadProgressPayload,
} from "../shared/ipc/contracts";

export {};

const escapeHtml = (unsafe: string): string => {
  return String(unsafe)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
};

const sessionAccountElement = document.getElementById("session-account");
const sessionAccountButton = document.getElementById("session-account-button");
const sessionAccountLabelElement = document.getElementById("session-account-label");
const sessionAccountMenuElement = document.getElementById("session-account-menu");
const sessionAccountManageButton = document.getElementById("session-account-manage");
const sessionAccountSignOutButton = document.getElementById("session-account-signout");
const libraryViewPickerElement = document.getElementById("library-view-picker");
const librarySummaryElement = document.getElementById("library-summary");
const libraryLastUpdatedElement = document.getElementById("library-last-updated");
const refreshLibraryButton = document.getElementById("refresh-library-button");
const refreshLibraryLabelElement = document.getElementById("refresh-library-label");
const downloadActivityElement = document.getElementById("download-activity");
const downloadActivityCountElement = document.getElementById("download-activity-count");
const downloadActivityListElement = document.getElementById("download-activity-list");
const filterPanelElement = document.getElementById("filter-panel");
const libraryGridElement = document.getElementById("library-grid");
const libraryAspectShellElement = document.getElementById("library-aspect-shell");
const panelLeftElement = document.querySelector<HTMLElement>(".panel-left");
const panelMiddleElement = document.querySelector<HTMLElement>(".panel-middle");
const gameDetailsShellElement = document.getElementById("game-details-shell");
const gameDetailsBackButton = document.getElementById("game-details-back-button");
const appTopHover = document.getElementById("app-top-hover");
const gameDetailsContentElement = document.getElementById("game-details-content");
const detailsHeroBg = document.getElementById("details-hero-bg");
const detailsTitleInfo = document.getElementById("details-title-info");
const detailsPlayButton = document.getElementById("details-play-button");
const detailsSettingsButton = document.getElementById("details-settings-button");
const detailsFavoriteButton = document.getElementById("details-favorite-button");
const detailsPropertiesButton = document.getElementById("details-properties-button");
const detailsDropdown = document.getElementById("details-dropdown");

if (
  !(sessionAccountElement instanceof HTMLElement)
  || !(sessionAccountButton instanceof HTMLButtonElement)
  || !(sessionAccountLabelElement instanceof HTMLElement)
  || !(sessionAccountMenuElement instanceof HTMLElement)
  || !(sessionAccountManageButton instanceof HTMLButtonElement)
  || !(sessionAccountSignOutButton instanceof HTMLButtonElement)
  || !(libraryViewPickerElement instanceof HTMLElement)
  || !(librarySummaryElement instanceof HTMLElement)
  || !(libraryLastUpdatedElement instanceof HTMLElement)
  || !(refreshLibraryButton instanceof HTMLButtonElement)
  || !(refreshLibraryLabelElement instanceof HTMLElement)
  || !(downloadActivityElement instanceof HTMLElement)
  || !(downloadActivityCountElement instanceof HTMLElement)
  || !(downloadActivityListElement instanceof HTMLElement)
  || !(filterPanelElement instanceof HTMLElement)
  || !(libraryGridElement instanceof HTMLElement)
  || !(libraryAspectShellElement instanceof HTMLElement)
  || !(panelLeftElement instanceof HTMLElement)
  || !(panelMiddleElement instanceof HTMLElement)
  || !(gameDetailsShellElement instanceof HTMLElement)
  || !(gameDetailsBackButton instanceof HTMLButtonElement)
  || !(appTopHover instanceof HTMLElement)
  || !(gameDetailsContentElement instanceof HTMLElement)
  || !(detailsHeroBg instanceof HTMLElement)
  || !(detailsTitleInfo instanceof HTMLElement)
  || !(detailsPlayButton instanceof HTMLButtonElement)
  || !(detailsSettingsButton instanceof HTMLButtonElement)
  || !(detailsFavoriteButton instanceof HTMLButtonElement)
  || !(detailsPropertiesButton instanceof HTMLButtonElement)
  || !(detailsDropdown instanceof HTMLElement)
) {
  throw new Error("Main page is missing required DOM elements");
}

// Back button visibility helper: show while hovering the top hotspot or the back button itself
{
  let hideTimer: number | null = null;
  const show = (): void => {
    hideTimer && window.clearTimeout(hideTimer);
    document.body.classList.add("show-back-button");
  };
  const hide = (): void => {
    hideTimer && window.clearTimeout(hideTimer);
    hideTimer = window.setTimeout(() => document.body.classList.remove("show-back-button"), 350);
  };

  appTopHover.addEventListener("mouseenter", show);
  appTopHover.addEventListener("mouseleave", hide);
  gameDetailsBackButton.addEventListener("mouseenter", show);
  gameDetailsBackButton.addEventListener("mouseleave", hide);
  gameDetailsBackButton.addEventListener("focus", show);
  gameDetailsBackButton.addEventListener("blur", hide);
}

import { store, isLibraryViewMode, isCollectionLibraryViewMode, isGameLibraryViewMode, type LibraryViewMode, findGameById } from "./libraryStore";
import { getSteamArtworkCandidates } from "./steamArtwork";
import { formatBytes, isFiniteNonNegativeNumber } from "../shared/utils/format";
const GRID_CARD_WIDTH_CSS_VAR = "--game-grid-card-min-width";
const GRID_CARD_WIDTH_DEFAULT_PX = 180;
const GRID_CARD_WIDTH_MIN_PX = 140;
const GRID_CARD_WIDTH_MAX_PX = 320;
const GRID_CARD_WIDTH_STEP_PX = 8;
const GRID_CARD_WIDTH_FINE_STEP_PX = 2;
const GRID_ZOOM_WHEEL_THRESHOLD_PX = 100;
const WHEEL_DELTA_LINE_HEIGHT_PX = 16;
const GRID_WHEEL_SMOOTHING_LERP = 0.16;
const GRID_WHEEL_SMOOTHING_MAX_STEP_PX = 180;
const GRID_WHEEL_SMOOTHING_MIN_WHEEL_DELTA_PX = 8;
const GRID_CARD_WIDTH_STORAGE_KEY = "catalyst.library.gridCardMinWidthPx";
const APP_NAME = "Catalyst";
const DOWNLOAD_POLL_INTERVAL_MS = 2500;
const DOWNLOAD_ETA_SMOOTHING_FACTOR = 0.35;
const DOWNLOAD_ETA_SAMPLE_MIN_SECONDS = 0.5;
const DOWNLOAD_ETA_STALE_MS = 15000;
const TOAST_DURATION_MS = 3200;
const LIBRARY_SOFT_LOCK_ASPECTS: ReadonlyArray<{ label: string; ratio: number }> = [
  { label: "16:9", ratio: 16 / 9 },
  { label: "21:9", ratio: 21 / 9 },
  { label: "32:9", ratio: 32 / 9 },
];

const clamp = (value: number, min: number, max: number): number => Math.min(max, Math.max(min, value));
// runtime platform alias used below
type RuntimePlatform = "windows" | "macos" | "linux" | "other";

const resolveToastRegion = (): HTMLElement => {
  const existingRegion = document.getElementById("launcher-toast-region");
  if (existingRegion instanceof HTMLElement) {
    return existingRegion;
  }

  const region = document.createElement("div");
  region.id = "launcher-toast-region";
  region.className = "launcher-toast-region";
  region.setAttribute("aria-live", "polite");
  region.setAttribute("aria-atomic", "false");
  document.body.append(region);
  return region;
};

const toastRegionElement = resolveToastRegion();

const collectSteamTagSuggestions = (games: GameResponse[]): string[] => {
  const tagsByKey = new Map<string, string>();

  for (const game of games) {
    for (const rawTag of game.steamTags ?? []) {
      const tag = rawTag.trim();
      if (tag.length === 0) {
        continue;
      }
      const key = tag.toLocaleLowerCase();
      if (!tagsByKey.has(key)) {
        tagsByKey.set(key, tag);
      }
    }
  }

  return [...tagsByKey.values()].sort((left, right) =>
    left.localeCompare(right, undefined, { sensitivity: "base" })
  );
};

// Download snapshot type moved to `libraryStore` and imported as `DownloadEtaSnapshot`.

const closeSessionAccountMenu = (): void => {
  sessionAccountMenuElement.hidden = true;
  sessionAccountElement.classList.remove("is-open");
  sessionAccountButton.setAttribute("aria-expanded", "false");
};

const openSessionAccountMenu = (): void => {
  sessionAccountMenuElement.hidden = false;
  sessionAccountElement.classList.add("is-open");
  sessionAccountButton.setAttribute("aria-expanded", "true");
};

const getSessionMenuActionItems = (): HTMLButtonElement[] => {
  return [sessionAccountManageButton, sessionAccountSignOutButton].filter((button) => !button.disabled);
};

const setSessionStatus = (steamConnected: boolean, isError = false): void => {
  store.steamLinked = steamConnected && !isError;
  sessionAccountLabelElement.textContent = APP_NAME;
  sessionAccountButton.classList.toggle("is-error", isError);
  sessionAccountManageButton.disabled = isError;
  sessionAccountSignOutButton.disabled = false;
  downloadActivityElement.classList.toggle("is-disabled", isError || !store.steamLinked);
  if (!store.steamLinked) {
    stopDownloadPolling();
    store.activeDownloads = [];
    store.previousActiveDownloadsByKey.clear();
    if (store.downloadCompletionRefreshTimer !== null) {
      window.clearTimeout(store.downloadCompletionRefreshTimer);
      store.downloadCompletionRefreshTimer = null;
    }
    store.downloadEtaByKey.clear();
    renderDownloadActivity();
  } else {
    renderDownloadActivity();
    startDownloadPolling();
  }
  renderLibraryLastUpdated();
  closeSessionAccountMenu();
};

const setLibrarySummary = (message: string): void => {
  librarySummaryElement.textContent = message;
  librarySummaryElement.classList.remove("status-error");
};

const formatLibraryRefreshAgeLabel = (elapsedMs: number): string => {
  const elapsedSeconds = Math.floor(elapsedMs / 1000);
  if (elapsedSeconds < 15) {
    return "Synced just now";
  }

  if (elapsedSeconds < 60) {
    return `Synced ${elapsedSeconds}s ago`;
  }

  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60) {
    return `Synced ${elapsedMinutes}m ago`;
  }

  const elapsedHours = Math.floor(elapsedMinutes / 60);
  if (elapsedHours < 24) {
    return `Synced ${elapsedHours}h ago`;
  }

  const elapsedDays = Math.floor(elapsedHours / 24);
  return `Synced ${elapsedDays}d ago`;
};

const renderLibraryLastUpdated = (): void => {
  if (store.isLoadingLibrary) {
    libraryLastUpdatedElement.textContent = "Syncing...";
    return;
  }

  if (store.lastLibraryRefreshAtMs === null) {
    libraryLastUpdatedElement.textContent = "Not synced yet";
    return;
  }

  libraryLastUpdatedElement.textContent = formatLibraryRefreshAgeLabel(Date.now() - store.lastLibraryRefreshAtMs);
};

const markLibraryAsUpdatedNow = (): void => {
  store.lastLibraryRefreshAtMs = Date.now();
  renderLibraryLastUpdated();
  if (store.libraryLastUpdatedTimer !== null) {
    return;
  }

  store.libraryLastUpdatedTimer = window.setInterval(() => {
    renderLibraryLastUpdated();
  }, 15000);
};

const stopLibraryLastUpdatedTimer = (): void => {
  if (store.libraryLastUpdatedTimer === null) {
    return;
  }

  window.clearInterval(store.libraryLastUpdatedTimer);
  store.libraryLastUpdatedTimer = null;
};

const showLauncherToast = (message: string, variant: "info" | "error" = "info"): void => {
  const toast = document.createElement("div");
  toast.className = "launcher-toast";
  if (variant === "error") {
    toast.classList.add("is-error");
  }
  toast.textContent = message;
  toast.setAttribute("role", variant === "error" ? "alert" : "status");
  toastRegionElement.append(toast);

  requestAnimationFrame(() => {
    toast.classList.add("is-visible");
  });

  window.setTimeout(() => {
    toast.classList.remove("is-visible");
    window.setTimeout(() => {
      toast.remove();
    }, 160);
  }, TOAST_DURATION_MS);
};

// --- Game details navigation / view-state handling ---
const openGameDetails = (gameId: string, pushHistory = true): void => {
  // Preserve scroll and view mode
  try {
    store.preservedLibraryScrollTop = libraryGridElement.scrollTop;
  } catch {
    store.preservedLibraryScrollTop = 0;
  }
  store.preservedLibraryViewMode = store.activeLibraryViewMode;

  store.appViewMode = "game-details";
  store.selectedGameId = gameId;

  // Hide left sidebar and library grid, show details panel
  panelLeftElement.hidden = true;
  panelMiddleElement.hidden = true;
  libraryGridElement.hidden = true;
  gameDetailsShellElement.hidden = false;

  // Minimal details content while fuller implementation is added later
  gameDetailsContentElement.textContent = "Loading details...";
  renderGameDetails(gameId);

  if (pushHistory) {
    try {
      history.pushState({ view: "game-details", gameId }, "", `#game/${encodeURIComponent(gameId)}`);
    } catch {
      // ignore
    }
  }
};

const closeGameDetails = (pushHistory = false): void => {
  store.appViewMode = "library";
  store.selectedGameId = null;

  // Restore UI
  panelLeftElement.hidden = false;
  panelMiddleElement.hidden = false;
  libraryGridElement.hidden = false;
  gameDetailsShellElement.hidden = true;

  // Restore scroll and view mode
  try {
    libraryGridElement.scrollTop = store.preservedLibraryScrollTop ?? 0;
    store.activeLibraryViewMode = store.preservedLibraryViewMode ?? store.activeLibraryViewMode;
  } catch {
    // ignore
  }

  if (pushHistory) {
    try {
      history.pushState({ view: "library" }, "", "#");
    } catch {
      // ignore
    }
  }
};

// Listen for card open events
document.addEventListener("open-game-details", (e: Event) => {
  const custom = e as CustomEvent<{ gameId: string }>;
  const id = custom?.detail?.gameId;
  if (typeof id === "string" && id.length > 0) {
    openGameDetails(id, true);
  }
});

// Back button
gameDetailsBackButton.addEventListener("click", () => {
  history.back();
});

// Handle browser history navigation
window.addEventListener("popstate", (ev: PopStateEvent) => {
  const state = ev.state as any;
  if (state && state.view === "game-details" && typeof state.gameId === "string") {
    openGameDetails(state.gameId, false);
    return;
  }

  // Default: return to library
  closeGameDetails(false);
});

// When properties or customization artwork change elsewhere, refresh open details if showing
window.addEventListener("game-customization-changed", (ev: Event) => {
  try {
    const ce = ev as CustomEvent;
    const gameId = ce?.detail?.gameId as string | undefined;
    if (gameId && store.appViewMode === "game-details" && store.selectedGameId === gameId) {
      renderGameDetails(gameId);
    }
  } catch {
    // ignore
  }
});

const renderGameDetails = (gameId: string): void => {
  console.debug("renderGameDetails called for", gameId);
  const game = findGameById(gameId) ?? store.gameById.get(gameId) ?? null;
  console.debug("resolved game:", game ? game.id : null);
  if (!game) {
    const playCellFallback = detailsTitleInfo.querySelector('.details-play-cell');
    const notFound = document.createElement('div');
    notFound.innerHTML = `<div><strong>Title</strong><div class=\"muted\">Game not found</div></div>`;
    if (playCellFallback) {
      detailsTitleInfo.replaceChildren(playCellFallback, notFound);
    } else {
      detailsTitleInfo.replaceChildren(notFound);
    }
    detailsHeroBg.style.backgroundImage = "";
    gameDetailsContentElement.textContent = "Game details unavailable.";
    return;
  }

  // Populate short activity/status info into the title row (preserve existing play cell)
  const lastPlayed = game.lastPlayedAt ? new Date(game.lastPlayedAt) : null;
  const lastPlayedLabel = lastPlayed
    ? new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(lastPlayed)
    : "Never played";
  const playtimeLabel = typeof game.playtimeMinutes === "number" ? `${Math.round(game.playtimeMinutes)} minutes` : "-";

  const playCell = detailsTitleInfo.querySelector('.details-play-cell') ?? document.createElement('div');
  const lastPlayedDiv = document.createElement('div');
  lastPlayedDiv.innerHTML = `<strong>Last Played</strong><div class="muted">${lastPlayedLabel}</div>`;
  const playtimeDiv = document.createElement('div');
  playtimeDiv.innerHTML = `<strong>Playtime</strong><div class="muted">${playtimeLabel}</div>`;
  const statusDiv = document.createElement('div');
  statusDiv.innerHTML = `<strong>Status</strong><div class="muted">${game.installed ? "Installed" : "Not installed"}</div>`;

  detailsTitleInfo.replaceChildren(playCell, lastPlayedDiv, playtimeDiv, statusDiv);
  // Determine hero background image using customization artwork -> steam candidates -> gradient fallback
  void (async () => {
    // Ensure a neutral gradient fallback is present immediately to avoid layout shift
    detailsHeroBg.style.backgroundImage = ""; // let CSS fallback (gradient) show via class
    detailsHeroBg.classList.add("details-hero-loading");

    // Helper: attempt to load the first image that succeeds from candidates
    const loadFirstAvailable = async (candidates: string[]): Promise<string | null> => {
      for (const url of candidates) {
        try {
          await new Promise<void>((resolve, reject) => {
            const img = new Image();
            let settled = false;
            const clean = () => {
              img.onload = null;
              img.onerror = null;
            };
            img.onload = () => { if (!settled) { settled = true; clean(); resolve(); } };
            img.onerror = () => { if (!settled) { settled = true; clean(); reject(new Error('error')); } };
            // start load
            img.src = url;
          });
          return url;
        } catch {
          // try next
          continue;
        }
      }
      return null;
    };

    // Build candidate list in priority order
    const customization = await getGameCustomizationArtworkForGame(game);
    const candidates: string[] = [];
    if (customization && typeof customization.background === "string" && customization.background.trim() !== "") {
      candidates.push(customization.background);
    }

    try {
      const steamCandidates = getSteamArtworkCandidates(game, "background") ?? [];
      for (const c of steamCandidates) candidates.push(c);
    } catch {
      // ignore
    }

    const chosen = await loadFirstAvailable(candidates);
    if (chosen) {
      // set background image with a smooth fade; remove loading marker
      detailsHeroBg.style.backgroundImage = `url('${chosen}')`;
    } else {
      // leave gradient/fallback in place
      detailsHeroBg.style.backgroundImage = "";
    }
    detailsHeroBg.classList.remove("details-hero-loading");
  })();

  // Debug: report UI visibility and content counts to help trace blank-details issue
  try {
    const shellHidden = gameDetailsShellElement.hidden;
    const middleHidden = panelMiddleElement.hidden;
    const gridHidden = libraryGridElement.hidden;
    const contentChildren = gameDetailsContentElement.childElementCount;
    const shellRect = gameDetailsShellElement.getBoundingClientRect();
    console.debug("renderGameDetails UI state:", { shellHidden, middleHidden, gridHidden, contentChildren, shellRect });
  } catch (err) {
    console.debug("renderGameDetails UI debug failed", err);
  }

  // Update action buttons
  detailsPlayButton.textContent = game.installed ? "Play" : "Install";
  // Keep the favorite button icon SVG intact; use aria-pressed and class to indicate state
  detailsFavoriteButton.setAttribute("aria-pressed", `${game.favorite ? "true" : "false"}`);
  detailsFavoriteButton.setAttribute("aria-label", game.favorite ? "Unfavorite" : "Favorite");
  detailsFavoriteButton.classList.toggle("is-favorited", !!game.favorite);
  // Fill main + side content with sections (use real data where available)
  gameDetailsContentElement.replaceChildren();
  const cols = document.createElement("div");
  cols.className = "game-details-columns";

  const main = document.createElement("div");
  main.className = "game-details-main-inner";

  // Activity / Timeline (activity-row moved into the title row above)
  const activitySection = document.createElement("section");
  activitySection.className = "details-section";
  activitySection.innerHTML = `
    <h3>Activity</h3>
    <div class="details-subsection">
      <h4>Timeline</h4>
      <p class="placeholder">Recent activity and news will appear here.</p>
    </div>
  `;

  // Achievements (placeholder-first)
  const achievementsSection = document.createElement("section");
  achievementsSection.className = "details-section";
  achievementsSection.innerHTML = `
    <h4>Achievements</h4>
    <p class="placeholder">Achievements are not available yet. Coming soon.</p>
  `;

  // Screenshots (placeholder grid)
  const screenshotsSection = document.createElement("section");
  screenshotsSection.className = "details-section";
  screenshotsSection.innerHTML = `
    <h4>Screenshots</h4>
    <p class="placeholder">No screenshots available. You can add screenshots via the Properties panel.</p>
  `;

  // Review and Notes (read-only placeholders for v1)
  const notesSection = document.createElement("section");
  notesSection.className = "details-section";
  notesSection.innerHTML = `
    <h4>Notes</h4>
    <div class="notes-card placeholder">Personal notes for this game will appear here (read-only in v1).</div>
  `;

  const reviewSection = document.createElement("section");
  reviewSection.className = "details-section";
  reviewSection.innerHTML = `
    <h4>Review</h4>
    <div class="review-card placeholder">Write a review for this game (coming soon — read-only placeholder).</div>
  `;

  main.append(activitySection, achievementsSection, screenshotsSection, notesSection, reviewSection);

  // Side column
  const side = document.createElement("aside");
  side.className = "game-details-side-inner";

  // Friends activity (placeholder; suggest connecting Steam)
  const friendsSection = document.createElement("section");
  friendsSection.className = "details-section";
  friendsSection.innerHTML = `
    <h4>Friends</h4>
    <p class="placeholder">${store.steamLinked ? "Friends activity will appear here." : "Connect Steam to show friends activity."}</p>
  `;

  // Trading cards (placeholder)
  const cardsSection = document.createElement("section");
  cardsSection.className = "details-section";
  cardsSection.innerHTML = `
    <h4>Trading Cards</h4>
    <p class="placeholder">Trading card details and progress will appear here.</p>
  `;

  // Installation details (real data if available)
  const installationSection = document.createElement("section");
  installationSection.className = "details-section";
  installationSection.innerHTML = `<h4>Installation</h4><p class="placeholder">Loading installation details…</p>`;

  side.append(friendsSection, cardsSection, installationSection);

  cols.append(main, side);
  gameDetailsContentElement.append(cols);

  // Async: fetch installation details and update installationSection if available
  void (async () => {
    try {
      const install = await getGameInstallationDetailsForGame(game);
      installationSection.replaceChildren();
      installationSection.className = "details-section";
      if (!install) {
        installationSection.innerHTML = `<h4>Installation</h4><p class=\"placeholder\">No installation data available.</p>`;
      } else {
        const list = document.createElement("div");
        list.className = "installation-list";
        const pathRow = document.createElement("div");
        pathRow.innerHTML = `<div><strong>Path</strong><div class=\"muted\">${install.installPath ?? "-"}</div></div>`;
        const sizeRow = document.createElement("div");
        sizeRow.innerHTML = `<div><strong>Installed Size</strong><div class=\"muted\">${formatBytes(install.sizeOnDiskBytes) ?? "-"}</div></div>`;
        list.append(pathRow, sizeRow);
        const h4 = document.createElement("h4");
        h4.textContent = "Installation";
        installationSection.append(h4);
        installationSection.append(list);
      }
    } catch (err) {
      installationSection.innerHTML = `<h4>Installation</h4><p class=\"placeholder\">Could not load installation details.</p>`;
    }
  })();
};



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
      store.downloadEtaByKey.delete(key);
      continue;
    }

    const currentBytesDownloaded = download.bytesDownloaded;
    const previousSnapshot = store.downloadEtaByKey.get(key);
    if (
      !previousSnapshot
      || currentBytesDownloaded < previousSnapshot.lastBytesDownloaded
      || nowMs <= previousSnapshot.lastSampleAtMs
    ) {
      store.downloadEtaByKey.set(key, {
        lastBytesDownloaded: currentBytesDownloaded,
        lastSampleAtMs: nowMs,
        smoothedBytesPerSecond: previousSnapshot?.smoothedBytesPerSecond ?? 0,
      });
      continue;
    }

    const elapsedSeconds = (nowMs - previousSnapshot.lastSampleAtMs) / 1000;
    let smoothedBytesPerSecond = previousSnapshot.smoothedBytesPerSecond;
    if (elapsedSeconds >= DOWNLOAD_ETA_SAMPLE_MIN_SECONDS) {
      const deltaBytes = currentBytesDownloaded - previousSnapshot.lastBytesDownloaded;
      if (deltaBytes > 0) {
        const instantaneousBytesPerSecond = deltaBytes / elapsedSeconds;
        if (Number.isFinite(instantaneousBytesPerSecond) && instantaneousBytesPerSecond > 0) {
          smoothedBytesPerSecond = smoothedBytesPerSecond > 0
            ? (
                smoothedBytesPerSecond * (1 - DOWNLOAD_ETA_SMOOTHING_FACTOR)
                + instantaneousBytesPerSecond * DOWNLOAD_ETA_SMOOTHING_FACTOR
              )
            : instantaneousBytesPerSecond;
        }
      }
    }

    store.downloadEtaByKey.set(key, {
      lastBytesDownloaded: currentBytesDownloaded,
      lastSampleAtMs: nowMs,
      smoothedBytesPerSecond,
    });
  }

  for (const key of [...store.downloadEtaByKey.keys()]) {
    if (!activeKeys.has(key)) {
      store.downloadEtaByKey.delete(key);
    }
  }
};

const getDownloadTransferRateLabel = (download: SteamDownloadProgressPayload): string | null => {
  if (download.progressSource === "directory-estimate") {
    return null;
  }
  const stateLabel = download.state.trim().toLocaleLowerCase();
  if (!(stateLabel.includes("download") || stateLabel === "updating")) {
    return null;
  }

  const etaSnapshot = store.downloadEtaByKey.get(getDownloadEtaKey(download));
  if (!etaSnapshot || etaSnapshot.smoothedBytesPerSecond <= 0) {
    return null;
  }

  if (Date.now() - etaSnapshot.lastSampleAtMs > DOWNLOAD_ETA_STALE_MS) {
    return null;
  }

  const speedLabel = formatBytes(etaSnapshot.smoothedBytesPerSecond);
  if (!speedLabel) {
    return null;
  }

  return `${speedLabel}/s`;
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

const renderDownloadActivity = (): void => {
  const activeCount = store.activeDownloads.length;
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
    emptyMessage.textContent = store.steamLinked
      ? "No active downloads"
      : "Connect Steam to view download activity";
    downloadActivityListElement.append(emptyMessage);
    return;
  }

  for (const download of store.activeDownloads) {
    const row = document.createElement("article");
    row.className = "download-activity-item";

    const header = document.createElement("div");
    header.className = "download-activity-item-header";

    const name = document.createElement("p");
    name.className = "download-activity-item-name";
    name.textContent = download.name;

    const state = document.createElement("p");
    state.className = "download-activity-item-state";
    state.textContent = download.state;

    header.append(name, state);
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

const setLibraryLoadingState = (isLoading: boolean): void => {
  store.isLoadingLibrary = isLoading;
  refreshLibraryButton.disabled = isLoading;
  refreshLibraryButton.classList.toggle("is-loading", isLoading);
  refreshLibraryButton.setAttribute("aria-busy", `${isLoading}`);
  refreshLibraryLabelElement.textContent = isLoading ? "Syncing" : "Refresh";
  renderLibraryLastUpdated();
};

const readGridCardWidthPx = (): number => {
  const inlineValue = Number.parseFloat(libraryGridElement.style.getPropertyValue(GRID_CARD_WIDTH_CSS_VAR));
  if (Number.isFinite(inlineValue) && inlineValue > 0) {
    return inlineValue;
  }

  const computedValue = Number.parseFloat(getComputedStyle(libraryGridElement).getPropertyValue(GRID_CARD_WIDTH_CSS_VAR));
  if (Number.isFinite(computedValue) && computedValue > 0) {
    return computedValue;
  }

  return GRID_CARD_WIDTH_DEFAULT_PX;
};

const readStoredGridCardWidthPx = (): number | null => {
  try {
    const storedValue = localStorage.getItem(GRID_CARD_WIDTH_STORAGE_KEY);
    if (!storedValue) {
      return null;
    }

    const parsed = Number.parseFloat(storedValue);
    if (!Number.isFinite(parsed) || parsed <= 0) {
      return null;
    }

    return clamp(Math.round(parsed), GRID_CARD_WIDTH_MIN_PX, GRID_CARD_WIDTH_MAX_PX);
  } catch {
    return null;
  }
};

const persistGridCardWidthPx = (value: number): void => {
  try {
    localStorage.setItem(GRID_CARD_WIDTH_STORAGE_KEY, `${value}`);
  } catch {
    // Ignore storage failures in restricted environments.
  }
};

const setGridCardWidthPx = (value: number, persistValue = true): void => {
  const clampedValue = clamp(Math.round(value), GRID_CARD_WIDTH_MIN_PX, GRID_CARD_WIDTH_MAX_PX);
  libraryGridElement.style.setProperty(GRID_CARD_WIDTH_CSS_VAR, `${clampedValue}px`);
  if (persistValue) {
    persistGridCardWidthPx(clampedValue);
  }
};

const normalizeWheelDeltaToPx = (event: WheelEvent): number => {
  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) {
    return event.deltaY * WHEEL_DELTA_LINE_HEIGHT_PX;
  }

  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) {
    return event.deltaY * window.innerHeight;
  }

  return event.deltaY;
};

const resolveRuntimePlatform = (): RuntimePlatform => {
  const userAgentDataPlatform = (
    navigator as Navigator & { userAgentData?: { platform?: string } }
  ).userAgentData?.platform?.toLocaleLowerCase();
  if (userAgentDataPlatform) {
    if (userAgentDataPlatform.includes("win")) {
      return "windows";
    }
    if (userAgentDataPlatform.includes("mac")) {
      return "macos";
    }
    if (userAgentDataPlatform.includes("linux")) {
      return "linux";
    }
  }

  const platform = navigator.platform.toLocaleLowerCase();
  if (platform.includes("win")) {
    return "windows";
  }
  if (platform.includes("mac")) {
    return "macos";
  }
  if (platform.includes("linux")) {
    return "linux";
  }

  const userAgent = navigator.userAgent.toLocaleLowerCase();
  if (userAgent.includes("windows")) {
    return "windows";
  }
  if (userAgent.includes("mac os")) {
    return "macos";
  }
  if (userAgent.includes("linux")) {
    return "linux";
  }

  return "other";
};

const isLikelyTrackpadWheelEvent = (event: WheelEvent): boolean => {
  if (event.deltaMode !== WheelEvent.DOM_DELTA_PIXEL) {
    return false;
  }

  // Horizontal deltas are commonly generated by trackpads.
  if (Math.abs(event.deltaX) > 0) {
    return true;
  }

  // Non-integer deltas are typically produced by high-resolution touchpads.
  if (!Number.isInteger(event.deltaY)) {
    return true;
  }

  // Some trackpads emit small integer deltas that used to slip past
  // detection. Treat moderately small deltas as trackpad input so we
  // don't intercept two-finger scrolling gestures.
  const TOUCHPAD_LIKELY_DELTA_PX = GRID_WHEEL_SMOOTHING_MIN_WHEEL_DELTA_PX * 3; // ~24px
  return Math.abs(event.deltaY) < TOUCHPAD_LIKELY_DELTA_PX;
};

const registerLinuxGridWheelSmoothing = (): (() => void) => {
  if (resolveRuntimePlatform() !== "linux") {
    return () => {};
  }

  const reducedMotionMediaQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
  let currentScrollTop = libraryGridElement.scrollTop;
  let targetScrollTop = currentScrollTop;
  let animationFrameId: number | null = null;

  const getMaxGridScrollTop = (): number => {
    return Math.max(libraryGridElement.scrollHeight - libraryGridElement.clientHeight, 0);
  };

  const syncGridScrollPosition = (): void => {
    if (animationFrameId !== null) {
      return;
    }
    currentScrollTop = libraryGridElement.scrollTop;
    targetScrollTop = currentScrollTop;
  };

  const animateGridScroll = (): void => {
    targetScrollTop = clamp(targetScrollTop, 0, getMaxGridScrollTop());
    currentScrollTop += (targetScrollTop - currentScrollTop) * GRID_WHEEL_SMOOTHING_LERP;

    if (Math.abs(targetScrollTop - currentScrollTop) < 0.35) {
      currentScrollTop = targetScrollTop;
    }

    libraryGridElement.scrollTop = currentScrollTop;
    if (currentScrollTop !== targetScrollTop) {
      animationFrameId = window.requestAnimationFrame(animateGridScroll);
      return;
    }

    animationFrameId = null;
  };

  const handleWheel = (event: WheelEvent): void => {
    if (!isGameLibraryViewMode(store.activeLibraryViewMode) || event.ctrlKey || event.metaKey) {
      return;
    }
    if (reducedMotionMediaQuery.matches || isLikelyTrackpadWheelEvent(event)) {
      return;
    }

    const rawDeltaPx = normalizeWheelDeltaToPx(event);
    if (rawDeltaPx === 0) {
      return;
    }

    const maxScrollTop = getMaxGridScrollTop();
    if (maxScrollTop <= 0) {
      return;
    }

    event.preventDefault();
    const deltaPx = clamp(rawDeltaPx, -GRID_WHEEL_SMOOTHING_MAX_STEP_PX, GRID_WHEEL_SMOOTHING_MAX_STEP_PX);
    currentScrollTop = libraryGridElement.scrollTop;
    if (animationFrameId === null) {
      targetScrollTop = currentScrollTop;
    }
    targetScrollTop = clamp(targetScrollTop + deltaPx, 0, maxScrollTop);

    if (animationFrameId === null) {
      animationFrameId = window.requestAnimationFrame(animateGridScroll);
    }
  };

  libraryGridElement.addEventListener("scroll", syncGridScrollPosition, { passive: true });
  libraryGridElement.addEventListener("wheel", handleWheel, { passive: false });

  return () => {
    libraryGridElement.removeEventListener("scroll", syncGridScrollPosition);
    libraryGridElement.removeEventListener("wheel", handleWheel);
    if (animationFrameId !== null) {
      window.cancelAnimationFrame(animationFrameId);
      animationFrameId = null;
    }
  };
};

const applyLibraryAspectSoftLock = (): void => {
  const viewportWidth = Math.max(window.innerWidth, 1);
  const viewportHeight = Math.max(window.innerHeight, 1);
  const viewportRatio = viewportWidth / viewportHeight;

  let targetAspect = LIBRARY_SOFT_LOCK_ASPECTS[0];
  let smallestRatioDistance = Number.POSITIVE_INFINITY;
  for (const candidate of LIBRARY_SOFT_LOCK_ASPECTS) {
    const ratioDistance = Math.abs(viewportRatio - candidate.ratio);
    if (ratioDistance < smallestRatioDistance) {
      smallestRatioDistance = ratioDistance;
      targetAspect = candidate;
    }
  }

  // Fill the viewport to avoid both clipping and side letterboxing.
  const frameWidth = viewportWidth;
  const frameHeight = viewportHeight;

  libraryAspectShellElement.style.setProperty("--library-aspect-width", `${Math.max(frameWidth, 1)}px`);
  libraryAspectShellElement.style.setProperty("--library-aspect-height", `${Math.max(frameHeight, 1)}px`);
  libraryAspectShellElement.style.setProperty("--library-aspect-ratio", `${targetAspect.ratio}`);
  libraryAspectShellElement.dataset.aspectLabel = targetAspect.label;
};

const registerGridZoomShortcut = (): void => {
  const initialWidth = readStoredGridCardWidthPx() ?? readGridCardWidthPx();
  setGridCardWidthPx(initialWidth, false);
  let accumulatedZoomDeltaPx = 0;

  libraryGridElement.addEventListener("wheel", (event) => {
    if (!event.ctrlKey || event.deltaY === 0) {
      return;
    }

    event.preventDefault();
    const deltaPx = normalizeWheelDeltaToPx(event);
    if (deltaPx === 0) {
      return;
    }

    if (accumulatedZoomDeltaPx !== 0 && Math.sign(accumulatedZoomDeltaPx) !== Math.sign(deltaPx)) {
      accumulatedZoomDeltaPx = 0;
    }

    accumulatedZoomDeltaPx += deltaPx;
    const steps = Math.trunc(Math.abs(accumulatedZoomDeltaPx) / GRID_ZOOM_WHEEL_THRESHOLD_PX);
    if (steps === 0) {
      return;
    }

    const cardWidthStepPx = event.shiftKey ? GRID_CARD_WIDTH_FINE_STEP_PX : GRID_CARD_WIDTH_STEP_PX;
    const zoomDirection = accumulatedZoomDeltaPx < 0 ? 1 : -1;
    const currentWidth = readGridCardWidthPx();
    setGridCardWidthPx(currentWidth + (zoomDirection * cardWidthStepPx * steps));

    const remainingDeltaPx = Math.abs(accumulatedZoomDeltaPx) % GRID_ZOOM_WHEEL_THRESHOLD_PX;
    accumulatedZoomDeltaPx = remainingDeltaPx * Math.sign(accumulatedZoomDeltaPx);
  }, { passive: false });
};

const setAllGames = (games: GameResponse[]): void => {
  store.allGames = games;
  store.gameById = new Map(games.map((game) => [game.id, game]));
  console.debug("setAllGames: loaded", games.length, "games; sample ids:", games.slice(0,10).map(g => g.id));
  filterPanel.setSteamTagSuggestions(collectSteamTagSuggestions(games));
  updateCollectionSuggestions();
};

const buildCollectionSuggestionList = (): string[] => {
  const suggestionsByKey = new Map<string, string>();
  const registerSuggestion = (suggestion: string): void => {
    const trimmedSuggestion = suggestion.trim();
    if (trimmedSuggestion.length === 0) {
      return;
    }

    const normalizedSuggestion = trimmedSuggestion.toLocaleLowerCase();
    if (!suggestionsByKey.has(normalizedSuggestion)) {
      suggestionsByKey.set(normalizedSuggestion, trimmedSuggestion);
    }
  };

  registerSuggestion(HIDDEN_GAMES_COLLECTION_NAME);
  for (const collection of store.allCollections) {
    registerSuggestion(collection.name);
  }

  return [...suggestionsByKey.values()].sort((left, right) =>
    left.localeCompare(right, undefined, { sensitivity: "base" })
  );
};

const updateCollectionSuggestions = (): void => {
  filterPanel.setCollectionSuggestions(buildCollectionSuggestionList());
};

const setAllCollections = (collections: CollectionResponse[]): void => {
  const sortedCollections = [...collections].sort((left, right) =>
    left.name.localeCompare(right.name, undefined, { sensitivity: "base" })
  );
  store.allCollections = sortedCollections;
  updateCollectionSuggestions();
};

const normalizeCollectionNameForMatch = (collectionName: string): string => {
  return collectionName.trim().toLocaleLowerCase();
};

const isHiddenGamesCollectionFilter = (collectionName: string): boolean => {
  return normalizeCollectionNameForMatch(collectionName) === normalizeCollectionNameForMatch(HIDDEN_GAMES_COLLECTION_NAME);
};

const isInstalledGame = (game: GameResponse): boolean => {
  return typeof game.installed === "boolean" ? game.installed : game.playtimeMinutes > 0;
};

const getGamesForLibraryViewMode = (
  games: GameResponse[],
  viewMode: LibraryViewMode
): GameResponse[] => {
  if (viewMode === "installed") {
    return games.filter((game) => isInstalledGame(game));
  }

  if (viewMode === "favorites") {
    return games.filter((game) => game.favorite);
  }

  return games;
};

const countHiddenGames = (): number => {
  return store.allGames.filter((game) => game.hideInLibrary === true).length;
};

const countVisibleFavoriteGames = (): number => {
  return store.allGames.filter((game) => game.favorite && game.hideInLibrary !== true).length;
};

const buildVisibleCollectionGameCounts = (): Map<string, number> => {
  const countsByCollection = new Map<string, number>();

  for (const game of store.allGames) {
    if (game.hideInLibrary === true) {
      continue;
    }

    const seenCollectionsForGame = new Set<string>();
    for (const collectionName of game.collections ?? []) {
      const normalizedCollectionName = normalizeCollectionNameForMatch(collectionName);
      if (normalizedCollectionName.length === 0 || seenCollectionsForGame.has(normalizedCollectionName)) {
        continue;
      }

      seenCollectionsForGame.add(normalizedCollectionName);
      const previousCount = countsByCollection.get(normalizedCollectionName) ?? 0;
      countsByCollection.set(normalizedCollectionName, previousCount + 1);
    }
  }

  return countsByCollection;
};

const buildCollectionSectionsForGames = (
  games: GameResponse[],
  collections: CollectionResponse[]
): GameGridSection[] => {
  if (collections.length === 0 || games.length === 0) {
    return [];
  }

  const sections: GameGridSection[] = collections.map((collection) => ({
    id: collection.id,
    title: collection.name,
    games: [],
  }));
  const collectionNameIndex = new Map<string, number>();
  for (let index = 0; index < collections.length; index += 1) {
    collectionNameIndex.set(normalizeCollectionNameForMatch(collections[index].name), index);
  }

  const uncategorizedSection: GameGridSection = {
    id: "uncategorized",
    title: "Uncategorized",
    games: [],
  };

  for (const game of games) {
    const gameCollections = game.collections ?? [];
    let targetSection: GameGridSection | null = null;
    let targetSectionIndex = Number.POSITIVE_INFINITY;

    for (const gameCollection of gameCollections) {
      const normalizedCollection = normalizeCollectionNameForMatch(gameCollection);
      if (normalizedCollection.length === 0) {
        continue;
      }
      const sectionIndex = collectionNameIndex.get(normalizedCollection);
      if (sectionIndex === undefined) {
        continue;
      }
      if (sectionIndex < targetSectionIndex) {
        targetSection = sections[sectionIndex];
        targetSectionIndex = sectionIndex;
      }
    }

    if (!targetSection) {
      uncategorizedSection.games.push(game);
      continue;
    }
    targetSection.games.push(game);
  }

  const populatedSections = sections.filter((section) => section.games.length > 0);
  if (uncategorizedSection.games.length > 0) {
    populatedSections.push(uncategorizedSection);
  }

  return populatedSections;
};

const upsertCollectionInState = (collection: CollectionResponse): void => {
  const existingIndex = store.allCollections.findIndex((existingCollection) => existingCollection.id === collection.id);
  if (existingIndex < 0) {
    setAllCollections([...store.allCollections, collection]);
    return;
  }

  const nextCollections = [...store.allCollections];
  nextCollections[existingIndex] = {
    ...nextCollections[existingIndex],
    ...collection,
  };
  setAllCollections(nextCollections);
};

const removeCollectionFromState = (collectionId: string): void => {
  setAllCollections(store.allCollections.filter((collection) => collection.id !== collectionId));
};

const updateCollectionNameInGames = (previousName: string, nextName: string | null): void => {
  const normalizedPreviousName = normalizeCollectionNameForMatch(previousName);
  if (normalizedPreviousName.length === 0) {
    return;
  }

  const normalizedNextName = nextName === null ? "" : normalizeCollectionNameForMatch(nextName);
  const nextCollectionName = nextName?.trim() ?? "";
  let stateChanged = false;
  const nextGames = store.allGames.map((game) => {
    if (!game.collections || game.collections.length === 0) {
      return game;
    }

    let gameChanged = false;
    const dedupedCollections: string[] = [];
    const seenCollections = new Set<string>();
    for (const rawCollectionName of game.collections) {
      const trimmedCollectionName = rawCollectionName.trim();
      if (trimmedCollectionName.length === 0) {
        gameChanged = true;
        continue;
      }

      const normalizedCollectionName = normalizeCollectionNameForMatch(trimmedCollectionName);
      if (normalizedCollectionName === normalizedPreviousName) {
        gameChanged = true;
        if (normalizedNextName.length === 0) {
          continue;
        }
        if (!seenCollections.has(normalizedNextName)) {
          dedupedCollections.push(nextCollectionName);
          seenCollections.add(normalizedNextName);
        }
        continue;
      }

      if (!seenCollections.has(normalizedCollectionName)) {
        dedupedCollections.push(trimmedCollectionName);
        seenCollections.add(normalizedCollectionName);
        continue;
      }

      gameChanged = true;
    }

    if (!gameChanged) {
      return game;
    }

    stateChanged = true;
    return {
      ...game,
      collections: dedupedCollections,
    };
  });

  if (!stateChanged) {
    return;
  }

  store.allGames = nextGames;
  store.gameById = new Map(nextGames.map((game) => [game.id, game]));
};

const resolveGameFromCard = (card: HTMLElement): GameResponse | null => {
  const gameId = card.dataset.gameId;
  if (!gameId) {
    return null;
  }

  return store.gameById.get(gameId) ?? null;
};

const updateGameInState = (
  gameId: string,
  update: (game: GameResponse) => GameResponse
): GameResponse | null => {
  const gameIndex = store.allGames.findIndex((game) => game.id === gameId);
  if (gameIndex < 0) {
    return null;
  }

  const updatedGame = update(store.allGames[gameIndex]);
  store.allGames[gameIndex] = updatedGame;
  store.gameById.set(updatedGame.id, updatedGame);
  return updatedGame;
};

const renderGameLibrary = (): void => {
  store.closeGameContextMenu?.();
  const collectionGridCleanupTarget = libraryGridElement as HTMLElement & {
    __collectionGridCleanup?: () => void;
  };
  collectionGridCleanupTarget.__collectionGridCleanup?.();
  collectionGridCleanupTarget.__collectionGridCleanup = undefined;
  const filters = filterPanel.getFilters();
  const viewScopedGames = getGamesForLibraryViewMode(store.allGames, store.activeLibraryViewMode);
  const showOnlyHiddenGames = isHiddenGamesCollectionFilter(filters.collection);
  const eligibleGameCount = viewScopedGames.filter((game) =>
    showOnlyHiddenGames ? game.hideInLibrary === true : game.hideInLibrary !== true
  ).length;
  const filteredGames = applyLibraryFilters(viewScopedGames, filters);
  const emptyMessage = store.allGames.length === 0
    ? "No games synced yet."
    : showOnlyHiddenGames
      ? store.activeLibraryViewMode === "installed"
        ? "No hidden installed games."
          : store.activeLibraryViewMode === "favorites"
          ? "No hidden favorite games."
          : "No hidden games."
      : eligibleGameCount === 0
        ? store.activeLibraryViewMode === "installed"
          ? viewScopedGames.length === 0
            ? "No installed games yet."
            : "All installed games are hidden. Select \"Hidden Games\" in the Collection filter to view them."
          : store.activeLibraryViewMode === "favorites"
            ? viewScopedGames.length === 0
              ? "No favorite games yet."
              : "All favorite games are hidden. Select \"Hidden Games\" in the Collection filter to view them."
            : "All games are hidden. Select \"Hidden Games\" in the Collection filter to view them."
        : store.activeLibraryViewMode === "installed"
          ? "No installed games match your current filters."
          : store.activeLibraryViewMode === "favorites"
            ? "No favorite games match your current filters."
            : "No games match your current filters.";
  const canRenderCollectionSections = store.allCollections.length > 0 && filters.collection.trim().length === 0;
  const sections = canRenderCollectionSections
    ? buildCollectionSectionsForGames(filteredGames, store.allCollections)
    : undefined;

  renderGameGrid({
    container: libraryGridElement,
    games: filteredGames,
    emptyMessage,
    sections,
  });
  if (store.activeLibraryViewMode === "installed") {
    setLibrarySummary(`${filteredGames.length} of ${eligibleGameCount} installed games shown.`);
    return;
  }

  if (store.activeLibraryViewMode === "favorites") {
    setLibrarySummary(`${filteredGames.length} of ${eligibleGameCount} favorite games shown.`);
    return;
  }

  setLibrarySummary(`${filteredGames.length} of ${eligibleGameCount} games shown.`);
};

const gamePropertiesPanel = createGamePropertiesPanel();
const installDialog = createInstallDialog();
const collectionNameDialog = createCollectionNameDialog();
const confirmationDialog = createConfirmationDialog();

const listCollectionsForGame = async (game: GameResponse): Promise<CollectionResponse[]> => {
  return ipcService.listCollections({
    provider: game.provider,
    externalId: game.externalId,
  });
};

const listCollectionsForUser = async (): Promise<CollectionResponse[]> => {
  return ipcService.listCollections();
};

const syncCollectionStateForGame = async (game: GameResponse): Promise<void> => {
  const collectionsSnapshot = await listCollectionsForGame(game);
  setAllCollections(collectionsSnapshot);
  updateGameInState(game.id, (existingGame) => ({
    ...existingGame,
    collections: collectionsSnapshot
      .filter((collection) => collection.containsGame)
      .map((collection) => collection.name),
  }));

  if (isCollectionLibraryViewMode(store.activeLibraryViewMode)) {
    renderCollectionLibrary();
  } else {
    renderGameLibrary();
  }
};

const createCollectionFromGrid = async (): Promise<void> => {
  const collectionName = await collectionNameDialog.open({
    title: "Create Collection",
    description: "Name your new collection.",
    confirmLabel: "Create",
    placeholder: "Collection name",
  });
  if (collectionName === null) {
    return;
  }

  try {
    const createdCollection = await ipcService.createCollection({
      name: collectionName,
    });
    upsertCollectionInState(createdCollection);
    showLauncherToast(`Created collection "${createdCollection.name}".`);
    setLibraryViewMode("collections", false);
    renderActiveLibraryView();
    void refreshLibrary(false);
  } catch (error) {
    const appError = normalizeAppError(error, "Could not create collection.");
    showLauncherToast(appError.message, "error");
    console.error(`[collections/create] ${appError.kind}:${appError.code} ${appError.message}`);
  }
};

const renameCollectionFromGrid = async (collection: CollectionGridItem): Promise<void> => {
  const renamedCollectionName = await collectionNameDialog.open({
    title: "Rename Collection",
    description: `Rename "${collection.name}".`,
    confirmLabel: "Save",
    initialValue: collection.name,
    placeholder: "Collection name",
  });
  if (renamedCollectionName === null) {
    return;
  }

  try {
    const updatedCollection = await ipcService.renameCollection({
      collectionId: collection.id,
      name: renamedCollectionName,
    });
    const previousCollectionName = collection.name;
    upsertCollectionInState(updatedCollection);
    updateCollectionNameInGames(previousCollectionName, updatedCollection.name);
    const activeCollectionFilter = normalizeCollectionNameForMatch(filterPanel.getFilters().collection);
    if (activeCollectionFilter === normalizeCollectionNameForMatch(previousCollectionName)) {
      filterPanel.setCollectionFilter(updatedCollection.name);
    }
    showLauncherToast(`Renamed collection to "${updatedCollection.name}".`);
    setLibraryViewMode("collections", false);
    renderActiveLibraryView();
    void refreshLibrary(false);
  } catch (error) {
    const appError = normalizeAppError(error, "Could not rename collection.");
    showLauncherToast(appError.message, "error");
    console.error(`[collections/rename] ${appError.kind}:${appError.code} ${appError.message}`);
  }
};

const deleteCollectionFromGrid = async (collection: CollectionGridItem): Promise<void> => {
  const shouldDelete = await confirmationDialog.open({
    title: "Delete Collection",
    description: `Delete collection "${collection.name}"? Games stay in your library.`,
    confirmLabel: "Delete",
    confirmTone: "danger",
  });
  if (!shouldDelete) {
    return;
  }

  try {
    await ipcService.deleteCollection({
      collectionId: collection.id,
    });
    removeCollectionFromState(collection.id);
    updateCollectionNameInGames(collection.name, null);
    const activeCollectionFilter = normalizeCollectionNameForMatch(filterPanel.getFilters().collection);
    if (activeCollectionFilter === normalizeCollectionNameForMatch(collection.name)) {
      filterPanel.setCollectionFilter("");
    }
    showLauncherToast(`Deleted collection "${collection.name}".`);
    setLibraryViewMode("collections", false);
    renderActiveLibraryView();
    void refreshLibrary(false);
  } catch (error) {
    const appError = normalizeAppError(error, "Could not delete collection.");
    showLauncherToast(appError.message, "error");
    console.error(`[collections/delete] ${appError.kind}:${appError.code} ${appError.message}`);
  }
};

const renderCollectionLibrary = (): void => {
  store.closeGameContextMenu?.();
  const favoritesCount = countVisibleFavoriteGames();
  const hiddenCount = countHiddenGames();
  const visibleCollectionCounts = buildVisibleCollectionGameCounts();
  const collectionItems: CollectionGridItem[] = store.allCollections.map((collection) => ({
    ...collection,
    gameCount: visibleCollectionCounts.get(normalizeCollectionNameForMatch(collection.name)) ?? 0,
  }));

  renderCollectionGrid({
    container: libraryGridElement,
    collections: collectionItems,
    favoritesCount,
    hiddenCount,
    onCreateCollection: () => {
      void createCollectionFromGrid();
    },
    onRenameCollection: (collection) => {
      void renameCollectionFromGrid(collection);
    },
    onDeleteCollection: (collection) => {
      void deleteCollectionFromGrid(collection);
    },
    onSelectFavorites: () => {
      setLibraryViewMode("favorites", false);
      filterPanel.setCollectionFilter("", false);
      filterPanel.setFilterBy("all", false);
      renderGameLibrary();
    },
    onSelectHidden: () => {
      setLibraryViewMode("games", false);
      filterPanel.setFilterBy("all", false);
      const appliedHiddenFilter = filterPanel.setCollectionFilter(HIDDEN_GAMES_COLLECTION_NAME, false);
      if (!appliedHiddenFilter) {
        filterPanel.setCollectionFilter("", false);
      }
      renderGameLibrary();
    },
    onSelectCollection: (collection) => {
      setLibraryViewMode("games", false);
      filterPanel.setFilterBy("all", false);
      const appliedCollectionFilter = filterPanel.setCollectionFilter(collection.name, false);
      if (!appliedCollectionFilter) {
        filterPanel.setCollectionFilter("", false);
      }
      renderGameLibrary();
    },
  });

  const collectionCount = store.allCollections.length + (hiddenCount > 0 ? 1 : 0);
  setLibrarySummary(`${collectionCount} collection${collectionCount === 1 ? "" : "s"}.`);
};

const renderActiveLibraryView = (): void => {
  if (isCollectionLibraryViewMode(store.activeLibraryViewMode)) {
    renderCollectionLibrary();
    return;
  }

  renderGameLibrary();
};

const libraryViewOptionButtons = Array.from(
  libraryViewPickerElement.querySelectorAll(".library-view-picker-option")
).filter((option): option is HTMLButtonElement => option instanceof HTMLButtonElement);
if (libraryViewOptionButtons.length === 0) {
  throw new Error("Library view picker is missing options");
}

const setLibraryViewMode = (viewMode: LibraryViewMode, render = true): void => {
  store.activeLibraryViewMode = viewMode;

  for (const optionButton of libraryViewOptionButtons) {
    const optionViewMode = optionButton.dataset.libraryView;
    const isSelected = optionViewMode === viewMode;
    optionButton.classList.toggle("is-selected", isSelected);
    optionButton.setAttribute("aria-selected", `${isSelected}`);
    optionButton.tabIndex = isSelected ? 0 : -1;
  }

  if (render) {
    renderActiveLibraryView();
  }
};

const setLibraryViewModeFromOptionButton = (optionButton: HTMLButtonElement): void => {
  const optionViewMode = optionButton.dataset.libraryView;
  if (!isLibraryViewMode(optionViewMode)) {
    return;
  }

  setLibraryViewMode(optionViewMode);
  optionButton.focus();
};

const filterPanel = createFilterPanel(filterPanelElement, () => {
  if (isGameLibraryViewMode(store.activeLibraryViewMode)) {
    renderGameLibrary();
  }
});

const listGameLanguagesForGame = async (game: GameResponse): Promise<string[]> => {
  try {
    return await ipcService.listGameLanguages({
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return [];
  }
};

const listGameCompatibilityToolsForGame = async (
  game: GameResponse
): Promise<GameCompatibilityToolOption[]> => {
  try {
    return await ipcService.listGameCompatibilityTools({
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return [];
  }
};

const listGameVersionBetasForGame = async (game: GameResponse): Promise<GameVersionBetasPayload> => {
  try {
    return await ipcService.listGameVersionBetas({
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return {
      options: [],
      warning: "Could not load beta branch metadata from Steam.",
    };
  }
};

const validateGameBetaAccessCodeForGame = async (
  game: GameResponse,
  accessCode: string
): Promise<GameBetaAccessCodeValidationResult> => {
  try {
    return await ipcService.validateGameBetaAccessCode({
      provider: game.provider,
      externalId: game.externalId,
      accessCode,
    });
  } catch {
    return {
      valid: false,
      message: "Could not validate this code right now.",
    };
  }
};

const getGamePrivacySettingsForGame = async (game: GameResponse): Promise<GamePrivacySettingsPayload | null> => {
  try {
    return await ipcService.getGamePrivacySettings({
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return null;
  }
};

const setGamePrivacySettingsForGame = async (
  game: GameResponse,
  settings: Pick<GamePrivacySettings, "hideInLibrary" | "markAsPrivate">
): Promise<void> => {
  await ipcService.setGamePrivacySettings({
    provider: game.provider,
    externalId: game.externalId,
    hideInLibrary: settings.hideInLibrary,
    markAsPrivate: settings.markAsPrivate,
  });
};

const updateGamePrivacySettingsForGame = async (
  game: GameResponse,
  settings: Partial<Pick<GamePrivacySettings, "hideInLibrary" | "markAsPrivate">>
): Promise<void> => {
  const currentSettings = await getGamePrivacySettingsForGame(game);
  await setGamePrivacySettingsForGame(game, {
    hideInLibrary: settings.hideInLibrary ?? currentSettings?.hideInLibrary ?? false,
    markAsPrivate: settings.markAsPrivate ?? currentSettings?.markAsPrivate ?? false,
  });
};

const clearGameOverlayDataForGame = async (game: GameResponse): Promise<void> => {
  await ipcService.clearGameOverlayData({
    provider: game.provider,
    externalId: game.externalId,
  });
};

const getGameInstallationDetailsForGame = async (game: GameResponse): Promise<GameInstallationDetailsPayload | null> => {
  try {
    return await ipcService.getGameInstallationDetails({
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return null;
  }
};

const getGameCustomizationArtworkForGame = async (
  game: GameResponse
): Promise<GameCustomizationArtworkPayload | null> => {
  try {
    return await ipcService.getGameCustomizationArtwork({
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return null;
  }
};

const listGameInstallLocationsForGame = async (game: GameResponse): Promise<GameInstallLocationPayload[]> => {
  try {
    return await ipcService.listGameInstallLocations({
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return [];
  }
};

const getGameInstallSizeEstimateForGame = async (game: GameResponse): Promise<number | null> => {
  try {
    return await ipcService.getGameInstallSizeEstimate({
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return null;
  }
};

const listSteamDownloadsForSession = async (): Promise<SteamDownloadProgressPayload[]> => {
  try {
    return await ipcService.listSteamDownloads();
  } catch (error) {
    console.error("Could not load Steam downloads.", error);
    return [];
  }
};

const isLikelyCompletedDownload = (download: SteamDownloadProgressPayload): boolean => {
  const normalizedPercent = normalizeDownloadPercent(download);
  if (normalizedPercent !== null && normalizedPercent >= 99.5) {
    return true;
  }

  if (
    isFiniteNonNegativeNumber(download.bytesDownloaded)
    && isFiniteNonNegativeNumber(download.bytesTotal)
    && download.bytesTotal > 0
    && download.bytesDownloaded >= download.bytesTotal
  ) {
    return true;
  }

  return false;
};

const markCompletedDownloadsAsInstalled = (downloads: SteamDownloadProgressPayload[]): void => {
  let didUpdateInstalledState = false;

  for (const download of downloads) {
    let updatedGame = updateGameInState(download.gameId, (existingGame) => ({
      ...existingGame,
      installed: true,
    }));

    if (!updatedGame) {
      const fallbackGame = store.allGames.find((game) =>
        game.provider === download.provider
        && game.externalId === download.externalId
      );
      if (fallbackGame) {
        updatedGame = updateGameInState(fallbackGame.id, (existingGame) => ({
          ...existingGame,
          installed: true,
        }));
      }
    }

    if (updatedGame) {
      didUpdateInstalledState = true;
    }
  }

  if (didUpdateInstalledState) {
    renderActiveLibraryView();
  }
};

const scheduleLibraryRefreshAfterDownloadCompletion = (): void => {
  if (store.downloadCompletionRefreshTimer !== null) {
    return;
  }

  // Trigger a refresh immediately (don't gate on visibility/focus) so that
  // completed downloads are reflected in the UI without requiring a manual
  // library sync. `refreshLibrary` already guards against concurrent loads.
  store.downloadCompletionRefreshTimer = window.setTimeout(() => {
    store.downloadCompletionRefreshTimer = null;
    void refreshLibrary(true);
  }, 0);
};

const refreshSteamDownloads = async (): Promise<void> => {
  if (!store.steamLinked || store.isDownloadPollInFlight) {
    return;
  }

  store.isDownloadPollInFlight = true;
  try {
    const latestDownloads = await listSteamDownloadsForSession();
    const latestDownloadsByKey = new Map<string, SteamDownloadProgressPayload>();
    for (const download of latestDownloads) {
      latestDownloadsByKey.set(getDownloadEtaKey(download), download);
    }

    const completedDownloads: SteamDownloadProgressPayload[] = [];
    for (const [previousKey, previousDownload] of store.previousActiveDownloadsByKey) {
      if (latestDownloadsByKey.has(previousKey)) {
        continue;
      }
      if (isLikelyCompletedDownload(previousDownload)) {
        completedDownloads.push(previousDownload);
      }
    }

    store.activeDownloads = latestDownloads;
    store.previousActiveDownloadsByKey = latestDownloadsByKey;
    updateDownloadEtaSnapshots(store.activeDownloads);
    if (completedDownloads.length > 0) {
      markCompletedDownloadsAsInstalled(completedDownloads);
      scheduleLibraryRefreshAfterDownloadCompletion();
    }
  } finally {
    store.isDownloadPollInFlight = false;
    renderDownloadActivity();
  }
};

const stopDownloadPolling = (): void => {
  if (store.downloadPollTimer !== null) {
    window.clearInterval(store.downloadPollTimer);
    store.downloadPollTimer = null;
  }
  store.isDownloadPollInFlight = false;
};

const startDownloadPolling = (): void => {
  stopDownloadPolling();
  void refreshSteamDownloads();
  store.downloadPollTimer = window.setInterval(() => {
    void refreshSteamDownloads();
  }, DOWNLOAD_POLL_INTERVAL_MS);
};

const getGamePropertiesSettingsForGame = async (
  game: GameResponse
): Promise<GamePropertiesPersistedSettings | null> => {
  try {
    return await ipcService.getGamePropertiesSettings({
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return null;
  }
};

const setGamePropertiesSettingsForGame = async (
  game: GameResponse,
  settings: GamePropertiesPersistedSettings
): Promise<void> => {
  await ipcService.setGamePropertiesSettings({
    provider: game.provider,
    externalId: game.externalId,
    settings,
  });
};

const browseGameInstalledFilesForGame = async (game: GameResponse): Promise<void> => {
  await ipcService.browseGameInstalledFiles({
    provider: game.provider,
    externalId: game.externalId,
  });
};

const backupGameFilesForGame = async (game: GameResponse): Promise<void> => {
  await ipcService.backupGameFiles({
    provider: game.provider,
    externalId: game.externalId,
  });
};

const verifyGameFilesForGame = async (game: GameResponse): Promise<void> => {
  await ipcService.verifyGameFiles({
    provider: game.provider,
    externalId: game.externalId,
  });
};

const addGameDesktopShortcutForGame = async (game: GameResponse): Promise<void> => {
  await ipcService.addGameDesktopShortcut({
    provider: game.provider,
    externalId: game.externalId,
  });
};

const openGameRecordingSettingsForGame = async (game: GameResponse): Promise<void> => {
  await ipcService.openGameRecordingSettings({
    provider: game.provider,
    externalId: game.externalId,
  });
};

const uninstallGameForGame = async (game: GameResponse): Promise<void> => {
  await ipcService.uninstallGame({
    provider: game.provider,
    externalId: game.externalId,
  });
};

const openGameProperties = async (game: GameResponse): Promise<void> => {
  const [
    collections,
    availableLanguages,
    availableCompatibilityTools,
    versionBetasPayload,
    privacySettings,
    installationDetails,
    customizationArtwork,
    persistedSettings,
  ] = await Promise.all([
    listCollectionsForGame(game),
    listGameLanguagesForGame(game),
    listGameCompatibilityToolsForGame(game),
    listGameVersionBetasForGame(game),
    getGamePrivacySettingsForGame(game),
    getGameInstallationDetailsForGame(game),
    getGameCustomizationArtworkForGame(game),
    getGamePropertiesSettingsForGame(game),
  ]);
  gamePropertiesPanel.open({
    game,
    collections: collections
      .filter((collection) => collection.containsGame)
      .map((collection) => collection.name),
    availableLanguages,
    availableCompatibilityTools,
    availableVersionOptions: versionBetasPayload.options,
    availableVersionOptionsWarning: versionBetasPayload.warning,
    persistedSettings: persistedSettings ?? undefined,
    saveSettings: async (settings) => {
      await setGamePropertiesSettingsForGame(game, settings);
      // Notify listeners that customization/settings may have changed for this game
      try {
        window.dispatchEvent(new CustomEvent("game-customization-changed", { detail: { gameId: game.id } }));
      } catch {
        // ignore
      }
    },
    installationDetails: installationDetails ?? undefined,
    customizationArtworkPaths: customizationArtwork ?? undefined,
    browseInstalledFiles: async () => {
      await browseGameInstalledFilesForGame(game);
    },
    backupInstalledFiles: async () => {
      await backupGameFilesForGame(game);
    },
    verifyInstalledFiles: async () => {
      await verifyGameFilesForGame(game);
    },
    privacySettings: privacySettings ?? undefined,
    setPrivacySettings: async (settings) => {
      await setGamePrivacySettingsForGame(game, settings);
      updateGameInState(game.id, (existingGame) => ({
        ...existingGame,
        hideInLibrary: settings.hideInLibrary,
      }));
      updateCollectionSuggestions();
      renderActiveLibraryView();
    },
    deleteOverlayData: async () => {
      await clearGameOverlayDataForGame(game);
    },
    validateBetaAccessCode: async (accessCode: string) => {
      return validateGameBetaAccessCodeForGame(game, accessCode);
    },
    openGameRecordingSettings: async () => {
      await openGameRecordingSettingsForGame(game);
    },
  });
};

const gameContextMenu = createGameContextMenu({
  actions: {
    addGameToCollection: async (game, collectionId) => {
      await ipcService.addGameToCollection({
        collectionId,
        provider: game.provider,
        externalId: game.externalId,
      });
      await syncCollectionStateForGame(game);
      const targetCollectionName = store.allCollections.find((collection) => collection.id === collectionId)?.name;
      if (targetCollectionName) {
        showLauncherToast(`Added "${game.name}" to "${targetCollectionName}".`);
      } else {
        showLauncherToast(`Updated collections for "${game.name}".`);
      }
      void refreshLibrary(false);
    },
    createCollectionAndAdd: async (game, name) => {
      const createdCollection = await ipcService.createCollection({ name });
      await ipcService.addGameToCollection({
        collectionId: createdCollection.id,
        provider: game.provider,
        externalId: game.externalId,
      });
      await syncCollectionStateForGame(game);
      showLauncherToast(`Created "${createdCollection.name}" and added "${game.name}".`);
      void refreshLibrary(false);
    },
    addDesktopShortcut: async (game) => {
      if (!game.installed) {
        showLauncherToast(`"${game.name}" is not currently installed.`, "error");
        return;
      }

      await addGameDesktopShortcutForGame(game);
      showLauncherToast(`Added desktop shortcut for "${game.name}".`);
    },
    backupGameFiles: async (game) => {
      if (!game.installed) {
        showLauncherToast(`"${game.name}" is not currently installed.`, "error");
        return;
      }

      await backupGameFilesForGame(game);
      showLauncherToast(`Opened backup flow for "${game.name}".`);
    },
    browseLocalFiles: async (game) => {
      if (!game.installed) {
        showLauncherToast(`"${game.name}" is not currently installed.`, "error");
        return;
      }

      await browseGameInstalledFilesForGame(game);
      showLauncherToast(`Opened local files for "${game.name}".`);
    },
    hideGameInLibrary: async (game) => {
      await updateGamePrivacySettingsForGame(game, {
        hideInLibrary: true,
      });
      updateGameInState(game.id, (existingGame) => ({
        ...existingGame,
        hideInLibrary: true,
      }));
      updateCollectionSuggestions();
      renderActiveLibraryView();
      showLauncherToast(`"${game.name}" is now hidden in your library.`);
    },
    unhideGameInLibrary: async (game) => {
      await updateGamePrivacySettingsForGame(game, {
        hideInLibrary: false,
      });
      updateGameInState(game.id, (existingGame) => ({
        ...existingGame,
        hideInLibrary: false,
      }));
      updateCollectionSuggestions();
      renderActiveLibraryView();
      showLauncherToast(`"${game.name}" has been removed from hidden games.`);
    },
    installGame: async (game) => {
      const [installLocations, installSizeBytes] = await Promise.all([
        listGameInstallLocationsForGame(game),
        getGameInstallSizeEstimateForGame(game),
      ]);
      const installRequest = await installDialog.open({
        game,
        locations: installLocations,
        installSizeBytes: typeof installSizeBytes === "number" ? installSizeBytes : undefined,
      });
      if (installRequest === null) {
        return;
      }

      await ipcService.installGame({
        provider: game.provider,
        externalId: game.externalId,
        installPath: installRequest.installPath,
        createDesktopShortcut: installRequest.createDesktopShortcut,
        createApplicationShortcut: installRequest.createApplicationShortcut,
      });
      showLauncherToast(`Queued "${game.name}" for install.`);
      void refreshSteamDownloads();
    },
    listCollections: listCollectionsForGame,
    markGamePrivate: async (game) => {
      await updateGamePrivacySettingsForGame(game, {
        markAsPrivate: true,
      });
      showLauncherToast(`"${game.name}" is now marked private.`);
    },
    openProperties: openGameProperties,
    setCustomArtwork: async (game) => {
      await openGameProperties(game);
    },
    playGame: async (game) => {
      await ipcService.playGame({
        provider: game.provider,
        externalId: game.externalId,
      });
    },
    setFavorite: async (game, favorite) => {
      await ipcService.setGameFavorite({
        favorite,
        provider: game.provider,
        externalId: game.externalId,
      });
      updateGameInState(game.id, (existingGame) => ({ ...existingGame, favorite }));
      renderGameLibrary();
    },
    uninstallGame: async (game) => {
      if (!game.installed) {
        showLauncherToast(`"${game.name}" is not currently installed.`, "error");
        return;
      }

      const shouldUninstall = await confirmationDialog.open({
        title: "Uninstall Game",
        description: `Uninstall "${game.name}"? Local files will be removed, but the game stays in your library.`,
        confirmLabel: "Uninstall",
        confirmTone: "danger",
      });
      if (!shouldUninstall) {
        return;
      }

      await uninstallGameForGame(game);
      updateGameInState(game.id, (existingGame) => ({
        ...existingGame,
        installed: false,
      }));
      renderActiveLibraryView();
      showLauncherToast(`Opened uninstall flow for "${game.name}".`);

      let refreshCompleted = false;
      let refreshFallbackTimer: number | null = null;
      const runRefresh = (): void => {
        if (refreshCompleted) {
          return;
        }
        refreshCompleted = true;
        if (refreshFallbackTimer !== null) {
          window.clearTimeout(refreshFallbackTimer);
          refreshFallbackTimer = null;
        }
        void refreshLibrary(true);
      };

      const handleFocus = (): void => {
        runRefresh();
      };

      window.addEventListener("focus", handleFocus, { once: true });
      refreshFallbackTimer = window.setTimeout(() => {
        runRefresh();
      }, 20000);
    },
  },
  container: libraryGridElement,
  onError: (message) => {
    console.error(message);
    showLauncherToast(message, "error");
  },
  resolveGameFromCard,
});
store.closeGameContextMenu = gameContextMenu.closeMenu;

// Wire details action buttons to reuse existing actions where possible
if (detailsSettingsButton instanceof HTMLButtonElement) {
  detailsSettingsButton.addEventListener("click", (e) => {
    const gameId = store.selectedGameId;
    if (!gameId) return;
    const game = findGameById(gameId) ?? store.gameById.get(gameId);
    if (!game) return;
    // Open the context menu anchored to the settings button if supported
    if (typeof gameContextMenu.openMenu === "function") {
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      gameContextMenu.openMenu(game, e.currentTarget as HTMLElement, rect.left + 8, rect.top + 8);
      return;
    }
  });
}

if (detailsPlayButton instanceof HTMLButtonElement) {
  detailsPlayButton.addEventListener("click", async () => {
    const gameId = store.selectedGameId;
    if (!gameId) return;
    const game = findGameById(gameId) ?? store.gameById.get(gameId);
    if (!game) return;

    if (game.installed) {
      await ipcService.playGame({ provider: game.provider, externalId: game.externalId });
    } else {
      const installLocations = await listGameInstallLocationsForGame(game);
      const installSizeBytes = await getGameInstallSizeEstimateForGame(game);
      const installRequest = await installDialog.open({ game, locations: installLocations, installSizeBytes: typeof installSizeBytes === "number" ? installSizeBytes : undefined });
      if (installRequest === null) return;
      await ipcService.installGame({ provider: game.provider, externalId: game.externalId, installPath: installRequest.installPath, createDesktopShortcut: installRequest.createDesktopShortcut, createApplicationShortcut: installRequest.createApplicationShortcut });
      showLauncherToast(`Queued "${game.name}" for install.`);
      void refreshSteamDownloads();
    }
  });
}

if (detailsFavoriteButton instanceof HTMLButtonElement) {
  detailsFavoriteButton.addEventListener("click", async () => {
    const gameId = store.selectedGameId;
    if (!gameId) return;
    const game = findGameById(gameId) ?? store.gameById.get(gameId);
    if (!game) return;
    const newFav = !game.favorite;
    await ipcService.setGameFavorite({ favorite: newFav, provider: game.provider, externalId: game.externalId });
    updateGameInState(game.id, (existing) => ({ ...existing, favorite: newFav }));
    renderGameLibrary();
    renderGameDetails(game.id);
  });
}

// Details button click handler removed: keep tooltip but do not open properties panel here.

sessionAccountButton.addEventListener("click", () => {
  if (sessionAccountMenuElement.hidden) {
    openSessionAccountMenu();
    return;
  }

  closeSessionAccountMenu();
});

// Details dropdown: render and toggle when the Details button is clicked
const renderDetailsDropdown = (gameId: string): void => {
  const game = findGameById(gameId) ?? store.gameById.get(gameId) ?? null;
  if (!game) {
    detailsDropdown.innerHTML = "";
    return;
  }

  // left: cover (or header) + short description
  const coverCandidates = getSteamArtworkCandidates(game, "cover");
  const coverUrl = (coverCandidates && coverCandidates.length > 0) ? coverCandidates[0] : (game.artworkUrl ?? "");
  const headerImage = (game as any).headerImage ?? (game.headerImage ?? undefined);
  const left = document.createElement("div");
  left.className = "dd-left";
  const img = document.createElement("img");
  // Prefer cover art (portrait) for the thumbnail; fall back to enriched header image
  img.src = coverUrl || headerImage || "";
  img.alt = `${game.name} cover`;
  const desc = document.createElement("div");
  desc.className = "dd-desc";
  desc.textContent = (game as any).shortDescription ?? (game.shortDescription ?? (game as any).description ?? "");
  left.append(img, desc);

  // center: metadata (create early so async metadata merge can update these elements)
  const center = document.createElement("div");
  center.className = "dd-center";
  const developers = Array.isArray((game as any).developers) ? (game as any).developers.join(", ") : (game.developers ? game.developers.join(", ") : "-");
  const publishers = Array.isArray((game as any).publishers) ? (game as any).publishers.join(", ") : (game.publishers ? game.publishers.join(", ") : "-");
  const franText = (game as any).franchise ?? game.franchise ?? "-";
  const releaseText = (game as any).release_date ?? (game as any).releaseDate ?? game.releaseDate ?? "-";
  const dev = document.createElement("div"); dev.className = "meta-row"; dev.innerHTML = `<div class="meta-label">Developer</div><div>${escapeHtml(developers)}</div>`;
  const pub = document.createElement("div"); pub.className = "meta-row"; pub.innerHTML = `<div class="meta-label">Publisher</div><div>${escapeHtml(publishers)}</div>`;
  const fran = document.createElement("div"); fran.className = "meta-row"; fran.innerHTML = `<div class="meta-label">Franchise</div><div>${escapeHtml(String(franText))}</div>`;
  const rel = document.createElement("div"); rel.className = "meta-row"; rel.innerHTML = `<div class="meta-label">Release Date</div><div>${escapeHtml(String(releaseText))}</div>`;
  center.append(dev, pub, fran, rel);

  // If store metadata is missing or stale in the frontend, fetch it on-demand
  (async () => {
    try {
      const meta = await import("./storeMetadata").then(m => m.fetchGameStoreMetadata(game.provider, game.externalId));
      if (meta) {
        console.debug("fetchGameStoreMetadata result:", meta);
        // merge into displayed elements
        if (!desc.textContent || desc.textContent.trim().length === 0) {
          desc.textContent = meta.shortDescription ?? "";
        }
        // update center fields if present: developer, publisher, franchise, release date
        const devEl = center.querySelector('.meta-row:nth-child(1) > div:nth-child(2)');
        if (devEl && meta.developers) devEl.textContent = meta.developers.join(', ');
        const pubEl = center.querySelector('.meta-row:nth-child(2) > div:nth-child(2)');
        if (pubEl && meta.publishers) pubEl.textContent = meta.publishers.join(', ');
        const franEl = center.querySelector('.meta-row:nth-child(3) > div:nth-child(2)');
        if (franEl) {
          const rawFr = (meta.franchise && meta.franchise.toString().trim().length > 0) ? meta.franchise : undefined;
          // Heuristic fallback: try to infer franchise from the game name (prefix before ':' or ' - ')
          let inferredFr: string | undefined = undefined;
          if (!rawFr) {
            try {
              const title = (game && game.name) ? String(game.name) : "";
              if (title.includes(":")) {
                inferredFr = title.split(":")[0].trim();
              } else if (title.includes(" - ")) {
                inferredFr = title.split(" - ")[0].trim();
              } else if (title.includes(" — ")) {
                inferredFr = title.split(" — ")[0].trim();
              }
              if (inferredFr && inferredFr.length === 0) inferredFr = undefined;
            } catch {
              inferredFr = undefined;
            }
          }
          // Fallback order: explicit franchise -> inferred from title -> publisher name -> '-'
          let publisherFallback: string | undefined = undefined;
          if (meta && Array.isArray((meta as any).publishers) && (meta as any).publishers.length > 0) {
            const p = (meta as any).publishers[0];
            if (p && String(p).trim().length > 0) publisherFallback = String(p).trim();
          }
          if (!publisherFallback && Array.isArray((game as any).publishers) && (game as any).publishers.length > 0) {
            const p = (game as any).publishers[0];
            if (p && String(p).trim().length > 0) publisherFallback = String(p).trim();
          }

          const franchiseVal = rawFr ?? inferredFr ?? publisherFallback;
          franEl.textContent = franchiseVal ?? "-";
          if (!rawFr && franchiseVal) console.debug("franchise filled from fallback:", franchiseVal);
        }
        const relEl = center.querySelector('.meta-row:nth-child(4) > div:nth-child(2)');
        if (relEl) relEl.textContent = meta.releaseDate ?? "-";
      }
    } catch (err) {
      // ignore
    }
  })();



  // right: features
  const right = document.createElement("div");
  right.className = "dd-right";
  // Render normalized features: prefer `game.features`, fall back to inferred flags. If frontend metadata is missing
  // we'll fetch it in the background and update the right column dynamically.
  const featureList: Array<{ key: string; label: string; icon?: string | null; tooltip?: string | null }> = [];
  if (Array.isArray((game as any).features) && (game as any).features.length > 0) {
    for (const f of (game as any).features) featureList.push({ key: f.key, label: f.label, icon: f.icon ?? null, tooltip: f.tooltip ?? null });
  }

  // fallback: render some basic inferred flags if none present
  if (featureList.length === 0) {
    if ((game as any).hasAchievements || game.hasAchievements) featureList.push({ key: "achievements", label: "Achievements", icon: "trophy" });
    if ((game as any).hasCloudSaves || game.hasCloudSaves) featureList.push({ key: "cloud-saves", label: "Cloud Saves", icon: "cloud" });
    const ctrlVal = (game as any).controllerSupport ?? game.controllerSupport;
    if (ctrlVal) featureList.push({ key: "controller-support", label: `Controller: ${ctrlVal}`, icon: "gamepad" });
  }

  for (const f of featureList) {
    const row = document.createElement("div"); row.className = `feature ${f.key}`;
    const label = escapeHtml(String(f.label ?? ""));
    const title = f.tooltip ? escapeHtml(String(f.tooltip)) : "";
    row.innerHTML = `<span class="dot" aria-hidden="true"></span><div title="${title}">${label}</div>`;
    right.append(row);
  }

  // If we don't have frontend metadata, fetch it and update the right column
  if ((!Array.isArray((game as any).features) || (game as any).features.length === 0)) {
    import("./storeMetadata").then(m => m.fetchGameStoreMetadata(game.provider, game.externalId)).then((meta) => {
      if (meta && Array.isArray((meta as any).features) && (meta as any).features.length > 0) {
        // clear and re-render
        right.innerHTML = "";
        for (const f of (meta as any).features) {
          const row = document.createElement("div"); row.className = `feature ${f.key}`;
          const label = escapeHtml(String(f.label ?? ""));
          const title = f.tooltip ? escapeHtml(String(f.tooltip)) : "";
          row.innerHTML = `<span class="dot" aria-hidden="true"></span><div title="${title}">${label}</div>`;
          right.append(row);
        }
      }
    }).catch(() => {});
  }

  detailsDropdown.replaceChildren(left, center, right);
};

const closeDetailsDropdown = (): void => {
  detailsDropdown.hidden = true;
  detailsDropdown.setAttribute("aria-hidden", "true");
  detailsPropertiesButton.setAttribute("aria-expanded", "false");
  document.body.classList.remove("details-dropdown-open");
};

const openDetailsDropdown = (gameId: string): void => {
  renderDetailsDropdown(gameId);
  detailsDropdown.hidden = false;
  detailsDropdown.setAttribute("aria-hidden", "false");
  detailsPropertiesButton.setAttribute("aria-expanded", "true");
  document.body.classList.add("details-dropdown-open");
};

// Toggle handler
detailsPropertiesButton.addEventListener("click", (ev) => {
  ev.stopPropagation();
  const gameId = store.selectedGameId;
  if (!gameId) return;
  if (detailsDropdown.hidden) {
    openDetailsDropdown(gameId);
  } else {
    closeDetailsDropdown();
  }
});

// Close on outside click
document.addEventListener("click", (ev) => {
  if (detailsDropdown.hidden) return;
  const target = ev.target as Node | null;
  if (!target) return;
  if (!detailsDropdown.contains(target) && target !== detailsPropertiesButton) {
    closeDetailsDropdown();
  }
});


sessionAccountButton.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    closeSessionAccountMenu();
    return;
  }

  if (event.key === "ArrowDown" || event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    openSessionAccountMenu();
    const firstActionItem = getSessionMenuActionItems()[0];
    firstActionItem?.focus();
  }
});

sessionAccountMenuElement.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    event.preventDefault();
    closeSessionAccountMenu();
    sessionAccountButton.focus();
    return;
  }

  if (event.key === "Tab") {
    closeSessionAccountMenu();
    return;
  }

  const actionItems = getSessionMenuActionItems();
  if (actionItems.length === 0) {
    return;
  }

  const activeElement = document.activeElement;
  if (!(activeElement instanceof HTMLButtonElement)) {
    return;
  }

  const focusedIndex = actionItems.indexOf(activeElement);
  if (focusedIndex < 0) {
    return;
  }

  if (event.key === "ArrowDown") {
    event.preventDefault();
    const nextIndex = (focusedIndex + 1) % actionItems.length;
    actionItems[nextIndex].focus();
    return;
  }

  if (event.key === "ArrowUp") {
    event.preventDefault();
    const previousIndex = (focusedIndex - 1 + actionItems.length) % actionItems.length;
    actionItems[previousIndex].focus();
    return;
  }

  if (event.key === "Home") {
    event.preventDefault();
    actionItems[0].focus();
    return;
  }

  if (event.key === "End") {
    event.preventDefault();
    actionItems[actionItems.length - 1].focus();
  }
});

for (const optionButton of libraryViewOptionButtons) {
  optionButton.addEventListener("click", () => {
    closeSessionAccountMenu();
    setLibraryViewModeFromOptionButton(optionButton);
  });
}

libraryViewPickerElement.addEventListener("keydown", (event) => {
  const activeElement = document.activeElement;
  const focusedIndex = activeElement instanceof HTMLButtonElement
    ? libraryViewOptionButtons.indexOf(activeElement)
    : -1;
  if (focusedIndex < 0) {
    return;
  }

  let nextIndex = focusedIndex;
  if (event.key === "ArrowRight" || event.key === "ArrowDown") {
    event.preventDefault();
    nextIndex = focusedIndex + 1;
  } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
    event.preventDefault();
    nextIndex = focusedIndex - 1;
  } else if (event.key === "Home") {
    event.preventDefault();
    nextIndex = 0;
  } else if (event.key === "End") {
    event.preventDefault();
    nextIndex = libraryViewOptionButtons.length - 1;
  } else {
    return;
  }

  closeSessionAccountMenu();
  const nextButton = libraryViewOptionButtons[(nextIndex + libraryViewOptionButtons.length) % libraryViewOptionButtons.length];
  if (nextButton) {
    setLibraryViewModeFromOptionButton(nextButton);
  }
});

document.addEventListener("pointerdown", (event) => {
  const target = event.target;
  if (!(target instanceof Node)) {
    return;
  }

  if (!sessionAccountElement.contains(target)) {
    closeSessionAccountMenu();
  }
});

sessionAccountManageButton.addEventListener("click", () => {
  closeSessionAccountMenu();
});

sessionAccountSignOutButton.addEventListener("click", () => {
  closeSessionAccountMenu();
  void (async () => {
    try {
      await ipcService.logout();
      window.location.replace("/index.html");
    } catch (error) {
      const appError = normalizeAppError(error, "Could not sign out.");
      console.error(`[auth/logout] ${appError.kind}:${appError.code} ${appError.message}`);
    }
  })();
});

const refreshLibrary = async (syncBeforeLoad = false, importSteamCollections = false): Promise<void> => {
  if (store.isLoadingLibrary) {
    return;
  }

  try {
    setLibraryLoadingState(true);

    if (syncBeforeLoad && store.steamLinked) {
      try {
        await ipcService.syncSteamLibrary();
      } catch (error) {
        const appError = normalizeAppError(error, "Steam sync failed. Loading cached library.");
        console.error(`[library/sync] ${appError.kind}:${appError.code} ${appError.message}`);
      }

      if (importSteamCollections) {
        try {
          await ipcService.importSteamCollections();
        } catch (error) {
          const appError = normalizeAppError(error, "Steam collection import failed.");
          console.error(`[collections/import_steam] ${appError.kind}:${appError.code} ${appError.message}`);
        }
      }

      // Kick off a non-blocking local Steam install scan in the background
      try {
        void ipcService.startLocalSteamScan();
      } catch (error) {
        const appError = normalizeAppError(error, "Could not start local Steam install scan.");
        console.warn(`[local-scan/start] ${appError.kind}:${appError.code} ${appError.message}`);
      }
    }

    const [library, collections] = await Promise.all([
      ipcService.getLibrary(),
      listCollectionsForUser().catch(() => []),
    ]);
    // (removed debug log)
    setAllGames(library.games);
    setAllCollections(collections);
    renderActiveLibraryView();
    markLibraryAsUpdatedNow();
  } catch (error) {
    setAllGames([]);
    setAllCollections([]);
    if (store.activeLibraryViewMode === "collections") {
      renderCollectionLibrary();
      setLibrarySummary("Could not load your collections.");
    } else {
      renderGameGrid({
        container: libraryGridElement,
        games: [],
        emptyMessage: "Could not load your library.",
      });
      setLibrarySummary("Could not load your library.");
    }
    librarySummaryElement.classList.add("status-error");
    const appError = normalizeAppError(error, "Could not load library.");
    console.error(`[library/load] ${appError.kind}:${appError.code} ${appError.message}`);
  } finally {
    setLibraryLoadingState(false);
  }
};

const refreshSession = async (): Promise<boolean> => {
  try {
    const session = await ipcService.getSession();
    if (!session) {
      window.location.replace("/index.html");
      return false;
    }

    setSessionStatus(session.steamLinked);
    return true;
  } catch (error) {
    const appError = normalizeAppError(error, "Could not load session data.");
    console.error(`[session/load] ${appError.kind}:${appError.code} ${appError.message}`);
    setSessionStatus(false, true);
    return false;
  }
};

refreshLibraryButton.addEventListener("click", () => {
  void refreshLibrary(true, true);
});

window.addEventListener("resize", applyLibraryAspectSoftLock);
window.addEventListener("beforeunload", stopDownloadPolling);
window.addEventListener("beforeunload", stopLibraryLastUpdatedTimer);

const initialize = async (): Promise<void> => {
  applyLibraryAspectSoftLock();
  registerGridZoomShortcut();
  const cleanupGridWheelSmoothing = registerLinuxGridWheelSmoothing();
  window.addEventListener("beforeunload", cleanupGridWheelSmoothing, { once: true });
  setLibraryViewMode("games", false);
  setLibrarySummary("Loading library...");
  renderLibraryLastUpdated();
  renderDownloadActivity();

  const hasSession = await refreshSession();
  if (!hasSession) {
    return;
  }

  await refreshLibrary(true);
};

void initialize();
