import { config as loadEnv } from "dotenv";
import { z } from "zod";

loadEnv();

const envSchema = z.object({
  NODE_ENV: z.enum(["development", "test", "production"]).default("development"),
  PORT: z.coerce.number().int().positive().default(4000),
  APP_BASE_URL: z.string().url().default("http://localhost:4000"),
  FRONTEND_BASE_URL: z.string().url().default("http://localhost:1420"),
  FRONTEND_STEAM_CALLBACK_PATH: z.string().default("/auth/steam/callback"),
  STEAM_API_KEY: z.string().default(""),
  STEAM_SYNC_INCLUDE_PLAYED_FREE_GAMES: z
    .union([z.literal("true"), z.literal("false")])
    .default("true")
    .transform((value) => value === "true")
});

const parsedEnv = envSchema.safeParse(process.env);

if (!parsedEnv.success) {
  throw new Error(`Invalid environment: ${parsedEnv.error.message}`);
}

export const env = parsedEnv.data;
