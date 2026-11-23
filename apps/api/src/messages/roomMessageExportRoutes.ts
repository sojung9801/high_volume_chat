import { Hono } from 'hono';
import { stream } from 'hono/streaming';

import type { DatabaseClient } from '../db/database.types.ts';
import { isRoomOwner } from '../rooms/isRoomOwner.ts';
import { roomIdSchema, roomOwnerRequestSchema } from '../rooms/room.schemas.ts';
import { CSV_UTF8_BOM, EXPORT_CSV_HEADER } from './message.constants.ts';
import { toCsvLine } from './message.utils.ts';
import { streamRoomMessages } from './streamRoomMessages.ts';

export function createRoomMessageExportRoutes(database: DatabaseClient): Hono {
  const routes = new Hono();

  // 열쇠가 주소(URL)에 남지 않도록 GET이 아니라 POST 본문으로 받는다.
  routes.post('/:roomId/export', async (c) => {
    const roomId = roomIdSchema.safeParse(c.req.param('roomId'));
    const body = roomOwnerRequestSchema.safeParse(await c.req.json().catch(() => null));
    if (!roomId.success || !body.success) {
      return c.json({ error: 'invalid_request' }, 400);
    }

    try {
      if (!(await isRoomOwner(database, roomId.data, body.data.ownerKey))) {
        return c.json({ error: 'not_room_owner' }, 403);
      }
    } catch (error) {
      console.error('failed to check room owner', error);
      return c.json({ error: 'room_store_unavailable' }, 503);
    }

    c.header('Content-Type', 'text/csv; charset=utf-8');
    c.header('Content-Disposition', `attachment; filename="chat-${roomId.data}.csv"`);
    return stream(c, async (output) => {
      await output.write(`${CSV_UTF8_BOM}${EXPORT_CSV_HEADER}\n`);
      for await (const messages of streamRoomMessages(database, roomId.data)) {
        await output.write(`${messages.map(toCsvLine).join('\n')}\n`);
      }
    });
  });

  return routes;
}
