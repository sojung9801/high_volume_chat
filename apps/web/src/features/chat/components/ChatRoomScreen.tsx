'use client';

import { useSavedNickname } from '../hooks/useSavedNickname';
import { ChatRoomPanel } from './ChatRoomPanel';

type ChatRoomScreenProps = {
  roomId: string;
};

/** 방 화면. 닉네임을 브라우저에서 꺼낸 뒤에 방에 접속한다. */
export function ChatRoomScreen({ roomId }: ChatRoomScreenProps) {
  const nickname = useSavedNickname();

  if (nickname === null) {
    return null;
  }
  return <ChatRoomPanel roomId={roomId} nickname={nickname} />;
}
