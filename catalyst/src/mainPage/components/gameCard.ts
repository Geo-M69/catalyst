import type { GameResponse } from "../types";
import { getSteamArtworkCandidates } from "../../shared/utils/artwork";

const MAX_ARTWORK_CANDIDATES = 6;
const ARTWORK_CANDIDATE_TIMEOUT_MS = 2500;
const FAILED_ARTWORK_CACHE_MAX_ENTRIES = 2000;
const FAILED_ARTWORK_CACHE_TTL_MS = 10 * 60 * 1000;
const LOADED_ARTWORK_CACHE_MAX_ENTRIES = 4000;
const LOADED_ARTWORK_CACHE_TTL_MS = 20 * 60 * 1000;
const ARTWORK_CACHE_PRUNE_MUTATION_INTERVAL = 40;
const ARTWORK_CACHE_PRUNE_INTERVAL_MS = 15 * 1000;
const ENABLE_GAME_CARD_ARTWORK_PREFETCH = true;
const ARTWORK_PREFETCH_CONCURRENCY_LIMIT = 12;
const failedArtworkCandidateTimestamps = new Map<string, number>();
const loadedArtworkCandidateTimestamps = new Map<string, number>();
const inFlightArtworkPrefetches = new Set<string>();

interface CachePruneState {
  lastPrunedAt: number;
  mutationsSincePrune: number;
}

const failedArtworkPruneState: CachePruneState = {
  lastPrunedAt: 0,
  mutationsSincePrune: 0,
};

const loadedArtworkPruneState: CachePruneState = {
  lastPrunedAt: 0,
  mutationsSincePrune: 0,
};

const dateFormatter = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "short",
  day: "numeric",
});

const formatPlaytime = (playtimeMinutes: number): string => {
  if (playtimeMinutes <= 0) {
    return "Never played";
  }

  const hours = playtimeMinutes / 60;
  if (hours < 1) {
    return `${playtimeMinutes}m played`;
  }

  return `${hours.toFixed(1)}h played`;
};

const formatLastSynced = (rawDate: string): string => {
  const parsed = Date.parse(rawDate);
  if (Number.isNaN(parsed)) {
    return "Synced date unavailable";
  }

  return `Synced ${dateFormatter.format(parsed)}`;
};

const initialsFromName = (name: string): string => {
  const words = name.trim().split(/\s+/).filter((part) => part.length > 0);
  return words.slice(0, 2).map((part) => part[0]?.toUpperCase() ?? "").join("") || "?";
};

const formatKind = (kind: GameResponse["kind"]): string => {
  if (kind === "demo") {
    return "Demo";
  }
  if (kind === "dlc") {
    return "DLC";
  }
  if (kind === "unknown") {
    return "Unknown";
  }
  return "Game";
};

const appendPlaceholder = (container: HTMLElement, gameName: string): void => {
  const placeholder = document.createElement("div");
  placeholder.className = "game-card-placeholder";
  placeholder.textContent = initialsFromName(gameName);
  container.append(placeholder);
};

const pruneTimestampCache = (
  cache: Map<string, number>,
  maxEntries: number,
  ttlMs: number,
  now: number,
  pruneState: CachePruneState
): void => {
  pruneState.mutationsSincePrune += 1;
  const shouldPrune = cache.size > maxEntries
    || pruneState.mutationsSincePrune >= ARTWORK_CACHE_PRUNE_MUTATION_INTERVAL
    || (now - pruneState.lastPrunedAt) >= ARTWORK_CACHE_PRUNE_INTERVAL_MS;
  if (!shouldPrune) {
    return;
  }

  for (const [candidate, timestamp] of cache) {
    if ((now - timestamp) > ttlMs) {
      cache.delete(candidate);
    }
  }

  if (cache.size <= maxEntries) {
    return;
  }

  const entriesByAge = [...cache.entries()]
    .sort((left, right) => left[1] - right[1]);
  const entriesToDelete = cache.size - maxEntries;
  for (let index = 0; index < entriesToDelete; index += 1) {
    const entry = entriesByAge[index];
    if (!entry) {
      break;
    }
    cache.delete(entry[0]);
  }

  pruneState.lastPrunedAt = now;
  pruneState.mutationsSincePrune = 0;
};

const pruneFailedArtworkCache = (now: number): void => {
  pruneTimestampCache(
    failedArtworkCandidateTimestamps,
    FAILED_ARTWORK_CACHE_MAX_ENTRIES,
    FAILED_ARTWORK_CACHE_TTL_MS,
    now,
    failedArtworkPruneState
  );
};

