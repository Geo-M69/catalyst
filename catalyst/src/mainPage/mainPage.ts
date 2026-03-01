import { invoke } from "@tauri-apps/api/core";
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
  type GameVersionBetaOption,
} from "./components/gamePropertiesPanel";
import { applyLibraryFilters } from "./filtering";
import type { CollectionResponse, GameResponse, LibraryResponse, PublicUser } from "./types";

export {};

const sessionAccountElement = document.getElementById("session-account");
const sessionAccountButton = document.getElementById("session-account-button");
const sessionAccountLabelElement = document.getElementById("session-account-label");
const sessionAccountMenuElement = document.getElementById("session-account-menu");
const sessionAccountManageButton = document.getElementById("session-account-manage");
const sessionAccountSignOutButton = document.getElementById("session-account-signout");
const libraryViewPickerElement = document.getElementById("library-view-picker");
const libraryViewPickerButton = document.getElementById("library-view-picker-button");
const libraryViewPickerLabelElement = document.getElementById("library-view-picker-label");
const libraryViewPickerMenuElement = document.getElementById("library-view-picker-menu");
const librarySummaryElement = document.getElementById("library-summary");
const refreshLibraryButton = document.getElementById("refresh-library-button");
const downloadActivityElement = document.getElementById("download-activity");
const downloadActivityButton = document.getElementById("download-activity-button");
const downloadActivityCountElement = document.getElementById("download-activity-count");
const downloadActivityPopoverElement = document.getElementById("download-activity-popover");
const downloadActivityListElement = document.getElementById("download-activity-list");
const filterPanelElement = document.getElementById("filter-panel");
const libraryGridElement = document.getElementById("library-grid");
const libraryAspectShellElement = document.getElementById("library-aspect-shell");

if (
  !(sessionAccountElement instanceof HTMLElement)
  || !(sessionAccountButton instanceof HTMLButtonElement)
  || !(sessionAccountLabelElement instanceof HTMLElement)
  || !(sessionAccountMenuElement instanceof HTMLElement)
  || !(sessionAccountManageButton instanceof HTMLButtonElement)
  || !(sessionAccountSignOutButton instanceof HTMLButtonElement)
  || !(libraryViewPickerElement instanceof HTMLElement)
  || !(libraryViewPickerButton instanceof HTMLButtonElement)
  || !(libraryViewPickerLabelElement instanceof HTMLElement)
  || !(libraryViewPickerMenuElement instanceof HTMLElement)
  || !(librarySummaryElement instanceof HTMLElement)
  || !(refreshLibraryButton instanceof HTMLButtonElement)
  || !(downloadActivityElement instanceof HTMLElement)
  || !(downloadActivityButton instanceof HTMLButtonElement)
  || !(downloadActivityCountElement instanceof HTMLElement)
  || !(downloadActivityPopoverElement instanceof HTMLElement)
  || !(downloadActivityListElement instanceof HTMLElement)
  || !(filterPanelElement instanceof HTMLElement)
  || !(libraryGridElement instanceof HTMLElement)
  || !(libraryAspectShellElement instanceof HTMLElement)
) {
  throw new Error("Main page is missing required DOM elements");
}

let allGames: GameResponse[] = [];
let gameById = new Map<string, GameResponse>();
let isLoadingLibrary = false;
let steamLinked = false;
const GRID_CARD_WIDTH_CSS_VAR = "--game-grid-card-min-width";
const GRID_CARD_WIDTH_DEFAULT_PX = 180;
const GRID_CARD_WIDTH_MIN_PX = 140;
const GRID_CARD_WIDTH_MAX_PX = 320;
const GRID_CARD_WIDTH_STEP_PX = 14;
const GRID_CARD_WIDTH_STORAGE_KEY = "catalyst.library.gridCardMinWidthPx";
const APP_NAME = "Catalyst";
const DOWNLOAD_POLL_INTERVAL_MS = 2500;
const TOAST_DURATION_MS = 3200;
const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"];
const LIBRARY_SOFT_LOCK_ASPECTS: ReadonlyArray<{ label: string; ratio: number }> = [
  { label: "16:9", ratio: 16 / 9 },
  { label: "21:9", ratio: 21 / 9 },
  { label: "32:9", ratio: 32 / 9 },
];

