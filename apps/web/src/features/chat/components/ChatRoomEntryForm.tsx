'use client';

import { useEffect, useRef, useState, type FormEvent } from 'react';
import { useRouter } from 'next/navigation';
import { useTranslations } from 'next-intl';

import { requestRoomCreation } from '../api/requestRoomCreation';
import { requestRoomIsOpen } from '../api/requestRoomIsOpen';
import {
  CHAT_ENTRY_NOTICE_MESSAGE_KEYS,
  CHAT_NICKNAME_MAX_LENGTH,
  CHAT_ROOM_ID_PATTERN,
} from '../chat.constants';
import type { ChatRoomEntryNotice } from '../chat.types';
import {
  generateRandomNickname,
  loadSavedNickname,
  saveNickname,
  saveRoomOwnerKey,
  takeLobbyNotice,
} from '../chat.utils';
import styles from './ChatRoom.module.css';

type ChatRoomEntryFormProps = {
  roomId: string;
  onRoomIdChange: (roomId: string) => void;
};

export function ChatRoomEntryForm({ roomId, onRoomIdChange }: ChatRoomEntryFormProps) {
  const t = useTranslations();
  const router = useRouter();
  const formRef = useRef<HTMLFormElement>(null);
  const [nickname, setNickname] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const [notice, setNotice] = useState<ChatRoomEntryNotice | null>(null);

  // 전에 쓰던 닉네임이 있으면 그대로, 없으면 새로 만든다. 방을 나왔다 들어와도 이름이 바뀌지 않는다.
  // 화면이 뜬 뒤에 채운다. 서버에서 미리 그릴 때 만들면 브라우저가 만든 값과 달라져 경고가 난다.
  useEffect(() => {
    setNickname(loadSavedNickname() ?? generateRandomNickname());
  }, []);

  // 방에서 내보내져 돌아왔으면 그 이유를 한 번 보여준다. 꺼내면서 지우므로 새로고침하면 사라진다.
  useEffect(() => {
    const leftNotice = takeLobbyNotice();
    if (leftNotice !== null) {
      setNotice(leftNotice);
    }
  }, []);

  const enterRoom = () => {
    // 닉네임은 주소에 싣지 않고 브라우저에 저장한다. 방 화면이 거기서 꺼내 쓴다.
    saveNickname(nickname.trim());
    router.push(`/rooms/${encodeURIComponent(roomId)}`);
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setNotice(null);

    let isOpen: boolean;
    try {
      isOpen = await requestRoomIsOpen(roomId);
    } catch {
      // 확인 자체가 실패하면 일단 들여보낸다. 방 화면이 다시 확인하고 재시도한다.
      isOpen = true;
    }
    if (!isOpen) {
      setNotice('roomNotFound');
      return;
    }
    enterRoom();
  };

  const handleCreateRoom = async () => {
    // 입장 버튼과 같은 입력 검사(필수, 형식)를 거친다.
    if (!formRef.current?.reportValidity()) {
      return;
    }

    setNotice(null);
    setIsCreating(true);
    try {
      const result = await requestRoomCreation(roomId);
      if (result.status === 'alreadyExists') {
        setNotice('roomAlreadyExists');
        return;
      }
      saveRoomOwnerKey(roomId, result.ownerKey);
      enterRoom();
    } catch {
      setNotice('createRoomFailed');
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <form ref={formRef} className={styles.entryForm} onSubmit={handleSubmit}>
      <label className={styles.entryField}>
        {t('messages.chat.room_id')}
        <input
          className={styles.textInput}
          value={roomId}
          pattern={CHAT_ROOM_ID_PATTERN}
          required
          onChange={(event) => onRoomIdChange(event.target.value)}
        />
      </label>
      <label className={styles.entryField}>
        {t('messages.chat.nickname')}
        <input
          className={styles.textInput}
          value={nickname}
          maxLength={CHAT_NICKNAME_MAX_LENGTH}
          required
          onChange={(event) => setNickname(event.target.value)}
        />
      </label>
      {notice && (
        <p className={styles.entryNotice} role="alert">
          {t(CHAT_ENTRY_NOTICE_MESSAGE_KEYS[notice])}
        </p>
      )}
      <button
        type="button"
        className={styles.secondaryButton}
        disabled={isCreating}
        onClick={handleCreateRoom}
      >
        {t('messages.chat.create_room')}
      </button>
      <button type="submit" className={styles.submitButton}>
        {t('common.action.enter')}
      </button>
    </form>
  );
}
