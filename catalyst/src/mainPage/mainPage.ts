import { invoke } from "@tauri-apps/api/core";
import { createFilterPanel } from "./components/filterPanel";
import { renderGameGrid } from "./components/gameGrid";
import { renderOptionsPanel } from "./components/optionsPanel";
import { applyLibraryFilters } from "./filtering";
import type { GameResponse, LibraryResponse, PublicUser } from "./types";

export {};

const sessionStatusElement = document.getElementById("session-status");
const librarySummaryElement = document.getElementById("library-summary");
const refreshLibraryButton = document.getElementById("refresh-library-button");
const filterPanelElement = document.getElementById("filter-panel");
const libraryGridElement = document.getElementById("library-grid");
const optionsListElement = document.getElementById("options-list");

if (
  !(sessionStatusElement instanceof HTMLElement)
  || !(librarySummaryElement instanceof HTMLElement)
  || !(refreshLibraryButton instanceof HTMLButtonElement)
  || !(filterPanelElement instanceof HTMLElement)
  || !(libraryGridElement instanceof HTMLElement)
  || !(optionsListElement instanceof HTMLElement)
) {
  throw new Error("Main page is missing required DOM elements");
}

let allGames: GameResponse[] = [];
let isLoadingLibrary = false;
const GRID_CARD_WIDTH_CSS_VAR = "--game-grid-card-min-width";
const GRID_CARD_WIDTH_DEFAULT_PX = 180;
const GRID_CARD_WIDTH_MIN_PX = 140;
const GRID_CARD_WIDTH_MAX_PX = 320;
const GRID_CARD_WIDTH_STEP_PX = 14;
const GRID_CARD_WIDTH_STORAGE_KEY = "catalyst.library.gridCardMinWidthPx";

const clamp = (value: number, min: number, max: number): number => Math.min(max, Math.max(min, value));

const setSessionStatus = (message: string, isError = false): void => {
  sessionStatusElement.textContent = message;
  sessionStatusElement.classList.toggle("status-error", isError);
};

const setLibrarySummary = (message: string, isError = false): void => {
  librarySummaryElement.textContent = message;
  librarySummaryElement.classList.toggle("status-error", isError);
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

const renderFilteredLibrary = (): void => {
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
  setLibrarySummary(`${filteredGames.length} of ${allGames.length} games shown.`);
};

const filterPanel = createFilterPanel(filterPanelElement, () => {
  renderFilteredLibrary();
});

const refreshLibrary = async (): Promise<void> => {
  if (isLoadingLibrary) {
    return;
  }

  try {
    setLibraryLoadingState(true);
    setLibrarySummary("Loading library...");

    const library = await invoke<LibraryResponse>("get_library");
    allGames = library.games;
    renderFilteredLibrary();
  } catch (error) {
    allGames = [];
    renderGameGrid({
      container: libraryGridElement,
      games: [],
      emptyMessage: "Could not load your library.",
    });
    setLibrarySummary(toErrorMessage(error, "Could not load library."), true);
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

    setSessionStatus(`Signed in as ${session.email}.`);
    return true;
  } catch (error) {
    setSessionStatus(toErrorMessage(error, "Could not load session data."), true);
    return false;
  }
};

refreshLibraryButton.addEventListener("click", () => {
  void refreshLibrary();
});

const initialize = async (): Promise<void> => {
  renderOptionsPanel(optionsListElement);
  registerGridZoomShortcut();

  const hasSession = await refreshSession();
  if (!hasSession) {
    return;
  }

  await refreshLibrary();
};

void initialize();
