export const HIDDEN_GAMES_COLLECTION_NAME = "Hidden Games";

export type {
  CollectionResponse,
  Feature,
  GameResponse,
  LibraryResponse,
  PublicUser,
} from "../shared/models/library";
import type { GameKind } from "../shared/models/library";
export type { GameKind } from "../shared/models/library";

export type FilterByOption =
  | "all"
  | "installed"
  | "not-installed"
  | "favorites"
  | "recently-played"
  | "never-played";
export type PlatformFilter = "all" | "windows" | "macos" | "linux";
export type SourceFilter = "all" | "steam" | "epic-games";
export type GameKindFilter = "all" | GameKind;
export type GenreFilter =
  | "all"
  | "action"
  | "adventure"
  | "casual"
  | "indie"
  | "massively-multiplayer"
  | "racing"
  | "rpg"
  | "simulation"
  | "sports"
  | "strategy";
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
