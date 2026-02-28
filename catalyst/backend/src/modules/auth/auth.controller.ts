import type { ParsedQs } from "qs";
import type { RequestHandler } from "express";

import { HttpError } from "../../shared/errors/http-error.js";

import { authService } from "./auth.service.js";

const extractQueryParams = (query: ParsedQs): URLSearchParams => {
  const params = new URLSearchParams();
  const queryValues = query as Record<string, string | string[] | ParsedQs | ParsedQs[] | undefined>;

  for (const [key, value] of Object.entries(queryValues)) {
    if (Array.isArray(value)) {
      for (const entry of value) {
        if (typeof entry === "string") {
          params.append(key, entry);
        }
      }
      continue;
    }

    if (typeof value === "string") {
      params.set(key, value);
    }
  }

  return params;
};

const getUserId = (value: unknown): string => {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new HttpError(400, "Missing required query parameter: userId", "VALIDATION_ERROR");
  }

  return value;
};

export const startSteamAuthController: RequestHandler = (req, res) => {
  const userId = getUserId(req.query.userId);
  const result = authService.startSteamAuth(userId);

  res.json({
    userId,
    ...result
  });
};

export const steamCallbackController: RequestHandler = async (req, res) => {
  const params = extractQueryParams(req.query);

  try {
    const result = await authService.completeSteamAuth(params);
    const redirectUrl = authService.buildFrontendCallbackUrl({
      status: "success",
      userId: result.userId,
      steamId: result.steamId,
      syncedGames: result.syncedGames
    });

    res.redirect(302, redirectUrl);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Steam callback failed";

    const redirectUrl = authService.buildFrontendCallbackUrl({
      status: "error",
      message
    });

    res.redirect(302, redirectUrl);
  }
};
