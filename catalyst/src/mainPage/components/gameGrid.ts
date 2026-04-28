import { createGameCard, prefetchGameCardArtwork } from "./gameCard";
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

interface RenderGameGridSkeletonArgs {
  container: HTMLElement;
  cardCount?: number;
}

interface CachedGameCard {
  element: HTMLElement;
  gameReference: GameResponse;
  lastTouchedAt: number;
}

interface GridMetrics {
  cardHeight: number;
  columnGap: number;
  columns: number;
  rowGap: number;
  rowStride: number;
}

interface DynamicOverscan {
  pixels: number;
  rows: number;
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

interface SkeletonRenderState {
  cardCount: number;
  root: HTMLDivElement;
}

const DEFAULT_CARD_HEIGHT_ESTIMATE_PX = 102;
const DEFAULT_CARD_WIDTH_MIN_PX = 180;
const GRID_CARD_BORDER_ESTIMATE_PX = 1;
const OVERSCAN_ROWS_MIN = 3;
const OVERSCAN_ROWS_MAX = 14;
const OVERSCAN_VISIBLE_CARD_RATIO = 0.7;
const OVERSCAN_PIXELS_MIN = 320;
const OVERSCAN_PIXELS_MAX = 1600;
const OVERSCAN_VELOCITY_BOOST_START_PX_PER_MS = 0.3;
const OVERSCAN_VELOCITY_BOOST_END_PX_PER_MS = 2.0;
const OVERSCAN_VELOCITY_BOOST_MAX_MULTIPLIER = 1.7;
const PREFETCH_AHEAD_ROWS = 3;
const PREFETCH_BEHIND_ROWS = 1;
const PREFETCH_ROWS_VELOCITY_BOOST_MAX_MULTIPLIER = 1.5;
const PREFETCH_BUDGET_VISIBLE_CARD_RATIO = 0.5;
const PREFETCH_BUDGET_MIN_GAMES_PER_FRAME = 10;
const PREFETCH_BUDGET_BASE_MAX_GAMES_PER_FRAME = 22;
const PREFETCH_BUDGET_ABSOLUTE_MAX_GAMES_PER_FRAME = 40;
const PREFETCH_VELOCITY_BOOST_START_PX_PER_MS = 0.3;
const PREFETCH_VELOCITY_BOOST_END_PX_PER_MS = 2.1;
const PREFETCH_VELOCITY_BOOST_MAX_MULTIPLIER = 2.0;
const SCROLL_SPEED_SMOOTHING = 0.22;
const SCROLL_SPEED_SAMPLE_MAX_DELTA_MS = 180;
const SCROLL_SPEED_IDLE_DECAY_DELAY_MS = 100;
const SCROLL_SPEED_IDLE_DECAY_TAU_MS = 260;
const SCROLLING_VISUAL_SUPPRESSION_IDLE_MS = 140;
const CARD_CACHE_MAX_ENTRIES = 520;
const GRID_PERF_DEBUG_STORAGE_KEY = "catalyst.debug.gridPerf";
const GRID_PERF_DEBUG_QUERY_PARAM = "debugGridPerf";
const GRID_PERF_LOG_INTERVAL_MS = 1500;
const GRID_PERF_LONGTASK_LOG_THROTTLE_MS = 500;
const GRID_SKELETON_MIN_CARDS = 6;
const GRID_SKELETON_MAX_CARDS = 48;

const collapsedSectionIds = new Set<string>();
const rendererByContainer = new WeakMap<HTMLElement, GameGridRenderer>();

declare global {
  interface Window {
    __CATALYST_DEBUG_GRID_PERF__?: boolean;
  }
}

const parseBooleanDebugValue = (value: string | null): boolean => {
  if (!value) {
    return false;
  }

  const normalized = value.trim().toLocaleLowerCase();
  return normalized === "1" || normalized === "true" || normalized === "on";
};

const resolveGridPerfDebugEnabled = (): boolean => {
  if (window.__CATALYST_DEBUG_GRID_PERF__ === true) {
    return true;
  }

  const queryValue = new URLSearchParams(window.location.search).get(GRID_PERF_DEBUG_QUERY_PARAM);
  if (parseBooleanDebugValue(queryValue)) {
    return true;
  }

  try {
    const storedValue = localStorage.getItem(GRID_PERF_DEBUG_STORAGE_KEY);
    return parseBooleanDebugValue(storedValue);
  } catch {
    return false;
  }
};

const GRID_PERF_DEBUG_ENABLED = resolveGridPerfDebugEnabled();

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

const createSkeletonCard = (): HTMLElement => {
  const card = document.createElement("article");
  card.className = "game-card game-card-skeleton";
  card.setAttribute("aria-hidden", "true");

  const media = document.createElement("div");
  media.className = "game-card-media";

  const mediaBlock = document.createElement("span");
  mediaBlock.className = "game-card-skeleton-block game-card-skeleton-media";
  media.append(mediaBlock);

  const overlay = document.createElement("div");
  overlay.className = "game-card-skeleton-overlay";

  const title = document.createElement("span");
  title.className = "game-card-skeleton-block game-card-skeleton-line is-title";

  const meta = document.createElement("span");
  meta.className = "game-card-skeleton-block game-card-skeleton-line is-meta";

  overlay.append(title, meta);
  card.append(media, overlay);
  return card;
};

const clamp = (value: number, min: number, max: number): number => {
  return Math.max(min, Math.min(value, max));
};

const resolveVelocityBoostFactor = (
  speedPxPerMs: number,
  startSpeedPxPerMs: number,
  endSpeedPxPerMs: number,
  maxMultiplier: number
): number => {
  if (maxMultiplier <= 1 || endSpeedPxPerMs <= startSpeedPxPerMs || speedPxPerMs <= startSpeedPxPerMs) {
    return 1;
  }

  const normalized = clamp(
    (speedPxPerMs - startSpeedPxPerMs) / (endSpeedPxPerMs - startSpeedPxPerMs),
    0,
    1
  );
  return 1 + (normalized * (maxMultiplier - 1));
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

const resolveSkeletonCardCount = (container: HTMLElement, requestedCount?: number): number => {
  if (Number.isFinite(requestedCount) && requestedCount !== undefined && requestedCount > 0) {
    return clamp(Math.floor(requestedCount), GRID_SKELETON_MIN_CARDS, GRID_SKELETON_MAX_CARDS);
  }

  const minCardWidth = resolveGridCardMinWidth(container);
  const estimatedColumnGap = 14;
  const estimatedRowGap = 22;
  const estimatedCardHeight = estimateCardHeight(minCardWidth);
  const availableWidth = Math.max(container.clientWidth, minCardWidth);
  const availableHeight = Math.max(container.clientHeight, estimatedCardHeight * 2);
  const columns = Math.max(1, Math.floor((availableWidth + estimatedColumnGap) / (minCardWidth + estimatedColumnGap)));
  const rows = Math.max(2, Math.ceil((availableHeight + estimatedRowGap) / (estimatedCardHeight + estimatedRowGap)));
  return clamp(columns * rows, GRID_SKELETON_MIN_CARDS, GRID_SKELETON_MAX_CARDS);
};

const resolveDynamicOverscan = (
  container: HTMLElement,
  metrics: GridMetrics,
  scrollSpeedPxPerMs: number
): DynamicOverscan => {
  if (container.clientHeight <= 0 || metrics.rowStride <= 0 || metrics.columns <= 0) {
    return {
      pixels: OVERSCAN_PIXELS_MIN,
      rows: OVERSCAN_ROWS_MIN,
    };
  }

  const visibleRows = Math.max(Math.ceil(container.clientHeight / metrics.rowStride), 1);
  const visibleCards = visibleRows * metrics.columns;
  const targetOverscanCards = Math.max(Math.ceil(visibleCards * OVERSCAN_VISIBLE_CARD_RATIO), metrics.columns);
  const baseRows = clamp(
    Math.ceil(targetOverscanCards / metrics.columns),
    OVERSCAN_ROWS_MIN,
    OVERSCAN_ROWS_MAX
  );
  const velocityBoost = resolveVelocityBoostFactor(
    scrollSpeedPxPerMs,
    OVERSCAN_VELOCITY_BOOST_START_PX_PER_MS,
    OVERSCAN_VELOCITY_BOOST_END_PX_PER_MS,
    OVERSCAN_VELOCITY_BOOST_MAX_MULTIPLIER
  );
  const rows = clamp(
    Math.ceil(baseRows * velocityBoost),
    OVERSCAN_ROWS_MIN,
    OVERSCAN_ROWS_MAX
  );
  const pixels = clamp(
    Math.ceil(rows * metrics.rowStride),
    OVERSCAN_PIXELS_MIN,
    OVERSCAN_PIXELS_MAX
  );

  return { pixels, rows };
};

const resolvePrefetchBudget = (
  container: HTMLElement,
  metrics: GridMetrics,
  scrollSpeedPxPerMs: number
): number => {
  if (container.clientHeight <= 0 || metrics.rowStride <= 0 || metrics.columns <= 0) {
    return PREFETCH_BUDGET_MIN_GAMES_PER_FRAME;
  }

  const visibleRows = Math.max(Math.ceil(container.clientHeight / metrics.rowStride), 1);
  const visibleCards = visibleRows * metrics.columns;
  const baseBudget = clamp(
    Math.ceil(visibleCards * PREFETCH_BUDGET_VISIBLE_CARD_RATIO),
    PREFETCH_BUDGET_MIN_GAMES_PER_FRAME,
    PREFETCH_BUDGET_BASE_MAX_GAMES_PER_FRAME
  );
  const velocityBoost = resolveVelocityBoostFactor(
    scrollSpeedPxPerMs,
    PREFETCH_VELOCITY_BOOST_START_PX_PER_MS,
    PREFETCH_VELOCITY_BOOST_END_PX_PER_MS,
    PREFETCH_VELOCITY_BOOST_MAX_MULTIPLIER
  );

  return clamp(
    Math.ceil(baseBudget * velocityBoost),
    PREFETCH_BUDGET_MIN_GAMES_PER_FRAME,
    PREFETCH_BUDGET_ABSOLUTE_MAX_GAMES_PER_FRAME
  );
};

const resolveSliceRange = (
  itemCount: number,
  metrics: GridMetrics,
  viewportStart: number,
  viewportEnd: number,
  overscanRows: number
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

  let startRow = Math.floor(safeViewportStart / metrics.rowStride) - overscanRows;
  let endRow = Math.ceil(safeViewportEnd / metrics.rowStride) - 1 + overscanRows;

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

  private skeletonState: SkeletonRenderState | null = null;

  private plainState: PlainRenderState | null = null;

  private sectionsState: SectionsRenderState | null = null;

  private readonly cardCache = new Map<string, CachedGameCard>();

  private cardCacheUsageCounter = 0;

  private perfWindowStartedAtMs = performance.now();

  private perfWindowSampleCount = 0;

  private perfWindowTotalRenderMs = 0;

  private perfWindowMaxRenderMs = 0;

  private perfWindowMaxVisibleCards = 0;

  private perfWindowEvictedCards = 0;

  private perfWindowLongTaskCount = 0;

  private perfWindowLongTaskTotalMs = 0;

  private perfWindowLongTaskMaxMs = 0;

  private scrollingVisualSuppressionTimer: number | null = null;

  private readonly longTaskObserver: PerformanceObserver | null = null;

  private lastLongTaskLoggedAtMs = 0;

  private viewportRenderRaf: number | null = null;

  private lastKnownScrollTop = 0;

  private scrollDirection: -1 | 0 | 1 = 0;

  private lastScrollSampleAtMs = performance.now();

  private smoothedScrollSpeedPxPerMs = 0;

  private readonly onScroll = (): void => {
    this.markScrollingVisualSuppression();
    const nextScrollTop = this.container.scrollTop;
    const scrollDelta = nextScrollTop - this.lastKnownScrollTop;
    const now = performance.now();
    const elapsedMs = now - this.lastScrollSampleAtMs;

    if (elapsedMs > 0 && elapsedMs <= SCROLL_SPEED_SAMPLE_MAX_DELTA_MS) {
      const instantaneousSpeedPxPerMs = Math.abs(scrollDelta) / elapsedMs;
      if (this.smoothedScrollSpeedPxPerMs <= 0) {
        this.smoothedScrollSpeedPxPerMs = instantaneousSpeedPxPerMs;
      } else {
        this.smoothedScrollSpeedPxPerMs = (
          this.smoothedScrollSpeedPxPerMs * (1 - SCROLL_SPEED_SMOOTHING)
        ) + (instantaneousSpeedPxPerMs * SCROLL_SPEED_SMOOTHING);
      }
    }

    this.lastScrollSampleAtMs = now;

    if (nextScrollTop > this.lastKnownScrollTop) {
      this.scrollDirection = 1;
    } else if (nextScrollTop < this.lastKnownScrollTop) {
      this.scrollDirection = -1;
    } else {
      this.scrollDirection = 0;
    }
    this.lastKnownScrollTop = nextScrollTop;
    this.scheduleViewportRender();
  };

  private markScrollingVisualSuppression(): void {
    this.container.classList.add("is-scrolling");
    if (this.scrollingVisualSuppressionTimer !== null) {
      window.clearTimeout(this.scrollingVisualSuppressionTimer);
    }

    this.scrollingVisualSuppressionTimer = window.setTimeout(() => {
      this.scrollingVisualSuppressionTimer = null;
      this.container.classList.remove("is-scrolling");
    }, SCROLLING_VISUAL_SUPPRESSION_IDLE_MS);
  }

  private readonly onResize = (): void => {
    this.scheduleViewportRender();
  };

  private readonly resizeObserver: ResizeObserver;

  private readonly mutationObserver: MutationObserver;

  constructor(container: HTMLElement) {
    this.container = container;
    this.lastKnownScrollTop = container.scrollTop;
    this.lastScrollSampleAtMs = performance.now();
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

    this.longTaskObserver = this.createLongTaskObserver();
  }

  render({ games, emptyMessage, sections }: Omit<RenderGameGridArgs, "container">): void {
    this.container.setAttribute("aria-busy", "false");
    this.skeletonState = null;

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

  renderSkeleton(cardCount?: number): void {
    this.container.setAttribute("aria-busy", "true");
    this.emptyState = null;
    this.plainState = null;
    this.sectionsState = null;

    const nextCardCount = resolveSkeletonCardCount(this.container, cardCount);
    if (!this.skeletonState || this.skeletonState.cardCount !== nextCardCount) {
      const root = document.createElement("div");
      root.className = "game-grid game-grid-skeleton";
      root.setAttribute("aria-hidden", "true");

      const cards: HTMLElement[] = [];
      for (let index = 0; index < nextCardCount; index += 1) {
        cards.push(createSkeletonCard());
      }

      root.replaceChildren(...cards);
      this.skeletonState = {
        cardCount: nextCardCount,
        root,
      };
    }

    if (this.container.firstElementChild !== this.skeletonState.root || this.container.childElementCount !== 1) {
      this.container.replaceChildren(this.skeletonState.root);
    }

    this.cancelViewportRender();
  }

  private renderEmpty(message: string): void {
    this.skeletonState = null;
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
    this.skeletonState = null;
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

    const renderStartMs = performance.now();

    const metrics = resolveGridMetrics(plainState.grid, this.container);
    const scrollSpeedPxPerMs = this.resolveEffectiveScrollSpeedPxPerMs();
    const overscan = resolveDynamicOverscan(this.container, metrics, scrollSpeedPxPerMs);
    const prefetchBudget = resolvePrefetchBudget(this.container, metrics, scrollSpeedPxPerMs);
    const viewportStart = this.container.scrollTop - overscan.pixels;
    const viewportEnd = this.container.scrollTop + this.container.clientHeight + overscan.pixels;
    const range = resolveSliceRange(
      plainState.games.length,
      metrics,
      viewportStart,
      viewportEnd,
      overscan.rows
    );

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
    this.prefetchGamesNearRange(plainState.games, range, metrics, prefetchBudget, scrollSpeedPxPerMs);
    this.recordPerfSample("plain", performance.now() - renderStartMs, visibleCards.length);
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
    this.skeletonState = null;
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

    const renderStartMs = performance.now();
    let visibleCardsTotal = 0;

    const viewportStart = this.container.scrollTop;
    const viewportEnd = viewportStart + this.container.clientHeight;
    const containerRect = this.container.getBoundingClientRect();
    const scrollSpeedPxPerMs = this.resolveEffectiveScrollSpeedPxPerMs();
    let prefetchBudget = 0;

    for (const sectionId of sectionsState.sectionOrder) {
      const sectionState = sectionsState.sectionsById.get(sectionId);
      if (!sectionState) {
        continue;
      }

      const metrics = resolveGridMetrics(sectionState.grid, this.container);
      const overscan = resolveDynamicOverscan(this.container, metrics, scrollSpeedPxPerMs);
      if (prefetchBudget <= 0) {
        prefetchBudget = resolvePrefetchBudget(this.container, metrics, scrollSpeedPxPerMs);
      }
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

      const localViewportStart = (viewportStart - contentTopInScrollSpace) - overscan.pixels;
      const localViewportEnd = (viewportEnd - contentTopInScrollSpace) + overscan.pixels;

      const range = resolveSliceRange(
        totalGames,
        metrics,
        localViewportStart,
        localViewportEnd,
        overscan.rows
      );

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
      visibleCardsTotal += visibleCards.length;

      if (prefetchBudget > 0) {
        const prefetched = this.prefetchGamesNearRange(
          sectionState.games,
          range,
          metrics,
          prefetchBudget,
          scrollSpeedPxPerMs
        );
        prefetchBudget -= prefetched;
      }
    }

    this.recordPerfSample("sections", performance.now() - renderStartMs, visibleCardsTotal);
  }

  private prefetchGamesNearRange(
    games: GameResponse[],
    range: VirtualSliceRange,
    metrics: GridMetrics,
    budget: number,
    scrollSpeedPxPerMs: number
  ): number {
    if (games.length === 0 || budget <= 0) {
      return 0;
    }

    const rowBoost = resolveVelocityBoostFactor(
      scrollSpeedPxPerMs,
      PREFETCH_VELOCITY_BOOST_START_PX_PER_MS,
      PREFETCH_VELOCITY_BOOST_END_PX_PER_MS,
      PREFETCH_ROWS_VELOCITY_BOOST_MAX_MULTIPLIER
    );
    const aheadRows = Math.max(Math.ceil(PREFETCH_AHEAD_ROWS * rowBoost), PREFETCH_AHEAD_ROWS);
    const behindRows = Math.max(
      Math.ceil(PREFETCH_BEHIND_ROWS * Math.min(rowBoost, 1.4)),
      PREFETCH_BEHIND_ROWS
    );
    const aheadCount = Math.max(aheadRows * metrics.columns, 0);
    const behindCount = Math.max(behindRows * metrics.columns, 0);
    if (aheadCount <= 0 && behindCount <= 0) {
      return 0;
    }

    const startIndex = clamp(range.startIndex, 0, games.length);
    const endIndexExclusive = clamp(range.endIndexExclusive, 0, games.length);

    const aheadStart = endIndexExclusive;
    const aheadEndExclusive = Math.min(games.length, aheadStart + aheadCount);
    const behindStart = Math.max(0, startIndex - behindCount);

    let prefetched = 0;
    const prefetchGameAt = (index: number): void => {
      if (prefetched >= budget) {
        return;
      }

      const game = games[index];
      if (!game) {
        return;
      }

      prefetchGameCardArtwork(game);
      prefetched += 1;
    };

    // Idle/initial renders default to prefetching ahead.
    if (this.scrollDirection >= 0) {
      for (let index = aheadStart; index < aheadEndExclusive && prefetched < budget; index += 1) {
        prefetchGameAt(index);
      }

      for (let index = behindStart; index < startIndex && prefetched < budget; index += 1) {
        prefetchGameAt(index);
      }
    } else {
      for (let index = startIndex - 1; index >= behindStart && prefetched < budget; index -= 1) {
        prefetchGameAt(index);
      }

      for (let index = aheadStart; index < aheadEndExclusive && prefetched < budget; index += 1) {
        prefetchGameAt(index);
      }
    }

    return prefetched;
  }

  private resolveEffectiveScrollSpeedPxPerMs(): number {
    const now = performance.now();
    const idleMs = now - this.lastScrollSampleAtMs;
    if (idleMs <= SCROLL_SPEED_IDLE_DECAY_DELAY_MS) {
      return this.smoothedScrollSpeedPxPerMs;
    }

    const decayExponent = -(
      (idleMs - SCROLL_SPEED_IDLE_DECAY_DELAY_MS)
      / SCROLL_SPEED_IDLE_DECAY_TAU_MS
    );
    return this.smoothedScrollSpeedPxPerMs * Math.exp(decayExponent);
  }

  private getOrCreateCard(game: GameResponse): HTMLElement {
    const cached = this.cardCache.get(game.id);
    if (cached && cached.gameReference === game) {
      cached.lastTouchedAt = this.nextCardCacheUsageCounter();
      return cached.element;
    }

    const nextCard = createGameCard(game);
    this.cardCache.set(game.id, {
      element: nextCard,
      gameReference: game,
      lastTouchedAt: this.nextCardCacheUsageCounter(),
    });
    this.trimCardCacheToLimit();
    return nextCard;
  }

  private nextCardCacheUsageCounter(): number {
    this.cardCacheUsageCounter += 1;
    return this.cardCacheUsageCounter;
  }

  private trimCardCacheToLimit(): void {
    if (this.cardCache.size <= CARD_CACHE_MAX_ENTRIES) {
      return;
    }

    const overflow = this.cardCache.size - CARD_CACHE_MAX_ENTRIES;
    const evictable: Array<{ id: string; lastTouchedAt: number }> = [];
    for (const [id, cached] of this.cardCache.entries()) {
      if (cached.element.isConnected) {
        continue;
      }

      evictable.push({
        id,
        lastTouchedAt: cached.lastTouchedAt,
      });
    }

    if (evictable.length === 0) {
      return;
    }

    evictable.sort((left, right) => left.lastTouchedAt - right.lastTouchedAt);
    const evictionCount = Math.min(overflow, evictable.length);
    for (let index = 0; index < evictionCount; index += 1) {
      const entry = evictable[index];
      if (!entry) {
        continue;
      }
      this.cardCache.delete(entry.id);
      this.perfWindowEvictedCards += 1;
    }
  }

  private recordPerfSample(mode: "plain" | "sections", renderMs: number, visibleCards: number): void {
    if (!GRID_PERF_DEBUG_ENABLED) {
      return;
    }

    this.perfWindowSampleCount += 1;
    this.perfWindowTotalRenderMs += renderMs;
    this.perfWindowMaxRenderMs = Math.max(this.perfWindowMaxRenderMs, renderMs);
    this.perfWindowMaxVisibleCards = Math.max(this.perfWindowMaxVisibleCards, visibleCards);

    const now = performance.now();
    const elapsedMs = now - this.perfWindowStartedAtMs;
    if (elapsedMs < GRID_PERF_LOG_INTERVAL_MS) {
      return;
    }

    const averageRenderMs = this.perfWindowSampleCount > 0
      ? this.perfWindowTotalRenderMs / this.perfWindowSampleCount
      : 0;

    const payload: Record<string, number | string> = {
      mode,
      sampleWindowMs: Math.round(elapsedMs),
      samples: this.perfWindowSampleCount,
      avgRenderMs: Number(averageRenderMs.toFixed(2)),
      maxRenderMs: Number(this.perfWindowMaxRenderMs.toFixed(2)),
      maxVisibleCards: this.perfWindowMaxVisibleCards,
      scrollTop: Math.round(this.container.scrollTop),
      scrollSpeedPxPerMs: Number(this.resolveEffectiveScrollSpeedPxPerMs().toFixed(3)),
      cacheSize: this.cardCache.size,
      evictedCards: this.perfWindowEvictedCards,
      longTaskCount: this.perfWindowLongTaskCount,
      longTaskTotalMs: Number(this.perfWindowLongTaskTotalMs.toFixed(2)),
      longTaskMaxMs: Number(this.perfWindowLongTaskMaxMs.toFixed(2)),
      longTaskObserverActive: this.longTaskObserver !== null ? "yes" : "no",
    };

    const memory = (
      performance as Performance & {
        memory?: {
          usedJSHeapSize: number;
        };
      }
    ).memory;
    if (memory && Number.isFinite(memory.usedJSHeapSize)) {
      payload["usedJsHeapMb"] = Number((memory.usedJSHeapSize / (1024 * 1024)).toFixed(1));
    }

    console.debug("[grid-perf]", payload);

    this.perfWindowStartedAtMs = now;
    this.perfWindowSampleCount = 0;
    this.perfWindowTotalRenderMs = 0;
    this.perfWindowMaxRenderMs = 0;
    this.perfWindowMaxVisibleCards = 0;
    this.perfWindowEvictedCards = 0;
    this.perfWindowLongTaskCount = 0;
    this.perfWindowLongTaskTotalMs = 0;
    this.perfWindowLongTaskMaxMs = 0;
  }

  private createLongTaskObserver(): PerformanceObserver | null {
    if (!GRID_PERF_DEBUG_ENABLED || typeof PerformanceObserver === "undefined") {
      return null;
    }

    try {
      const observer = new PerformanceObserver((list) => {
        let entriesCount = 0;
        let totalDurationMs = 0;
        let maxDurationMs = 0;

        for (const entry of list.getEntries()) {
          entriesCount += 1;
          totalDurationMs += entry.duration;
          maxDurationMs = Math.max(maxDurationMs, entry.duration);
        }

        if (entriesCount <= 0) {
          return;
        }

        this.perfWindowLongTaskCount += entriesCount;
        this.perfWindowLongTaskTotalMs += totalDurationMs;
        this.perfWindowLongTaskMaxMs = Math.max(this.perfWindowLongTaskMaxMs, maxDurationMs);

        const now = performance.now();
        if ((now - this.lastLongTaskLoggedAtMs) < GRID_PERF_LONGTASK_LOG_THROTTLE_MS) {
          return;
        }

        this.lastLongTaskLoggedAtMs = now;
        console.debug("[grid-longtask]", {
          entries: entriesCount,
          totalDurationMs: Number(totalDurationMs.toFixed(2)),
          maxDurationMs: Number(maxDurationMs.toFixed(2)),
          scrollTop: Math.round(this.container.scrollTop),
          mode: this.sectionsState ? "sections" : "plain",
        });
      });

      observer.observe({
        entryTypes: ["longtask"],
      });
      return observer;
    } catch {
      return null;
    }
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

  if (sections) {
    renderer.render({ games, emptyMessage, sections });
    return;
  }

  renderer.render({ games, emptyMessage });
};

export const renderGameGridSkeleton = ({ container, cardCount }: RenderGameGridSkeletonArgs): void => {
  let renderer = rendererByContainer.get(container);
  if (!renderer) {
    renderer = new GameGridRenderer(container);
    rendererByContainer.set(container, renderer);
  }

  renderer.renderSkeleton(cardCount);
};
