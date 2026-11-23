import type { ChatRoomMessageHistory } from '../chat.types';
import { getApiBaseUrl } from '../chat.utils';

/** `before`가 null이면 가장 최근 페이지를 가져온다. */
export async function requestRoomMessageHistory(
  roomId: string,
  before: string | null,
  signal: AbortSignal,
): Promise<ChatRoomMessageHistory> {
  const url = new URL(`/v1/rooms/${encodeURIComponent(roomId)}/messages`, getApiBaseUrl());
  if (before !== null) {
    url.searchParams.set('before', before);
  }

  const response = await fetch(url, { signal });
  if (!response.ok) {
    throw new Error(`message history request failed with ${response.status}`);
  }
  return (await response.json()) as ChatRoomMessageHistory;
}
