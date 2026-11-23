import { Hono } from 'hono';

import type { ApiEnv } from '../config/env.types.ts';
import type { DatabaseClient } from '../db/database.types.ts';
import { closeGatewayRoom } from '../gateway/gatewayInternalClient.ts';
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
    let generation: number | null;
    try {
      generation = await insertRoom(
        database,
        body.data.roomId,
        hashOwnerKey(ownerKey),
        env.ROOM_TTL_HOURS,
      );
    } catch (error) {
      console.error('failed to create room', error);
      return c.json({ error: 'room_store_unavailable' }, 503);
    }
    if (generation === null) {
      return c.json({ error: 'room_already_exists' }, 409);
    }

    // 같은 ID의 이전 방이 게이트웨이에 아직 떠 있을 수 있다 (유효기간은 지났는데 정리 작업이 돌기 전).
    // 새 방보다 앞선 회차는 전부 닫아서, 이전 참여자가 새 방에 남지 않게 한다.
    try {
      await closeGatewayRoom(env.GATEWAY_INTERNAL_URL, body.data.roomId, generation - 1);
    } catch (error) {
      // 실패해도 새 회차의 첫 입장 때 게이트웨이가 이전 방을 닫는다.
      console.error('room created but gateway could not close the previous one', error);
    }

    const response: RoomCreateResponse = { roomId: body.data.roomId, ownerKey };
    return c.json(response, 201);
  });

  return routes;
}
