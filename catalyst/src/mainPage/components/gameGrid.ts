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

interface CachedGameCard {
  element: HTMLElement;
  gameReference: GameResponse;
}

interface GridMetrics {
  cardHeight: number;
  columnGap: number;
  columns: number;
  rowGap: number;
  rowStride: number;
}

interface SectionRenderState {
  content: HTMLDivElement;
  count: HTMLSpanElement;
  element: HTMLElement;
  games: GameResponse[];
  grid: HTMLDivElement;
  header: HTMLButtonElement;
  id: string;
  title: HTMLHeadingElement;
  topSpacer: HTMLDivElement;
  bottomSpacer: HTMLDivElement;
}

interface VirtualSliceRange {
  endIndexExclusive: number;
  fullHeight: number;
  startIndex: number;
  topHeight: number;
  bottomHeight: number;
}

interface EmptyRenderState {
  emptyState: HTMLParagraphElement;
}

interface PlainRenderState {
  bottomSpacer: HTMLDivElement;
  games: GameResponse[];
  grid: HTMLDivElement;
  root: HTMLDivElement;
  topSpacer: HTMLDivElement;
}

interface SectionsRenderState {
  root: HTMLDivElement;
  sectionOrder: string[];
  sectionsById: Map<string, SectionRenderState>;
}

const DEFAULT_CARD_HEIGHT_ESTIMATE_PX = 102;
const DEFAULT_CARD_WIDTH_MIN_PX = 180;
const GRID_CARD_BORDER_ESTIMATE_PX = 1;
const OVERSCAN_ROWS = 3;
const VISIBILITY_OVERSCAN_PX = 320;

const collapsedSectionIds = new Set<string>();
const rendererByContainer = new WeakMap<HTMLElement, GameGridRenderer>();

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
  sectionContent.style.overflow = "visible";
};

const setSectionContentCollapsed = (sectionContent: HTMLElement): void => {
  sectionContent.style.maxHeight = "0px";
  sectionContent.style.opacity = "0";
  sectionContent.style.overflow = "hidden";
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
    sectionContent.style.overflow = "hidden";
    sectionContent.style.maxHeight = `${sectionContent.scrollHeight}px`;
    sectionContent.style.opacity = "1";
    window.requestAnimationFrame(() => {
      setSectionContentCollapsed(sectionContent);
    });
    return;
  }

  setSectionInteractivity(sectionGrid, false);
  sectionContent.style.overflow = "hidden";
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

const createVirtualSpacer = (): HTMLDivElement => {
  const spacer = document.createElement("div");
  spacer.className = "game-grid-virtual-spacer";
  spacer.setAttribute("aria-hidden", "true");
  return spacer;
};

const patchChildren = (container: HTMLElement, nextChildren: HTMLElement[]): void => {
  let cursor = container.firstElementChild as HTMLElement | null;

  for (const nextChild of nextChildren) {
    if (cursor === nextChild) {
      cursor = cursor.nextElementSibling as HTMLElement | null;
      continue;
    }

    container.insertBefore(nextChild, cursor);
  }

  while (cursor) {
    const nextCursor = cursor.nextElementSibling as HTMLElement | null;
    cursor.remove();
    cursor = nextCursor;
  }
};

const clamp = (value: number, min: number, max: number): number => {
  return Math.max(min, Math.min(value, max));
};

const resolveGridCardMinWidth = (container: HTMLElement): number => {
  const explicitValue = Number.parseFloat(getComputedStyle(container).getPropertyValue("--game-grid-card-min-width"));
  if (Number.isFinite(explicitValue) && explicitValue > 0) {
    return explicitValue;
  }

  return DEFAULT_CARD_WIDTH_MIN_PX;
};

const estimateCardHeight = (cardWidth: number): number => {
  const mediaHeight = Math.max(cardWidth - (GRID_CARD_BORDER_ESTIMATE_PX * 2), 0) * (9 / 16);
  return Math.max(DEFAULT_CARD_HEIGHT_ESTIMATE_PX, mediaHeight + (GRID_CARD_BORDER_ESTIMATE_PX * 2));
};

