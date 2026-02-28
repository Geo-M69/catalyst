import type { User } from "../users/users.types.js";

export interface SessionRecord {
  tokenHash: string;
  userId: string;
  createdAt: string;
  expiresAt: string;
  lastSeenAt: string;
}

export interface SessionAuthResult {
  user: User;
  sessionToken: string;
}
