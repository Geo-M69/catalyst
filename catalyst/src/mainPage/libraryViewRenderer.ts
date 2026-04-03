import { type CollectionGridItem, renderCollectionGrid } from "./components/collectionGrid";
import { type FilterPanelController } from "./components/filterPanel";
import { type GameGridSection, renderGameGrid } from "./components/gameGrid";
import { applyLibraryFilters } from "./filtering";
import { HIDDEN_GAMES_COLLECTION_NAME, type CollectionResponse, type GameResponse } from "./types";
import {
  isCollectionLibraryViewMode,
  libraryCatalogStore,
  libraryViewStore,
  type LibraryViewMode,
} from "./stores";

interface LibraryViewRendererOptions {
  libraryGridElement: HTMLElement;
  filterPanel: FilterPanelController;
  setLibrarySummary: (message: string) => void;
  setLibraryViewMode: (viewMode: LibraryViewMode, render?: boolean) => void;
  onCreateCollection: () => void;
  onRenameCollection: (collection: CollectionGridItem) => void;
  onDeleteCollection: (collection: CollectionGridItem) => void;
}

interface LibraryViewRenderer {
  renderGameLibrary: () => void;
  renderCollectionLibrary: () => void;
  renderActiveLibraryView: () => void;
}

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
  return libraryCatalogStore.allGames.filter((game) => game.hideInLibrary === true).length;
};

const countVisibleFavoriteGames = (): number => {
  return libraryCatalogStore.allGames.filter((game) => game.favorite && game.hideInLibrary !== true).length;
};

const buildVisibleCollectionGameCounts = (): Map<string, number> => {
  const countsByCollection = new Map<string, number>();

  for (const game of libraryCatalogStore.allGames) {
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
    const collection = collections[index];
    if (collection) {
      collectionNameIndex.set(normalizeCollectionNameForMatch(collection.name), index);
    }
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
        targetSection = sections[sectionIndex] ?? null;
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

export const createLibraryViewRenderer = ({
  libraryGridElement,
  filterPanel,
  setLibrarySummary,
  setLibraryViewMode,
  onCreateCollection,
  onRenameCollection,
  onDeleteCollection,
}: LibraryViewRendererOptions): LibraryViewRenderer => {
  const renderGameLibrary = (): void => {
    libraryViewStore.closeGameContextMenu?.();
    const collectionGridCleanupTarget = libraryGridElement as HTMLElement & {
      __collectionGridCleanup?: () => void;
    };
    collectionGridCleanupTarget.__collectionGridCleanup?.();
    delete collectionGridCleanupTarget.__collectionGridCleanup;
    const filters = filterPanel.getFilters();
    const viewScopedGames = getGamesForLibraryViewMode(libraryCatalogStore.allGames, libraryViewStore.activeLibraryViewMode);
    const showOnlyHiddenGames = isHiddenGamesCollectionFilter(filters.collection);
    const eligibleGameCount = viewScopedGames.filter((game) =>
      showOnlyHiddenGames ? game.hideInLibrary === true : game.hideInLibrary !== true
    ).length;
    const filteredGames = applyLibraryFilters(viewScopedGames, filters);
    const emptyMessage = libraryCatalogStore.allGames.length === 0
      ? "No games synced yet."
      : showOnlyHiddenGames
        ? libraryViewStore.activeLibraryViewMode === "installed"
          ? "No hidden installed games."
          : libraryViewStore.activeLibraryViewMode === "favorites"
            ? "No hidden favorite games."
            : "No hidden games."
        : eligibleGameCount === 0
          ? libraryViewStore.activeLibraryViewMode === "installed"
            ? viewScopedGames.length === 0
              ? "No installed games yet."
              : "All installed games are hidden. Select \"Hidden Games\" in the Collection filter to view them."
            : libraryViewStore.activeLibraryViewMode === "favorites"
              ? viewScopedGames.length === 0
                ? "No favorite games yet."
                : "All favorite games are hidden. Select \"Hidden Games\" in the Collection filter to view them."
              : "All games are hidden. Select \"Hidden Games\" in the Collection filter to view them."
          : libraryViewStore.activeLibraryViewMode === "installed"
            ? "No installed games match your current filters."
            : libraryViewStore.activeLibraryViewMode === "favorites"
              ? "No favorite games match your current filters."
              : "No games match your current filters.";
    const canRenderCollectionSections = libraryCatalogStore.allCollections.length > 0 && filters.collection.trim().length === 0;
    const sections = canRenderCollectionSections
      ? buildCollectionSectionsForGames(filteredGames, libraryCatalogStore.allCollections)
      : undefined;

    renderGameGrid({
      container: libraryGridElement,
      games: filteredGames,
      emptyMessage,
      ...(sections ? { sections } : {}),
    });
    if (libraryViewStore.activeLibraryViewMode === "installed") {
      setLibrarySummary(`${filteredGames.length} of ${eligibleGameCount} installed games shown.`);
      return;
    }

    if (libraryViewStore.activeLibraryViewMode === "favorites") {
      setLibrarySummary(`${filteredGames.length} of ${eligibleGameCount} favorite games shown.`);
      return;
    }

    setLibrarySummary(`${filteredGames.length} of ${eligibleGameCount} games shown.`);
  };

  const renderCollectionLibrary = (): void => {
    libraryViewStore.closeGameContextMenu?.();
    const favoritesCount = countVisibleFavoriteGames();
    const hiddenCount = countHiddenGames();
    const visibleCollectionCounts = buildVisibleCollectionGameCounts();
    const collectionItems: CollectionGridItem[] = libraryCatalogStore.allCollections.map((collection) => ({
      ...collection,
      gameCount: visibleCollectionCounts.get(normalizeCollectionNameForMatch(collection.name)) ?? 0,
    }));

    renderCollectionGrid({
      container: libraryGridElement,
      collections: collectionItems,
      favoritesCount,
      hiddenCount,
      onCreateCollection: () => {
        onCreateCollection();
      },
      onRenameCollection: (collection) => {
        onRenameCollection(collection);
      },
      onDeleteCollection: (collection) => {
        onDeleteCollection(collection);
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

    const collectionCount = libraryCatalogStore.allCollections.length + (hiddenCount > 0 ? 1 : 0);
    setLibrarySummary(`${collectionCount} collection${collectionCount === 1 ? "" : "s"}.`);
  };

  const renderActiveLibraryView = (): void => {
    if (isCollectionLibraryViewMode(libraryViewStore.activeLibraryViewMode)) {
      renderCollectionLibrary();
      return;
    }

    renderGameLibrary();
  };

  return {
    renderGameLibrary,
    renderCollectionLibrary,
    renderActiveLibraryView,
  };
};
