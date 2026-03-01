import { invoke } from "@tauri-apps/api/core";
import { createFilterPanel } from "./components/filterPanel";
import { createGameContextMenu } from "./components/gameContextMenu";
import { renderGameGrid } from "./components/gameGrid";
import { createInstallDialog } from "./components/installDialog";
import {
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
const librarySummaryElement = document.getElementById("library-summary");
const refreshLibraryButton = document.getElementById("refresh-library-button");
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
  || !(librarySummaryElement instanceof HTMLElement)
  || !(refreshLibraryButton instanceof HTMLButtonElement)
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
const LIBRARY_SOFT_LOCK_ASPECTS: ReadonlyArray<{ label: string; ratio: number }> = [
  { label: "16:9", ratio: 16 / 9 },
  { label: "21:9", ratio: 21 / 9 },
  { label: "32:9", ratio: 32 / 9 },
];

const clamp = (value: number, min: number, max: number): number => Math.min(max, Math.max(min, value));
let closeGameContextMenu: (() => void) | null = null;

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

const collectCollectionSuggestions = (games: GameResponse[]): string[] => {
  const collectionsByKey = new Map<string, string>();

  for (const game of games) {
    for (const rawCollection of game.collections ?? []) {
      const collection = rawCollection.trim();
      if (collection.length === 0) {
        continue;
      }
      const key = collection.toLocaleLowerCase();
      if (!collectionsByKey.has(key)) {
        collectionsByKey.set(key, collection);
      }
    }
  }

  return [...collectionsByKey.values()].sort((left, right) =>
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
  closeSessionAccountMenu();
};

const setLibrarySummaryCounts = (filteredCount: number, totalCount: number): void => {
  librarySummaryElement.textContent = `${filteredCount} of ${totalCount} games shown.`;
  librarySummaryElement.classList.remove("status-error");
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
  filterPanel.setCollectionSuggestions(collectCollectionSuggestions(games));
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

const renderFilteredLibrary = (): void => {
  closeGameContextMenu?.();
  const filters = filterPanel.getFilters();
  const filteredGames = applyLibraryFilters(allGames, filters);
  const emptyMessage = allGames.length === 0
    ? "No games synced yet."
    : "No games match your current filters.";

  renderGameGrid({
    container: libraryGridElement,
    games: filteredGames,
    emptyMessage,
  });
  setLibrarySummaryCounts(filteredGames.length, allGames.length);
};

const filterPanel = createFilterPanel(filterPanelElement, () => {
  renderFilteredLibrary();
});

const gamePropertiesPanel = createGamePropertiesPanel();
const installDialog = createInstallDialog();

const listCollectionsForGame = async (game: GameResponse): Promise<CollectionResponse[]> => {
  return invoke<CollectionResponse[]>("list_collections", {
    provider: game.provider,
    externalId: game.externalId,
  });
};

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
  const [collections, availableLanguages, versionBetasPayload, privacySettings, installationDetails, persistedSettings] = await Promise.all([
    listCollectionsForGame(game),
    listGameLanguagesForGame(game),
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
    },
    createCollectionAndAdd: async (game, name) => {
      const createdCollection = await invoke<CollectionResponse>("create_collection", { name });
      await invoke("add_game_to_collection", {
        collectionId: createdCollection.id,
        provider: game.provider,
        externalId: game.externalId,
      });
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
      updateGameInState(game.id, (existingGame) => ({ ...existingGame, installed: true }));
      renderFilteredLibrary();
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
      renderFilteredLibrary();
    },
  },
  container: libraryGridElement,
  onError: (message) => {
    console.error(message);
  },
  resolveGameFromCard,
});
closeGameContextMenu = gameContextMenu.closeMenu;

sessionAccountButton.addEventListener("click", () => {
  if (sessionAccountMenuElement.hidden) {
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

document.addEventListener("pointerdown", (event) => {
  const target = event.target;
  if (target instanceof Node && !sessionAccountElement.contains(target)) {
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
      await invoke("logout");
      window.location.replace("/");
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

    const library = await invoke<LibraryResponse>("get_library");
    setAllGames(library.games);
    renderFilteredLibrary();
  } catch (error) {
    setAllGames([]);
    renderGameGrid({
      container: libraryGridElement,
      games: [],
      emptyMessage: "Could not load your library.",
    });
    setLibrarySummaryCounts(0, 0);
    console.error(toErrorMessage(error, "Could not load library."));
  } finally {
    setLibraryLoadingState(false);
  }
};

const refreshSession = async (): Promise<boolean> => {
  try {
    const session = await invoke<PublicUser | null>("get_session");
    if (!session) {
      window.location.replace("/");
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

const initialize = async (): Promise<void> => {
  applyLibraryAspectSoftLock();
  registerGridZoomShortcut();
  setLibrarySummaryCounts(0, 0);

  const hasSession = await refreshSession();
  if (!hasSession) {
    return;
  }

  await refreshLibrary(true);
};

void initialize();