const clamp = (value: number, min: number, max: number): number => Math.min(max, Math.max(min, value));
let closeGameContextMenu: (() => void) | null = null;
let downloadPollTimer: number | null = null;
let isDownloadPollInFlight = false;
let activeDownloads: SteamDownloadProgressPayload[] = [];
let allCollections: CollectionResponse[] = [];

type LibraryViewMode = "games" | "collections";
const LIBRARY_VIEW_LABELS: Record<LibraryViewMode, string> = {
  games: "Game Library",
  collections: "Collections",
};
let activeLibraryViewMode: LibraryViewMode = "games";

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

interface GameVersionBetasPayload {
  options: GameVersionBetaOption[];
  warning?: string;
}

interface GamePrivacySettingsPayload {
  hideInLibrary: boolean;
  markAsPrivate: boolean;
  overlayDataDeleted: boolean;
}

interface GameInstallationDetailsPayload {
  installPath?: string;
  sizeOnDiskBytes?: number;
}

interface GameInstallLocationPayload {
  path: string;
  freeSpaceBytes?: number;
}

interface SteamDownloadProgressPayload {
  gameId: string;
  provider: string;
  externalId: string;
  name: string;
  state: string;
  bytesDownloaded?: number;
  bytesTotal?: number;
  progressPercent?: number;
}

type GameInstallSizeEstimatePayload = number | null;

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
  steamLinked = steamConnected && !isError;
  sessionAccountLabelElement.textContent = APP_NAME;
  sessionAccountButton.classList.toggle("is-error", isError);
  sessionAccountManageButton.disabled = isError;
  sessionAccountSignOutButton.disabled = false;
  downloadActivityButton.disabled = isError || !steamLinked;
  if (!steamLinked) {
    stopDownloadPolling();
    activeDownloads = [];
    renderDownloadActivity();
    closeDownloadActivityPopover();
  } else {
    startDownloadPolling();
  }
  closeSessionAccountMenu();
};

