import { ipcService } from "../shared/ipc/client";

export interface GameStoreMetadata {
  developers?: string[];
  publishers?: string[];
  franchise?: string | null;
  releaseDate?: string | null;
  shortDescription?: string | null;
  headerImage?: string | null;
  hasAchievements?: boolean;
  achievementsCount?: number | null;
  hasCloudSaves?: boolean;
  cloudDetails?: string | null;
  controllerSupport?: string | null;
}

export const fetchGameStoreMetadata = async (provider: string, externalId: string): Promise<GameStoreMetadata | null> => {
  try {
    const res = await ipcService.getGameStoreMetadata({ provider, externalId } as any);
    return res as GameStoreMetadata;
  } catch (err) {
    console.warn("fetchGameStoreMetadata failed", err);
    return null;
  }
};
