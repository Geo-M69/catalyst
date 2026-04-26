import type { GameResponse } from "../types";
import { getSteamArtworkCandidates } from "../../shared/utils/artwork";

const MAX_ARTWORK_CANDIDATES = 2;
const FAILED_ARTWORK_CACHE_MAX_ENTRIES = 2000;
const FAILED_ARTWORK_CACHE_TTL_MS = 10 * 60 * 1000;
const failedArtworkCandidateTimestamps = new Map<string, number>();

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

const pruneFailedArtworkCache = (now: number): void => {
  for (const [candidate, failedAt] of failedArtworkCandidateTimestamps) {
    if ((now - failedAt) > FAILED_ARTWORK_CACHE_TTL_MS) {
      failedArtworkCandidateTimestamps.delete(candidate);
    }
  }

  if (failedArtworkCandidateTimestamps.size <= FAILED_ARTWORK_CACHE_MAX_ENTRIES) {
    return;
  }

  const entriesByFailureAge = [...failedArtworkCandidateTimestamps.entries()]
    .sort((left, right) => left[1] - right[1]);
  const entriesToDelete = failedArtworkCandidateTimestamps.size - FAILED_ARTWORK_CACHE_MAX_ENTRIES;
  for (let index = 0; index < entriesToDelete; index += 1) {
    const entry = entriesByFailureAge[index];
    if (!entry) {
      break;
    }
    failedArtworkCandidateTimestamps.delete(entry[0]);
  }
};

const markArtworkCandidateAsFailed = (candidate: string): void => {
  const now = Date.now();
  failedArtworkCandidateTimestamps.set(candidate, now);
  pruneFailedArtworkCache(now);
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

export const createGameCard = (game: GameResponse): HTMLElement => {
  const card = document.createElement("article");
  card.className = "game-card";
  card.tabIndex = 0;
  card.dataset.gameId = game.id;
  card.dataset.gameProvider = game.provider;
  card.dataset.gameExternalId = game.externalId;
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
    image.loading = "lazy";
    image.decoding = "async";
    image.fetchPriority = "low";

    let candidateIndex = 0;

    image.addEventListener("error", () => {
      const failedCandidate = artworkCandidates[candidateIndex];
      if (failedCandidate) {
        markArtworkCandidateAsFailed(failedCandidate);
      }

      candidateIndex += 1;
      if (candidateIndex < artworkCandidates.length) {
        image.src = artworkCandidates[candidateIndex];
        return;
      }

      image.remove();
      appendPlaceholder(media, game.name);
    });

    image.src = artworkCandidates[candidateIndex];
    media.append(image);
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

  if (game.installed) {
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
