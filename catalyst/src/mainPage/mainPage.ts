import type { CollectionGridItem } from "./components/collectionGrid";
import { createConfirmationDialog } from "./components/confirmationDialog";
import { createFilterPanel } from "./components/filterPanel";
import { createCollectionNameDialog } from "./components/collectionNameDialog";
import { createGameContextMenu } from "./components/gameContextMenu";
import { renderGameGrid } from "./components/gameGrid";
import { createReviewCard, createReviewPlaceholder, Review } from "./components/reviewCard";
import { createDetailsDropdownView } from "./detailsDropdownView";
import { createDownloadActivityView } from "./downloadActivityView";
import { createLibraryStatusView } from "./libraryStatusView";
import { loadReviewForGame, saveReviewForGame } from "./reviewStore";
import { createLibraryViewRenderer } from "./libraryViewRenderer";
import {
  createGamePropertiesPanel,
} from "./components/gamePropertiesPanel";
import type {
  GameBetaAccessCodeValidationResult,
  GameCompatibilityToolOption,
  GamePrivacySettings,
  GamePropertiesPersistedSettings,
} from "../shared/ipc/gamePropertiesTypes";
import {
  HIDDEN_GAMES_COLLECTION_NAME,
  type CollectionResponse,
  type GameResponse,
} from "./types";
import { convertFileSrc } from "@tauri-apps/api/core";
import { ipcService } from "../shared/ipc/client";
import { normalizeAppError } from "../shared/ipc/errors";
import {
  collectSteamTagSuggestions,
  formatLibraryRefreshAgeLabel,
  openSteamConnectedUrl,
} from "./libraryUiHelpers";
import {
  detailsViewStore,
  downloadStore,
  findGameById,
  isCollectionLibraryViewMode,
  isGameLibraryViewMode,
  isLibraryViewMode,
  libraryCatalogStore,
  librarySyncStore,
  libraryViewStore,
  sessionStore,
  type LibraryViewMode,
} from "./stores";
import type {
  GameAchievementsPayload,
  GameActivityTimelineItemPayload,
  GameActivityTimelinePayload,
  GameDlcPayload,
  GameFriendActivityEntryPayload,
  GameFriendsActivityPayload,
  GameCustomizationArtworkPayload,
  GameInstallationDetailsPayload,
  GamePrivacySettingsPayload,
  GameReviewPayload,
  GameTradingCardsPayload,
  GameScreenshotPayload,
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
const startupGateElement = document.getElementById("startup-gate");
const startupGateStatusElement = document.getElementById("startup-gate-status");
const startupStepSessionElement = document.getElementById("startup-step-session");
const startupStepConfigElement = document.getElementById("startup-step-config");
const startupStepLibraryElement = document.getElementById("startup-step-library");
const startupRetryButtonElement = document.getElementById("startup-gate-retry");

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
  || !(startupGateElement instanceof HTMLElement)
  || !(startupGateStatusElement instanceof HTMLElement)
  || !(startupStepSessionElement instanceof HTMLElement)
  || !(startupStepConfigElement instanceof HTMLElement)
  || !(startupStepLibraryElement instanceof HTMLElement)
  || !(startupRetryButtonElement instanceof HTMLButtonElement)
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

import { getSteamArtworkCandidates } from "./steamArtwork";
import { isFiniteNonNegativeNumber } from "../shared/utils/format";
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
const ENABLE_LINUX_GRID_WHEEL_SMOOTHING = false;
const GRID_CARD_WIDTH_STORAGE_KEY = "catalyst.library.gridCardMinWidthPx";
const APP_NAME = "Catalyst";
const DOWNLOAD_POLL_INTERVAL_MS = 2500;
const DOWNLOAD_COMPLETION_REFRESH_RETRY_DELAY_MS = 12_000;
const DOWNLOAD_COMPLETION_REFRESH_MAX_ATTEMPTS = 6;
const UNINSTALL_VERIFICATION_RETRY_DELAY_MS = 12_000;
const UNINSTALL_VERIFICATION_MAX_ATTEMPTS = 20;
const DOWNLOAD_ETA_SMOOTHING_FACTOR = 0.35;
const DOWNLOAD_ETA_SAMPLE_MIN_SECONDS = 0.5;
const DOWNLOAD_ETA_STALE_MS = 15000;
const TOAST_DURATION_MS = 3200;
const DLC_CDN_CAPSULE_URL = "https://cdn.cloudflare.steamstatic.com/steam/apps";
const STEAM_REVIEW_MISSING_WARNING_PATTERN = /no public steam review found/i;
const STARTUP_TIMEOUT_MS = 20_000;
const STARTUP_CLOSE_DURATION_MS = 320;
const LIBRARY_SOFT_LOCK_ASPECTS: ReadonlyArray<{ label: string; ratio: number }> = [
  { label: "16:9", ratio: 16 / 9 },
  { label: "21:9", ratio: 21 / 9 },
  { label: "32:9", ratio: 32 / 9 },
];

const clamp = (value: number, min: number, max: number): number => Math.min(max, Math.max(min, value));
// runtime platform alias used below
type RuntimePlatform = "windows" | "macos" | "linux" | "other";
type StartupStepState = "pending" | "active" | "done" | "error";
type SessionRefreshResult = "ready" | "redirecting" | "failed";
type AppHistoryState = {
  gameId?: string;
  view?: "game-details" | "library";
};

interface PendingUninstallVerification {
  externalId: string;
  gameId: string;
  provider: string;
}

let startupAttemptToken = 0;
let uninstallVerificationTimer: number | null = null;
let uninstallVerificationAttemptCount = 0;
let isUninstallVerificationInFlight = false;
const pendingUninstallVerificationByKey = new Map<string, PendingUninstallVerification>();

const runTaskWithTimeout = async <T>(
  task: Promise<T>,
  timeoutMs: number,
  timeoutMessage: string
): Promise<T> => {
  let timeoutId: number | null = null;
  const timeoutTask = new Promise<never>((_, reject) => {
    timeoutId = window.setTimeout(() => {
      reject(new Error(timeoutMessage));
    }, timeoutMs);
  });

  try {
    return await Promise.race([task, timeoutTask]);
  } finally {
    if (timeoutId !== null) {
      window.clearTimeout(timeoutId);
    }
  }
};

const waitForLibraryLoadToFinish = async (): Promise<void> => {
  if (!librarySyncStore.isLoadingLibrary) {
    return;
  }

  await new Promise<void>((resolve) => {
    const pollInterval = window.setInterval(() => {
      if (!librarySyncStore.isLoadingLibrary) {
        window.clearInterval(pollInterval);
        resolve();
      }
    }, 120);
  });
};

const getErrorMessage = (error: unknown, fallback: string): string => {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  if (
    typeof error === "object"
    && error !== null
    && "message" in error
    && typeof (error as { message?: unknown }).message === "string"
    && (error as { message: string }).message.trim()
  ) {
    return (error as { message: string }).message;
  }

  return fallback;
};

const setStartupStepState = (stepElement: HTMLElement, state: StartupStepState): void => {
  stepElement.dataset.state = state;
};

const setStartupStatus = (message: string, isError = false): void => {
  startupGateStatusElement.textContent = message;
  startupGateElement.classList.toggle("is-error", isError);
};

const showStartupGate = (): void => {
  startupGateElement.hidden = false;
  startupGateElement.classList.remove("is-closing", "is-error");
  startupRetryButtonElement.hidden = true;
  startupRetryButtonElement.disabled = false;
  document.body.classList.add("startup-pending");
};

const hideStartupGate = (): void => {
  startupGateElement.classList.remove("is-error");
  startupGateElement.classList.add("is-closing");
  document.body.classList.remove("startup-pending");
  window.setTimeout(() => {
    startupGateElement.hidden = true;
  }, STARTUP_CLOSE_DURATION_MS);
};

const resetStartupUi = (): void => {
  setStartupStepState(startupStepSessionElement, "pending");
  setStartupStepState(startupStepConfigElement, "pending");
  setStartupStepState(startupStepLibraryElement, "pending");
  setStartupStatus("Starting up...");
};

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
  sessionStore.steamLinked = steamConnected && !isError;
  sessionAccountLabelElement.textContent = APP_NAME;
  sessionAccountButton.classList.toggle("is-error", isError);
  sessionAccountManageButton.disabled = isError;
  sessionAccountSignOutButton.disabled = false;
  downloadActivityElement.classList.toggle("is-disabled", isError || !sessionStore.steamLinked);
  if (!sessionStore.steamLinked) {
    stopDownloadPolling();
    downloadStore.activeDownloads = [];
    downloadStore.previousActiveDownloadsByKey.clear();
    if (downloadStore.downloadCompletionRefreshTimer !== null) {
      window.clearTimeout(downloadStore.downloadCompletionRefreshTimer);
      downloadStore.downloadCompletionRefreshTimer = null;
    }
    downloadStore.pendingInstallVerificationByKey.clear();
    downloadStore.downloadCompletionRefreshAttemptCount = 0;
    downloadStore.isDownloadCompletionRefreshInFlight = false;
    pendingUninstallVerificationByKey.clear();
    uninstallVerificationAttemptCount = 0;
    isUninstallVerificationInFlight = false;
    if (uninstallVerificationTimer !== null) {
      window.clearTimeout(uninstallVerificationTimer);
      uninstallVerificationTimer = null;
    }
    let didClearUninstallingState = false;
    libraryCatalogStore.allGames = libraryCatalogStore.allGames.map((game) => {
      if (game.uninstalling !== true) {
        return game;
      }
      didClearUninstallingState = true;
      return {
        ...game,
        uninstalling: false,
      };
    });
    if (didClearUninstallingState) {
      libraryCatalogStore.gameById = new Map(
        libraryCatalogStore.allGames.map((game) => [game.id, game])
      );
    }
    downloadStore.downloadEtaByKey.clear();
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

const {
  renderLibraryLastUpdated,
  markLibraryAsUpdatedNow,
  stopLibraryLastUpdatedTimer,
  setLibraryLoadingState,
} = createLibraryStatusView({
  libraryLastUpdatedElement,
  refreshLibraryButton,
  refreshLibraryLabelElement,
  state: librarySyncStore,
  formatLibraryRefreshAgeLabel,
});

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
    detailsViewStore.preservedLibraryScrollTop = libraryGridElement.scrollTop;
  } catch {
    detailsViewStore.preservedLibraryScrollTop = 0;
  }
  detailsViewStore.preservedLibraryViewMode = libraryViewStore.activeLibraryViewMode;

  detailsViewStore.appViewMode = "game-details";
  detailsViewStore.selectedGameId = gameId;

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
  detailsViewStore.appViewMode = "library";
  detailsViewStore.selectedGameId = null;

  // Restore UI
  panelLeftElement.hidden = false;
  panelMiddleElement.hidden = false;
  libraryGridElement.hidden = false;
  gameDetailsShellElement.hidden = true;

  // Restore scroll and view mode
  try {
    libraryGridElement.scrollTop = detailsViewStore.preservedLibraryScrollTop ?? 0;
    libraryViewStore.activeLibraryViewMode = detailsViewStore.preservedLibraryViewMode ?? libraryViewStore.activeLibraryViewMode;
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
  const state = ev.state as AppHistoryState | null;
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
    if (gameId && detailsViewStore.appViewMode === "game-details" && detailsViewStore.selectedGameId === gameId) {
      renderGameDetails(gameId);
    }
  } catch {
    // ignore
  }
});

