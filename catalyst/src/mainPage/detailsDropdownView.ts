import { buildDetailsDropdownSnapshot, resolveFranchiseLabel } from "./detailsDropdownMetadata";
import { getSteamArtworkCandidates } from "./steamArtwork";
import { fetchGameStoreMetadata } from "./storeMetadata";
import type { GameResponse } from "./types";

interface DetailsDropdownViewOptions {
  detailsDropdown: HTMLElement;
  detailsPropertiesButton: HTMLButtonElement;
  resolveGameById: (gameId: string) => GameResponse | null;
  escapeHtml: (unsafe: string) => string;
}

interface DetailsDropdownView {
  renderDetailsDropdown: (gameId: string) => void;
  closeDetailsDropdown: () => void;
  openDetailsDropdown: (gameId: string) => void;
}

export const createDetailsDropdownView = ({
  detailsDropdown,
  detailsPropertiesButton,
  resolveGameById,
  escapeHtml,
}: DetailsDropdownViewOptions): DetailsDropdownView => {
  const renderDetailsDropdown = (gameId: string): void => {
    const game = resolveGameById(gameId);
    if (!game) {
      detailsDropdown.innerHTML = "";
      return;
    }

    const coverCandidates = getSteamArtworkCandidates(game, "cover");
    const coverUrl = (coverCandidates && coverCandidates.length > 0) ? coverCandidates[0] : (game.artworkUrl ?? "");
    const metadataSnapshot = buildDetailsDropdownSnapshot(game);
    const headerImage = metadataSnapshot.headerImage;
    const left = document.createElement("div");
    left.className = "dd-left";
    const img = document.createElement("img");
    img.src = coverUrl || headerImage || "";
    img.alt = `${game.name} cover`;
    const desc = document.createElement("div");
    desc.className = "dd-desc";
    desc.textContent = metadataSnapshot.description;
    left.append(img, desc);

    const center = document.createElement("div");
    center.className = "dd-center";
    const dev = document.createElement("div"); dev.className = "meta-row"; dev.innerHTML = `<div class="meta-label">Developer</div><div>${escapeHtml(metadataSnapshot.developersText)}</div>`;
    const pub = document.createElement("div"); pub.className = "meta-row"; pub.innerHTML = `<div class="meta-label">Publisher</div><div>${escapeHtml(metadataSnapshot.publishersText)}</div>`;
    const fran = document.createElement("div"); fran.className = "meta-row"; fran.innerHTML = `<div class="meta-label">Franchise</div><div>${escapeHtml(metadataSnapshot.franchiseText)}</div>`;
    const rel = document.createElement("div"); rel.className = "meta-row"; rel.innerHTML = `<div class="meta-label">Release Date</div><div>${escapeHtml(metadataSnapshot.releaseDateText)}</div>`;
    center.append(dev, pub, fran, rel);

    void (async () => {
      try {
        const meta = await fetchGameStoreMetadata(game.provider, game.externalId);
        if (meta) {
          if (!desc.textContent || desc.textContent.trim().length === 0) {
            desc.textContent = meta.shortDescription ?? "";
          }

          const devEl = center.querySelector('.meta-row:nth-child(1) > div:nth-child(2)');
          if (devEl && meta.developers) {
            devEl.textContent = meta.developers.join(", ");
          }
          const pubEl = center.querySelector('.meta-row:nth-child(2) > div:nth-child(2)');
          if (pubEl && meta.publishers) {
            pubEl.textContent = meta.publishers.join(", ");
          }
          const franEl = center.querySelector('.meta-row:nth-child(3) > div:nth-child(2)');
          if (franEl) {
            const franchiseVal = resolveFranchiseLabel(game, meta, metadataSnapshot.primaryPublisher);
            franEl.textContent = franchiseVal ?? "-";
          }
          const relEl = center.querySelector('.meta-row:nth-child(4) > div:nth-child(2)');
          if (relEl) {
            relEl.textContent = meta.releaseDate ?? metadataSnapshot.releaseDateText;
          }
        }
      } catch {
        // ignore
      }
    })();

    detailsDropdown.replaceChildren(left, center);
  };

  const closeDetailsDropdown = (): void => {
    detailsDropdown.hidden = true;
    detailsDropdown.setAttribute("aria-hidden", "true");
    detailsPropertiesButton.setAttribute("aria-expanded", "false");
    document.body.classList.remove("details-dropdown-open");
  };

  const openDetailsDropdown = (gameId: string): void => {
    renderDetailsDropdown(gameId);
    detailsDropdown.hidden = false;
    detailsDropdown.setAttribute("aria-hidden", "false");
    detailsPropertiesButton.setAttribute("aria-expanded", "true");
    document.body.classList.add("details-dropdown-open");
  };

  return {
    renderDetailsDropdown,
    closeDetailsDropdown,
    openDetailsDropdown,
  };
};
