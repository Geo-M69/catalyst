import type { ParsedQs } from "qs";
import type { Request, RequestHandler } from "express";
import { z } from "zod";

import { HttpError } from "../../shared/errors/http-error.js";

import { authService } from "./auth.service.js";
import { sessionService } from "./session.service.js";

const credentialsSchema = z.object({
  email: z.string().email(),
  password: z.string().min(8).max(128)
});

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

const getAuthenticatedUser = (request: Request): NonNullable<Request["authUser"]> => {
  if (!request.authUser) {
    throw new HttpError(401, "Not authenticated", "UNAUTHORIZED");
  }

  return request.authUser;
};

const parseCredentials = (payload: unknown): z.infer<typeof credentialsSchema> => {
  const parsed = credentialsSchema.safeParse(payload);
  if (!parsed.success) {
    throw new HttpError(400, "Invalid credentials payload", "VALIDATION_ERROR");
  }

  return parsed.data;
};

export const registerController: RequestHandler = async (req, res, next) => {
  try {
    const credentials = parseCredentials(req.body);
    const result = await sessionService.register(credentials.email, credentials.password);

    sessionService.setSessionCookie(res, result.sessionToken);

    res.status(201).json({
      user: authService.toPublicUser(result.user)
    });
  } catch (error) {
    next(error);
  }
};

export const loginController: RequestHandler = async (req, res, next) => {
  try {
    const credentials = parseCredentials(req.body);
    const result = await sessionService.login(credentials.email, credentials.password);

    sessionService.setSessionCookie(res, result.sessionToken);

    res.json({
      user: authService.toPublicUser(result.user)
    });
  } catch (error) {
    next(error);
  }
};

export const logoutController: RequestHandler = (req, res, next) => {
  try {
    if (!req.sessionToken) {
      throw new HttpError(401, "Not authenticated", "UNAUTHORIZED");
    }

    sessionService.invalidateSession(req.sessionToken);
    sessionService.clearSessionCookie(res);

    res.status(204).send();
  } catch (error) {
    next(error);
  }
};

export const sessionController: RequestHandler = (req, res, next) => {
  try {
    const user = getAuthenticatedUser(req);

    res.json({
      user: authService.toPublicUser(user)
    });
  } catch (error) {
    next(error);
  }
};

export const startSteamAuthController: RequestHandler = (req, res, next) => {
  try {
    const user = getAuthenticatedUser(req);
    const result = authService.startSteamAuth(user.id);

    res.json(result);
  } catch (error) {
    next(error);
  }
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
