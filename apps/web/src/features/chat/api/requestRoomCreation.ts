import { HTTP_STATUS_CONFLICT } from '../chat.constants';
import type { ChatRoomCreationResult } from '../chat.types';
import { getApiBaseUrl } from '../chat.utils';

export async function requestRoomCreation(roomId: string): Promise<ChatRoomCreationResult> {
  const response = await fetch(new URL('/v1/rooms', getApiBaseUrl()), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ roomId }),
  });

  // 같은 ID의 방이 이미 있다. 오류가 아니라 사용자에게 알려줄 정상 결과다.
  if (response.status === HTTP_STATUS_CONFLICT) {
    return { status: 'alreadyExists' };
  }
  if (!response.ok) {
    throw new Error(`room creation failed with ${response.status}`);
  }

  const created = (await response.json()) as { ownerKey: string };
  return { status: 'created', ownerKey: created.ownerKey };
}