const pruneLoadedArtworkCache = (now: number): void => {
  pruneTimestampCache(
    loadedArtworkCandidateTimestamps,
    LOADED_ARTWORK_CACHE_MAX_ENTRIES,
    LOADED_ARTWORK_CACHE_TTL_MS,
    now,
    loadedArtworkPruneState
  );
};

const markArtworkCandidateAsFailed = (candidate: string): void => {
  const now = Date.now();
  failedArtworkCandidateTimestamps.set(candidate, now);
  pruneFailedArtworkCache(now);
};

const markArtworkCandidateAsLoaded = (candidate: string): void => {
  if (!ENABLE_GAME_CARD_ARTWORK_PREFETCH) {
    return;
  }

  const now = Date.now();
  loadedArtworkCandidateTimestamps.set(candidate, now);
  pruneLoadedArtworkCache(now);
};

const isArtworkCandidateRecentlyFailed = (candidate: string): boolean => {
  const now = Date.now();
  const failedAt = failedArtworkCandidateTimestamps.get(candidate);
  if (failedAt === undefined) {
    return false;
  }

  if ((now - failedAt) > FAILED_ARTWORK_CACHE_TTL_MS) {
    failedArtworkCandidateTimestamps.delete(candidate);
    return false;
  }

  return true;
};

const isArtworkCandidateRecentlyLoaded = (candidate: string): boolean => {
  const now = Date.now();
  const loadedAt = loadedArtworkCandidateTimestamps.get(candidate);
  if (loadedAt === undefined) {
    return false;
  }

  if ((now - loadedAt) > LOADED_ARTWORK_CACHE_TTL_MS) {
    loadedArtworkCandidateTimestamps.delete(candidate);
    return false;
  }

  return true;
};

const prefetchArtworkCandidate = (candidate: string): void => {
  if (
    inFlightArtworkPrefetches.size >= ARTWORK_PREFETCH_CONCURRENCY_LIMIT
    || !ENABLE_GAME_CARD_ARTWORK_PREFETCH
    || !candidate
    || inFlightArtworkPrefetches.has(candidate)
    || isArtworkCandidateRecentlyLoaded(candidate)
    || isArtworkCandidateRecentlyFailed(candidate)
  ) {
    return;
  }

  inFlightArtworkPrefetches.add(candidate);

  const image = new Image();
  image.decoding = "async";
  image.fetchPriority = "low";

  const cleanup = (): void => {
    inFlightArtworkPrefetches.delete(candidate);
  };

  image.addEventListener("load", () => {
    markArtworkCandidateAsLoaded(candidate);
    cleanup();
  }, { once: true });

  image.addEventListener("error", () => {
    // Prefetch failures are often transient and should not suppress real loads.
    cleanup();
  }, { once: true });

  image.src = candidate;
};

const getArtworkCandidates = (game: GameResponse): string[] => {
  const candidates: string[] = [];
  const seen = new Set<string>();

  for (const candidate of getSteamArtworkCandidates(game, "wide-cover")) {
    const normalizedCandidate = candidate.trim();
    if (!normalizedCandidate || seen.has(normalizedCandidate)) {
      continue;
    }

    seen.add(normalizedCandidate);
    if (isArtworkCandidateRecentlyFailed(normalizedCandidate)) {
      continue;
    }

    candidates.push(normalizedCandidate);
    if (candidates.length >= MAX_ARTWORK_CANDIDATES) {
      break;
    }
  }

  return candidates;
};

export const prefetchGameCardArtwork = (game: GameResponse): void => {
  if (!ENABLE_GAME_CARD_ARTWORK_PREFETCH) {
    return;
  }

  const primaryCandidate = getArtworkCandidates(game)[0];
  if (!primaryCandidate) {
    return;
  }

  prefetchArtworkCandidate(primaryCandidate);
};

