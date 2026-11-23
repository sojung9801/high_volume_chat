import { GATEWAY_REQUEST_TIMEOUT_MS } from '../rooms/room.constants.ts';
import { roomStatsListSchema } from '../rooms/room.schemas.ts';
import type { RoomStats } from '../rooms/room.types.ts';

/** 게이트웨이 내부 포트에서 방 현황을 가져온다. 게이트웨이가 여러 대가 되면 여기서 합산한다. */
export async function fetchGatewayRoomStats(gatewayInternalUrl: string): Promise<RoomStats[]> {
  const response = await fetch(new URL('/internal/rooms', gatewayInternalUrl), {
    signal: AbortSignal.timeout(GATEWAY_REQUEST_TIMEOUT_MS),
  });
  if (!response.ok) {
    throw new Error(`gateway responded with ${response.status}`);
  }
  return roomStatsListSchema.parse(await response.json());
}

/** 게이트웨이에 붙어 있는 이 방의 연결을 모두 내보낸다. */
export async function closeGatewayRoom(gatewayInternalUrl: string, roomId: string): Promise<void> {
  const response = await fetch(
    new URL(`/internal/rooms/${encodeURIComponent(roomId)}/close`, gatewayInternalUrl),
    { method: 'POST', signal: AbortSignal.timeout(GATEWAY_REQUEST_TIMEOUT_MS) },
  );
  if (!response.ok) {
    throw new Error(`gateway responded with ${response.status}`);
  }
}
