import { getApiBaseUrl } from '../chat.utils';

/** 방의 채팅 내역 전체를 CSV로 받는다. 방장 열쇠가 맞아야 한다. */
export async function requestRoomMessageExport(roomId: string, ownerKey: string): Promise<Blob> {
  const response = await fetch(
    new URL(`/v1/rooms/${encodeURIComponent(roomId)}/export`, getApiBaseUrl()),
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ownerKey }),
    },
  );
  if (!response.ok) {
    throw new Error(`message export failed with ${response.status}`);
  }
  return response.blob();
}
