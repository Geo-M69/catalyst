import type { RequestHandler } from "express";

import { env } from "../../config/env.js";
import { sessionService } from "../../modules/auth/session.service.js";
import { HttpError } from "../errors/http-error.js";

const getSessionTokenFromRequest = (rawCookie: unknown): string | null => {
  if (typeof rawCookie !== "string" || rawCookie.trim().length === 0) {
    return null;
  }

  return rawCookie;
};

export const requireSession: RequestHandler = (req, res, next) => {
  try {
    const sessionToken = getSessionTokenFromRequest(req.cookies?.[env.SESSION_COOKIE_NAME]);
    if (!sessionToken) {
      throw new HttpError(401, "Not authenticated", "UNAUTHORIZED");
    }

    const user = sessionService.getUserBySessionToken(sessionToken);
    if (!user) {
      sessionService.clearSessionCookie(res);
      throw new HttpError(401, "Session expired or invalid", "UNAUTHORIZED");
    }

    req.authUser = user;
    req.sessionToken = sessionToken;
    next();
  } catch (error) {
    next(error);
  }
};
