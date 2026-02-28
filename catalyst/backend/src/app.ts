import cookieParser from "cookie-parser";
import cors from "cors";
import express from "express";

import { env } from "./config/env.js";
import { authRouter } from "./modules/auth/auth.routes.js";
import { libraryRouter } from "./modules/library/library.routes.js";
import { steamIntegrationRouter } from "./modules/integrations/steam/steam.routes.js";
import { errorHandler } from "./shared/middleware/error-handler.js";
import { notFoundHandler } from "./shared/middleware/not-found.js";

export const app = express();

if (env.TRUST_PROXY) {
  app.set("trust proxy", 1);
}

app.use(
  cors({
    origin: env.FRONTEND_BASE_URL,
    credentials: true
  })
);

app.use(cookieParser());
app.use(express.json());

app.get("/health", (_req, res) => {
  res.json({
    status: "ok"
  });
});

app.use("/auth", authRouter);
app.use("/integrations/steam", steamIntegrationRouter);
app.use("/library", libraryRouter);

app.use(notFoundHandler);
app.use(errorHandler);
