import { config as loadEnv } from "dotenv";
import { z } from "zod";

loadEnv();

const booleanStringSchema = z
  .union([z.literal("true"), z.literal("false")])
  .transform((value) => value === "true");

const envSchema = z.object({
  NODE_ENV: z.enum(["development", "test", "production"]).default("development"),
  PORT: z.coerce.number().int().positive().default(4000),
  APP_BASE_URL: z.string().url().default("http://localhost:4000"),
  FRONTEND_BASE_URL: z.string().url().default("http://localhost:1420"),
  FRONTEND_STEAM_CALLBACK_PATH: z.string().default("/auth/steam/callback"),
  DATABASE_PATH: z.string().default("./data/catalyst.db"),
  SESSION_COOKIE_NAME: z.string().min(1).default("catalyst_sid"),
  SESSION_TTL_DAYS: z.coerce.number().int().positive().default(30),
  SESSION_COOKIE_SECURE: booleanStringSchema.default("false"),
  TRUST_PROXY: booleanStringSchema.default("false"),
  RATE_LIMIT_WINDOW_MS: z.coerce.number().int().positive().default(60_000),
  RATE_LIMIT_MAX_AUTH: z.coerce.number().int().positive().default(20),
  RATE_LIMIT_MAX_STEAM: z.coerce.number().int().positive().default(30),
  STEAM_API_KEY: z.string().default(""),
  STEAM_SYNC_INCLUDE_PLAYED_FREE_GAMES: booleanStringSchema.default("true")
});

const parsedEnv = envSchema.safeParse(process.env);

if (!parsedEnv.success) {
  throw new Error(`Invalid environment: ${parsedEnv.error.message}`);
}

export const env = parsedEnv.data;
