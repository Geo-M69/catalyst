import { createCollectionNameDialog } from "./collectionNameDialog";
import type { CollectionResponse, GameResponse } from "../types";

interface GameContextMenuActions {
  addGameToCollection: (game: GameResponse, collectionId: string) => Promise<void>;
  installGame: (game: GameResponse) => Promise<void>;
  listCollections: (game: GameResponse) => Promise<CollectionResponse[]>;
  openProperties: (game: GameResponse) => Promise<void>;
  playGame: (game: GameResponse) => Promise<void>;
  setFavorite: (game: GameResponse, favorite: boolean) => Promise<void>;
  createCollectionAndAdd: (game: GameResponse, name: string) => Promise<void>;
}

interface CreateGameContextMenuArgs {
  actions: GameContextMenuActions;
  container: HTMLElement;
  onError: (message: string) => void;
  resolveGameFromCard: (card: HTMLElement) => GameResponse | null;
}

export interface GameContextMenuController {
  closeMenu: () => void;
}

const VIEWPORT_PADDING_PX = 8;
const SUBMENU_GAP_PX = 8;

const toErrorMessage = (error: unknown, fallbackMessage: string): string => {
  if (typeof error === "string" && error.trim().length > 0) {
    return error;
  }

  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }

  return fallbackMessage;
};

const clamp = (value: number, min: number, max: number): number => Math.max(min, Math.min(value, max));

const focusCycledButton = (
  buttons: HTMLButtonElement[],
  activeElement: Element | null,
  direction: "next" | "previous"
): void => {
  if (buttons.length === 0) {
    return;
  }

  const activeIndex = activeElement instanceof HTMLButtonElement ? buttons.indexOf(activeElement) : -1;
  if (activeIndex < 0) {
    buttons[0].focus();
    return;
  }

  const nextIndex = direction === "next"
    ? (activeIndex + 1) % buttons.length
    : (activeIndex - 1 + buttons.length) % buttons.length;
  buttons[nextIndex].focus();
};

const resolveCardFromTarget = (target: EventTarget | null): HTMLElement | null => {
  if (!(target instanceof Element)) {
    return null;
  }

  const cardElement = target.closest(".game-card");
  return cardElement instanceof HTMLElement ? cardElement : null;
};

const positionWithinViewport = (element: HTMLElement, x: number, y: number): void => {
  const { innerWidth, innerHeight } = window;
  const width = element.offsetWidth;
  const height = element.offsetHeight;
  const left = clamp(x, VIEWPORT_PADDING_PX, Math.max(VIEWPORT_PADDING_PX, innerWidth - width - VIEWPORT_PADDING_PX));
  const top = clamp(y, VIEWPORT_PADDING_PX, Math.max(VIEWPORT_PADDING_PX, innerHeight - height - VIEWPORT_PADDING_PX));
  element.style.left = `${left}px`;
  element.style.top = `${top}px`;
};