const resolveGridMetrics = (grid: HTMLElement, container: HTMLElement): GridMetrics => {
  const computedStyle = getComputedStyle(grid);
  const columnGap = Number.parseFloat(computedStyle.columnGap);
  const rowGap = Number.parseFloat(computedStyle.rowGap);
  const safeColumnGap = Number.isFinite(columnGap) ? columnGap : 0;
  const safeRowGap = Number.isFinite(rowGap) ? rowGap : 0;

  const gridWidth = Math.max(grid.getBoundingClientRect().width, 1);
  const minCardWidth = resolveGridCardMinWidth(container);
  const columns = Math.max(1, Math.floor((gridWidth + safeColumnGap) / (minCardWidth + safeColumnGap)));
  const cardWidth = Math.max((gridWidth - (safeColumnGap * (columns - 1))) / columns, 1);
  const cardHeight = estimateCardHeight(cardWidth);

  return {
    cardHeight,
    columnGap: safeColumnGap,
    columns,
    rowGap: safeRowGap,
    rowStride: cardHeight + safeRowGap,
  };
};

const resolveSliceRange = (
  itemCount: number,
  metrics: GridMetrics,
  viewportStart: number,
  viewportEnd: number
): VirtualSliceRange => {
  if (itemCount === 0) {
    return {
      endIndexExclusive: 0,
      fullHeight: 0,
      startIndex: 0,
      topHeight: 0,
      bottomHeight: 0,
    };
  }

  const totalRows = Math.ceil(itemCount / metrics.columns);
  const fullHeight = (totalRows * metrics.cardHeight) + (Math.max(0, totalRows - 1) * metrics.rowGap);
  const safeViewportStart = clamp(viewportStart, 0, fullHeight);
  const safeViewportEnd = clamp(viewportEnd, 0, fullHeight);

  if (safeViewportEnd <= 0 || safeViewportStart >= fullHeight) {
    return {
      endIndexExclusive: 0,
      fullHeight,
      startIndex: 0,
      topHeight: 0,
      bottomHeight: fullHeight,
    };
  }

  let startRow = Math.floor(safeViewportStart / metrics.rowStride) - OVERSCAN_ROWS;
  let endRow = Math.ceil(safeViewportEnd / metrics.rowStride) - 1 + OVERSCAN_ROWS;

  startRow = clamp(startRow, 0, Math.max(totalRows - 1, 0));
  endRow = clamp(endRow, startRow, Math.max(totalRows - 1, 0));

  const startIndex = startRow * metrics.columns;
  const endIndexExclusive = Math.min(itemCount, (endRow + 1) * metrics.columns);

  const renderedCount = Math.max(endIndexExclusive - startIndex, 0);
  const renderedRows = renderedCount > 0
    ? Math.ceil(renderedCount / metrics.columns)
    : 0;

  const topHeight = startRow * metrics.rowStride;
  const renderedHeight = renderedRows > 0
    ? (renderedRows * metrics.cardHeight) + (Math.max(renderedRows - 1, 0) * metrics.rowGap)
    : 0;
  const bottomHeight = Math.max(fullHeight - topHeight - renderedHeight, 0);

  return {
    endIndexExclusive,
    fullHeight,
    startIndex,
    topHeight,
    bottomHeight,
  };
};

class GameGridRenderer {
  private readonly container: HTMLElement;

  private emptyState: EmptyRenderState | null = null;

  private plainState: PlainRenderState | null = null;

  private sectionsState: SectionsRenderState | null = null;

  private readonly cardCache = new Map<string, CachedGameCard>();

  private viewportRenderRaf: number | null = null;

  private readonly onScroll = (): void => {
    this.scheduleViewportRender();
  };

  private readonly onResize = (): void => {
    this.scheduleViewportRender();
  };

  private readonly resizeObserver: ResizeObserver;

