export interface UserIntegrations {
  steamId?: string;
}

export interface User {
  id: string;
  integrations: UserIntegrations;
  createdAt: string;
  updatedAt: string;
}
