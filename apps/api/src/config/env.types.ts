export type ApiEnv = {
  API_PORT: number;
  JOIN_TOKEN_SECRET: string;
  ADMIN_API_TOKEN: string;
  GATEWAY_INTERNAL_URL: string;
  GATEWAY_PUBLIC_WS_URL: string;
  WEB_ORIGIN: string;
  DATABASE_URL: string;
  ROOM_TTL_HOURS: number;
  HISTORY_PAGE_SIZE: number;
};