  private readonly mutationObserver: MutationObserver;

  constructor(container: HTMLElement) {
    this.container = container;
    this.container.addEventListener("scroll", this.onScroll, { passive: true });
    window.addEventListener("resize", this.onResize);

    this.resizeObserver = new ResizeObserver(() => {
      this.scheduleViewportRender();
    });
    this.resizeObserver.observe(this.container);

    this.mutationObserver = new MutationObserver(() => {
      this.scheduleViewportRender();
    });
    this.mutationObserver.observe(this.container, {
      attributes: true,
      attributeFilter: ["style", "class"],
    });
  }

  render({ games, emptyMessage, sections }: Omit<RenderGameGridArgs, "container">): void {
    const hasSections = Array.isArray(sections) && sections.length > 0;
    if (games.length === 0 || (hasSections && sections.every((section) => section.games.length === 0))) {
      this.renderEmpty(emptyMessage);
      this.pruneCardCache(new Set<string>());
      return;
    }

    if (hasSections && sections) {
      this.renderSections(sections);
      const activeIds = new Set<string>();
      for (const section of sections) {
        for (const game of section.games) {
          activeIds.add(game.id);
        }
      }
      this.pruneCardCache(activeIds);
      return;
    }

    this.renderPlain(games);
    const activeIds = new Set(games.map((game) => game.id));
    this.pruneCardCache(activeIds);
  }

  private renderEmpty(message: string): void {
    this.plainState = null;
    this.sectionsState = null;

    if (!this.emptyState) {
      const emptyStateElement = document.createElement("p");
      emptyStateElement.className = "library-empty-state";
      this.emptyState = { emptyState: emptyStateElement };
    }

    this.emptyState.emptyState.textContent = message;

    if (this.container.firstElementChild !== this.emptyState.emptyState || this.container.childElementCount !== 1) {
      this.container.replaceChildren(this.emptyState.emptyState);
    }

    this.cancelViewportRender();
  }

  private ensurePlainState(): PlainRenderState {
    if (this.plainState) {
      return this.plainState;
    }

    const root = document.createElement("div");
    root.className = "game-grid-virtualized";

    const topSpacer = createVirtualSpacer();
    const grid = document.createElement("div");
    grid.className = "game-grid";
    const bottomSpacer = createVirtualSpacer();

    root.append(topSpacer, grid, bottomSpacer);

    this.plainState = {
      bottomSpacer,
      games: [],
      grid,
      root,
      topSpacer,
    };

    return this.plainState;
  }

  private renderPlain(games: GameResponse[]): void {
    this.emptyState = null;
    this.sectionsState = null;

    const plainState = this.ensurePlainState();
    plainState.games = games;

    if (this.container.firstElementChild !== plainState.root || this.container.childElementCount !== 1) {
      this.container.replaceChildren(plainState.root);
    }

    this.renderPlainViewport();
  }

  private renderPlainViewport(): void {
    const plainState = this.plainState;
    if (!plainState) {
      return;
    }

    const metrics = resolveGridMetrics(plainState.grid, this.container);
    const viewportStart = this.container.scrollTop - VISIBILITY_OVERSCAN_PX;
    const viewportEnd = this.container.scrollTop + this.container.clientHeight + VISIBILITY_OVERSCAN_PX;
    const range = resolveSliceRange(plainState.games.length, metrics, viewportStart, viewportEnd);

    plainState.topSpacer.style.height = `${range.topHeight}px`;
    plainState.bottomSpacer.style.height = `${range.bottomHeight}px`;

    const visibleCards: HTMLElement[] = [];
    for (let index = range.startIndex; index < range.endIndexExclusive; index += 1) {
      const game = plainState.games[index];
      if (!game) {
        continue;
      }
      visibleCards.push(this.getOrCreateCard(game));
    }

    patchChildren(plainState.grid, visibleCards);
  }

