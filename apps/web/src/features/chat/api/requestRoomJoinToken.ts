import { HTTP_STATUS_NOT_FOUND } from '../chat.constants';
import type { ChatRoomJoinToken, ChatRoomJoinTokenResult } from '../chat.types';
import { getApiBaseUrl } from '../chat.utils';

export async function requestRoomJoinToken(
  roomId: string,
  nickname: string,
  /** 방장이면 보관해 둔 열쇠, 아니면 null. */
  ownerKey: string | null,
  signal: AbortSignal,
): Promise<ChatRoomJoinTokenResult> {
  const response = await fetch(
    new URL(`/v1/rooms/${encodeURIComponent(roomId)}/join-tokens`, getApiBaseUrl()),
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(ownerKey === null ? { nickname } : { nickname, ownerKey }),
      signal,
    },
  );

  // 방이 없거나 닫혔다. 다시 시도해도 소용없으므로 일시적인 실패와 구분해서 알린다.
  if (response.status === HTTP_STATUS_NOT_FOUND) {
    return { status: 'roomNotFound' };
  }
  if (!response.ok) {
    throw new Error(`join token request failed with ${response.status}`);
  }
  return { status: 'issued', joinToken: (await response.json()) as ChatRoomJoinToken };
}
