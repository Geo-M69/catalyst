import { Router } from "express";

import { requireSession } from "../../shared/middleware/auth-session.js";
import { authRateLimiter, steamRateLimiter } from "../../shared/middleware/rate-limit.js";
import {
  loginController,
  logoutController,
  registerController,
  sessionController,
  startSteamAuthController,
  steamCallbackController
} from "./auth.controller.js";

export const authRouter = Router();

authRouter.post("/register", authRateLimiter, registerController);
authRouter.post("/login", authRateLimiter, loginController);
authRouter.post("/logout", requireSession, logoutController);
authRouter.get("/session", requireSession, sessionController);
authRouter.get("/steam/start", steamRateLimiter, startSteamAuthController);
authRouter.get("/steam/callback", steamRateLimiter, steamCallbackController);
