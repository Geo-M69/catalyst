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
