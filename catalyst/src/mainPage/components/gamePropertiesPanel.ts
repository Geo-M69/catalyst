import type { GameResponse } from "../types";

export interface GamePropertiesInput {
  game: GameResponse;
  collections: string[];
}

export interface GamePropertiesPanelController {
  close: () => void;
  open: (input: GamePropertiesInput) => void;
}

const formatBoolean = (value: boolean): string => value ? "Yes" : "No";

const formatCollections = (collections: string[]): string => {
  if (collections.length === 0) {
    return "None";
  }

  return collections.join(", ");
};

export const createGamePropertiesPanel = (): GamePropertiesPanelController => {
  const backdrop = document.createElement("div");
  backdrop.className = "game-properties-backdrop";
  backdrop.hidden = true;

  const panel = document.createElement("section");
  panel.className = "game-properties-panel";
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-modal", "true");
  panel.setAttribute("aria-labelledby", "game-properties-title");

  const header = document.createElement("header");
  header.className = "game-properties-header";

  const title = document.createElement("h3");
  title.id = "game-properties-title";
  title.className = "game-properties-title";
  title.textContent = "Game Properties";

  const closeButton = document.createElement("button");
  closeButton.type = "button";
  closeButton.className = "game-properties-close session-account-item";
  closeButton.textContent = "Close";

  const detailList = document.createElement("dl");
  detailList.className = "game-properties-list";

  header.append(title, closeButton);
  panel.append(header, detailList);
  backdrop.append(panel);
  document.body.append(backdrop);

  const close = (): void => {
    if (backdrop.hidden) {
      return;
    }

    backdrop.hidden = true;
  };

  const renderRow = (label: string, value: string): void => {
    const labelElement = document.createElement("dt");
    labelElement.className = "game-properties-label";
    labelElement.textContent = label;

    const valueElement = document.createElement("dd");
    valueElement.className = "game-properties-value";
    valueElement.textContent = value;

    detailList.append(labelElement, valueElement);
  };

  const open = (input: GamePropertiesInput): void => {
    detailList.replaceChildren();

    renderRow("Name", input.game.name);
    renderRow("Provider", input.game.provider);
    renderRow("External ID", input.game.externalId);
    renderRow("Kind", input.game.kind);
    renderRow("Installed", formatBoolean(input.game.installed));
    renderRow("Playtime (minutes)", `${input.game.playtimeMinutes}`);
    renderRow("Last Synced", input.game.lastSyncedAt);
    renderRow("Favorite", formatBoolean(input.game.favorite));
    renderRow("Collections", formatCollections(input.collections));

    backdrop.hidden = false;
    closeButton.focus();
  };

  closeButton.addEventListener("click", close);

  backdrop.addEventListener("pointerdown", (event) => {
    if (event.target === backdrop) {
      close();
    }
  });

  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !backdrop.hidden) {
      event.preventDefault();
      close();
    }
  });

  return {
    close,
    open,
  };
};
