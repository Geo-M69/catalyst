import { app } from "./app.js";
import { env } from "./config/env.js";
import { logger } from "./config/logger.js";
import { startSyncWorker } from "./modules/sync/sync.worker.js";
import { initializeDatabase } from "./shared/db/database.js";

initializeDatabase();

const server = app.listen(env.PORT, () => {
  logger.info(`Backend listening on http://localhost:${env.PORT}`);
});

startSyncWorker();

const shutdown = (): void => {
  logger.info("Shutting down backend");

  server.close((error) => {
    if (error) {
      logger.error("Failed to close server cleanly", error);
      process.exit(1);
    }

    process.exit(0);
  });
};

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
