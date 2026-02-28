import { steamService } from "../integrations/steam/steam.service.js";
import { usersService } from "../users/users.service.js";

class SyncService {
  public async syncUserLibrary(userId: string): Promise<{ steam: number }> {
    const user = usersService.getRequiredUser(userId);

    let steamSynced = 0;
    if (user.integrations.steamId) {
      steamSynced = await steamService.syncOwnedGames(userId);
    }

    return {
      steam: steamSynced
    };
  }
}

export const syncService = new SyncService();
