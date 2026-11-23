import { z } from 'zod';

import type { ApiEnv } from './env.types.ts';

const envSchema = z.object({
  API_PORT: z.coerce.number().int().positive(),
  JOIN_TOKEN_SECRET: z.string().min(32),
  ADMIN_API_TOKEN: z.string().min(16),
  GATEWAY_INTERNAL_URL: z.url(),
  GATEWAY_PUBLIC_WS_URL: z.url(),
  WEB_ORIGIN: z.url(),
  DATABASE_URL: z.url(),
  ROOM_TTL_HOURS: z.coerce.number().int().positive(),
  HISTORY_PAGE_SIZE: z.coerce.number().int().positive().max(200),
});

export function loadEnv(source: NodeJS.ProcessEnv): ApiEnv {
  const parsed = envSchema.safeParse(source);
  if (!parsed.success) {
    throw new Error(`Invalid environment:\n${z.prettifyError(parsed.error)}`);
  }
  return parsed.data;
}