const getGameInstallStatusLabel = (game: GameResponse): string => {
  if (game.uninstalling === true) {
    return "Uninstalling...";
  }
  return game.installed ? "Installed" : "Not installed";
};

const getGamePrimaryActionLabel = (game: GameResponse): string => {
  if (game.uninstalling === true) {
    return "Uninstalling...";
  }
  return game.installed ? "Play" : "Install";
};

const syncDetailsInstallStatusUi = (): void => {
  if (detailsViewStore.appViewMode !== "game-details") {
    return;
  }

  const selectedGameId = detailsViewStore.selectedGameId;
  if (!selectedGameId) {
    return;
  }

  const selectedGame = findGameById(selectedGameId) ?? libraryCatalogStore.gameById.get(selectedGameId) ?? null;
  if (!selectedGame) {
    return;
  }

  detailsPlayButton.textContent = getGamePrimaryActionLabel(selectedGame);
  detailsPlayButton.disabled = selectedGame.uninstalling === true;

  const statusValue = document.getElementById("details-status-value");
  if (statusValue instanceof HTMLElement) {
    statusValue.textContent = getGameInstallStatusLabel(selectedGame);
  }
};

const renderGameDetails = (gameId: string, forceFriendsActivityRefresh = false): void => {
  console.debug("renderGameDetails called for", gameId);
  const game = findGameById(gameId) ?? libraryCatalogStore.gameById.get(gameId) ?? null;
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
  const playtimeLabel = typeof game.playtimeMinutes === "number"
    ? game.playtimeMinutes > 0
      ? `${(game.playtimeMinutes / 60).toFixed(1)} hours`
      : "Never played"
    : "-";

  const playCell = detailsTitleInfo.querySelector('.details-play-cell') ?? document.createElement('div');
  const lastPlayedDiv = document.createElement('div');
  lastPlayedDiv.innerHTML = `<strong>Last Played</strong><div class="muted">${lastPlayedLabel}</div>`;
  const playtimeDiv = document.createElement('div');
  playtimeDiv.innerHTML = `<strong>Playtime</strong><div class="muted">${playtimeLabel}</div>`;
  const statusDiv = document.createElement('div');
  const statusLabel = document.createElement("strong");
  statusLabel.textContent = "Status";
  const statusValue = document.createElement("div");
  statusValue.className = "muted";
  statusValue.id = "details-status-value";
  statusValue.textContent = getGameInstallStatusLabel(game);
  statusDiv.replaceChildren(statusLabel, statusValue);

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
  detailsPlayButton.textContent = getGamePrimaryActionLabel(game);
  detailsPlayButton.disabled = game.uninstalling === true;
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
  activitySection.className = "details-section details-activity-section";
  activitySection.innerHTML = `
    <h3>Activity</h3>
    <div class="details-subsection game-activity-timeline">
      <p class="placeholder">Loading recent activity…</p>
    </div>
  `;
  const activityTimelineSection = activitySection.querySelector(".game-activity-timeline");
  if (!(activityTimelineSection instanceof HTMLElement)) {
    throw new Error("Activity timeline section is missing");
  }

  const formatTimelineDateLabel = (isoDate: string): string => {
    const parsedDate = new Date(isoDate);
    if (Number.isNaN(parsedDate.valueOf())) {
      return "RECENT";
    }
    return new Intl.DateTimeFormat(undefined, {
      month: "long",
      day: "numeric",
    }).format(parsedDate).toLocaleUpperCase();
  };

  const formatTimelineTimeLabel = (isoDate: string): string => {
    const parsedDate = new Date(isoDate);
    if (Number.isNaN(parsedDate.valueOf())) {
      return "";
    }
    return new Intl.DateTimeFormat(undefined, {
      hour: "numeric",
      minute: "2-digit",
    }).format(parsedDate);
  };

  const renderActivityPlaceholder = (message: string): void => {
    activityTimelineSection.replaceChildren();
    const placeholder = document.createElement("p");
    placeholder.className = "placeholder";
    placeholder.textContent = message;
    activityTimelineSection.append(placeholder);
  };

  const createTimelineAchievementCard = (item: GameActivityTimelineItemPayload): HTMLElement => {
    const card = document.createElement("article");
    card.className = "activity-timeline-achievement";

    if (item.imageUrl?.trim()) {
      const image = document.createElement("img");
      image.className = "activity-timeline-achievement-icon";
      image.src = item.imageUrl.trim();
      image.alt = item.title;
      image.loading = "lazy";
      card.append(image);
    } else {
      const fallback = document.createElement("div");
      fallback.className = "activity-timeline-achievement-icon is-fallback";
      fallback.textContent = "A";
      card.append(fallback);
    }

    const content = document.createElement("div");
    content.className = "activity-timeline-achievement-content";
    const title = document.createElement("p");
    title.className = "activity-timeline-achievement-title";
    title.textContent = item.title;
    const description = document.createElement("p");
    description.className = "activity-timeline-achievement-description";
    description.textContent = item.description?.trim() || item.subtitle?.trim() || "Achievement unlocked";
    const time = document.createElement("p");
    time.className = "activity-timeline-achievement-time";
    time.textContent = formatTimelineTimeLabel(item.occurredAt);
    content.append(title, description, time);

    card.append(content);
    return card;
  };

  const createTimelineNewsCard = (
    item: GameActivityTimelineItemPayload,
    defaultWideCoverCandidates: string[]
  ): HTMLElement => {
    const isCompact = item.presentation === "compact";
    const card = document.createElement("article");
    card.className = `activity-timeline-news${item.isMajorUpdate ? " is-major-update" : ""}${isCompact ? " is-compact" : " is-featured"}`;

    const imageCandidates: string[] = [];
    const seenImageCandidates = new Set<string>();
    const addImageCandidate = (value?: string | null): void => {
      const trimmed = value?.trim();
      if (!trimmed || seenImageCandidates.has(trimmed)) {
        return;
      }
      seenImageCandidates.add(trimmed);
      imageCandidates.push(trimmed);

      const placeholderMatch = trimmed.match(/\{steam_clan_image\}\/([a-z0-9/_\-.]+)/i);
      if (placeholderMatch?.[1]) {
        const normalizedPath = placeholderMatch[1].replace(/^\/+/, "");
        for (const host of [
          "https://clan.akamai.steamstatic.com/images",
          "https://clan.cloudflare.steamstatic.com/images",
          "https://cdn.cloudflare.steamstatic.com/steamcommunity/public/images/clans",
        ]) {
          addImageCandidate(`${host}/${normalizedPath}`);
        }
      }

      const hostMatch = trimmed.match(
        /^https?:\/\/(?:clan\.(?:akamai|cloudflare)\.steamstatic\.com\/images|cdn\.cloudflare\.steamstatic\.com\/steamcommunity\/public\/images\/clans)\/([a-z0-9/_\-.]+)$/i
      );
      if (hostMatch?.[1]) {
        const normalizedPath = hostMatch[1].replace(/^\/+/, "");
        for (const host of [
          "https://clan.akamai.steamstatic.com/images",
          "https://clan.cloudflare.steamstatic.com/images",
          "https://cdn.cloudflare.steamstatic.com/steamcommunity/public/images/clans",
        ]) {
          addImageCandidate(`${host}/${normalizedPath}`);
        }
      }
    };

    if (!isCompact) {
      addImageCandidate(item.imageUrl);
      for (const candidate of defaultWideCoverCandidates) {
        addImageCandidate(candidate);
      }
    }

    if (!isCompact && imageCandidates.length > 0) {
      const image = document.createElement("img");
      image.className = "activity-timeline-news-image";
      image.alt = `${item.title} artwork`;
      image.loading = "lazy";
      let candidateIndex = 0;
      image.addEventListener("error", () => {
        candidateIndex += 1;
        if (candidateIndex < imageCandidates.length) {
          image.src = imageCandidates[candidateIndex] ?? "";
          return;
        }
        image.remove();
      });
      image.src = imageCandidates[candidateIndex] ?? "";
      card.append(image);
    }

    const content = document.createElement("div");
    content.className = "activity-timeline-news-content";
    const meta = document.createElement("p");
    meta.className = "activity-timeline-news-meta";
    meta.textContent = "NEWS";

    const link = item.url?.trim();
    if (link) {
      const titleLink = document.createElement("a");
      titleLink.className = "activity-timeline-news-title-link";
      titleLink.href = link;
      titleLink.target = "_blank";
      titleLink.rel = "noopener noreferrer";
      titleLink.textContent = item.title;
      content.append(meta, titleLink);
    } else {
      const title = document.createElement("h5");
      title.className = "activity-timeline-news-title";
      title.textContent = item.title;
      content.append(meta, title);
    }

    const description = item.description?.trim();
    if (description && !isCompact) {
      const body = document.createElement("p");
      body.className = "activity-timeline-news-description";
      body.textContent = description;
      content.append(body);
    }

    card.append(content);
    return card;
  };

  const renderActivityTimeline = (
    timeline: GameActivityTimelinePayload,
    defaultWideCoverCandidates: string[]
  ): void => {
    activityTimelineSection.replaceChildren();

    if (timeline.warning?.trim()) {
      const warning = document.createElement("p");
      warning.className = "activity-timeline-warning";
      warning.textContent = timeline.warning.trim();
      activityTimelineSection.append(warning);
    }

    if (!Array.isArray(timeline.items) || timeline.items.length === 0) {
      const emptyState = document.createElement("p");
      emptyState.className = "placeholder";
      emptyState.textContent = "No recent activity available for this title.";
      activityTimelineSection.append(emptyState);
      return;
    }

    const groupsByDay = new Map<string, GameActivityTimelineItemPayload[]>();
    for (const item of timeline.items) {
      const dateKey = new Date(item.occurredAt).toLocaleDateString("en-CA");
      const group = groupsByDay.get(dateKey);
      if (group) {
        group.push(item);
      } else {
        groupsByDay.set(dateKey, [item]);
      }
    }

    for (const [, dayItems] of groupsByDay) {
      const dayGroup = document.createElement("section");
      dayGroup.className = "activity-timeline-day";

      const dayLabel = document.createElement("p");
      dayLabel.className = "activity-timeline-day-label";
      dayLabel.textContent = formatTimelineDateLabel(dayItems[0]?.occurredAt ?? "");
      dayGroup.append(dayLabel);

      const achievementItems = dayItems.filter((item) => item.kind === "achievement");
      if (achievementItems.length > 0) {
        const achievementGroup = document.createElement("div");
        achievementGroup.className = "activity-timeline-achievement-group";

        const achievementHeader = document.createElement("p");
        achievementHeader.className = "activity-timeline-achievement-group-label";
        achievementHeader.textContent = achievementItems.length === 1
          ? "Unlocked 1 achievement"
          : `Unlocked ${achievementItems.length} achievements`;
        achievementGroup.append(achievementHeader);

        const achievementRow = document.createElement("div");
        achievementRow.className = "activity-timeline-achievement-row";
        for (const achievementItem of achievementItems) {
          achievementRow.append(createTimelineAchievementCard(achievementItem));
        }
        achievementGroup.append(achievementRow);
        dayGroup.append(achievementGroup);
      }

      const newsItems = dayItems.filter((item) => item.kind !== "achievement");
      for (const newsItem of newsItems) {
        dayGroup.append(createTimelineNewsCard(newsItem, defaultWideCoverCandidates));
      }

      activityTimelineSection.append(dayGroup);
    }
  };

  const resolveTimelineWideCoverCandidates = async (targetGame: GameResponse): Promise<string[]> => {
    const candidates: string[] = [];
    const seen = new Set<string>();

    const addCandidate = (value?: string | null): void => {
      const trimmed = value?.trim();
      if (!trimmed || seen.has(trimmed)) {
        return;
      }
      seen.add(trimmed);
      candidates.push(trimmed);
    };

    try {
      const customization = await getGameCustomizationArtworkForGame(targetGame);
      const localWideCoverPath = customization?.wideCover?.trim();
      if (localWideCoverPath && localWideCoverPath.length > 0) {
        try {
          addCandidate(convertFileSrc(localWideCoverPath));
        } catch {
          addCandidate(localWideCoverPath);
        }
      }
    } catch {
      // ignore and continue with Steam CDN candidates
    }

    try {
      const wideCoverCandidates = getSteamArtworkCandidates(targetGame, "wide-cover");
      for (const candidate of wideCoverCandidates) {
        addCandidate(candidate);
      }
    } catch {
      // ignore and continue with metadata fallback
    }

    addCandidate(targetGame.headerImage);
    addCandidate(targetGame.artworkUrl);

    return candidates;
  };

  // Achievements (placeholder-first)
  const achievementsSection = document.createElement("section");
  achievementsSection.className = "details-section achievements-section";
  achievementsSection.innerHTML = `
    <h4>Achievements</h4>
    <div class="achievements-summary">
      <p class="achievements-count placeholder">Achievements are not available yet. Coming soon.</p>
      <div class="achievements-progress">
        <div class="achievements-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="0">
          <div class="achievements-bar-fill" style="width:0%"></div>
        </div>
      </div>
      <div class="achievements-icons" aria-hidden="false"></div>
      <p class="achievements-view-link"><a href="#" target="_blank" rel="noopener noreferrer">View achievements</a></p>
    </div>
  `;

  const renderAchievementsPlaceholder = (message: string): void => {
    achievementsSection.replaceChildren();
    const heading = document.createElement("h4");
    heading.textContent = "Achievements";
    const placeholder = document.createElement("p");
    placeholder.className = "placeholder";
    placeholder.textContent = message;
    achievementsSection.append(heading, placeholder);
  };

  const renderAchievements = (payload: GameAchievementsPayload): void => {
    achievementsSection.replaceChildren();
    const heading = document.createElement("h4");
    heading.textContent = "Achievements";

    const entries = payload.entries ?? [];
    if (entries.length === 0) {
      const empty = document.createElement("p");
      empty.className = "placeholder";
      empty.textContent = payload.warning ?? "No achievements available for this title.";
      achievementsSection.append(heading, empty);
      return;
    }

    const summary = document.createElement("div");
    summary.className = "achievements-summary";

    const total = payload.total ?? 0;
    const unlocked = payload.unlockedCount ?? 0;
    const percent = payload.percent ?? (total > 0 ? (unlocked / total) * 100 : 0);

    const countP = document.createElement("p");
    countP.className = "achievements-count";
    countP.textContent = `You've unlocked ${unlocked}/${total} (${Math.round(percent)}%)`;

    const progressWrap = document.createElement("div");
    progressWrap.className = "achievements-progress";
    const bar = document.createElement("div");
    bar.className = "achievements-bar";
    bar.setAttribute("role", "progressbar");
    bar.setAttribute("aria-valuemin", "0");
    bar.setAttribute("aria-valuemax", "100");
    bar.setAttribute("aria-valuenow", `${Math.round(percent)}`);
    const fill = document.createElement("div");
    fill.className = "achievements-bar-fill";
    fill.style.width = `${Math.max(0, Math.min(100, percent))}%`;
    bar.append(fill);
    progressWrap.append(bar);

    const iconsRow = document.createElement("div");
    iconsRow.className = "achievements-icons";

    const maxVisible = 10;
    let shown = 0;
    for (const entry of entries) {
      if (shown >= maxVisible) break;
      const iconWrap = document.createElement("div");
      iconWrap.className = `achievements-icon ${entry.unlocked ? "is-unlocked" : "is-locked"}`;
      iconWrap.setAttribute("title", entry.name + (entry.unlockedAt ? ` — ${entry.unlockedAt}` : ""));
      if (entry.icon) {
        const img = document.createElement("img");
        img.src = entry.icon;
        img.alt = entry.name;
        img.loading = "lazy";
        iconWrap.append(img);
      } else {
        const fallback = document.createElement("div");
        fallback.className = "achievements-icon-fallback";
        fallback.textContent = entry.name.slice(0, 1).toUpperCase() || "?";
        iconWrap.append(fallback);
      }
      iconsRow.append(iconWrap);
      shown += 1;
    }

    if (entries.length > maxVisible) {
      const more = document.createElement("div");
      more.className = "achievements-more";
      more.textContent = `+${entries.length - maxVisible}`;
      iconsRow.append(more);
    }

    const viewLinkP = document.createElement("p");
    viewLinkP.className = "achievements-view-link";
    const viewLink = document.createElement("a");
    const appId = parseInt(game.externalId || "", 10);
    if (!Number.isNaN(appId)) {
      viewLink.href = `https://steamcommunity.com/stats/${appId}/achievements`;
    } else {
      viewLink.href = `https://store.steampowered.com/`;
    }
    viewLink.target = "_blank";
    viewLink.rel = "noopener noreferrer";
    viewLink.textContent = "View My Achievements";
    viewLinkP.append(viewLink);

    summary.append(countP, progressWrap, iconsRow, viewLinkP);
    achievementsSection.append(heading, summary);
  };

  const tradingCardsSection = document.createElement("section");
  tradingCardsSection.className = "details-section trading-cards-section";
  tradingCardsSection.hidden = true;
  tradingCardsSection.innerHTML = `
    <h4>Trading Cards</h4>
    <p class="placeholder">Loading trading card progress…</p>
  `;

  const hideTradingCardsSection = (): void => {
    tradingCardsSection.hidden = true;
  };

  const showTradingCardsSection = (): void => {
    tradingCardsSection.hidden = false;
  };

  const renderTradingCardsPlaceholder = (message: string): void => {
    showTradingCardsSection();
    tradingCardsSection.replaceChildren();
    const heading = document.createElement("h4");
    heading.textContent = "Trading Cards";
    const placeholder = document.createElement("p");
    placeholder.className = "placeholder";
    placeholder.textContent = message;
    tradingCardsSection.append(heading, placeholder);
  };

  const renderTradingCards = (payload: GameTradingCardsPayload): void => {
    showTradingCardsSection();
    tradingCardsSection.replaceChildren();

    const heading = document.createElement("h4");
    heading.textContent = "Trading Cards";
    tradingCardsSection.append(heading);

    if (payload.warning?.trim()) {
      const warning = document.createElement("p");
      warning.className = "activity-timeline-warning";
      warning.textContent = payload.warning.trim();
      tradingCardsSection.append(warning);
    }

    if (!payload.supported) {
      hideTradingCardsSection();
      return;
    }

    const summary = document.createElement("div");
    summary.className = "trading-cards-summary";

    const totalCards = Math.max(0, payload.totalCards ?? 0);
    const ownedCards = Math.max(0, payload.ownedCards ?? 0);
    const cardsLeft = Math.max(0, totalCards - ownedCards);

    const cardsRow = document.createElement("div");
    cardsRow.className = "trading-cards-row";

    const progress = document.createElement("p");
    progress.className = "trading-cards-progress";
    progress.textContent = totalCards > 0
      ? `${cardsLeft} trading cards left to collect (${ownedCards}/${totalCards} owned)`
      : "No trading-card set information is available for this title yet.";

    const cards = payload.cards ?? [];
    if (cards.length > 0) {
      for (const card of cards) {
        const tile = document.createElement("div");
        tile.className = `trading-cards-tile${card.isOwned ? " is-owned" : " is-missing"}`;
        tile.setAttribute("title", card.isOwned
          ? `${card.name} (${Math.max(1, card.ownedCount)} owned)`
          : `${card.name} (missing)`);
        tile.setAttribute("aria-label", card.name);

        const imageUrl = card.imageUrl?.trim();
        if (imageUrl && imageUrl.length > 0) {
          const image = document.createElement("img");
          image.src = imageUrl;
          image.alt = card.name;
          image.loading = "lazy";
          tile.append(image);
        } else {
          const fallback = document.createElement("div");
          fallback.className = "trading-cards-fallback";
          fallback.textContent = card.name.slice(0, 1).toLocaleUpperCase() || "?";
          tile.append(fallback);
        }

        if (card.ownedCount > 1) {
          const count = document.createElement("span");
          count.className = "trading-cards-owned-count";
          count.textContent = `x${card.ownedCount}`;
          tile.append(count);
        }
        cardsRow.append(tile);
      }
    } else {
      const empty = document.createElement("p");
      empty.className = "placeholder";
      empty.textContent = "Could not load per-card tile data yet.";
      cardsRow.append(empty);
    }

    const viewLinkWrap = document.createElement("p");
    viewLinkWrap.className = "trading-cards-view-link";
    const viewLink = document.createElement("a");
    viewLink.href = payload.viewUrl?.trim() || "https://steamcommunity.com/tradingcards/";
    viewLink.target = "_blank";
    viewLink.rel = "noopener noreferrer";
    viewLink.textContent = "View My Trading Cards";
    viewLinkWrap.append(viewLink);

    summary.append(progress, cardsRow, viewLinkWrap);
    tradingCardsSection.append(summary);
  };

  const dlcSection = document.createElement("section");
  dlcSection.className = "details-section details-dlc-section";
  dlcSection.innerHTML = `
    <h4>DLC</h4>
    <p class="placeholder">Loading downloadable content…</p>
  `;

  const renderDlcPlaceholder = (message: string): void => {
    dlcSection.replaceChildren();
    const heading = document.createElement("h4");
    heading.textContent = "DLC";
    const placeholder = document.createElement("p");
    placeholder.className = "placeholder";
    placeholder.textContent = message;
    dlcSection.append(heading, placeholder);
  };

  const renderDlc = (payload: GameDlcPayload): void => {
    dlcSection.replaceChildren();
    const entries = payload.entries ?? [];
    dlcSection.hidden = false;

    // If no DLC metadata was returned, show a helpful placeholder instead of hiding the section.
    if (entries.length === 0) {
      const heading = document.createElement("h4");
      heading.textContent = "DLC";
      const placeholder = document.createElement("p");
      placeholder.className = "placeholder";
      placeholder.textContent = "No DLC metadata available. Try syncing your Steam library to import owned DLC.";
      dlcSection.append(heading, placeholder);
      return;
    }
    const headingRow = document.createElement("div");
    headingRow.className = "details-dlc-header";

    const heading = document.createElement("h4");
    heading.textContent = "DLC";
    headingRow.append(heading);

    dlcSection.append(headingRow);

    if (payload.warning?.trim()) {
      const warning = document.createElement("p");
      warning.className = "activity-timeline-warning";
      warning.textContent = payload.warning.trim();
      dlcSection.append(warning);
    }

    const list = document.createElement("div");
    list.className = "details-dlc-list";

    for (const entry of entries) {
      const targetGameId = entry.id?.trim() || `${entry.provider}:${entry.externalId}`;
      const dlcGame = findGameById(targetGameId) ?? libraryCatalogStore.gameById.get(targetGameId) ?? null;
      const shouldUseStoreLink = !entry.inLibrary || entry.provider.trim().toLocaleLowerCase() !== "steam" || !dlcGame;
      const primaryActionLabel = shouldUseStoreLink ? "View in store" : (entry.installed ? "Play" : "Install");

      const tile = document.createElement("button");
      tile.type = "button";
      tile.className = "details-dlc-tile";
      tile.setAttribute("aria-label", `${entry.name}: ${primaryActionLabel}`);

      const artwork = document.createElement("img");
      artwork.className = "details-dlc-image";
      artwork.alt = `${entry.name} artwork`;
      artwork.loading = "lazy";
      const appId = encodeURIComponent(entry.externalId);
      artwork.src = `${DLC_CDN_CAPSULE_URL}/${appId}/header.jpg`;
      artwork.addEventListener("error", () => {
        artwork.src = `${DLC_CDN_CAPSULE_URL}/${appId}/capsule_467x181.jpg`;
      }, { once: true });
      tile.append(artwork);

      tile.addEventListener("click", async () => {
        const defaultStoreUrl = `https://store.steampowered.com/app/${encodeURIComponent(entry.externalId)}`;
        const storeUrl = entry.storeUrl?.trim() || defaultStoreUrl;
        const steamStoreUrl = `steam://openurl/${storeUrl}`;
        const previousBusyState = tile.getAttribute("aria-busy");
        tile.setAttribute("aria-busy", "true");
        tile.disabled = true;
        try {
          if (shouldUseStoreLink) {
            await openSteamConnectedUrl(steamStoreUrl, storeUrl);
            return;
          }

          if (!dlcGame) {
            await openSteamConnectedUrl(steamStoreUrl, storeUrl);
            return;
          }

          if (dlcGame.installed) {
            await ipcService.playGame({ provider: dlcGame.provider, externalId: dlcGame.externalId });
            return;
          }

          await installGameForGame(dlcGame);
          showLauncherToast(`Queued "${dlcGame.name}" for install.`);
          void refreshSteamDownloads();
        } catch (error) {
          const appError = normalizeAppError(error, "Could not run this DLC action.");
          showLauncherToast(appError.message, "error");
        } finally {
          tile.disabled = false;
          tile.setAttribute("aria-busy", previousBusyState ?? "false");
        }
      });

      list.append(tile);
    }

    const footer = document.createElement("div");
    footer.className = "details-dlc-footer";

    const storeLink = document.createElement("button");
    storeLink.type = "button";
    storeLink.className = "details-dlc-link";
    storeLink.textContent = "View DLC In Store";
    storeLink.addEventListener("click", async () => {
      const storeUrl = `https://store.steampowered.com/dlc/${encodeURIComponent(game.externalId)}`;
      try {
        await openSteamConnectedUrl(`steam://openurl/${storeUrl}`, storeUrl);
      } catch (error) {
        const appError = normalizeAppError(error, "Could not open DLC store.");
        showLauncherToast(appError.message, "error");
      }
    });

    const manageLink = document.createElement("button");
    manageLink.type = "button";
    manageLink.className = "details-dlc-link details-dlc-manage-link";
    manageLink.textContent = `Manage My ${entries.length} DLC${entries.length === 1 ? "" : "s"}`;
    manageLink.addEventListener("click", async () => {
      const manageFallbackUrl = `https://store.steampowered.com/dlc/${encodeURIComponent(game.externalId)}`;
      const steamManageUrl = `steam://nav/games/details/${encodeURIComponent(game.externalId)}`;
      try {
        await openSteamConnectedUrl(steamManageUrl, manageFallbackUrl);
      } catch (error) {
        const appError = normalizeAppError(error, "Could not open Steam DLC manager.");
        showLauncherToast(appError.message, "error");
      }
    });

    footer.append(storeLink, manageLink);
    dlcSection.append(list, footer);
  };

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
  const reviewHeading = document.createElement("h4");
  reviewHeading.textContent = "Review";
  reviewSection.append(reviewHeading);

  // Try to load a persisted review for this game (localStorage for v1)
  const persisted = loadReviewForGame(game.provider, game.externalId);
  let reviewToShow: Review | null = null;
  if (persisted) {
    reviewToShow = persisted;
  } else {
    // Fallback sample uses current total playtime and is not a review-time snapshot.
    reviewToShow = {
      id: `r-${game.id}`,
      userId: "me",
      gameId: game.id,
      recommended: true,
      text: "Write your review and save it to capture playtime at review time.",
      playtimeMinutes: typeof game.playtimeMinutes === "number" ? game.playtimeMinutes : 0,
      playtimeCapturedAtReview: false,
      createdAt: new Date().toISOString(),
      likes: 1,
      comments: 0,
    };
  }

  let activeReviewCard = reviewToShow ? createReviewCard(reviewToShow) : createReviewPlaceholder();
  reviewSection.append(activeReviewCard);

  // Wire editing action: open an inline editor and persist playtime at save
  document.addEventListener("open-review-edit", (ev: Event) => {
    const ce = ev as CustomEvent<{ reviewId: string }>;
    const id = ce?.detail?.reviewId;
    if (!id) return;

    // Replace card with an editor form
    const form = document.createElement("form");
    form.className = "review-edit-form";
    form.innerHTML = `
      <div style="display:flex;gap:.5rem;align-items:center;margin-bottom:.5rem;">
        <label style="font-weight:700">Recommend?</label>
        <select name="recommended">
          <option value="true">Recommended</option>
          <option value="false">Not Recommended</option>
        </select>
      </div>
      <div style="margin-bottom:.5rem;">
        <textarea name="text" rows="4" style="width:100%;padding:.5rem;border-radius:8px;border:1px solid rgba(255,255,255,0.06);background:transparent;color:var(--color-text);"></textarea>
      </div>
      <div style="display:flex;gap:.5rem;justify-content:flex-end;margin-top:.5rem;">
        <button type="button" class="secondary-button review-cancel">Cancel</button>
        <button type="submit" class="primary-button">Save Review</button>
      </div>
    `;

    // Pre-fill with existing if available
    const existing = loadReviewForGame(game.provider, game.externalId);
    const sel = form.querySelector<HTMLSelectElement>("select[name='recommended']");
    const ta = form.querySelector<HTMLTextAreaElement>("textarea[name='text']");
    if (existing) {
      if (sel) sel.value = existing.recommended ? "true" : "false";
      if (ta) ta.value = existing.text;
    } else {
      if (ta) ta.value = reviewToShow?.text ?? "";
    }

    activeReviewCard.replaceWith(form);

    const cancelBtn = form.querySelector<HTMLButtonElement>(".review-cancel");
    cancelBtn?.addEventListener("click", () => {
      form.replaceWith(activeReviewCard);
    });

    form.addEventListener("submit", (submitEv) => {
      submitEv.preventDefault();
      const recommendedVal = sel?.value === "true";
      const textVal = ta?.value?.trim() ?? "";
      const currentPlaytimeMinutes = typeof game.playtimeMinutes === "number" ? game.playtimeMinutes : 0;
      const baselineReview = existing ?? reviewToShow;
      const baselineHasCapturedSnapshot = baselineReview?.playtimeCapturedAtReview === true;
      const playtimeAtReviewMinutes = baselineHasCapturedSnapshot && typeof baselineReview?.playtimeMinutes === "number"
        ? baselineReview.playtimeMinutes
        : currentPlaytimeMinutes;
      const createdAt = baselineHasCapturedSnapshot
        ? (baselineReview?.createdAt ?? new Date().toISOString())
        : new Date().toISOString();

      const newReview: Review = {
        id: id,
        userId: "me",
        gameId: game.id,
        recommended: recommendedVal,
        text: textVal,
        // Keep original review-time snapshot when editing an existing review.
        playtimeMinutes: playtimeAtReviewMinutes,
        playtimeCapturedAtReview: true,
        createdAt,
        likes: existing?.likes ?? baselineReview?.likes ?? 0,
        comments: existing?.comments ?? baselineReview?.comments ?? 0,
      };

      saveReviewForGame(game.provider, game.externalId, newReview);

      const newCard = createReviewCard(newReview);
      activeReviewCard = newCard;
      reviewToShow = newReview;
      form.replaceWith(newCard);
      showLauncherToast("Review saved", "info");
    });
  });

  main.append(activitySection, notesSection);

  // Side column
  const side = document.createElement("aside");
  side.className = "game-details-side-inner";

  // Friends activity
  const friendsSection = document.createElement("section");
  friendsSection.className = "details-section";
  friendsSection.innerHTML = `<h4>Friends</h4><p class="placeholder">Loading friends activity…</p>`;
  let friendsActivityRefreshToken = 0;

  const renderFriendsPlaceholder = (message: string): void => {
    friendsActivityRefreshToken += 1;
    friendsSection.replaceChildren();
    const heading = document.createElement("h4");
    heading.textContent = "Friends";
    const placeholder = document.createElement("p");
    placeholder.className = "placeholder";
    placeholder.textContent = message;
    friendsSection.append(heading, placeholder);
  };

  const formatFriendCountLabel = (
    count: number,
    singularText: string,
    pluralText: string
  ): string => {
    if (count === 1) {
      return `1 friend ${singularText}`;
    }
    return `${count} friends ${pluralText}`;
  };

  const createFriendAvatar = (friend: GameFriendActivityEntryPayload): HTMLElement => {
    const avatar = document.createElement("div");
    avatar.className = "friends-activity-avatar";
    avatar.setAttribute("title", friend.personaName);
    avatar.setAttribute("aria-label", friend.personaName);

    const avatarUrl = friend.avatarUrl?.trim();
    if (avatarUrl && avatarUrl.length > 0) {
      const image = document.createElement("img");
      image.src = avatarUrl;
      image.alt = friend.personaName;
      image.loading = "lazy";
      avatar.append(image);
      return avatar;
    }

    const initials = friend.personaName.trim().slice(0, 1).toLocaleUpperCase() || "?";
    avatar.textContent = initials;
    return avatar;
  };

  const createFriendGroup = (label: string, friends: GameFriendActivityEntryPayload[]): HTMLElement => {
    const group = document.createElement("div");
    group.className = "friends-activity-group";

    const groupLabel = document.createElement("p");
    groupLabel.className = "friends-activity-label";
    groupLabel.textContent = label;

    const avatars = document.createElement("div");
    avatars.className = "friends-activity-avatars";
    const visibleFriends = friends.slice(0, 10);
    for (const friend of visibleFriends) {
      avatars.append(createFriendAvatar(friend));
    }
    const remainingCount = friends.length - visibleFriends.length;
    if (remainingCount > 0) {
      const more = document.createElement("span");
      more.className = "friends-activity-more";
      more.textContent = `+${remainingCount}`;
      avatars.append(more);
    }

    group.append(groupLabel, avatars);
    return group;
  };

  const renderFriendsActivity = (activity: GameFriendsActivityPayload): void => {
    friendsSection.replaceChildren();
    const heading = document.createElement("h4");
    heading.textContent = "Friends";
    friendsSection.append(heading);

    const playedFriends = activity.playedFriends ?? [];
    const hasPlayedFriends = playedFriends.length > 0;

    if (activity.warning?.trim()) {
      const warning = document.createElement("p");
      warning.className = "placeholder";
      warning.textContent = activity.warning.trim();
      friendsSection.append(warning);
    }

    if (!hasPlayedFriends) {
      const emptyState = document.createElement("p");
      emptyState.className = "placeholder";
      emptyState.textContent = "No friend activity found for this game.";
      friendsSection.append(emptyState);
      return;
    }

    if (hasPlayedFriends) {
      friendsSection.append(
        createFriendGroup(
          formatFriendCountLabel(
            playedFriends.length,
            "has played previously",
            "have played previously"
          ),
          playedFriends
        )
      );
    }
  };

  // Notes section (placeholder for future work)
  const sideNotesSection = document.createElement("section");
  sideNotesSection.className = "details-section";
  sideNotesSection.innerHTML = `<h4>Notes</h4><p class="placeholder">Notes coming soon.</p>`;

  // Place screenshots into the side column (keeps ordering consistent with other side widgets)
  screenshotsSection.className = "details-section"; // ensure side styling

  side.append(
    friendsSection,
    screenshotsSection,
    reviewSection,
    achievementsSection,
    tradingCardsSection,
    dlcSection,
    sideNotesSection
  );

  cols.append(main, side);
  gameDetailsContentElement.append(cols);

  const isActiveGameDetailsView = (): boolean => (
    detailsViewStore.appViewMode === "game-details" && detailsViewStore.selectedGameId === game.id
  );
  let reviewRefreshToken = 0;
  let timelineRefreshToken = 0;
  let achievementsRefreshToken = 0;

  // Async: fetch Steam review using stale-while-revalidate.
  void (async () => {
    const normalizedProvider = game.provider.trim().toLocaleLowerCase();
    if (normalizedProvider !== "steam") {
      return;
    }

    const reviewToken = ++reviewRefreshToken;
    const applyReviewPayload = (reviewPayload: GameReviewPayload | null, isRevalidation: boolean): void => {
      const steamReviewPayload = reviewPayload?.review;
      if (steamReviewPayload) {
        const steamReview: Review = {
          id: steamReviewPayload.id || `steam-review-${game.id}`,
          userId: "me",
          gameId: game.id,
          recommended: steamReviewPayload.recommended,
          text: steamReviewPayload.text?.trim() || "No review text available.",
          playtimeMinutes: Math.max(0, Math.round(steamReviewPayload.playtimeMinutes ?? 0)),
          playtimeCapturedAtReview: true,
          createdAt: steamReviewPayload.createdAt ?? new Date().toISOString(),
          likes: Math.max(0, Math.round(steamReviewPayload.likes ?? 0)),
          comments: Math.max(0, Math.round(steamReviewPayload.comments ?? 0)),
        };

        reviewToShow = steamReview;
        if (!activeReviewCard.isConnected) {
          return;
        }
        const steamReviewCard = createReviewCard(steamReview);
        activeReviewCard.replaceWith(steamReviewCard);
        activeReviewCard = steamReviewCard;
        return;
      }

      const warningMessage = reviewPayload?.warning?.trim();
      const missingReviewWarning = STEAM_REVIEW_MISSING_WARNING_PATTERN.test(warningMessage ?? "");
      if (isRevalidation && !missingReviewWarning) {
        return;
      }
      if (!activeReviewCard.isConnected) {
        return;
      }

      const unavailableCard = document.createElement("div");
      unavailableCard.className = "review-card placeholder";
      unavailableCard.textContent = warningMessage
        || "Steam review data is unavailable for this title/account.";
      activeReviewCard.replaceWith(unavailableCard);
      activeReviewCard = unavailableCard;
      reviewToShow = null;
    };

    const cachedReviewPayload = await getGameReviewForGame(game, false);
    if (!isActiveGameDetailsView() || reviewToken !== reviewRefreshToken) {
      return;
    }
    applyReviewPayload(cachedReviewPayload, false);

    void (async () => {
      const refreshedReviewPayload = await getGameReviewForGame(game, true);
      if (!isActiveGameDetailsView() || reviewToken !== reviewRefreshToken) {
        return;
      }
      if (!refreshedReviewPayload) {
        return;
      }
      applyReviewPayload(refreshedReviewPayload, true);
    })();
  })();

  // Async: fetch timeline using stale-while-revalidate.
  void (async () => {
    const normalizedProvider = game.provider.trim().toLocaleLowerCase();
    if (normalizedProvider !== "steam") {
      renderActivityPlaceholder("Recent timeline activity is currently available for Steam games.");
      return;
    }

    const timelineToken = ++timelineRefreshToken;
    const [timeline, timelineWideCoverCandidates] = await Promise.all([
      getGameActivityTimelineForGame(game, forceFriendsActivityRefresh),
      resolveTimelineWideCoverCandidates(game),
    ]);
    if (!isActiveGameDetailsView() || timelineToken !== timelineRefreshToken) {
      return;
    }
    if (!timeline) {
      renderActivityPlaceholder("Could not load activity timeline.");
    } else {
      renderActivityTimeline(timeline, timelineWideCoverCandidates);
    }

    if (forceFriendsActivityRefresh) {
      return;
    }

    void (async () => {
      const refreshedTimeline = await getGameActivityTimelineForGame(game, true);
      if (!isActiveGameDetailsView() || timelineToken !== timelineRefreshToken) {
        return;
      }
      if (!refreshedTimeline) {
        return;
      }
      renderActivityTimeline(refreshedTimeline, timelineWideCoverCandidates);
    })();
  })();

  // Async: fetch achievements using stale-while-revalidate.
  void (async () => {
    const normalizedProvider = game.provider.trim().toLocaleLowerCase();
    if (!sessionStore.steamLinked) {
      renderAchievementsPlaceholder("Connect Steam to view achievements.");
      return;
    }
    if (normalizedProvider !== "steam") {
      renderAchievementsPlaceholder("Achievements are currently available for Steam games.");
      return;
    }

    const achievementsToken = ++achievementsRefreshToken;

    try {
      const payload = await ipcService.getGameAchievements({
        provider: game.provider,
        externalId: game.externalId,
        forceRefresh: false,
      });
      if (!isActiveGameDetailsView() || achievementsToken !== achievementsRefreshToken) {
        return;
      }
      renderAchievements(payload);

      void (async () => {
        try {
          const refreshedPayload = await ipcService.getGameAchievements({
            provider: game.provider,
            externalId: game.externalId,
            forceRefresh: true,
          });
          if (!isActiveGameDetailsView() || achievementsToken !== achievementsRefreshToken) {
            return;
          }
          renderAchievements(refreshedPayload);
        } catch (refreshError) {
          console.error("getGameAchievements background refresh failed:", refreshError);
        }
      })();
    } catch (err: unknown) {
      console.error("getGameAchievements failed:", err);
      const message = err instanceof Error ? err.message : "Could not load achievements right now.";
      renderAchievementsPlaceholder(message);
    }
  })();

  // Async: fetch trading-card summary and link out to Steam inventory view
  void (async () => {
    const normalizedProvider = game.provider.trim().toLocaleLowerCase();
    if (normalizedProvider !== "steam") {
      hideTradingCardsSection();
      return;
    }

    if (!sessionStore.steamLinked) {
      renderTradingCardsPlaceholder("Connect Steam to view trading-card progress.");
      return;
    }

    const tradingCards = await getGameTradingCardsForGame(game, false);
    if (detailsViewStore.appViewMode !== "game-details" || detailsViewStore.selectedGameId !== game.id) {
      return;
    }
    if (!tradingCards) {
      renderTradingCardsPlaceholder("Could not load trading-card progress.");
      return;
    }
    if (!tradingCards.supported) {
      hideTradingCardsSection();
      return;
    }
    renderTradingCards(tradingCards);
  })();

  // Async: fetch DLC rows for the right-side panel.
  void (async () => {
    const normalizedProvider = game.provider.trim().toLocaleLowerCase();
    if (normalizedProvider !== "steam") {
      renderDlcPlaceholder("DLC details are currently available for Steam games.");
      return;
    }

    const dlcPayload = await getGameDlcForGame(game, false);
    if (detailsViewStore.appViewMode !== "game-details" || detailsViewStore.selectedGameId !== game.id) {
      return;
    }
    if (!dlcPayload) {
      renderDlcPlaceholder("Could not load DLC details right now.");
      return;
    }

    renderDlc(dlcPayload);
  })();

  // Async: fetch screenshots and render gallery (exposed for manual refresh)
  const fetchAndRenderScreenshots = async (): Promise<void> => {
    const normalizedProvider = game.provider.trim().toLocaleLowerCase();
    screenshotsSection.replaceChildren();

    const heading = document.createElement("h4");
    heading.textContent = "Recordings and Screenshots";
    screenshotsSection.append(heading);

    if (normalizedProvider !== "steam") {
      const placeholder = document.createElement("p");
      placeholder.className = "placeholder";
      placeholder.textContent = "Screenshots are currently available for local Steam installs.";
      screenshotsSection.append(placeholder);
      return;
    }

    try {
      const shots = await getGameScreenshotsForGame(game);
      if (detailsViewStore.appViewMode !== "game-details" || detailsViewStore.selectedGameId !== game.id) return;

      if (!Array.isArray(shots) || shots.length === 0) {
        const empty = document.createElement("p");
        empty.className = "placeholder";
        empty.textContent = "No screenshots available. You can add screenshots via the Properties panel.";
        screenshotsSection.append(empty);
      } else {
        const grid = document.createElement("div");
        grid.className = "screenshots-grid";
        for (const shot of shots) {
          const tile = document.createElement("div");
          tile.className = "screenshot-tile";
          const img = document.createElement("img");
          try {
            img.src = convertFileSrc(shot.path);
          } catch {
            img.src = shot.path;
          }
          img.alt = shot.id;
          img.loading = "lazy";
          tile.append(img);
          grid.append(tile);
        }

        const manageBtn = document.createElement("button");
        manageBtn.type = "button";
        manageBtn.className = "details-dlc-link";
        manageBtn.textContent = "Manage my recordings and screenshots";
        manageBtn.addEventListener("click", async () => {
          try {
            await ipcService.openGameRecordingSettings({ provider: game.provider, externalId: game.externalId });
          } catch (err) {
            showLauncherToast(normalizeAppError(err, "Could not open recording settings").message, "error");
          }
        });

        screenshotsSection.append(grid, manageBtn);
      }
    } catch (err) {
      const placeholder = document.createElement("p");
      placeholder.className = "placeholder";
      placeholder.textContent = "Could not load screenshots right now.";
      screenshotsSection.append(placeholder);
    }
  };

  // initial load
  void fetchAndRenderScreenshots();

  void (async () => {
    const normalizedProvider = game.provider.trim().toLocaleLowerCase();
    if (!sessionStore.steamLinked) {
      renderFriendsPlaceholder("Connect Steam to view friends activity.");
      return;
    }
    if (normalizedProvider !== "steam") {
      renderFriendsPlaceholder("Friends activity is currently available for Steam games.");
      return;
    }

    const friendsActivity = await getGameFriendsActivityForGame(game, forceFriendsActivityRefresh);
    if (detailsViewStore.appViewMode !== "game-details" || detailsViewStore.selectedGameId !== game.id) {
      return;
    }
    if (!friendsActivity) {
      renderFriendsPlaceholder("Could not load friends activity.");
      return;
    }
    renderFriendsActivity(friendsActivity);

    if (!forceFriendsActivityRefresh) {
      const refreshToken = ++friendsActivityRefreshToken;
      void (async () => {
        const refreshedFriendsActivity = await getGameFriendsActivityForGame(game, true);
        if (detailsViewStore.appViewMode !== "game-details" || detailsViewStore.selectedGameId !== game.id) {
          return;
        }
        if (refreshToken !== friendsActivityRefreshToken) {
          return;
        }
        if (!refreshedFriendsActivity) {
          return;
        }
        renderFriendsActivity(refreshedFriendsActivity);
      })();
    }
  })();

  // Notes are currently static; installation details removed per UI update.
};

