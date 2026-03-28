import { type CollectionResponse, type GameResponse } from "./types";
import type { SteamDownloadProgressPayload } from "../shared/ipc/contracts";

export interface DownloadEtaSnapshot {
  lastBytesDownloaded: number;
  lastSampleAtMs: number;
  smoothedBytesPerSecond: number;
}

export type LibraryViewMode = "games" | "installed" | "favorites" | "collections";

export const isLibraryViewMode = (value: string | undefined): value is LibraryViewMode => {
  return value === "games" || value === "installed" || value === "favorites" || value === "collections";
};

export const isCollectionLibraryViewMode = (viewMode: LibraryViewMode): viewMode is "collections" => {
  return viewMode === "collections";
};

export const isGameLibraryViewMode = (viewMode: LibraryViewMode): viewMode is Exclude<LibraryViewMode, "collections"> => {
  return viewMode !== "collections";
};

export const store = {
  allGames: [] as GameResponse[],
  gameById: new Map<string, GameResponse>(),
  // App-level view mode: 'library' or 'game-details'
  appViewMode: "library" as "library" | "game-details",
  // Currently selected game id when in `game-details` view
  selectedGameId: null as string | null,
  // Preserve scroll / view state when opening details so we can restore on back
  preservedLibraryScrollTop: 0,
  preservedLibraryViewMode: "games" as LibraryViewMode,
  isLoadingLibrary: false,
  steamLinked: false,
  closeGameContextMenu: null as (() => void) | null,
  downloadPollTimer: null as number | null,
  isDownloadPollInFlight: false,
  activeDownloads: [] as SteamDownloadProgressPayload[],
  previousActiveDownloadsByKey: new Map<string, SteamDownloadProgressPayload>(),
  downloadCompletionRefreshTimer: null as number | null,
  downloadEtaByKey: new Map<string, DownloadEtaSnapshot>(),
  allCollections: [] as CollectionResponse[],
  lastLibraryRefreshAtMs: null as number | null,
  libraryLastUpdatedTimer: null as number | null,
  activeLibraryViewMode: "games" as LibraryViewMode,
};

// Basic selectors/helpers
export const getAllGames = (): GameResponse[] => store.allGames;
export const findGameById = (id: string): GameResponse | undefined => store.gameById.get(id);
