import rateLimit from "express-rate-limit";

import { env } from "../../config/env.js";

const buildRateLimitMessage = (message: string) => ({
  error: {
    code: "RATE_LIMITED",
    message
  }
});

export const authRateLimiter = rateLimit({
  windowMs: env.RATE_LIMIT_WINDOW_MS,
  max: env.RATE_LIMIT_MAX_AUTH,
  standardHeaders: true,
  legacyHeaders: false,
  message: buildRateLimitMessage("Too many authentication requests. Please wait and try again.")
});

export const steamRateLimiter = rateLimit({
  windowMs: env.RATE_LIMIT_WINDOW_MS,
  max: env.RATE_LIMIT_MAX_STEAM,
  standardHeaders: true,
  legacyHeaders: false,
  message: buildRateLimitMessage("Too many Steam requests. Please wait and try again.")
});
