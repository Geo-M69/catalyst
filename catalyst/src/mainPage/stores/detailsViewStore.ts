import type { LibraryViewMode } from "./libraryViewTypes";

export const detailsViewStore = {
  appViewMode: "library" as "library" | "game-details",
  selectedGameId: null as string | null,
  preservedLibraryScrollTop: 0,
  preservedLibraryViewMode: "games" as LibraryViewMode,
};
