import { serve } from '@hono/node-server';
import { Hono } from 'hono';
import { cors } from 'hono/cors';

import { loadEnv } from './config/env.ts';
import { createDatabaseClient } from './db/createDatabaseClient.ts';
import { createRoomMessageExportRoutes } from './messages/roomMessageExportRoutes.ts';
import { createRoomMessageHistoryRoutes } from './messages/roomMessageHistoryRoutes.ts';
import { createAdminRoomRoutes } from './rooms/adminRoomRoutes.ts';
import { closeExpiredRooms } from './rooms/closeExpiredRooms.ts';
import { createOpenRoomListRoutes } from './rooms/openRoomListRoutes.ts';
import { createRoomCloseRoutes } from './rooms/roomCloseRoutes.ts';
import { createRoomCreationRoutes } from './rooms/roomCreationRoutes.ts';
import { ROOM_EXPIRY_SWEEP_INTERVAL_MS } from './rooms/room.constants.ts';
import { createRoomJoinTokenRoutes } from './rooms/roomJoinTokenRoutes.ts';

const env = loadEnv(process.env);
const database = createDatabaseClient(env.DATABASE_URL);
const app = new Hono();

app.use('/v1/rooms', cors({ origin: env.WEB_ORIGIN }));
app.use('/v1/rooms/*', cors({ origin: env.WEB_ORIGIN }));
app.get('/healthz', (c) => c.text('ok'));
app.route('/v1/rooms', createRoomCreationRoutes(env, database));
app.route('/v1/rooms', createOpenRoomListRoutes(env, database));
app.route('/v1/rooms', createRoomJoinTokenRoutes(env, database));
app.route('/v1/rooms', createRoomMessageHistoryRoutes(env, database));
app.route('/v1/rooms', createRoomMessageExportRoutes(database));
app.route('/v1/rooms', createRoomCloseRoutes(env, database));
app.route('/v1/admin', createAdminRoomRoutes(env));

// 유효기간이 지난 방을 주기적으로 닫는다.
setInterval(() => {
  closeExpiredRooms(database, env)
    .then((roomIds) => {
      if (roomIds.length > 0) {
        console.log(`closed expired rooms: ${roomIds.join(', ')}`);
      }
    })
    .catch((error: unknown) => {
      console.error('failed to close expired rooms', error);
    });
}, ROOM_EXPIRY_SWEEP_INTERVAL_MS);

serve({ fetch: app.fetch, port: env.API_PORT }, (info) => {
  console.log(`api listening on :${info.port}`);
});
