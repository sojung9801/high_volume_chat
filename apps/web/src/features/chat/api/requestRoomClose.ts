import { getApiBaseUrl } from '../chat.utils';

/** 방을 닫는다. 방장 열쇠가 맞아야 한다. 닫히면 모든 참여자가 내보내진다. */
export async function requestRoomClose(roomId: string, ownerKey: string): Promise<void> {
  const response = await fetch(
    new URL(`/v1/rooms/${encodeURIComponent(roomId)}/close`, getApiBaseUrl()),
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ownerKey }),
    },
  );
  if (!response.ok) {
    throw new Error(`room close failed with ${response.status}`);
  }
}
