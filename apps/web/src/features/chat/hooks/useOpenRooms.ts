'use client';

import { useEffect, useState } from 'react';

import { requestOpenRooms } from '../api/requestOpenRooms';
import { CHAT_OPEN_ROOMS_REFRESH_MS } from '../chat.constants';
import type { ChatOpenRoom } from '../chat.types';

/**
 * 진행 중인 방 목록. 화면에 떠 있는 동안 일정 간격으로 다시 불러온다.
 * 아직 한 번도 못 불러왔으면 `null`이다 ("방이 없음"과 구분하기 위해서다).
 */
export function useOpenRooms(): ChatOpenRoom[] | null {
  const [rooms, setRooms] = useState<ChatOpenRoom[] | null>(null);

  useEffect(() => {
    const abortController = new AbortController();

    const load = async () => {
      try {
        setRooms(await requestOpenRooms(abortController.signal));
      } catch {
        // 못 불러오면 직전 목록을 그대로 둔다. 다음 주기에 다시 시도한다.
      }
    };

    void load();
    const timer = setInterval(load, CHAT_OPEN_ROOMS_REFRESH_MS);
    return () => {
      abortController.abort();
      clearInterval(timer);
    };
  }, []);

  return rooms;
}
