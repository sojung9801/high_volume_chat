import { Hono } from 'hono';

import type { ApiEnv } from '../config/env.types.ts';
import type { DatabaseClient } from '../db/database.types.ts';
import { closeGatewayRoom } from '../gateway/gatewayInternalClient.ts';
import { isRoomOwner } from './isRoomOwner.ts';
import { markRoomClosed } from './markRoomClosed.ts';
import { roomIdSchema, roomOwnerRequestSchema } from './room.schemas.ts';

export function createRoomCloseRoutes(env: ApiEnv, database: DatabaseClient): Hono {
  const routes = new Hono();

  routes.post('/:roomId/close', async (c) => {
    const roomId = roomIdSchema.safeParse(c.req.param('roomId'));
    const body = roomOwnerRequestSchema.safeParse(await c.req.json().catch(() => null));
    if (!roomId.success || !body.success) {
      return c.json({ error: 'invalid_request' }, 400);
    }

    try {
      if (!(await isRoomOwner(database, roomId.data, body.data.ownerKey))) {
        return c.json({ error: 'not_room_owner' }, 403);
      }
      // 순서가 중요하다. 먼저 DB에 닫힘을 기록해 새 입장을 막고, 그다음 붙어 있는 사람을 내보낸다.
      // 반대로 하면 내보낸 사람이 재접속해서 다시 들어올 틈이 생긴다.
      await markRoomClosed(database, roomId.data);
    } catch (error) {
      console.error('failed to close room', error);
      return c.json({ error: 'room_store_unavailable' }, 503);
    }

    try {
      await closeGatewayRoom(env.GATEWAY_INTERNAL_URL, roomId.data);
    } catch (error) {
      // 방은 이미 닫혔다. 남아 있는 연결은 끊기는 대로 다시 들어오지 못한다.
      console.error('room closed but gateway could not be told', error);
    }
    return c.json({ roomId: roomId.data });
  });

  return routes;
}
