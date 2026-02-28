import type { RequestHandler } from "express";

import { HttpError } from "../../shared/errors/http-error.js";

import { libraryService } from "./library.service.js";

const getUserIdFromQuery = (value: unknown): string => {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new HttpError(400, "Missing required query parameter: userId", "VALIDATION_ERROR");
  }

  return value;
};

export const getLibraryController: RequestHandler = (req, res) => {
  const userId = getUserIdFromQuery(req.query.userId);
  const games = libraryService.listUserGames(userId);

  res.json({
    userId,
    total: games.length,
    games
  });
};
