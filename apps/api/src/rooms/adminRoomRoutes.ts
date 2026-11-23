import { Hono } from 'hono';
import { bearerAuth } from 'hono/bearer-auth';

import type { ApiEnv } from '../config/env.types.ts';
import { fetchGatewayRoomStats } from '../gateway/gatewayInternalClient.ts';

export function createAdminRoomRoutes(env: ApiEnv): Hono {
  const routes = new Hono();

  routes.use('*', bearerAuth({ token: env.ADMIN_API_TOKEN }));

  routes.get('/rooms', async (c) => {
    try {
      return c.json({ rooms: await fetchGatewayRoomStats(env.GATEWAY_INTERNAL_URL) });
    } catch (error) {
      console.error('failed to fetch gateway room stats', error);
      return c.json({ error: 'gateway_unavailable' }, 502);
    }
  });

  return routes;
}
