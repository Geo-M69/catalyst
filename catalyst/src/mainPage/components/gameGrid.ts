import { createGameCard } from "./gameCard";
import type { GameResponse } from "../types";

export interface GameGridSection {
  id: string;
  title: string;
  games: GameResponse[];
}

interface RenderGameGridArgs {
  container: HTMLElement;
  games: GameResponse[];
  emptyMessage: string;
  sections?: GameGridSection[];
}

const collapsedSectionIds = new Set<string>();

const resolveSectionId = (section: GameGridSection): string => {
  const normalizedId = section.id.trim();
  if (normalizedId.length > 0) {
    return normalizedId.replace(/[^a-zA-Z0-9_-]/g, "-");
  }

  const normalizedTitle = section.title.trim();
  if (normalizedTitle.length > 0) {
    return normalizedTitle.toLocaleLowerCase().replace(/[^a-z0-9_-]/g, "-");
  }

  return "section-unnamed";
};

const setSectionInteractivity = (sectionGrid: HTMLElement, isCollapsed: boolean): void => {
  sectionGrid.setAttribute("aria-hidden", `${isCollapsed}`);
  if (isCollapsed) {
    sectionGrid.setAttribute("inert", "");
    return;
  }

  sectionGrid.removeAttribute("inert");
};

const setSectionContentExpanded = (sectionContent: HTMLElement): void => {
  sectionContent.style.maxHeight = "none";
  sectionContent.style.opacity = "1";
};

const setSectionContentCollapsed = (sectionContent: HTMLElement): void => {
  sectionContent.style.maxHeight = "0px";
  sectionContent.style.opacity = "0";
};

const animateSectionContent = (
  sectionContent: HTMLElement,
  sectionGrid: HTMLElement,
  isCollapsed: boolean
): void => {
  sectionContent.style.transition = "";
  sectionContent.style.transition = "max-height 220ms ease, opacity 180ms ease";

  if (isCollapsed) {
    setSectionInteractivity(sectionGrid, true);
    sectionContent.style.maxHeight = `${sectionContent.scrollHeight}px`;
    sectionContent.style.opacity = "1";
    window.requestAnimationFrame(() => {
      setSectionContentCollapsed(sectionContent);
    });
    return;
  }

  setSectionInteractivity(sectionGrid, false);
  setSectionContentCollapsed(sectionContent);
  const targetHeight = sectionContent.scrollHeight;
  window.requestAnimationFrame(() => {
    sectionContent.style.maxHeight = `${targetHeight}px`;
    sectionContent.style.opacity = "1";
  });

  const handleTransitionEnd = (event: TransitionEvent): void => {
    if (event.propertyName !== "max-height") {
      return;
    }
    sectionContent.removeEventListener("transitionend", handleTransitionEnd);
    setSectionContentExpanded(sectionContent);
  };
  sectionContent.addEventListener("transitionend", handleTransitionEnd);
};

export const renderGameGrid = ({ container, games, emptyMessage, sections }: RenderGameGridArgs): void => {
  container.replaceChildren();

  const hasSections = Array.isArray(sections) && sections.length > 0;
  if (games.length === 0 || (hasSections && sections.every((section) => section.games.length === 0))) {
    const emptyState = document.createElement("p");
    emptyState.className = "library-empty-state";
    emptyState.textContent = emptyMessage;
    container.append(emptyState);
    return;
  }

  if (hasSections) {
    const sectionsRoot = document.createElement("div");
    sectionsRoot.className = "game-grid-sections";
    const availableSectionIds = new Set<string>();
    for (const section of sections) {
      if (section.games.length > 0) {
        availableSectionIds.add(resolveSectionId(section));
      }
    }

    for (const collapsedSectionId of [...collapsedSectionIds]) {
      if (!availableSectionIds.has(collapsedSectionId)) {
        collapsedSectionIds.delete(collapsedSectionId);
      }
    }

    for (const section of sections) {
      if (section.games.length === 0) {
        continue;
      }

      const sectionId = resolveSectionId(section);
      const isCollapsed = collapsedSectionIds.has(sectionId);
      const sectionContentId = `game-grid-section-${sectionId}`;

      const sectionElement = document.createElement("section");
      sectionElement.className = "game-grid-section";
      sectionElement.setAttribute("aria-label", `${section.title} (${section.games.length})`);
      sectionElement.classList.toggle("is-collapsed", isCollapsed);

      const header = document.createElement("button");
      header.type = "button";
      header.className = "game-grid-section-header game-grid-section-toggle";
      header.setAttribute("aria-expanded", `${!isCollapsed}`);
      header.setAttribute("aria-controls", sectionContentId);

      const title = document.createElement("h3");
      title.className = "game-grid-section-title";
      title.textContent = section.title;

      const count = document.createElement("span");
      count.className = "game-grid-section-count";
      count.textContent = `${section.games.length}`;

      const caret = document.createElement("span");
      caret.className = "game-grid-section-caret";
      caret.setAttribute("aria-hidden", "true");

      const line = document.createElement("div");
      line.className = "game-grid-section-line";
      line.setAttribute("aria-hidden", "true");

      header.append(caret, title, count, line);

      const sectionContent = document.createElement("div");
      sectionContent.className = "game-grid-section-content";
      sectionContent.id = sectionContentId;

      const sectionGrid = document.createElement("div");
      sectionGrid.className = "game-grid";
      for (const game of section.games) {
        sectionGrid.append(createGameCard(game));
      }
      sectionContent.append(sectionGrid);

      if (isCollapsed) {
        setSectionInteractivity(sectionGrid, true);
        setSectionContentCollapsed(sectionContent);
      } else {
        setSectionInteractivity(sectionGrid, false);
        setSectionContentExpanded(sectionContent);
      }

      header.addEventListener("click", () => {
        const nextCollapsed = !sectionElement.classList.contains("is-collapsed");
        sectionElement.classList.toggle("is-collapsed", nextCollapsed);
        header.setAttribute("aria-expanded", `${!nextCollapsed}`);
        animateSectionContent(sectionContent, sectionGrid, nextCollapsed);
        if (nextCollapsed) {
          collapsedSectionIds.add(sectionId);
        } else {
          collapsedSectionIds.delete(sectionId);
        }
      });

      sectionElement.append(header, sectionContent);
      sectionsRoot.append(sectionElement);
    }

    container.append(sectionsRoot);
    return;
  }

  const grid = document.createElement("div");
  grid.className = "game-grid";
  for (const game of games) {
    grid.append(createGameCard(game));
  }

  container.append(grid);
};