  private ensureSectionsState(): SectionsRenderState {
    if (this.sectionsState) {
      return this.sectionsState;
    }

    const root = document.createElement("div");
    root.className = "game-grid-sections";

    this.sectionsState = {
      root,
      sectionOrder: [],
      sectionsById: new Map<string, SectionRenderState>(),
    };

    return this.sectionsState;
  }

  private createSectionState(sectionId: string): SectionRenderState {
    const element = document.createElement("section");
    element.className = "game-grid-section";

    const header = document.createElement("button");
    header.type = "button";
    header.className = "game-grid-section-header game-grid-section-toggle";

    const caret = document.createElement("span");
    caret.className = "game-grid-section-caret";
    caret.setAttribute("aria-hidden", "true");

    const title = document.createElement("h3");
    title.className = "game-grid-section-title";

    const count = document.createElement("span");
    count.className = "game-grid-section-count";

    const line = document.createElement("div");
    line.className = "game-grid-section-line";
    line.setAttribute("aria-hidden", "true");

    header.append(caret, title, count, line);

    const content = document.createElement("div");
    content.className = "game-grid-section-content";

    const topSpacer = createVirtualSpacer();
    const grid = document.createElement("div");
    grid.className = "game-grid";
    const bottomSpacer = createVirtualSpacer();
    content.append(topSpacer, grid, bottomSpacer);

    const state: SectionRenderState = {
      content,
      count,
      element,
      games: [],
      grid,
      header,
      id: sectionId,
      title,
      topSpacer,
      bottomSpacer,
    };

    header.addEventListener("click", () => {
      const nextCollapsed = !state.element.classList.contains("is-collapsed");
      state.element.classList.toggle("is-collapsed", nextCollapsed);
      state.header.setAttribute("aria-expanded", `${!nextCollapsed}`);
      animateSectionContent(state.content, state.grid, nextCollapsed);

      if (nextCollapsed) {
        collapsedSectionIds.add(state.id);
      } else {
        collapsedSectionIds.delete(state.id);
        this.scheduleViewportRender();
      }
    });

    element.append(header, content);

    return state;
  }

  private renderSections(sections: GameGridSection[]): void {
    this.emptyState = null;
    this.plainState = null;

    const sectionsState = this.ensureSectionsState();

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

    const nextOrder: string[] = [];
    const nextElements: HTMLElement[] = [];

    for (const section of sections) {
      if (section.games.length === 0) {
        continue;
      }

      const sectionId = resolveSectionId(section);
      nextOrder.push(sectionId);

      let sectionState = sectionsState.sectionsById.get(sectionId);
      if (!sectionState) {
        sectionState = this.createSectionState(sectionId);
        sectionsState.sectionsById.set(sectionId, sectionState);
      }

      sectionState.games = section.games;
      sectionState.title.textContent = section.title;
      sectionState.count.textContent = `${section.games.length}`;

      const sectionContentId = `game-grid-section-${sectionId}`;
      sectionState.content.id = sectionContentId;
      sectionState.header.setAttribute("aria-controls", sectionContentId);
      sectionState.element.setAttribute("aria-label", `${section.title} (${section.games.length})`);

      const isCollapsed = collapsedSectionIds.has(sectionId);
      sectionState.element.classList.toggle("is-collapsed", isCollapsed);
      sectionState.header.setAttribute("aria-expanded", `${!isCollapsed}`);

      if (isCollapsed) {
        setSectionInteractivity(sectionState.grid, true);
        setSectionContentCollapsed(sectionState.content);
      } else {
        setSectionInteractivity(sectionState.grid, false);
        setSectionContentExpanded(sectionState.content);
      }

      nextElements.push(sectionState.element);
    }

    for (const previousSectionId of sectionsState.sectionOrder) {
      if (nextOrder.includes(previousSectionId)) {
        continue;
      }
      const staleSection = sectionsState.sectionsById.get(previousSectionId);
      if (staleSection) {
        staleSection.element.remove();
      }
      sectionsState.sectionsById.delete(previousSectionId);
    }

    sectionsState.sectionOrder = nextOrder;

    if (this.container.firstElementChild !== sectionsState.root || this.container.childElementCount !== 1) {
      this.container.replaceChildren(sectionsState.root);
    }

    patchChildren(sectionsState.root, nextElements);
    this.renderSectionsViewport();
  }