const {
  getDownloadEtaKey,
  updateDownloadEtaSnapshots,
  normalizeDownloadPercent,
  renderDownloadActivity,
} = createDownloadActivityView({
  downloadActivityElement,
  downloadActivityCountElement,
  downloadActivityListElement,
  state: downloadStore,
  isSteamLinked: () => sessionStore.steamLinked,
  downloadEtaSmoothingFactor: DOWNLOAD_ETA_SMOOTHING_FACTOR,
  downloadEtaSampleMinSeconds: DOWNLOAD_ETA_SAMPLE_MIN_SECONDS,
  downloadEtaStaleMs: DOWNLOAD_ETA_STALE_MS,
});

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
  if (!ENABLE_LINUX_GRID_WHEEL_SMOOTHING || resolveRuntimePlatform() !== "linux") {
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
    if (!isGameLibraryViewMode(libraryViewStore.activeLibraryViewMode) || event.ctrlKey || event.metaKey) {
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
  const nextGames = games.map((game) => {
    const key = `${game.provider.trim().toLocaleLowerCase()}:${game.externalId.trim()}`;
    return {
      ...game,
      uninstalling: pendingUninstallVerificationByKey.has(key),
    };
  });
  libraryCatalogStore.allGames = nextGames;
  libraryCatalogStore.gameById = new Map(nextGames.map((game) => [game.id, game]));
  console.debug("setAllGames: loaded", nextGames.length, "games; sample ids:", nextGames.slice(0,10).map(g => g.id));
  filterPanel.setSteamTagSuggestions(collectSteamTagSuggestions(nextGames));
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
  for (const collection of libraryCatalogStore.allCollections) {
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
  libraryCatalogStore.allCollections = sortedCollections;
  updateCollectionSuggestions();
};

const normalizeCollectionNameForMatch = (collectionName: string): string => {
  return collectionName.trim().toLocaleLowerCase();
};

const upsertCollectionInState = (collection: CollectionResponse): void => {
  const existingIndex = libraryCatalogStore.allCollections.findIndex((existingCollection) => existingCollection.id === collection.id);
  if (existingIndex < 0) {
    setAllCollections([...libraryCatalogStore.allCollections, collection]);
    return;
  }

  const nextCollections = [...libraryCatalogStore.allCollections];
  nextCollections[existingIndex] = {
    ...nextCollections[existingIndex],
    ...collection,
  };
  setAllCollections(nextCollections);
};

const removeCollectionFromState = (collectionId: string): void => {
  setAllCollections(libraryCatalogStore.allCollections.filter((collection) => collection.id !== collectionId));
};

const updateCollectionNameInGames = (previousName: string, nextName: string | null): void => {
  const normalizedPreviousName = normalizeCollectionNameForMatch(previousName);
  if (normalizedPreviousName.length === 0) {
    return;
  }

  const normalizedNextName = nextName === null ? "" : normalizeCollectionNameForMatch(nextName);
  const nextCollectionName = nextName?.trim() ?? "";
  let stateChanged = false;
  const nextGames = libraryCatalogStore.allGames.map((game) => {
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

  libraryCatalogStore.allGames = nextGames;
  libraryCatalogStore.gameById = new Map(nextGames.map((game) => [game.id, game]));
};

const resolveGameFromCard = (card: HTMLElement): GameResponse | null => {
  const gameId = card.dataset.gameId;
  if (!gameId) {
    return null;
  }

  return libraryCatalogStore.gameById.get(gameId) ?? null;
};

const updateGameInState = (
  gameId: string,
  update: (game: GameResponse) => GameResponse
): GameResponse | null => {
  const gameIndex = libraryCatalogStore.allGames.findIndex((game) => game.id === gameId);
  if (gameIndex < 0) {
    return null;
  }

  const updatedGame = update(libraryCatalogStore.allGames[gameIndex]);
  libraryCatalogStore.allGames[gameIndex] = updatedGame;
  libraryCatalogStore.gameById.set(updatedGame.id, updatedGame);
  return updatedGame;
};

const gamePropertiesPanel = createGamePropertiesPanel();
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

  if (isCollectionLibraryViewMode(libraryViewStore.activeLibraryViewMode)) {
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

const libraryViewOptionButtons = Array.from(
  libraryViewPickerElement.querySelectorAll(".library-view-picker-option")
).filter((option): option is HTMLButtonElement => option instanceof HTMLButtonElement);
if (libraryViewOptionButtons.length === 0) {
  throw new Error("Library view picker is missing options");
}

const setLibraryViewMode = (viewMode: LibraryViewMode, render = true): void => {
  libraryViewStore.activeLibraryViewMode = viewMode;

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
  if (isGameLibraryViewMode(libraryViewStore.activeLibraryViewMode)) {
    renderGameLibrary();
  }
});

const {
  renderGameLibrary,
  renderCollectionLibrary,
  renderActiveLibraryView,
} = createLibraryViewRenderer({
  libraryGridElement,
  filterPanel,
  setLibrarySummary,
  setLibraryViewMode: (viewMode, render) => {
    setLibraryViewMode(viewMode, render);
  },
  onCreateCollection: () => {
    void createCollectionFromGrid();
  },
  onRenameCollection: (collection) => {
    void renameCollectionFromGrid(collection);
  },
  onDeleteCollection: (collection) => {
    void deleteCollectionFromGrid(collection);
  },
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

const getGameInstallationDetailsForIdentity = async (
  provider: string,
  externalId: string
): Promise<GameInstallationDetailsPayload | null> => {
  try {
    return await ipcService.getGameInstallationDetails({
      provider,
      externalId,
    });
  } catch {
    return null;
  }
};

const getGameInstallationDetailsForGame = async (game: GameResponse): Promise<GameInstallationDetailsPayload | null> => {
  return getGameInstallationDetailsForIdentity(game.provider, game.externalId);
};

const getGameFriendsActivityForGame = async (
  game: GameResponse,
  forceRefresh = false
): Promise<GameFriendsActivityPayload | null> => {
  try {
    return await ipcService.getGameFriendsActivity({
      provider: game.provider,
      externalId: game.externalId,
      forceRefresh,
    });
  } catch {
    return null;
  }
};

const getGameActivityTimelineForGame = async (
  game: GameResponse,
  forceRefresh = false
): Promise<GameActivityTimelinePayload | null> => {
  try {
    return await ipcService.getGameActivityTimeline({
      provider: game.provider,
      externalId: game.externalId,
      forceRefresh,
    });
  } catch {
    return null;
  }
};

const getGameTradingCardsForGame = async (
  game: GameResponse,
  forceRefresh = false
): Promise<GameTradingCardsPayload | null> => {
  try {
    return await ipcService.getGameTradingCards({
      provider: game.provider,
      externalId: game.externalId,
      forceRefresh,
    });
  } catch {
    return null;
  }
};

const getGameDlcForGame = async (
  game: GameResponse,
  forceRefresh = false
): Promise<GameDlcPayload | null> => {
  try {
    return await ipcService.getGameDlc({
      provider: game.provider,
      externalId: game.externalId,
      forceRefresh,
    });
  } catch {
    return null;
  }
};

const getGameReviewForGame = async (
  game: GameResponse,
  forceRefresh = false
): Promise<GameReviewPayload | null> => {
  try {
    return await ipcService.getGameReview({
      provider: game.provider,
      externalId: game.externalId,
      forceRefresh,
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

const getGameScreenshotsForGame = async (
  game: GameResponse
): Promise<GameScreenshotPayload[]> => {
  try {
    return await ipcService.getGameScreenshots({ provider: game.provider, externalId: game.externalId });
  } catch (err) {
    console.error("Could not load screenshots:", err);
    return [];
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

const getGameInstallationDetailsForDownload = async (
  download: SteamDownloadProgressPayload
): Promise<GameInstallationDetailsPayload | null> => {
  return getGameInstallationDetailsForIdentity(download.provider, download.externalId);
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

const getGameIdentityKey = (provider: string, externalId: string): string => {
  return `${provider.trim().toLocaleLowerCase()}:${externalId.trim()}`;
};

interface GameStateTarget {
  externalId: string;
  gameId?: string;
  provider: string;
}

const applyGameStateMutationInState = (
  target: GameStateTarget,
  mutate: (game: GameResponse) => GameResponse
): boolean => {
  let updatedGame: GameResponse | null = null;

  if (typeof target.gameId === "string" && target.gameId.trim().length > 0) {
    updatedGame = updateGameInState(target.gameId, mutate);
  }

  if (!updatedGame) {
    const normalizedProvider = target.provider.trim().toLocaleLowerCase();
    const fallbackGame = libraryCatalogStore.allGames.find((game) =>
      game.provider.trim().toLocaleLowerCase() === normalizedProvider
      && game.externalId === target.externalId
    );
    if (fallbackGame) {
      updatedGame = updateGameInState(fallbackGame.id, mutate);
    }
  }

  return updatedGame !== null;
};

const setGameInstalledStateInState = (
  target: GameStateTarget,
  installed: boolean
): boolean => {
  return applyGameStateMutationInState(target, (existingGame) => ({
    ...existingGame,
    installed,
    uninstalling: installed ? existingGame.uninstalling : false,
  }));
};

const setGameUninstallingStateInState = (
  target: GameStateTarget,
  uninstalling: boolean
): boolean => {
  return applyGameStateMutationInState(target, (existingGame) => ({
    ...existingGame,
    uninstalling,
  }));
};

const isGameInstalledInState = (target: GameStateTarget): boolean => {
  if (typeof target.gameId === "string" && target.gameId.trim().length > 0) {
    const gameById = libraryCatalogStore.gameById.get(target.gameId);
    if (gameById?.installed) {
      return true;
    }
  }

  const normalizedProvider = target.provider.trim().toLocaleLowerCase();
  return libraryCatalogStore.allGames.some((game) =>
    game.installed
    && game.externalId === target.externalId
    && game.provider.trim().toLocaleLowerCase() === normalizedProvider
  );
};

const clearPendingUninstallingGameState = (): boolean => {
  let didUpdateGameState = false;
  for (const target of pendingUninstallVerificationByKey.values()) {
    if (setGameUninstallingStateInState(target, false)) {
      didUpdateGameState = true;
    }
  }
  return didUpdateGameState;
};

const markCompletedDownloadsAsInstalled = (downloads: SteamDownloadProgressPayload[]): void => {
  let didUpdateInstalledState = false;

  for (const download of downloads) {
    if (setGameInstalledStateInState(download, true)) {
      didUpdateInstalledState = true;
    }
  }

  if (didUpdateInstalledState) {
    renderActiveLibraryView();
    syncDetailsInstallStatusUi();
  }
};

const scheduleDownloadCompletionRefresh = (delayMs: number): void => {
  if (downloadStore.downloadCompletionRefreshTimer !== null) {
    if (delayMs > 0) {
      return;
    }
    window.clearTimeout(downloadStore.downloadCompletionRefreshTimer);
    downloadStore.downloadCompletionRefreshTimer = null;
  }

  downloadStore.downloadCompletionRefreshTimer = window.setTimeout(() => {
    downloadStore.downloadCompletionRefreshTimer = null;
    void runDownloadCompletionRefresh();
  }, delayMs);
};

const queueDownloadInstallVerification = (downloads: SteamDownloadProgressPayload[]): void => {
  if (!sessionStore.steamLinked || downloads.length === 0) {
    return;
  }

  let didQueue = false;
  for (const download of downloads) {
    const key = getDownloadEtaKey(download);
    if (downloadStore.pendingInstallVerificationByKey.has(key)) {
      continue;
    }
    downloadStore.pendingInstallVerificationByKey.set(key, download);
    didQueue = true;
  }

  if (!didQueue) {
    return;
  }

  scheduleDownloadCompletionRefresh(0);
};

const runDownloadCompletionRefresh = async (): Promise<void> => {
  if (!sessionStore.steamLinked) {
    downloadStore.pendingInstallVerificationByKey.clear();
    downloadStore.downloadCompletionRefreshAttemptCount = 0;
    return;
  }

  if (downloadStore.pendingInstallVerificationByKey.size === 0) {
    downloadStore.downloadCompletionRefreshAttemptCount = 0;
    return;
  }

  if (downloadStore.isDownloadCompletionRefreshInFlight) {
    return;
  }

  downloadStore.isDownloadCompletionRefreshInFlight = true;
  let didUpdateInstalledState = false;
  try {
    const queuedDownloads = [...downloadStore.pendingInstallVerificationByKey.values()];
    const installChecks = await Promise.all(queuedDownloads.map(async (download) => {
      if (isGameInstalledInState(download)) {
        return { key: getDownloadEtaKey(download), installed: true };
      }

      const installationDetails = await getGameInstallationDetailsForDownload(download);
      const installPath = installationDetails?.installPath?.trim();
      const isInstalled = typeof installPath === "string" && installPath.length > 0;
      if (isInstalled && setGameInstalledStateInState(download, true)) {
        didUpdateInstalledState = true;
      }
      return { key: getDownloadEtaKey(download), installed: isInstalled };
    }));

    for (const installCheck of installChecks) {
      if (installCheck.installed) {
        downloadStore.pendingInstallVerificationByKey.delete(installCheck.key);
      }
    }
  } finally {
    downloadStore.isDownloadCompletionRefreshInFlight = false;
  }

  if (didUpdateInstalledState) {
    renderActiveLibraryView();
    syncDetailsInstallStatusUi();
  }

  if (downloadStore.pendingInstallVerificationByKey.size === 0) {
    downloadStore.downloadCompletionRefreshAttemptCount = 0;
    return;
  }

  downloadStore.downloadCompletionRefreshAttemptCount += 1;
  if (downloadStore.downloadCompletionRefreshAttemptCount >= DOWNLOAD_COMPLETION_REFRESH_MAX_ATTEMPTS) {
    downloadStore.pendingInstallVerificationByKey.clear();
    downloadStore.downloadCompletionRefreshAttemptCount = 0;
    return;
  }

  scheduleDownloadCompletionRefresh(DOWNLOAD_COMPLETION_REFRESH_RETRY_DELAY_MS);
};

const scheduleUninstallVerification = (delayMs: number): void => {
  if (uninstallVerificationTimer !== null) {
    if (delayMs > 0) {
      return;
    }
    window.clearTimeout(uninstallVerificationTimer);
    uninstallVerificationTimer = null;
  }

  uninstallVerificationTimer = window.setTimeout(() => {
    uninstallVerificationTimer = null;
    void runUninstallVerification();
  }, delayMs);
};

const queueGameUninstallVerification = (game: GameResponse): void => {
  if (!sessionStore.steamLinked) {
    return;
  }

  const key = getGameIdentityKey(game.provider, game.externalId);
  const pendingTarget: PendingUninstallVerification = {
    gameId: game.id,
    provider: game.provider,
    externalId: game.externalId,
  };
  pendingUninstallVerificationByKey.set(key, pendingTarget);
  if (setGameUninstallingStateInState(pendingTarget, true)) {
    renderActiveLibraryView();
    syncDetailsInstallStatusUi();
  }
  scheduleUninstallVerification(0);
};

const runUninstallVerification = async (): Promise<void> => {
  if (!sessionStore.steamLinked) {
    const didClearPendingState = clearPendingUninstallingGameState();
    pendingUninstallVerificationByKey.clear();
    uninstallVerificationAttemptCount = 0;
    if (didClearPendingState) {
      renderActiveLibraryView();
      syncDetailsInstallStatusUi();
    }
    return;
  }

  if (pendingUninstallVerificationByKey.size === 0) {
    uninstallVerificationAttemptCount = 0;
    return;
  }

  if (isUninstallVerificationInFlight) {
    return;
  }

  isUninstallVerificationInFlight = true;
  let didUpdateGameState = false;
  try {
    const pendingTargets = [...pendingUninstallVerificationByKey.entries()];
    const verificationChecks = await Promise.all(pendingTargets.map(async ([key, target]) => {
      if (!isGameInstalledInState(target)) {
        if (setGameUninstallingStateInState(target, false)) {
          didUpdateGameState = true;
        }
        return { key, isUninstalled: true };
      }

      const installationDetails = await getGameInstallationDetailsForIdentity(
        target.provider,
        target.externalId
      );
      const installPath = installationDetails?.installPath?.trim();
      const isInstalled = typeof installPath === "string" && installPath.length > 0;
      if (!isInstalled && setGameInstalledStateInState(target, false)) {
        didUpdateGameState = true;
      }
      return { key, isUninstalled: !isInstalled };
    }));

    for (const verificationCheck of verificationChecks) {
      if (verificationCheck.isUninstalled) {
        pendingUninstallVerificationByKey.delete(verificationCheck.key);
      }
    }
  } finally {
    isUninstallVerificationInFlight = false;
  }

  if (didUpdateGameState) {
    renderActiveLibraryView();
    syncDetailsInstallStatusUi();
  }

  if (pendingUninstallVerificationByKey.size === 0) {
    uninstallVerificationAttemptCount = 0;
    return;
  }

  uninstallVerificationAttemptCount += 1;
  if (uninstallVerificationAttemptCount >= UNINSTALL_VERIFICATION_MAX_ATTEMPTS) {
    const didClearPendingState = clearPendingUninstallingGameState();
    pendingUninstallVerificationByKey.clear();
    uninstallVerificationAttemptCount = 0;
    if (didClearPendingState) {
      renderActiveLibraryView();
      syncDetailsInstallStatusUi();
    }
    return;
  }

  scheduleUninstallVerification(UNINSTALL_VERIFICATION_RETRY_DELAY_MS);
};

const refreshSteamDownloads = async (): Promise<void> => {
  if (!sessionStore.steamLinked || downloadStore.isDownloadPollInFlight) {
    return;
  }

  downloadStore.isDownloadPollInFlight = true;
  try {
    const latestDownloads = await listSteamDownloadsForSession();
    const latestDownloadsByKey = new Map<string, SteamDownloadProgressPayload>();
    for (const download of latestDownloads) {
      latestDownloadsByKey.set(getDownloadEtaKey(download), download);
    }

    const completedDownloads: SteamDownloadProgressPayload[] = [];
    const disappearedDownloads: SteamDownloadProgressPayload[] = [];
    for (const [previousKey, previousDownload] of downloadStore.previousActiveDownloadsByKey) {
      if (latestDownloadsByKey.has(previousKey)) {
        continue;
      }
      disappearedDownloads.push(previousDownload);
      if (isLikelyCompletedDownload(previousDownload)) {
        completedDownloads.push(previousDownload);
      }
    }

    downloadStore.activeDownloads = latestDownloads;
    downloadStore.previousActiveDownloadsByKey = latestDownloadsByKey;
    updateDownloadEtaSnapshots(downloadStore.activeDownloads);
    if (completedDownloads.length > 0) {
      markCompletedDownloadsAsInstalled(completedDownloads);
    }
    if (disappearedDownloads.length > 0) {
      queueDownloadInstallVerification(disappearedDownloads);
    }
  } finally {
    downloadStore.isDownloadPollInFlight = false;
    renderDownloadActivity();
  }
};

const stopDownloadPolling = (): void => {
  if (downloadStore.downloadPollTimer !== null) {
    window.clearInterval(downloadStore.downloadPollTimer);
    downloadStore.downloadPollTimer = null;
  }
  downloadStore.isDownloadPollInFlight = false;
};

const startDownloadPolling = (): void => {
  stopDownloadPolling();
  void refreshSteamDownloads();
  downloadStore.downloadPollTimer = window.setInterval(() => {
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

const installGameForGame = async (game: GameResponse): Promise<void> => {
  await ipcService.installGame({
    provider: game.provider,
    externalId: game.externalId,
    installPath: "Steam default install location",
    createDesktopShortcut: true,
    createApplicationShortcut: true,
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
      const targetCollectionName = libraryCatalogStore.allCollections.find((collection) => collection.id === collectionId)?.name;
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
      await installGameForGame(game);
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
      if (game.uninstalling === true) {
        showLauncherToast(`"${game.name}" is already uninstalling.`, "error");
        return;
      }

      await uninstallGameForGame(game);
      showLauncherToast(`Opened uninstall flow for "${game.name}".`);
      queueGameUninstallVerification(game);
    },
  },
  container: libraryGridElement,
  onError: (message) => {
    console.error(message);
    showLauncherToast(message, "error");
  },
  resolveGameFromCard,
});
libraryViewStore.closeGameContextMenu = gameContextMenu.closeMenu;

// Wire details action buttons to reuse existing actions where possible
if (detailsSettingsButton instanceof HTMLButtonElement) {
  detailsSettingsButton.addEventListener("click", (e) => {
    const gameId = detailsViewStore.selectedGameId;
    if (!gameId) return;
    const game = findGameById(gameId) ?? libraryCatalogStore.gameById.get(gameId);
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
    const gameId = detailsViewStore.selectedGameId;
    if (!gameId) return;
    const game = findGameById(gameId) ?? libraryCatalogStore.gameById.get(gameId);
    if (!game) return;

    if (game.uninstalling === true) {
      showLauncherToast(`"${game.name}" is currently uninstalling.`, "error");
      return;
    }

    if (game.installed) {
      await ipcService.playGame({ provider: game.provider, externalId: game.externalId });
    } else {
      await installGameForGame(game);
      showLauncherToast(`Queued "${game.name}" for install.`);
      void refreshSteamDownloads();
    }
  });
}

if (detailsFavoriteButton instanceof HTMLButtonElement) {
  detailsFavoriteButton.addEventListener("click", async () => {
    const gameId = detailsViewStore.selectedGameId;
    if (!gameId) return;
    const game = findGameById(gameId) ?? libraryCatalogStore.gameById.get(gameId);
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

const { closeDetailsDropdown, openDetailsDropdown } = createDetailsDropdownView({
  detailsDropdown,
  detailsPropertiesButton,
  resolveGameById: (gameId) => findGameById(gameId) ?? libraryCatalogStore.gameById.get(gameId) ?? null,
  escapeHtml,
});

// Toggle handler
detailsPropertiesButton.addEventListener("click", (ev) => {
  ev.stopPropagation();
  const gameId = detailsViewStore.selectedGameId;
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

const refreshLibrary = async (
  syncBeforeLoad = false,
  importSteamCollections = false,
  throwOnError = false
): Promise<void> => {
  if (librarySyncStore.isLoadingLibrary) {
    if (!throwOnError) {
      return;
    }
    await waitForLibraryLoadToFinish();
  }

  if (librarySyncStore.isLoadingLibrary) {
    return;
  }

  try {
    setLibraryLoadingState(true);

    if (syncBeforeLoad && sessionStore.steamLinked) {
      try {
        await runTaskWithTimeout(
          ipcService.syncSteamLibrary(),
          90_000,
          "Steam sync timed out. Loading cached library."
        );
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
    if (libraryViewStore.activeLibraryViewMode === "collections") {
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
    if (throwOnError) {
      throw new Error(appError.message);
    }
  } finally {
    setLibraryLoadingState(false);
  }
};

const refreshSession = async (throwOnError = false): Promise<SessionRefreshResult> => {
  try {
    const session = await ipcService.getSession();
    if (!session) {
      window.location.replace("/index.html");
      return "redirecting";
    }

    setSessionStatus(session.steamLinked);
    return "ready";
  } catch (error) {
    const appError = normalizeAppError(error, "Could not load session data.");
    console.error(`[session/load] ${appError.kind}:${appError.code} ${appError.message}`);
    setSessionStatus(false, true);
    if (throwOnError) {
      throw new Error(appError.message);
    }
    return "failed";
  }
};

refreshLibraryButton.addEventListener("click", () => {
  void (async () => {
    await refreshLibrary(true, true);
    if (detailsViewStore.appViewMode === "game-details" && detailsViewStore.selectedGameId) {
      renderGameDetails(detailsViewStore.selectedGameId, true);
    }
  })();
});

const applyStartupCoreConfiguration = (): void => {
  setLibraryViewMode("games", false);
  setLibrarySummary("Loading library...");
  renderLibraryLastUpdated();
  renderDownloadActivity();
};

const markActiveStartupStepAsError = (): void => {
  const steps = [startupStepSessionElement, startupStepConfigElement, startupStepLibraryElement];
  const activeStep = steps.find((stepElement) => stepElement.dataset.state === "active");
  if (activeStep) {
    setStartupStepState(activeStep, "error");
  }
};

const runStartupInitialization = async (): Promise<void> => {
  const attemptToken = ++startupAttemptToken;
  showStartupGate();
  resetStartupUi();
  const ensureCurrentStartupAttempt = (): void => {
    if (attemptToken !== startupAttemptToken) {
      throw new Error("Startup attempt was superseded.");
    }
  };

  try {
    const startupResult = await runTaskWithTimeout((async (): Promise<SessionRefreshResult> => {
      setStartupStepState(startupStepSessionElement, "active");
      setStartupStatus("Restoring your session...");
      const sessionStatus = await refreshSession(true);
      ensureCurrentStartupAttempt();
      if (sessionStatus === "redirecting") {
        setStartupStepState(startupStepSessionElement, "done");
        setStartupStatus("Redirecting to sign in...");
        return "redirecting";
      }

      setStartupStepState(startupStepSessionElement, "done");
      setStartupStepState(startupStepConfigElement, "active");
      setStartupStatus("Loading core configuration...");
      applyStartupCoreConfiguration();
      setStartupStepState(startupStepConfigElement, "done");

      setStartupStepState(startupStepLibraryElement, "active");
      setStartupStatus("Syncing Steam library and loading required data...");
      await refreshLibrary(true, true, true);
      ensureCurrentStartupAttempt();
      setStartupStepState(startupStepLibraryElement, "done");
      return "ready";
    })(), STARTUP_TIMEOUT_MS, "Startup timed out. Please retry.");

    if (attemptToken !== startupAttemptToken) {
      return;
    }

    if (startupResult === "redirecting") {
      return;
    }

    setStartupStatus("Library is ready.");
    hideStartupGate();
  } catch (error) {
    if (attemptToken !== startupAttemptToken) {
      return;
    }

    markActiveStartupStepAsError();
    setStartupStatus(getErrorMessage(error, "Startup failed. Please retry."), true);
    startupRetryButtonElement.hidden = false;
    startupRetryButtonElement.disabled = false;
    if (startupAttemptToken === attemptToken) {
      startupAttemptToken += 1;
    }
  }
};

startupRetryButtonElement.addEventListener("click", () => {
  void runStartupInitialization();
});

window.addEventListener("resize", applyLibraryAspectSoftLock);
window.addEventListener("beforeunload", stopDownloadPolling);
window.addEventListener("beforeunload", stopLibraryLastUpdatedTimer);

const initialize = async (): Promise<void> => {
  applyLibraryAspectSoftLock();
  registerGridZoomShortcut();
  const cleanupGridWheelSmoothing = registerLinuxGridWheelSmoothing();
  window.addEventListener("beforeunload", cleanupGridWheelSmoothing, { once: true });
  await runStartupInitialization();
};

void initialize();
