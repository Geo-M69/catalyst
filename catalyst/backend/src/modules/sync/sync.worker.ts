import { logger } from "../../config/logger.js";
import { sessionService } from "../auth/session.service.js";

const SESSION_CLEANUP_INTERVAL_MS = 60 * 60 * 1000;

export const startSyncWorker = (): void => {
  sessionService.cleanupExpiredSessions();

  const timer = setInterval(() => {
    sessionService.cleanupExpiredSessions();
  }, SESSION_CLEANUP_INTERVAL_MS);

  timer.unref();
  logger.info("Sync worker initialized");
};
