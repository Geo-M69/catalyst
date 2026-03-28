import type { Review } from "./components/reviewCard";

const STORAGE_PREFIX = "catalyst.review.";

const storageKeyFor = (provider: string, externalId: string) => `${STORAGE_PREFIX}${provider}::${externalId}`;

export const loadReviewForGame = (provider: string, externalId: string): Review | null => {
  try {
    const raw = localStorage.getItem(storageKeyFor(provider, externalId));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Review;
    return parsed;
  } catch {
    return null;
  }
};

export const saveReviewForGame = (provider: string, externalId: string, review: Review): void => {
  try {
    localStorage.setItem(storageKeyFor(provider, externalId), JSON.stringify(review));
  } catch {
    // ignore storage errors
  }
};

export const clearReviewForGame = (provider: string, externalId: string): void => {
  try { localStorage.removeItem(storageKeyFor(provider, externalId)); } catch { }
};
