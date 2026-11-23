'use client';

import { useEffect, useState } from 'react';

import { generateRandomNickname, loadSavedNickname, saveNickname } from '../chat.utils';

/**
 * 이 브라우저의 닉네임. 저장된 것이 없으면 새로 만들어 저장한다
 * (첫 화면을 거치지 않고 방 링크로 바로 들어온 경우).
 * 브라우저 저장소는 화면이 뜬 뒤에만 읽을 수 있어서 그 전에는 `null`이다.
 */
export function useSavedNickname(): string | null {
  const [nickname, setNickname] = useState<string | null>(null);

  useEffect(() => {
    const saved = loadSavedNickname();
    if (saved !== null) {
      setNickname(saved);
      return;
    }
    const generated = generateRandomNickname();
    saveNickname(generated);
    setNickname(generated);
  }, []);

  return nickname;
}
