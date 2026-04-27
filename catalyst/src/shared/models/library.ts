export interface PublicUser {
  id: string;
  email: string;
  steamLinked: boolean;
  steamId?: string;
}

export type GameKind = "game" | "demo" | "dlc" | "unknown";

export interface Feature {
  key: string;
  label: string;
  icon?: string;
  tooltip?: string;
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
  hideInLibrary?: boolean;
  developers?: string[];
  publishers?: string[];
  franchise?: string;
  releaseDate?: string;
  shortDescription?: string;
  headerImage?: string;
  features?: Feature[];
  hasAchievements?: boolean;
  achievementsCount?: number | null;
  hasCloudSaves?: boolean;
  cloudDetails?: string | null;
  controllerSupport?: string | null;
  uninstalling?: boolean;
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
