import type { RequestHandler } from "express";

import { HttpError } from "../../../shared/errors/http-error.js";

import { steamService } from "./steam.service.js";

const getUserId = (value: unknown): string => {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new HttpError(400, "Missing required userId", "VALIDATION_ERROR");
  }

  return value;
};

export const syncSteamGamesController: RequestHandler = async (req, res) => {
  const userId = getUserId(req.body.userId);
  const syncedGames = await steamService.syncOwnedGames(userId);

  res.status(202).json({
    userId,
    provider: "steam",
    syncedGames
  });
};

export const steamStatusController: RequestHandler = (req, res) => {
  const userId = getUserId(req.query.userId);
  const status = steamService.getLinkStatus(userId);

  res.json({
    userId,
    provider: "steam",
    ...status
  });
};
