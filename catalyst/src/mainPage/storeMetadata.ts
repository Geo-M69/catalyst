import { ipcService } from "../shared/ipc/client";
import type { GameStoreMetadataPayload } from "../shared/ipc/contracts";

export type GameStoreMetadata = GameStoreMetadataPayload;

export const fetchGameStoreMetadata = async (provider: string, externalId: string): Promise<GameStoreMetadata | null> => {
  try {
    return await ipcService.getGameStoreMetadata({ provider, externalId });
  } catch (err) {
    console.warn("fetchGameStoreMetadata failed", err);
    return null;
  }
};
