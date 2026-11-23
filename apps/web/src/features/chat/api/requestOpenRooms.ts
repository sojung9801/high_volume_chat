import type { ChatOpenRoom } from '../chat.types';
import { getApiBaseUrl } from '../chat.utils';

export async function requestOpenRooms(signal: AbortSignal): Promise<ChatOpenRoom[]> {
  const response = await fetch(new URL('/v1/rooms', getApiBaseUrl()), { signal });
  if (!response.ok) {
    throw new Error(`open room list request failed with ${response.status}`);
  }
  const body = (await response.json()) as { rooms: ChatOpenRoom[] };
  return body.rooms;
}
