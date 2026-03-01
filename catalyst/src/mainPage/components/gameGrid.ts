import { createGameCard } from "./gameCard";
import type { GameResponse } from "../types";

interface RenderGameGridArgs {
  container: HTMLElement;
  games: GameResponse[];
  emptyMessage: string;
}

export const renderGameGrid = ({ container, games, emptyMessage }: RenderGameGridArgs): void => {
  container.replaceChildren();

  if (games.length === 0) {
    const emptyState = document.createElement("p");
    emptyState.className = "library-empty-state";
    emptyState.textContent = emptyMessage;
    container.append(emptyState);
    return;
  }

  const grid = document.createElement("div");
  grid.className = "game-grid";

  for (const game of games) {
    grid.append(createGameCard(game));
  }

  container.append(grid);
};
