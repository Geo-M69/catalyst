export type GameProvider = "steam" | "epic" | "gog";

export interface Game {
  id: string;
  provider: GameProvider;
  externalId: string;
  name: string;
  playtimeMinutes: number;
  artworkUrl?: string;
  lastSyncedAt: string;
}
