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

  // 4xx는 요청 자체가 받아들여지지 않은 것이다 (없는 방, 닫힌 방, 잘못된 닉네임).
  // 다시 보내도 같은 답이므로, 재시도할 일시적인 실패(5xx, 연결 실패)와 구분해서 알린다.
  if (response.status >= 400 && response.status < 500) {
    return { status: 'refused' };
  }
  if (!response.ok) {
    throw new Error(`join token request failed with ${response.status}`);
  }
  return { status: 'issued', joinToken: (await response.json()) as ChatRoomJoinToken };
}