export const createGameCard = (game: GameResponse): HTMLElement => {
  const card = document.createElement("article");
  card.className = "game-card";
  card.tabIndex = 0;
  card.dataset["gameId"] = game.id;
  card.dataset["gameProvider"] = game.provider;
  card.dataset["gameExternalId"] = game.externalId;
  card.setAttribute("aria-label", `${game.name} (${game.provider})`);

  // Dispatch a global event to request opening the game details view. Main page
  // listens for `open-game-details` and performs the navigation + UI changes.
  const openDetails = (): void => {
    console.debug("gameCard.openDetails dispatching", game.id);
    const evt = new CustomEvent("open-game-details", { detail: { gameId: game.id }, bubbles: true });
    card.dispatchEvent(evt);
  };

  card.addEventListener("click", () => {
    // Ignore clicks that came from interactive children (e.g., context menus)
    openDetails();
  });

  card.addEventListener("keydown", (e: KeyboardEvent) => {
    if (e.key === "Enter") {
      openDetails();
      e.preventDefault();
    }
  });

  const media = document.createElement("div");
  media.className = "game-card-media";

  const artworkCandidates = getArtworkCandidates(game);
  if (artworkCandidates.length > 0) {
    const image = document.createElement("img");
    image.className = "game-card-image";
    image.alt = `${game.name} cover art`;
    image.decoding = "async";
    image.fetchPriority = "auto";

    let candidateIndex = 0;
    let candidateTimeoutId: number | null = null;

    const clearCandidateTimeout = (): void => {
      if (candidateTimeoutId === null) {
        return;
      }
      window.clearTimeout(candidateTimeoutId);
      candidateTimeoutId = null;
    };

    const renderPlaceholder = (): void => {
      clearCandidateTimeout();
      image.remove();
      appendPlaceholder(media, game.name);
    };

    const tryCurrentCandidate = (): void => {
      const currentCandidate = artworkCandidates[candidateIndex];
      if (!currentCandidate) {
        renderPlaceholder();
        return;
      }

      clearCandidateTimeout();
      candidateTimeoutId = window.setTimeout(() => {
        const timedOutCandidate = artworkCandidates[candidateIndex];
        if (timedOutCandidate) {
          markArtworkCandidateAsFailed(timedOutCandidate);
        }
        candidateIndex += 1;
        if (candidateIndex < artworkCandidates.length) {
          tryCurrentCandidate();
          return;
        }
        renderPlaceholder();
      }, ARTWORK_CANDIDATE_TIMEOUT_MS);

      image.src = currentCandidate;
    };

    image.addEventListener("load", () => {
      clearCandidateTimeout();
      const loadedCandidate = artworkCandidates[candidateIndex];
      if (loadedCandidate) {
        markArtworkCandidateAsLoaded(loadedCandidate);
      }
    });

    image.addEventListener("error", () => {
      clearCandidateTimeout();
      const failedCandidate = artworkCandidates[candidateIndex];
      if (failedCandidate) {
        markArtworkCandidateAsFailed(failedCandidate);
      }

      candidateIndex += 1;
      if (candidateIndex < artworkCandidates.length) {
        tryCurrentCandidate();
        return;
      }

      renderPlaceholder();
    });

    const firstCandidate = artworkCandidates[candidateIndex];
    if (firstCandidate) {
      media.append(image);
      tryCurrentCandidate();
    } else {
      appendPlaceholder(media, game.name);
    }
  } else {
    appendPlaceholder(media, game.name);
  }

  const body = document.createElement("div");
  body.className = "game-card-body";

  const title = document.createElement("h3");
  title.className = "game-card-title";
  title.textContent = game.name;

  const statusRow = document.createElement("div");
  statusRow.className = "game-card-status-row";
  if (game.favorite) {
    const favoriteBadge = document.createElement("span");
    favoriteBadge.className = "game-card-badge game-card-badge-favorite";
    favoriteBadge.textContent = "Favorite";
    statusRow.append(favoriteBadge);
  }

  if (game.uninstalling === true) {
    const uninstallingBadge = document.createElement("span");
    uninstallingBadge.className = "game-card-badge";
    uninstallingBadge.textContent = "Uninstalling...";
    statusRow.append(uninstallingBadge);
  } else if (game.installed) {
    const installedBadge = document.createElement("span");
    installedBadge.className = "game-card-badge";
    installedBadge.textContent = "Installed";
    statusRow.append(installedBadge);
  }

  const provider = document.createElement("p");
  provider.className = "game-card-meta";
  provider.textContent = `${game.provider.toUpperCase()} (${formatKind(game.kind)})`;

  const playtime = document.createElement("p");
  playtime.className = "game-card-meta";
  playtime.textContent = formatPlaytime(game.playtimeMinutes);

  const synced = document.createElement("p");
  synced.className = "game-card-meta subtle";
  synced.textContent = formatLastSynced(game.lastSyncedAt);

  if (statusRow.childElementCount > 0) {
    body.append(title, statusRow, provider, playtime, synced);
  } else {
    body.append(title, provider, playtime, synced);
  }
  card.append(media, body);

  return card;
};
