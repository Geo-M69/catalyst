import { Router } from "express";

import { requireSession } from "../../shared/middleware/auth-session.js";
import { getLibraryController } from "./library.controller.js";

export const libraryRouter = Router();

libraryRouter.get("/", requireSession, getLibraryController);
