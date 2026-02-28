import { Router } from "express";

import { startSteamAuthController, steamCallbackController } from "./auth.controller.js";

export const authRouter = Router();

authRouter.get("/steam/start", startSteamAuthController);
authRouter.get("/steam/callback", steamCallbackController);
