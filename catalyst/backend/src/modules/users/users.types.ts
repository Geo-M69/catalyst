export interface UserIntegrations {
  steamId?: string;
}

export interface User {
  id: string;
  email: string;
  integrations: UserIntegrations;
  createdAt: string;
  updatedAt: string;
}

export interface AuthUserRecord {
  user: User;
  passwordHash: string;
}
