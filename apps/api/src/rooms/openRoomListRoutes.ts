import { Hono } from 'hono';

import type { ApiEnv } from '../config/env.types.ts';
import type { DatabaseClient } from '../db/database.types.ts';
import { findOpenRooms } from './findOpenRooms.ts';
import { isRoomOpen } from './isRoomOpen.ts';
import { roomIdSchema } from './room.schemas.ts';
import type { OpenRoomListResponse } from './room.types.ts';

export function createOpenRoomListRoutes(env: ApiEnv, database: DatabaseClient): Hono {
  const routes = new Hono();

  routes.get('/', async (c) => {
    try {
      const response: OpenRoomListResponse = { rooms: await findOpenRooms(database, env.ROOM_TTL_HOURS) };
      return c.json(response);
    } catch (error) {
      console.error('failed to list open rooms', error);
      return c.json({ error: 'room_store_unavailable' }, 503);
    }
  });

  /** 방 하나가 열려 있는지. 입장 전에 화면이 확인한다. */
  routes.get('/:roomId', async (c) => {
    const roomId = roomIdSchema.safeParse(c.req.param('roomId'));
    if (!roomId.success) {
      return c.json({ error: 'invalid_request' }, 400);
    }
    try {
      if (!(await isRoomOpen(database, roomId.data, env.ROOM_TTL_HOURS))) {
        return c.json({ error: 'room_not_found' }, 404);
      }
    } catch (error) {
      console.error('failed to check room', error);
      return c.json({ error: 'room_store_unavailable' }, 503);
    }
    return c.json({ roomId: roomId.data });
  });

  return routes;
}