  private renderSectionsViewport(): void {
    const sectionsState = this.sectionsState;
    if (!sectionsState) {
      return;
    }

    const viewportStart = this.container.scrollTop;
    const viewportEnd = viewportStart + this.container.clientHeight;
    const containerRect = this.container.getBoundingClientRect();

    for (const sectionId of sectionsState.sectionOrder) {
      const sectionState = sectionsState.sectionsById.get(sectionId);
      if (!sectionState) {
        continue;
      }

      const metrics = resolveGridMetrics(sectionState.grid, this.container);
      const totalGames = sectionState.games.length;

      const totalRows = totalGames > 0 ? Math.ceil(totalGames / metrics.columns) : 0;
      const fullHeight = totalRows > 0
        ? (totalRows * metrics.cardHeight) + (Math.max(totalRows - 1, 0) * metrics.rowGap)
        : 0;

      if (collapsedSectionIds.has(sectionId)) {
        sectionState.topSpacer.style.height = "0px";
        sectionState.bottomSpacer.style.height = `${fullHeight}px`;
        patchChildren(sectionState.grid, []);
        continue;
      }

      const contentTopInScrollSpace = (
        sectionState.content.getBoundingClientRect().top
        - containerRect.top
      ) + this.container.scrollTop;

      const localViewportStart = (viewportStart - contentTopInScrollSpace) - VISIBILITY_OVERSCAN_PX;
      const localViewportEnd = (viewportEnd - contentTopInScrollSpace) + VISIBILITY_OVERSCAN_PX;

      const range = resolveSliceRange(totalGames, metrics, localViewportStart, localViewportEnd);

      sectionState.topSpacer.style.height = `${range.topHeight}px`;
      sectionState.bottomSpacer.style.height = `${range.bottomHeight}px`;

      const visibleCards: HTMLElement[] = [];
      for (let index = range.startIndex; index < range.endIndexExclusive; index += 1) {
        const game = sectionState.games[index];
        if (!game) {
          continue;
        }
        visibleCards.push(this.getOrCreateCard(game));
      }

      patchChildren(sectionState.grid, visibleCards);
    }
  }

  private getOrCreateCard(game: GameResponse): HTMLElement {
    const cached = this.cardCache.get(game.id);
    if (cached && cached.gameReference === game) {
      return cached.element;
    }

    const nextCard = createGameCard(game);
    this.cardCache.set(game.id, {
      element: nextCard,
      gameReference: game,
    });
    return nextCard;
  }

  private pruneCardCache(activeIds: Set<string>): void {
    for (const cachedId of [...this.cardCache.keys()]) {
      if (activeIds.has(cachedId)) {
        continue;
      }
      this.cardCache.delete(cachedId);
    }
  }

  private scheduleViewportRender(): void {
    if (this.viewportRenderRaf !== null) {
      return;
    }

    this.viewportRenderRaf = window.requestAnimationFrame(() => {
      this.viewportRenderRaf = null;
      if (this.plainState) {
        this.renderPlainViewport();
        return;
      }

      if (this.sectionsState) {
        this.renderSectionsViewport();
      }
    });
  }

  private cancelViewportRender(): void {
    if (this.viewportRenderRaf === null) {
      return;
    }

    window.cancelAnimationFrame(this.viewportRenderRaf);
    this.viewportRenderRaf = null;
  }
}

export const renderGameGrid = ({ container, games, emptyMessage, sections }: RenderGameGridArgs): void => {
  let renderer = rendererByContainer.get(container);
  if (!renderer) {
    renderer = new GameGridRenderer(container);
    rendererByContainer.set(container, renderer);
  }

  renderer.render({ games, emptyMessage, sections });
};
