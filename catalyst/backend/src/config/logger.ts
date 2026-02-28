export const logger = {
  info: (message: string, meta?: unknown): void => {
    if (meta === undefined) {
      console.log(`[INFO] ${message}`);
      return;
    }

    console.log(`[INFO] ${message}`, meta);
  },
  warn: (message: string, meta?: unknown): void => {
    if (meta === undefined) {
      console.warn(`[WARN] ${message}`);
      return;
    }

    console.warn(`[WARN] ${message}`, meta);
  },
  error: (message: string, meta?: unknown): void => {
    if (meta === undefined) {
      console.error(`[ERROR] ${message}`);
      return;
    }

    console.error(`[ERROR] ${message}`, meta);
  }
};
