import { Router } from "express";

import { syncSteamGamesController, steamStatusController } from "./steam.controller.js";

export const steamIntegrationRouter = Router();

steamIntegrationRouter.post("/sync", syncSteamGamesController);
steamIntegrationRouter.get("/status", steamStatusController);
