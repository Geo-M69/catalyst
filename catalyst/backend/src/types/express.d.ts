export {};

declare global {
  namespace Express {
    interface Request {
      authUser?: {
        id: string;
        email: string;
        integrations: {
          steamId?: string;
        };
        createdAt: string;
        updatedAt: string;
      };
      sessionToken?: string;
    }
  }
}
