import type { LibraryViewMode } from "./libraryViewTypes";

export const libraryViewStore = {
  activeLibraryViewMode: "games" as LibraryViewMode,
  closeGameContextMenu: null as (() => void) | null,
};
