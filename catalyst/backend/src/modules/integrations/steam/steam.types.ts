export interface SteamOwnedGame {
  appid: number;
  name?: string;
  playtime_forever?: number;
  img_logo_url?: string;
}

export interface SteamOwnedGamesApiResponse {
  response?: {
    game_count?: number;
    games?: SteamOwnedGame[];
  };
}
