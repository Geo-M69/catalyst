export interface Review {
  id: string;
  userId: string;
  gameId: string;
  recommended: boolean;
  text: string;
  playtimeMinutes: number;
  playtimeCapturedAtReview?: boolean;
  createdAt: string; // ISO
  likes: number;
  comments: number;
}

export const createReviewCard = (review: Review): HTMLElement => {
  const card = document.createElement("article");
  card.className = "review-card";
  card.dataset.reviewId = review.id;

  // Header
  const header = document.createElement("div");
  header.className = "review-card-header";

  const status = document.createElement("div");
  status.className = `review-recommend ${review.recommended ? "is-positive" : "is-negative"}`;
  status.textContent = review.recommended ? "Recommended" : "Not Recommended";

  const meta = document.createElement("div");
  meta.className = "review-meta";

  const formatPlaytimeLabel = (minutes: number, capturedAtReview: boolean): string => {
    if (!capturedAtReview) {
      if (!minutes || minutes <= 0) return "No playtime yet";
      const hours = minutes / 60;
      if (hours < 1) return `${minutes}m currently played`;
      return `${hours.toFixed(1)}h currently played`;
    }

    if (!minutes || minutes <= 0) return "Playtime unavailable at review time";
    const hours = minutes / 60;
    if (hours < 1) return `${minutes}m when reviewed`;
    return `${hours.toFixed(1)}h when reviewed`;
  };

  const playtimeAtReview = document.createElement("div");
  playtimeAtReview.className = "review-meta-playtime";
  playtimeAtReview.textContent = formatPlaytimeLabel(
    review.playtimeMinutes,
    review.playtimeCapturedAtReview === true
  );

  meta.append(playtimeAtReview);
  header.append(status, meta);

  // Body
  const body = document.createElement("div");
  body.className = "review-card-body";
  const text = document.createElement("p");
  text.className = "review-card-text";
  text.textContent = review.text;
  body.append(text);

  // Footer
  const footer = document.createElement("div");
  footer.className = "review-card-footer";
  const stats = document.createElement("div");
  stats.className = "review-stats";
  const likes = document.createElement("span");
  likes.className = "review-likes";
  likes.textContent = `Likes: ${review.likes}`;
  const comments = document.createElement("span");
  comments.className = "review-comments";
  comments.textContent = `Comments: ${review.comments}`;
  stats.append(likes, comments);

  const actions = document.createElement("div");
  actions.className = "review-actions";
  const editLink = document.createElement("button");
  editLink.className = "review-edit-button";
  editLink.type = "button";
  editLink.textContent = "Edit My Review";
  editLink.addEventListener("click", () => {
    const evt = new CustomEvent("open-review-edit", { detail: { reviewId: review.id }, bubbles: true });
    card.dispatchEvent(evt);
  });

  actions.append(editLink);

  footer.append(stats, actions);

  card.append(header, body, footer);
  return card;
};

export const createReviewPlaceholder = (label = "Write a review for this game") => {
  const div = document.createElement("div");
  div.className = "review-card placeholder";
  div.textContent = `${label} (coming soon — read-only placeholder).`;
  return div;
};
