import { randomUUID } from 'node:crypto';

import { Hono } from 'hono';

import type { ApiEnv } from '../config/env.types.ts';
import type { DatabaseClient } from '../db/database.types.ts';
import { isRoomOpen } from './isRoomOpen.ts';
import { isRoomOwner } from './isRoomOwner.ts';
import { signRoomJoinToken } from './joinToken.ts';
import { JOIN_TOKEN_TTL_SEC } from './room.constants.ts';
import { roomIdSchema, roomJoinRequestSchema } from './room.schemas.ts';
import type { RoomJoinTokenResponse } from './room.types.ts';

export function createRoomJoinTokenRoutes(env: ApiEnv, database: DatabaseClient): Hono {
  const routes = new Hono();

  routes.post('/:roomId/join-tokens', async (c) => {
    const roomId = roomIdSchema.safeParse(c.req.param('roomId'));
    const body = roomJoinRequestSchema.safeParse(await c.req.json().catch(() => null));
    if (!roomId.success || !body.success) {
      return c.json({ error: 'invalid_request' }, 400);
    }

    // 만들어진 방에만 들어갈 수 있다. 게이트웨이는 여기서 발급한 토큰만 받으므로
    // 이 한 곳만 막으면 없는 방이나 닫힌 방에는 아무도 접속하지 못한다.
    let isOpen: boolean;
    let isOwner = false;
    try {
      isOpen = await isRoomOpen(database, roomId.data, env.ROOM_TTL_HOURS);
      if (isOpen && body.data.ownerKey !== undefined) {
        isOwner = await isRoomOwner(database, roomId.data, body.data.ownerKey);
      }
    } catch (error) {
      console.error('failed to check room', error);
      return c.json({ error: 'room_store_unavailable' }, 503);
    }
    if (!isOpen) {
      return c.json({ error: 'room_not_found' }, 404);
    }

    // 계정 시스템이 붙기 전까지는 접속마다 익명 ID를 발급한다.
    const token = await signRoomJoinToken(
      { userId: randomUUID(), roomId: roomId.data, nickname: body.data.nickname, isOwner },
      env.JOIN_TOKEN_SECRET,
    );

    const response: RoomJoinTokenResponse = {
      token,
      gatewayWsUrl: env.GATEWAY_PUBLIC_WS_URL,
      expiresInSec: JOIN_TOKEN_TTL_SEC,
      isOwner,
    };
    return c.json(response);
  });

  return routes;
}
