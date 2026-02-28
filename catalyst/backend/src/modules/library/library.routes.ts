import { Router } from "express";

import { getLibraryController } from "./library.controller.js";

export const libraryRouter = Router();

libraryRouter.get("/", getLibraryController);
