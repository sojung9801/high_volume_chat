import { Hono } from 'hono';

import type { ApiEnv } from '../config/env.types.ts';
import type { DatabaseClient } from '../db/database.types.ts';
import { roomIdSchema } from '../rooms/room.schemas.ts';
import { findRoomMessages } from './findRoomMessages.ts';
import type { RoomMessageCursor, RoomMessageHistoryResponse } from './message.types.ts';
import { formatMessageCursor, parseMessageCursor } from './message.utils.ts';

export function createRoomMessageHistoryRoutes(env: ApiEnv, database: DatabaseClient): Hono {
  const routes = new Hono();

  routes.get('/:roomId/messages', async (c) => {
    const roomId = roomIdSchema.safeParse(c.req.param('roomId'));
    const rawBefore = c.req.query('before');
    let before: RoomMessageCursor | null = null;
    if (rawBefore !== undefined) {
      before = parseMessageCursor(rawBefore);
    }
    if (!roomId.success || (rawBefore !== undefined && before === null)) {
      return c.json({ error: 'invalid_request' }, 400);
    }

    let messages;
    try {
      messages = await findRoomMessages(database, roomId.data, before, env.HISTORY_PAGE_SIZE);
    } catch (error) {
      console.error('failed to load room messages', error);
      return c.json({ error: 'history_unavailable' }, 503);
    }

    // 한 페이지를 꽉 채웠으면 더 오래된 것이 있을 수 있다.
    const oldest = messages[0];
    const response: RoomMessageHistoryResponse = {
      messages,
      olderCursor:
        oldest !== undefined && messages.length === env.HISTORY_PAGE_SIZE
          ? formatMessageCursor(oldest)
          : null,
    };
    return c.json(response);
  });

  return routes;
}
