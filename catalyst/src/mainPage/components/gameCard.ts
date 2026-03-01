import type { GameResponse } from "../types";

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

const appendPlaceholder = (container: HTMLElement, gameName: string): void => {
  const placeholder = document.createElement("div");
  placeholder.className = "game-card-placeholder";
  placeholder.textContent = initialsFromName(gameName);
  container.append(placeholder);
};

const getArtworkCandidates = (game: GameResponse): string[] => {
  const candidates: string[] = [];
  const seen = new Set<string>();

  const pushCandidate = (value: string | undefined): void => {
    const trimmed = value?.trim();
    if (!trimmed || seen.has(trimmed)) {
      return;
    }
    seen.add(trimmed);
    candidates.push(trimmed);
  };

  if (game.provider.toLowerCase() === "steam" && /^\d+$/.test(game.externalId)) {
    const appId = game.externalId;
    pushCandidate(`https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/capsule_616x353.jpg`);
    pushCandidate(`https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/header.jpg`);
    pushCandidate(`https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/library_600x900_2x.jpg`);
  }

  pushCandidate(game.artworkUrl);
  return candidates;
};

export const createGameCard = (game: GameResponse): HTMLElement => {
  const card = document.createElement("article");
  card.className = "game-card";

  const media = document.createElement("div");
  media.className = "game-card-media";

  const artworkCandidates = getArtworkCandidates(game);
  if (artworkCandidates.length > 0) {
    const image = document.createElement("img");
    image.className = "game-card-image";
    image.alt = `${game.name} cover art`;
    image.loading = "lazy";
    let candidateIndex = 0;

    image.addEventListener("error", () => {
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

  const provider = document.createElement("p");
  provider.className = "game-card-meta";
  provider.textContent = game.provider.toUpperCase();

  const playtime = document.createElement("p");
  playtime.className = "game-card-meta";
  playtime.textContent = formatPlaytime(game.playtimeMinutes);

  const synced = document.createElement("p");
  synced.className = "game-card-meta subtle";
  synced.textContent = formatLastSynced(game.lastSyncedAt);

  body.append(title, provider, playtime, synced);
  card.append(media, body);

  return card;
};
