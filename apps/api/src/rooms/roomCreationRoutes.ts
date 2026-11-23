import { Hono } from 'hono';

import type { ApiEnv } from '../config/env.types.ts';
import type { DatabaseClient } from '../db/database.types.ts';
import { insertRoom } from './insertRoom.ts';
import { generateOwnerKey, hashOwnerKey } from './ownerKey.ts';
import { roomCreateRequestSchema } from './room.schemas.ts';
import type { RoomCreateResponse } from './room.types.ts';

export function createRoomCreationRoutes(env: ApiEnv, database: DatabaseClient): Hono {
  const routes = new Hono();

  routes.post('/', async (c) => {
    const body = roomCreateRequestSchema.safeParse(await c.req.json().catch(() => null));
    if (!body.success) {
      return c.json({ error: 'invalid_request' }, 400);
    }

    const ownerKey = generateOwnerKey();
    let isCreated: boolean;
    try {
      isCreated = await insertRoom(
        database,
        body.data.roomId,
        hashOwnerKey(ownerKey),
        env.ROOM_TTL_HOURS,
      );
    } catch (error) {
      console.error('failed to create room', error);
      return c.json({ error: 'room_store_unavailable' }, 503);
    }
    if (!isCreated) {
      return c.json({ error: 'room_already_exists' }, 409);
    }

    const response: RoomCreateResponse = { roomId: body.data.roomId, ownerKey };
    return c.json(response, 201);
  });

  return routes;
}
