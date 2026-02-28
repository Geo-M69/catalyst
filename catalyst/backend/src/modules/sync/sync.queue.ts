import { syncService } from "./sync.service.js";

class SyncQueue {
  public async enqueueUserSync(userId: string): Promise<void> {
    await syncService.syncUserLibrary(userId);
  }
}

export const syncQueue = new SyncQueue();
