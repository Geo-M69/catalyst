import type { Request, RequestHandler } from "express";

import { HttpError } from "../../../shared/errors/http-error.js";

import { steamService } from "./steam.service.js";

const getAuthenticatedUserId = (request: Request): string => {
  if (!request.authUser) {
    throw new HttpError(401, "Not authenticated", "UNAUTHORIZED");
  }

  return request.authUser.id;
};

export const syncSteamGamesController: RequestHandler = async (req, res, next) => {
  try {
    const userId = getAuthenticatedUserId(req);
    const syncedGames = await steamService.syncOwnedGames(userId);

    res.status(202).json({
      userId,
      provider: "steam",
      syncedGames
    });
  } catch (error) {
    next(error);
  }
};

export const steamStatusController: RequestHandler = (req, res, next) => {
  try {
    const userId = getAuthenticatedUserId(req);
    const status = steamService.getLinkStatus(userId);

    res.json({
      userId,
      provider: "steam",
      ...status
    });
  } catch (error) {
    next(error);
  }
};
