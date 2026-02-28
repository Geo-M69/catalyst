import type { Request, RequestHandler } from "express";

import { HttpError } from "../../shared/errors/http-error.js";

import { libraryService } from "./library.service.js";

const getAuthenticatedUserId = (request: Request): string => {
  if (!request.authUser) {
    throw new HttpError(401, "Not authenticated", "UNAUTHORIZED");
  }

  return request.authUser.id;
};

export const getLibraryController: RequestHandler = (req, res, next) => {
  try {
    const userId = getAuthenticatedUserId(req);
    const games = libraryService.listUserGames(userId);

    res.json({
      userId,
      total: games.length,
      games
    });
  } catch (error) {
    next(error);
  }
};