export const createGameContextMenu = ({
  actions,
  container,
  onError,
  resolveGameFromCard,
}: CreateGameContextMenuArgs): GameContextMenuController => {
  const collectionNameDialog = createCollectionNameDialog();
  const menu = document.createElement("div");
  menu.className = "game-context-menu";
  menu.setAttribute("role", "menu");
  menu.hidden = true;

  const primaryButton = document.createElement("button");
  primaryButton.type = "button";
  primaryButton.className = "game-context-menu-item";

  const favoriteButton = document.createElement("button");
  favoriteButton.type = "button";
  favoriteButton.className = "game-context-menu-item";

  const collectionsButton = document.createElement("button");
  collectionsButton.type = "button";
  collectionsButton.className = "game-context-menu-item game-context-menu-has-submenu";
  collectionsButton.textContent = "Add to Collection";
  collectionsButton.setAttribute("aria-haspopup", "menu");
  collectionsButton.setAttribute("aria-expanded", "false");

  const propertiesButton = document.createElement("button");
  propertiesButton.type = "button";
  propertiesButton.className = "game-context-menu-item";
  propertiesButton.textContent = "Properties";

  menu.append(primaryButton, favoriteButton, collectionsButton, propertiesButton);

  const collectionSubmenu = document.createElement("div");
  collectionSubmenu.className = "game-context-submenu";
  collectionSubmenu.setAttribute("role", "menu");
  collectionSubmenu.hidden = true;

  document.body.append(menu, collectionSubmenu);

  let activeCard: HTMLElement | null = null;
  let activeGame: GameResponse | null = null;
  let collectionActionButtons: HTMLButtonElement[] = [];
  let submenuRequestId = 0;

  const mainActionButtons = [primaryButton, favoriteButton, collectionsButton, propertiesButton];

  const closeCollectionSubmenu = (): void => {
    collectionSubmenu.hidden = true;
    collectionSubmenu.replaceChildren();
    collectionActionButtons = [];
    collectionsButton.setAttribute("aria-expanded", "false");
    collectionsButton.classList.remove("is-open");
  };

  const closeMenu = (): void => {
    if (menu.hidden) {
      return;
    }

    submenuRequestId += 1;
    menu.hidden = true;
    closeCollectionSubmenu();
    activeGame = null;
    activeCard = null;
  };

  const runMenuAction = async (action: () => Promise<void>, fallbackMessage: string): Promise<void> => {
    try {
      await action();
    } catch (error) {
      onError(toErrorMessage(error, fallbackMessage));
    } finally {
      closeMenu();
    }
  };

  const positionCollectionSubmenu = (): void => {
    const menuRect = menu.getBoundingClientRect();
    let left = menuRect.right + SUBMENU_GAP_PX;
    if (left + collectionSubmenu.offsetWidth + VIEWPORT_PADDING_PX > window.innerWidth) {
      left = menuRect.left - collectionSubmenu.offsetWidth - SUBMENU_GAP_PX;
    }

    const buttonRect = collectionsButton.getBoundingClientRect();
    const preferredTop = buttonRect.top;
    const maxTop = Math.max(
      VIEWPORT_PADDING_PX,
      window.innerHeight - collectionSubmenu.offsetHeight - VIEWPORT_PADDING_PX
    );
    const top = clamp(preferredTop, VIEWPORT_PADDING_PX, maxTop);

    collectionSubmenu.style.left = `${left}px`;
    collectionSubmenu.style.top = `${top}px`;
  };

  const renderCollectionButtons = (
    collections: CollectionResponse[],
    onCollectionClick: (collection: CollectionResponse) => void
  ): void => {
    collectionSubmenu.replaceChildren();

    for (const collection of collections) {
      const collectionButton = document.createElement("button");
      collectionButton.type = "button";
      collectionButton.className = "game-context-menu-item";
      collectionButton.textContent = collection.containsGame ? `${collection.name} (Added)` : collection.name;
      if (collection.containsGame) {
        collectionButton.classList.add("is-selected");
      }
      collectionButton.addEventListener("click", () => {
        onCollectionClick(collection);
      });
      collectionSubmenu.append(collectionButton);
    }

    const createCollectionButton = document.createElement("button");
    createCollectionButton.type = "button";
    createCollectionButton.className = "game-context-menu-item";
    createCollectionButton.textContent = "+ Create New Collection...";
    createCollectionButton.addEventListener("click", () => {
      const selectedGame = activeGame;
      if (!selectedGame) {
        return;
      }

      closeMenu();
      void (async () => {
        const name = await collectionNameDialog.open();
        if (name === null) {
          return;
        }

        try {
          await actions.createCollectionAndAdd(selectedGame, name);
        } catch (error) {
          onError(toErrorMessage(error, "Could not create collection."));
        }
      })();
    });
    collectionSubmenu.append(createCollectionButton);

    collectionActionButtons = Array.from(
      collectionSubmenu.querySelectorAll("button")
    ).filter((button): button is HTMLButtonElement => button instanceof HTMLButtonElement);
  };

  const openCollectionSubmenu = async (focusFirstButton: boolean): Promise<void> => {
    const game = activeGame;
    if (!game || menu.hidden) {
      return;
    }

    const requestId = ++submenuRequestId;
    collectionSubmenu.replaceChildren();
    const loadingButton = document.createElement("button");
    loadingButton.type = "button";
    loadingButton.className = "game-context-menu-item";
    loadingButton.disabled = true;
    loadingButton.textContent = "Loading collections...";
    collectionSubmenu.append(loadingButton);
    collectionActionButtons = [loadingButton];
    collectionSubmenu.hidden = false;
    collectionsButton.setAttribute("aria-expanded", "true");
    collectionsButton.classList.add("is-open");
    positionCollectionSubmenu();

    try {
      const collections = await actions.listCollections(game);
      if (requestId !== submenuRequestId || menu.hidden) {
        return;
      }

      renderCollectionButtons(collections, (collection) => {
        const currentGame = activeGame;
        if (!currentGame) {
          return;
        }

        void runMenuAction(
          () => actions.addGameToCollection(currentGame, collection.id),
          "Could not add game to collection."
        );
      });
      collectionSubmenu.hidden = false;
      collectionsButton.setAttribute("aria-expanded", "true");
      collectionsButton.classList.add("is-open");
      positionCollectionSubmenu();
      if (focusFirstButton) {
        collectionActionButtons[0]?.focus();
      }
    } catch (error) {
      if (requestId !== submenuRequestId) {
        return;
      }

      onError(toErrorMessage(error, "Could not load collections."));
      closeCollectionSubmenu();
    }
  };

  const openMenu = (game: GameResponse, card: HTMLElement, x: number, y: number): void => {
    activeGame = game;
    activeCard = card;
    closeCollectionSubmenu();
    primaryButton.textContent = game.installed ? "Play" : "Install";
    favoriteButton.textContent = game.favorite ? "Remove from Favorites" : "Add to Favorites";
    menu.hidden = false;
    positionWithinViewport(menu, x, y);
    primaryButton.focus();
  };

  primaryButton.addEventListener("click", () => {
    const game = activeGame;
    if (!game) {
      return;
    }

    if (game.installed) {
      void runMenuAction(() => actions.playGame(game), "Could not launch game.");
      return;
    }

    void runMenuAction(() => actions.installGame(game), "Could not install game.");
  });

  favoriteButton.addEventListener("click", () => {
    const game = activeGame;
    if (!game) {
      return;
    }

    void runMenuAction(
      () => actions.setFavorite(game, !game.favorite),
      "Could not update favorite status."
    );
  });

  propertiesButton.addEventListener("click", () => {
    const game = activeGame;
    if (!game) {
      return;
    }

    void runMenuAction(() => actions.openProperties(game), "Could not load game properties.");
  });

  collectionsButton.addEventListener("click", () => {
    if (!collectionSubmenu.hidden) {
      closeCollectionSubmenu();
      return;
    }

    void openCollectionSubmenu(true);
  });

  collectionsButton.addEventListener("pointerenter", () => {
    if (!menu.hidden) {
      void openCollectionSubmenu(false);
    }
  });

  menu.addEventListener("focusin", (event) => {
    if (event.target instanceof HTMLButtonElement && event.target !== collectionsButton) {
      closeCollectionSubmenu();
    }
  });

  menu.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      const cardToFocus = activeCard;
      closeMenu();
      cardToFocus?.focus();
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusCycledButton(mainActionButtons, document.activeElement, "next");
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      focusCycledButton(mainActionButtons, document.activeElement, "previous");
      return;
    }

    if (event.key === "ArrowRight" && document.activeElement === collectionsButton) {
      event.preventDefault();
      void openCollectionSubmenu(true);
      return;
    }

    if (event.key === "Enter" || event.key === " ") {
      const activeElement = document.activeElement;
      if (activeElement instanceof HTMLButtonElement && mainActionButtons.includes(activeElement)) {
        event.preventDefault();
        activeElement.click();
      }
    }
  });

  collectionSubmenu.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      closeCollectionSubmenu();
      collectionsButton.focus();
      return;
    }

    if (event.key === "ArrowLeft") {
      event.preventDefault();
      closeCollectionSubmenu();
      collectionsButton.focus();
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusCycledButton(collectionActionButtons, document.activeElement, "next");
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      focusCycledButton(collectionActionButtons, document.activeElement, "previous");
      return;
    }

    if (event.key === "Enter" || event.key === " ") {
      const activeElement = document.activeElement;
      if (activeElement instanceof HTMLButtonElement && collectionActionButtons.includes(activeElement)) {
        event.preventDefault();
        activeElement.click();
      }
    }
  });

  container.addEventListener("contextmenu", (event) => {
    const card = resolveCardFromTarget(event.target);
    if (!card) {
      return;
    }

    const game = resolveGameFromCard(card);
    if (!game) {
      return;
    }

    event.preventDefault();
    openMenu(game, card, event.clientX, event.clientY);
  });

  container.addEventListener("keydown", (event) => {
    if (event.key !== "ContextMenu" && !(event.key === "F10" && event.shiftKey)) {
      return;
    }

    const card = resolveCardFromTarget(event.target);
    if (!card) {
      return;
    }

    const game = resolveGameFromCard(card);
    if (!game) {
      return;
    }

    event.preventDefault();
    const cardRect = card.getBoundingClientRect();
    openMenu(game, card, cardRect.left + 24, cardRect.top + 24);
  });

  document.addEventListener("pointerdown", (event) => {
    if (menu.hidden) {
      return;
    }

    const target = event.target;
    if (!(target instanceof Node)) {
      closeMenu();
      return;
    }

    if (!menu.contains(target) && !collectionSubmenu.contains(target)) {
      closeMenu();
    }
  });

  window.addEventListener("blur", closeMenu);

  return {
    closeMenu,
  };
};
