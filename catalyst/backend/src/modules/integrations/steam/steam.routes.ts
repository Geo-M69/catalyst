import { Router } from "express";

import { requireSession } from "../../../shared/middleware/auth-session.js";
import { steamRateLimiter } from "../../../shared/middleware/rate-limit.js";
import { syncSteamGamesController, steamStatusController } from "./steam.controller.js";

export const steamIntegrationRouter = Router();

steamIntegrationRouter.use(requireSession);
steamIntegrationRouter.post("/sync", steamRateLimiter, syncSteamGamesController);
steamIntegrationRouter.get("/status", steamRateLimiter, steamStatusController);
