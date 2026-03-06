export { getSteamArtworkCandidates } from "../../mainPage/steamArtwork";

export type { SteamLibraryArtworkKind } from "../../mainPage/steamArtwork";

export const addUniqueCandidate = (
	candidate: string | undefined,
	seen: Set<string>,
	candidates: string[]
): void => {
	const trimmed = candidate?.trim();
	if (!trimmed || seen.has(trimmed)) {
		return;
	}

	seen.add(trimmed);
	candidates.push(trimmed);
};

export const addCandidates = (values: string[], seen: Set<string>, candidates: string[]): void => {
	for (const v of values) {
		addUniqueCandidate(v, seen, candidates);
	}
};
