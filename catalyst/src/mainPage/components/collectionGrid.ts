export interface CollectionGridItem {
  id: string;
  name: string;
  gameCount: number;
}

interface RenderCollectionGridArgs {
  container: HTMLElement;
  collections: CollectionGridItem[];
  favoritesCount?: number;
  onCreateCollection?: () => void;
  onDeleteCollection?: (collection: CollectionGridItem) => void;
  onRenameCollection?: (collection: CollectionGridItem) => void;
  onSelectFavorites?: () => void;
  onSelectCollection?: (collection: CollectionGridItem) => void;
}

interface CollectionGridContainer extends HTMLElement {
  __collectionGridCleanup?: () => void;
}

const clamp = (value: number, min: number, max: number): number => Math.max(min, Math.min(value, max));

export const renderCollectionGrid = ({
  container,
  collections,
  favoritesCount = 0,
  onCreateCollection,
  onDeleteCollection,
  onRenameCollection,
  onSelectFavorites,
  onSelectCollection,
}: RenderCollectionGridArgs): void => {
  const cleanupContainer = container as CollectionGridContainer;
  cleanupContainer.__collectionGridCleanup?.();
  cleanupContainer.__collectionGridCleanup = undefined;

  container.replaceChildren();

  const grid = document.createElement("div");
  grid.className = "collection-grid";

  const contextMenu = document.createElement("div");
  contextMenu.className = "collection-grid-context-menu";
  contextMenu.setAttribute("role", "menu");
  contextMenu.hidden = true;

  const renameAction = document.createElement("button");
  renameAction.type = "button";
  renameAction.className = "collection-grid-context-menu-option";
  renameAction.setAttribute("role", "menuitem");
  renameAction.textContent = "Rename";

  const deleteAction = document.createElement("button");
  deleteAction.type = "button";
  deleteAction.className = "collection-grid-context-menu-option is-danger";
  deleteAction.setAttribute("role", "menuitem");
  deleteAction.textContent = "Delete";

  contextMenu.append(renameAction, deleteAction);
  document.body.append(contextMenu);

  let contextMenuCollection: CollectionGridItem | null = null;

  const closeContextMenu = (): void => {
    contextMenu.hidden = true;
    contextMenuCollection = null;
  };

  const openContextMenu = (collection: CollectionGridItem, clientX: number, clientY: number): void => {
    contextMenuCollection = collection;
    contextMenu.hidden = false;
    contextMenu.style.left = `${clientX}px`;
    contextMenu.style.top = `${clientY}px`;

    const menuRect = contextMenu.getBoundingClientRect();
    const maxLeft = Math.max(8, window.innerWidth - menuRect.width - 8);
    const maxTop = Math.max(8, window.innerHeight - menuRect.height - 8);
    const clampedLeft = clamp(clientX, 8, maxLeft);
    const clampedTop = clamp(clientY, 8, maxTop);
    contextMenu.style.left = `${clampedLeft}px`;
    contextMenu.style.top = `${clampedTop}px`;
  };

  renameAction.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    const selectedCollection = contextMenuCollection;
    closeContextMenu();
    if (selectedCollection) {
      onRenameCollection?.(selectedCollection);
    }
  });

  deleteAction.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    const selectedCollection = contextMenuCollection;
    closeContextMenu();
    if (selectedCollection) {
      onDeleteCollection?.(selectedCollection);
    }
  });

  const createTile = document.createElement("button");
  createTile.type = "button";
  createTile.className = "collection-grid-create";
  createTile.setAttribute("aria-label", "Create a new collection");
  createTile.innerHTML = `
    <span class="collection-grid-create-plus" aria-hidden="true">+</span>
    <span class="collection-grid-create-label">Create a new collection</span>
  `;
  createTile.addEventListener("click", () => {
    closeContextMenu();
    onCreateCollection?.();
  });
  grid.append(createTile);

  if (favoritesCount > 0) {
    const favoritesTile = document.createElement("article");
    favoritesTile.className = "collection-grid-card collection-grid-card-favorites";

    const openFavoritesButton = document.createElement("button");
    openFavoritesButton.type = "button";
    openFavoritesButton.className = "collection-grid-card-open";
    openFavoritesButton.setAttribute("aria-label", `Favorites (${favoritesCount} games)`);

    const favoritesName = document.createElement("span");
    favoritesName.className = "collection-grid-card-name";
    favoritesName.textContent = "Favorites";

    const favoritesCountText = document.createElement("span");
    favoritesCountText.className = "collection-grid-card-count";
    favoritesCountText.textContent = `(${favoritesCount})`;

    openFavoritesButton.append(favoritesName, favoritesCountText);
    openFavoritesButton.addEventListener("click", () => {
      closeContextMenu();
      onSelectFavorites?.();
    });

    favoritesTile.append(openFavoritesButton);
    grid.append(favoritesTile);
  }

  const handlePointerDown = (event: PointerEvent): void => {
    const target = event.target;
    if (target instanceof Node && contextMenu.contains(target)) {
      return;
    }
    closeContextMenu();
  };

  const handleWindowKeyDown = (event: KeyboardEvent): void => {
    if (event.key === "Escape") {
      closeContextMenu();
    }
  };

  const handleWindowBlur = (): void => {
    closeContextMenu();
  };

  const handleContainerScroll = (): void => {
    closeContextMenu();
  };

  document.addEventListener("pointerdown", handlePointerDown);
  window.addEventListener("keydown", handleWindowKeyDown);
  window.addEventListener("blur", handleWindowBlur);
  container.addEventListener("scroll", handleContainerScroll, { passive: true });

  cleanupContainer.__collectionGridCleanup = () => {
    document.removeEventListener("pointerdown", handlePointerDown);
    window.removeEventListener("keydown", handleWindowKeyDown);
    window.removeEventListener("blur", handleWindowBlur);
    container.removeEventListener("scroll", handleContainerScroll);
    contextMenu.remove();
  };

  if (collections.length === 0) {
    container.append(grid);
    return;
  }

  for (const collection of collections) {
    const collectionTile = document.createElement("article");
    collectionTile.className = "collection-grid-card";

    const openCollectionButton = document.createElement("button");
    openCollectionButton.type = "button";
    openCollectionButton.className = "collection-grid-card-open";
    openCollectionButton.setAttribute("aria-label", `${collection.name} (${collection.gameCount} games)`);

    const name = document.createElement("span");
    name.className = "collection-grid-card-name";
    name.textContent = collection.name;

    const count = document.createElement("span");
    count.className = "collection-grid-card-count";
    count.textContent = `(${collection.gameCount})`;

    openCollectionButton.append(name, count);
    openCollectionButton.addEventListener("click", () => {
      closeContextMenu();
      onSelectCollection?.(collection);
    });
    openCollectionButton.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      event.stopPropagation();
      openContextMenu(collection, event.clientX, event.clientY);
    });
    openCollectionButton.addEventListener("keydown", (event) => {
      const isKeyboardContextMenu = event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey);
      if (!isKeyboardContextMenu) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      const bounds = openCollectionButton.getBoundingClientRect();
      const menuAnchorX = Math.round(bounds.left + bounds.width / 2);
      const menuAnchorY = Math.round(bounds.top + Math.min(bounds.height, 40));
      openContextMenu(collection, menuAnchorX, menuAnchorY);
    });

    collectionTile.append(openCollectionButton);
    grid.append(collectionTile);
  }

  container.append(grid);
};