const setLibrarySummary = (message: string): void => {
  librarySummaryElement.textContent = message;
  librarySummaryElement.classList.remove("status-error");
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

const formatBytes = (sizeInBytes?: number): string | null => {
  if (typeof sizeInBytes !== "number" || !Number.isFinite(sizeInBytes) || sizeInBytes <= 0) {
    return null;
  }

  let unitIndex = 0;
  let value = sizeInBytes;
  while (value >= 1024 && unitIndex < BYTE_UNITS.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const fractionDigits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(fractionDigits)} ${BYTE_UNITS[unitIndex]}`;
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

const closeDownloadActivityPopover = (): void => {
  downloadActivityPopoverElement.hidden = true;
  downloadActivityElement.classList.remove("is-open");
  downloadActivityButton.setAttribute("aria-expanded", "false");
};

const openDownloadActivityPopover = (): void => {
  downloadActivityPopoverElement.hidden = false;
  downloadActivityElement.classList.add("is-open");
  downloadActivityButton.setAttribute("aria-expanded", "true");
};

const renderDownloadActivity = (): void => {
  const activeCount = activeDownloads.length;
  downloadActivityButton.classList.toggle("has-active-downloads", activeCount > 0);
  downloadActivityCountElement.hidden = activeCount <= 0;
  downloadActivityCountElement.textContent = `${activeCount}`;
  downloadActivityButton.setAttribute(
    "aria-label",
    activeCount > 0 ? `${activeCount} active download${activeCount === 1 ? "" : "s"}` : "Downloads"
  );

  downloadActivityListElement.replaceChildren();
  if (activeCount === 0) {
    const emptyMessage = document.createElement("p");
    emptyMessage.className = "download-activity-empty";
    emptyMessage.textContent = "No active downloads";
    downloadActivityListElement.append(emptyMessage);
    return;
  }

  for (const download of activeDownloads) {
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
    const downloadedLabel = formatBytes(download.bytesDownloaded);
    const totalLabel = formatBytes(download.bytesTotal);
    if (downloadedLabel && totalLabel) {
      meta.textContent = normalizedPercent !== null
        ? `${downloadedLabel} / ${totalLabel} (${Math.round(normalizedPercent)}%)`
        : `${downloadedLabel} / ${totalLabel}`;
    } else if (totalLabel) {
      meta.textContent = `Total ${totalLabel}`;
    } else if (normalizedPercent !== null) {
      meta.textContent = `${Math.round(normalizedPercent)}%`;
    } else {
      meta.textContent = download.state;
    }

    row.append(meta);
    downloadActivityListElement.append(row);
  }
};

const toErrorMessage = (error: unknown, fallbackMessage: string): string => {
  if (typeof error === "string" && error.trim().length > 0) {
    return error;
  }

  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }

  return fallbackMessage;
};

const setLibraryLoadingState = (isLoading: boolean): void => {
  isLoadingLibrary = isLoading;
  refreshLibraryButton.disabled = isLoading;
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

  // Use a "cover" fit so the main layout always fills the body without bars.
  let frameWidth = viewportWidth;
  let frameHeight = Math.ceil(frameWidth / targetAspect.ratio);
  if (frameHeight < viewportHeight) {
    frameHeight = viewportHeight;
    frameWidth = Math.ceil(frameHeight * targetAspect.ratio);
  }

  libraryAspectShellElement.style.setProperty("--library-aspect-width", `${Math.max(frameWidth, 1)}px`);
  libraryAspectShellElement.style.setProperty("--library-aspect-height", `${Math.max(frameHeight, 1)}px`);
  libraryAspectShellElement.style.setProperty("--library-aspect-ratio", `${targetAspect.ratio}`);
  libraryAspectShellElement.dataset.aspectLabel = targetAspect.label;
};

const registerGridZoomShortcut = (): void => {
  const initialWidth = readStoredGridCardWidthPx() ?? readGridCardWidthPx();
  setGridCardWidthPx(initialWidth, false);

  libraryGridElement.addEventListener("wheel", (event) => {
    if (!event.ctrlKey || event.deltaY === 0) {
      return;
    }

    event.preventDefault();
    const delta = event.deltaY < 0 ? GRID_CARD_WIDTH_STEP_PX : -GRID_CARD_WIDTH_STEP_PX;
    setGridCardWidthPx(readGridCardWidthPx() + delta);
  }, { passive: false });
};

const setAllGames = (games: GameResponse[]): void => {
  allGames = games;
  gameById = new Map(games.map((game) => [game.id, game]));
  filterPanel.setSteamTagSuggestions(collectSteamTagSuggestions(games));
};

const setAllCollections = (collections: CollectionResponse[]): void => {
  const sortedCollections = [...collections].sort((left, right) =>
    left.name.localeCompare(right.name, undefined, { sensitivity: "base" })
  );
  allCollections = sortedCollections;
  filterPanel.setCollectionSuggestions(allCollections.map((collection) => collection.name));
};

const normalizeCollectionNameForMatch = (collectionName: string): string => {
  return collectionName.trim().toLocaleLowerCase();
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
  const existingIndex = allCollections.findIndex((existingCollection) => existingCollection.id === collection.id);
  if (existingIndex < 0) {
    setAllCollections([...allCollections, collection]);
    return;
  }

  const nextCollections = [...allCollections];
  nextCollections[existingIndex] = {
    ...nextCollections[existingIndex],
    ...collection,
  };
  setAllCollections(nextCollections);
};

const removeCollectionFromState = (collectionId: string): void => {
  setAllCollections(allCollections.filter((collection) => collection.id !== collectionId));
};

const updateCollectionNameInGames = (previousName: string, nextName: string | null): void => {
  const normalizedPreviousName = normalizeCollectionNameForMatch(previousName);
  if (normalizedPreviousName.length === 0) {
    return;
  }

  const normalizedNextName = nextName === null ? "" : normalizeCollectionNameForMatch(nextName);
  const nextCollectionName = nextName?.trim() ?? "";
  let stateChanged = false;
  const nextGames = allGames.map((game) => {
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

  allGames = nextGames;
  gameById = new Map(nextGames.map((game) => [game.id, game]));
};

const resolveGameFromCard = (card: HTMLElement): GameResponse | null => {
  const gameId = card.dataset.gameId;
  if (!gameId) {
    return null;
  }

  return gameById.get(gameId) ?? null;
};

const updateGameInState = (
  gameId: string,
  update: (game: GameResponse) => GameResponse
): GameResponse | null => {
  const gameIndex = allGames.findIndex((game) => game.id === gameId);
  if (gameIndex < 0) {
    return null;
  }

  const updatedGame = update(allGames[gameIndex]);
  allGames[gameIndex] = updatedGame;
  gameById.set(updatedGame.id, updatedGame);
  return updatedGame;
};

const renderGameLibrary = (): void => {
  closeGameContextMenu?.();
  const collectionGridCleanupTarget = libraryGridElement as HTMLElement & {
    __collectionGridCleanup?: () => void;
  };
  collectionGridCleanupTarget.__collectionGridCleanup?.();
  collectionGridCleanupTarget.__collectionGridCleanup = undefined;
  const filters = filterPanel.getFilters();
  const filteredGames = applyLibraryFilters(allGames, filters);
  const emptyMessage = allGames.length === 0
    ? "No games synced yet."
    : "No games match your current filters.";
  const canRenderCollectionSections = allCollections.length > 0 && filters.collection.trim().length === 0;
  const sections = canRenderCollectionSections
    ? buildCollectionSectionsForGames(filteredGames, allCollections)
    : undefined;

  renderGameGrid({
    container: libraryGridElement,
    games: filteredGames,
    emptyMessage,
    sections,
  });
  setLibrarySummary(`${filteredGames.length} of ${allGames.length} games shown.`);
};

const gamePropertiesPanel = createGamePropertiesPanel();
const installDialog = createInstallDialog();
const collectionNameDialog = createCollectionNameDialog();
const confirmationDialog = createConfirmationDialog();

const listCollectionsForGame = async (game: GameResponse): Promise<CollectionResponse[]> => {
  return invoke<CollectionResponse[]>("list_collections", {
    provider: game.provider,
    externalId: game.externalId,
  });
};

const listCollectionsForUser = async (): Promise<CollectionResponse[]> => {
  return invoke<CollectionResponse[]>("list_collections");
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

  if (activeLibraryViewMode === "games") {
    renderGameLibrary();
  } else {
    renderCollectionLibrary();
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
    const createdCollection = await invoke<CollectionResponse>("create_collection", {
      name: collectionName,
    });
    upsertCollectionInState(createdCollection);
    showLauncherToast(`Created collection "${createdCollection.name}".`);
    setLibraryViewMode("collections", false);
    renderActiveLibraryView();
    void refreshLibrary(false);
  } catch (error) {
    showLauncherToast(toErrorMessage(error, "Could not create collection."), "error");
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
    const updatedCollection = await invoke<CollectionResponse>("rename_collection", {
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
    showLauncherToast(toErrorMessage(error, "Could not rename collection."), "error");
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
    await invoke("delete_collection", {
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
    showLauncherToast(toErrorMessage(error, "Could not delete collection."), "error");
  }
};

const renderCollectionLibrary = (): void => {
  closeGameContextMenu?.();
  const favoritesCount = allGames.filter((game) => game.favorite).length;
  renderCollectionGrid({
    container: libraryGridElement,
    collections: allCollections,
    favoritesCount,
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
      setLibraryViewMode("games", false);
      filterPanel.setCollectionFilter("", false);
      filterPanel.setFilterBy("favorites", false);
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

  const collectionCount = allCollections.length;
  setLibrarySummary(`${collectionCount} collection${collectionCount === 1 ? "" : "s"}.`);
};

const renderActiveLibraryView = (): void => {
  if (activeLibraryViewMode === "collections") {
    renderCollectionLibrary();
    return;
  }

  renderGameLibrary();
};

const closeLibraryViewPicker = (): void => {
  libraryViewPickerMenuElement.hidden = true;
  libraryViewPickerElement.classList.remove("is-open");
  libraryViewPickerButton.setAttribute("aria-expanded", "false");
};

const openLibraryViewPicker = (): void => {
  libraryViewPickerMenuElement.hidden = false;
  libraryViewPickerElement.classList.add("is-open");
  libraryViewPickerButton.setAttribute("aria-expanded", "true");
};

const libraryViewOptionButtons = Array.from(
  libraryViewPickerMenuElement.querySelectorAll(".library-view-picker-option")
).filter((option): option is HTMLButtonElement => option instanceof HTMLButtonElement);
if (libraryViewOptionButtons.length === 0) {
  throw new Error("Library view picker is missing options");
}

const setLibraryViewMode = (viewMode: LibraryViewMode, render = true): void => {
  activeLibraryViewMode = viewMode;
  libraryViewPickerLabelElement.textContent = LIBRARY_VIEW_LABELS[viewMode];

  for (const optionButton of libraryViewOptionButtons) {
    const optionViewMode = optionButton.dataset.libraryView;
    const isSelected = optionViewMode === viewMode;
    optionButton.classList.toggle("is-selected", isSelected);
    optionButton.setAttribute("aria-selected", `${isSelected}`);
  }

  if (render) {
    renderActiveLibraryView();
  }
};

const focusLibraryViewOptionByIndex = (index: number): void => {
  if (libraryViewOptionButtons.length === 0) {
    return;
  }

  const boundedIndex = Math.max(0, Math.min(index, libraryViewOptionButtons.length - 1));
  libraryViewOptionButtons[boundedIndex]?.focus();
};

const focusSelectedLibraryViewOption = (): void => {
  const selectedIndex = libraryViewOptionButtons.findIndex((optionButton) =>
    optionButton.dataset.libraryView === activeLibraryViewMode
  );
  focusLibraryViewOptionByIndex(selectedIndex >= 0 ? selectedIndex : 0);
};

const filterPanel = createFilterPanel(filterPanelElement, () => {
  if (activeLibraryViewMode === "games") {
    renderGameLibrary();
  }
});

const listGameLanguagesForGame = async (game: GameResponse): Promise<string[]> => {
  try {
    return await invoke<string[]>("list_game_languages", {
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
    return await invoke<GameCompatibilityToolOption[]>("list_game_compatibility_tools", {
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return [];
  }
};

const listGameVersionBetasForGame = async (game: GameResponse): Promise<GameVersionBetasPayload> => {
  try {
    return await invoke<GameVersionBetasPayload>("list_game_versions_betas", {
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
    return await invoke<GameBetaAccessCodeValidationResult>("validate_game_beta_access_code", {
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
    return await invoke<GamePrivacySettingsPayload>("get_game_privacy_settings", {
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
  await invoke("set_game_privacy_settings", {
    provider: game.provider,
    externalId: game.externalId,
    hideInLibrary: settings.hideInLibrary,
    markAsPrivate: settings.markAsPrivate,
  });
};

const clearGameOverlayDataForGame = async (game: GameResponse): Promise<void> => {
  await invoke("clear_game_overlay_data", {
    provider: game.provider,
    externalId: game.externalId,
  });
};

const getGameInstallationDetailsForGame = async (game: GameResponse): Promise<GameInstallationDetailsPayload | null> => {
  try {
    return await invoke<GameInstallationDetailsPayload>("get_game_installation_details", {
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return null;
  }
};

const listGameInstallLocationsForGame = async (game: GameResponse): Promise<GameInstallLocationPayload[]> => {
  try {
    return await invoke<GameInstallLocationPayload[]>("list_game_install_locations", {
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return [];
  }
};

const getGameInstallSizeEstimateForGame = async (game: GameResponse): Promise<number | null> => {
  try {
    return await invoke<GameInstallSizeEstimatePayload>("get_game_install_size_estimate", {
      provider: game.provider,
      externalId: game.externalId,
    });
  } catch {
    return null;
  }
};

const listSteamDownloadsForSession = async (): Promise<SteamDownloadProgressPayload[]> => {
  try {
    return await invoke<SteamDownloadProgressPayload[]>("list_steam_downloads");
  } catch {
    return [];
  }
};

const refreshSteamDownloads = async (): Promise<void> => {
  if (!steamLinked || isDownloadPollInFlight) {
    return;
  }

  isDownloadPollInFlight = true;
  try {
    activeDownloads = await listSteamDownloadsForSession();
  } finally {
    isDownloadPollInFlight = false;
    renderDownloadActivity();
  }
};

const stopDownloadPolling = (): void => {
  if (downloadPollTimer !== null) {
    window.clearInterval(downloadPollTimer);
    downloadPollTimer = null;
  }
  isDownloadPollInFlight = false;
};

const startDownloadPolling = (): void => {
  stopDownloadPolling();
  void refreshSteamDownloads();
  downloadPollTimer = window.setInterval(() => {
    void refreshSteamDownloads();
  }, DOWNLOAD_POLL_INTERVAL_MS);
};

const getGamePropertiesSettingsForGame = async (
  game: GameResponse
): Promise<GamePropertiesPersistedSettings | null> => {
  try {
    return await invoke<GamePropertiesPersistedSettings>("get_game_properties_settings", {
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
  await invoke("set_game_properties_settings", {
    provider: game.provider,
    externalId: game.externalId,
    settings,
  });
};

const browseGameInstalledFilesForGame = async (game: GameResponse): Promise<void> => {
  await invoke("browse_game_installed_files", {
    provider: game.provider,
    externalId: game.externalId,
  });
};

const backupGameFilesForGame = async (game: GameResponse): Promise<void> => {
  await invoke("backup_game_files", {
    provider: game.provider,
    externalId: game.externalId,
  });
};

const verifyGameFilesForGame = async (game: GameResponse): Promise<void> => {
  await invoke("verify_game_files", {
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
    persistedSettings,
  ] = await Promise.all([
    listCollectionsForGame(game),
    listGameLanguagesForGame(game),
    listGameCompatibilityToolsForGame(game),
    listGameVersionBetasForGame(game),
    getGamePrivacySettingsForGame(game),
    getGameInstallationDetailsForGame(game),
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
    },
    installationDetails: installationDetails ?? undefined,
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
    },
    deleteOverlayData: async () => {
      await clearGameOverlayDataForGame(game);
    },
    validateBetaAccessCode: async (accessCode: string) => {
      return validateGameBetaAccessCodeForGame(game, accessCode);
    },
  });
};

const gameContextMenu = createGameContextMenu({
  actions: {
    addGameToCollection: async (game, collectionId) => {
      await invoke("add_game_to_collection", {
        collectionId,
        provider: game.provider,
        externalId: game.externalId,
      });
      await syncCollectionStateForGame(game);
      const targetCollectionName = allCollections.find((collection) => collection.id === collectionId)?.name;
      if (targetCollectionName) {
        showLauncherToast(`Added "${game.name}" to "${targetCollectionName}".`);
      } else {
        showLauncherToast(`Updated collections for "${game.name}".`);
      }
      void refreshLibrary(false);
    },
    createCollectionAndAdd: async (game, name) => {
      const createdCollection = await invoke<CollectionResponse>("create_collection", { name });
      await invoke("add_game_to_collection", {
        collectionId: createdCollection.id,
        provider: game.provider,
        externalId: game.externalId,
      });
      await syncCollectionStateForGame(game);
      showLauncherToast(`Created "${createdCollection.name}" and added "${game.name}".`);
      void refreshLibrary(false);
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

      await invoke("install_game", {
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
    openProperties: openGameProperties,
    playGame: async (game) => {
      await invoke("play_game", {
        provider: game.provider,
        externalId: game.externalId,
      });
    },
    setFavorite: async (game, favorite) => {
      await invoke("set_game_favorite", {
        favorite,
        provider: game.provider,
        externalId: game.externalId,
      });
      updateGameInState(game.id, (existingGame) => ({ ...existingGame, favorite }));
      renderGameLibrary();
    },
  },
  container: libraryGridElement,
  onError: (message) => {
    console.error(message);
    showLauncherToast(message, "error");
  },
  resolveGameFromCard,
});
closeGameContextMenu = gameContextMenu.closeMenu;

sessionAccountButton.addEventListener("click", () => {
  if (sessionAccountMenuElement.hidden) {
    closeLibraryViewPicker();
    closeDownloadActivityPopover();
    openSessionAccountMenu();
    return;
  }

  closeSessionAccountMenu();
});

sessionAccountButton.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    closeSessionAccountMenu();
    return;
  }

  if (event.key === "ArrowDown" || event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    closeLibraryViewPicker();
    closeDownloadActivityPopover();
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

libraryViewPickerButton.addEventListener("click", () => {
  if (libraryViewPickerMenuElement.hidden) {
    closeSessionAccountMenu();
    closeDownloadActivityPopover();
    openLibraryViewPicker();
    focusSelectedLibraryViewOption();
    return;
  }

  closeLibraryViewPicker();
});

libraryViewPickerButton.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    closeLibraryViewPicker();
    return;
  }

  if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    closeSessionAccountMenu();
    closeDownloadActivityPopover();
    openLibraryViewPicker();
    focusSelectedLibraryViewOption();
  }
});

for (const optionButton of libraryViewOptionButtons) {
  optionButton.addEventListener("click", () => {
    const optionViewMode = optionButton.dataset.libraryView;
    if (optionViewMode !== "games" && optionViewMode !== "collections") {
      return;
    }

    setLibraryViewMode(optionViewMode);
    closeLibraryViewPicker();
    libraryViewPickerButton.focus();
  });
}

libraryViewPickerMenuElement.addEventListener("keydown", (event) => {
  const activeElement = document.activeElement;
  const focusedIndex = activeElement instanceof HTMLButtonElement
    ? libraryViewOptionButtons.indexOf(activeElement)
    : -1;

  if (event.key === "Escape") {
    event.preventDefault();
    closeLibraryViewPicker();
    libraryViewPickerButton.focus();
    return;
  }

  if (event.key === "ArrowDown") {
    event.preventDefault();
    focusLibraryViewOptionByIndex(Math.min(focusedIndex + 1, libraryViewOptionButtons.length - 1));
    return;
  }

  if (event.key === "ArrowUp") {
    event.preventDefault();
    focusLibraryViewOptionByIndex(Math.max(focusedIndex - 1, 0));
    return;
  }

  if (event.key === "Home") {
    event.preventDefault();
    focusLibraryViewOptionByIndex(0);
    return;
  }

  if (event.key === "End") {
    event.preventDefault();
    focusLibraryViewOptionByIndex(libraryViewOptionButtons.length - 1);
    return;
  }

  if (event.key === "Enter" || event.key === " ") {
    if (activeElement instanceof HTMLButtonElement && focusedIndex >= 0) {
      event.preventDefault();
      activeElement.click();
    }
  }
});

downloadActivityButton.addEventListener("click", () => {
  if (downloadActivityButton.disabled) {
    return;
  }

  if (downloadActivityPopoverElement.hidden) {
    closeLibraryViewPicker();
    closeSessionAccountMenu();
    openDownloadActivityPopover();
    return;
  }

  closeDownloadActivityPopover();
});

downloadActivityButton.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    closeDownloadActivityPopover();
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

  if (!downloadActivityElement.contains(target)) {
    closeDownloadActivityPopover();
  }

  if (!libraryViewPickerElement.contains(target)) {
    closeLibraryViewPicker();
  }
});

window.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    closeDownloadActivityPopover();
    closeLibraryViewPicker();
  }
});

sessionAccountManageButton.addEventListener("click", () => {
  closeSessionAccountMenu();
});

sessionAccountSignOutButton.addEventListener("click", () => {
  closeSessionAccountMenu();
  void (async () => {
    try {
      await invoke("logout");
      window.location.replace("/index.html");
    } catch (error) {
      console.error(toErrorMessage(error, "Could not sign out."));
    }
  })();
});

const refreshLibrary = async (syncBeforeLoad = false, importSteamCollections = false): Promise<void> => {
  if (isLoadingLibrary) {
    return;
  }

  try {
    setLibraryLoadingState(true);

    if (syncBeforeLoad && steamLinked) {
      try {
        await invoke("sync_steam_library");
      } catch (error) {
        console.error(toErrorMessage(error, "Steam sync failed. Loading cached library."));
      }

      if (importSteamCollections) {
        try {
          await invoke("import_steam_collections");
        } catch (error) {
          console.error(toErrorMessage(error, "Steam collection import failed."));
        }
      }
    }

    const [library, collections] = await Promise.all([
      invoke<LibraryResponse>("get_library"),
      listCollectionsForUser().catch(() => []),
    ]);
    setAllGames(library.games);
    setAllCollections(collections);
    renderActiveLibraryView();
  } catch (error) {
    setAllGames([]);
    setAllCollections([]);
    if (activeLibraryViewMode === "collections") {
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
    console.error(toErrorMessage(error, "Could not load library."));
  } finally {
    setLibraryLoadingState(false);
  }
};

const refreshSession = async (): Promise<boolean> => {
  try {
    const session = await invoke<PublicUser | null>("get_session");
    if (!session) {
      window.location.replace("/index.html");
      return false;
    }

    setSessionStatus(session.steamLinked);
    return true;
  } catch (error) {
    console.error(toErrorMessage(error, "Could not load session data."));
    setSessionStatus(false, true);
    return false;
  }
};

refreshLibraryButton.addEventListener("click", () => {
  void refreshLibrary(true, true);
});

window.addEventListener("resize", applyLibraryAspectSoftLock);
window.addEventListener("beforeunload", stopDownloadPolling);

const initialize = async (): Promise<void> => {
  applyLibraryAspectSoftLock();
  registerGridZoomShortcut();
  setLibraryViewMode("games", false);
  closeLibraryViewPicker();
  setLibrarySummary("Loading library...");
  renderDownloadActivity();
  closeDownloadActivityPopover();

  const hasSession = await refreshSession();
  if (!hasSession) {
    return;
  }

  await refreshLibrary(true);
};

void initialize();
