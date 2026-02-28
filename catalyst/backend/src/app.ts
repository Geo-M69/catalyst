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

const allowedOrigins = new Set<string>([env.FRONTEND_BASE_URL]);

if (env.NODE_ENV !== "production") {
  allowedOrigins.add("http://localhost:1420");
  allowedOrigins.add("http://127.0.0.1:1420");
  allowedOrigins.add("tauri://localhost");
  allowedOrigins.add("http://tauri.localhost");
  allowedOrigins.add("null");
}

app.use(
  cors({
    origin: (requestOrigin, callback) => {
      if (!requestOrigin || allowedOrigins.has(requestOrigin)) {
        callback(null, true);
        return;
      }

      callback(new Error(`CORS origin not allowed: ${requestOrigin}`));
    },
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
