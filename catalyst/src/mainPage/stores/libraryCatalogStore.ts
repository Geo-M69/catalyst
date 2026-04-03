import { type CollectionResponse, type GameResponse } from "../types";

export const libraryCatalogStore = {
  allGames: [] as GameResponse[],
  gameById: new Map<string, GameResponse>(),
  allCollections: [] as CollectionResponse[],
};

export const getAllGames = (): GameResponse[] => libraryCatalogStore.allGames;

export const findGameById = (id: string): GameResponse | undefined => {
  return libraryCatalogStore.gameById.get(id);
};
