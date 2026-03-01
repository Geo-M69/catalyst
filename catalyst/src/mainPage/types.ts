export interface PublicUser {
  id: string;
  email: string;
  steamLinked: boolean;
  steamId?: string;
}

export interface GameResponse {
  id: string;
  provider: string;
  externalId: string;
  name: string;
  kind: GameKind;
  playtimeMinutes: number;
  artworkUrl?: string;
  lastSyncedAt: string;
  installed: boolean;
  favorite: boolean;
  lastPlayedAt?: string;
  platforms?: string[];
  genres?: string[];
  steamTags?: string[];
  collections?: string[];
}

export interface CollectionResponse {
  id: string;
  name: string;
  gameCount: number;
  containsGame: boolean;
}

export interface LibraryResponse {
  userId: string;
  total: number;
  games: GameResponse[];
}

export type FilterByOption =
  | "all"
  | "installed"
  | "not-installed"
  | "favorites"
  | "recently-played"
  | "never-played";
export type PlatformFilter = "all" | "windows" | "macos" | "linux";
export type SourceFilter = "all" | "steam" | "epic-games";
export type GameKind = "game" | "demo" | "dlc" | "unknown";
export type GameKindFilter = "all" | GameKind;
export type GenreFilter = "all" | "action" | "rpg" | "strategy" | "simulation" | "fps";
export type SortOption =
  | "alphabetical"
  | "alphabetical-reverse"
  | "least-played"
  | "most-played";

export interface LibraryFilters {
  search: string;
  steamTag: string;
  collection: string;
  filterBy: FilterByOption;
  platform: PlatformFilter;
  source: SourceFilter;
  kind: GameKindFilter;
  genre: GenreFilter;
  sortBy: SortOption;
}
