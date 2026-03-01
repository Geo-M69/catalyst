import { invoke } from "@tauri-apps/api/core";
import { createFilterPanel } from "./components/filterPanel";
import { createGameContextMenu } from "./components/gameContextMenu";
import { renderGameGrid } from "./components/gameGrid";
import { createGamePropertiesPanel } from "./components/gamePropertiesPanel";
import { renderOptionsPanel } from "./components/optionsPanel";
import { applyLibraryFilters } from "./filtering";
import type { CollectionResponse, GameResponse, LibraryResponse, PublicUser } from "./types";

export {};

const sessionAccountElement = document.getElementById("session-account");
const sessionAccountButton = document.getElementById("session-account-button");
const sessionAccountLabelElement = document.getElementById("session-account-label");
const sessionAccountMenuElement = document.getElementById("session-account-menu");
const sessionAccountSteamButton = document.getElementById("session-account-steam");
const sessionAccountSteamIndicator = document.getElementById("session-account-steam-indicator");
const sessionAccountLinkedButton = document.getElementById("session-account-linked");
const sessionAccountSettingsButton = document.getElementById("session-account-settings");
const librarySummaryElement = document.getElementById("library-summary");
const refreshLibraryButton = document.getElementById("refresh-library-button");
const filterPanelElement = document.getElementById("filter-panel");
const libraryGridElement = document.getElementById("library-grid");
const optionsListElement = document.getElementById("options-list");
const panelRightElement = document.querySelector(".panel-right");

if (
  !(sessionAccountElement instanceof HTMLElement)
  || !(sessionAccountButton instanceof HTMLButtonElement)
  || !(sessionAccountLabelElement instanceof HTMLElement)
  || !(sessionAccountMenuElement instanceof HTMLElement)
  || !(sessionAccountSteamButton instanceof HTMLButtonElement)
  || !(sessionAccountSteamIndicator instanceof HTMLElement)
  || !(sessionAccountLinkedButton instanceof HTMLButtonElement)
  || !(sessionAccountSettingsButton instanceof HTMLButtonElement)
  || !(librarySummaryElement instanceof HTMLElement)
  || !(refreshLibraryButton instanceof HTMLButtonElement)
  || !(filterPanelElement instanceof HTMLElement)
  || !(libraryGridElement instanceof HTMLElement)
  || !(optionsListElement instanceof HTMLElement)
  || !(panelRightElement instanceof HTMLElement)
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

const clamp = (value: number, min: number, max: number): number => Math.min(max, Math.max(min, value));
let optionHighlightTimeoutId: number | null = null;
let closeGameContextMenu: (() => void) | null = null;

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
  return [sessionAccountLinkedButton, sessionAccountSettingsButton].filter((button) => !button.disabled);
};

const setSessionStatus = (steamConnected: boolean, isError = false): void => {
  steamLinked = steamConnected && !isError;
  sessionAccountLabelElement.textContent = APP_NAME;
  sessionAccountSteamButton.classList.toggle("is-connected", steamConnected && !isError);
  sessionAccountSteamButton.classList.toggle("is-disconnected", !steamConnected || isError);
  sessionAccountSteamIndicator.setAttribute("aria-label", steamConnected && !isError ? "Steam connected" : "Steam disconnected");
  sessionAccountButton.classList.toggle("is-error", isError);
  sessionAccountLinkedButton.disabled = isError;
  sessionAccountSettingsButton.disabled = isError;
  closeSessionAccountMenu();
};

const setLibrarySummaryCounts = (filteredCount: number, totalCount: number): void => {
  librarySummaryElement.textContent = `${filteredCount} of ${totalCount} games shown.`;
  librarySummaryElement.classList.remove("status-error");
};

const focusOptionsPanel = (titleToHighlight: string | null): void => {
  panelRightElement.scrollIntoView({ behavior: "smooth", block: "start" });

  const optionItems = Array.from(optionsListElement.querySelectorAll(".option-item"));
  const highlightedItem = optionItems.find((optionItem) => {
    if (!(optionItem instanceof HTMLElement)) {
      return false;
    }

    if (titleToHighlight === null) {
      return true;
    }

    const titleElement = optionItem.querySelector(".option-title");
    return titleElement?.textContent?.trim() === titleToHighlight;
  });

  if (!(highlightedItem instanceof HTMLElement)) {
    return;
  }

  highlightedItem.scrollIntoView({ behavior: "smooth", block: "nearest" });
  highlightedItem.classList.add("option-item-highlight");

  if (optionHighlightTimeoutId !== null) {
    window.clearTimeout(optionHighlightTimeoutId);
  }

  optionHighlightTimeoutId = window.setTimeout(() => {
    highlightedItem.classList.remove("option-item-highlight");
    optionHighlightTimeoutId = null;
  }, 1400);
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

const listCollectionsForGame = async (game: GameResponse): Promise<CollectionResponse[]> => {
  return invoke<CollectionResponse[]>("list_collections", {
    provider: game.provider,
    externalId: game.externalId,
  });
};

const openGameProperties = async (game: GameResponse): Promise<void> => {
  const collections = await listCollectionsForGame(game);
  gamePropertiesPanel.open({
    game,
    collections: collections
      .filter((collection) => collection.containsGame)
      .map((collection) => collection.name),
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
      await invoke("install_game", {
        provider: game.provider,
        externalId: game.externalId,
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

sessionAccountLinkedButton.addEventListener("click", () => {
  closeSessionAccountMenu();
  focusOptionsPanel("Connected Accounts");
});

sessionAccountSettingsButton.addEventListener("click", () => {
  closeSessionAccountMenu();
  focusOptionsPanel(null);
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

const initialize = async (): Promise<void> => {
  renderOptionsPanel(optionsListElement);
  registerGridZoomShortcut();
  setLibrarySummaryCounts(0, 0);

  const hasSession = await refreshSession();
  if (!hasSession) {
    return;
  }

  await refreshLibrary(true);
};

void initialize();
