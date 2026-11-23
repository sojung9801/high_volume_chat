import { HTTP_STATUS_NOT_FOUND } from '../chat.constants';
import { getApiBaseUrl } from '../chat.utils';

/** 방이 만들어져 있고 열려 있는지 확인한다. */
export async function requestRoomIsOpen(roomId: string): Promise<boolean> {
  const response = await fetch(new URL(`/v1/rooms/${encodeURIComponent(roomId)}`, getApiBaseUrl()));
  if (response.status === HTTP_STATUS_NOT_FOUND) {
    return false;
  }
  if (!response.ok) {
    throw new Error(`room check failed with ${response.status}`);
  }
  return true;
}
