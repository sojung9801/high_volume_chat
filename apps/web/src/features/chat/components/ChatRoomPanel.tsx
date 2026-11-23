'use client';

import { useCallback, useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useTranslations } from 'next-intl';

import { requestRoomClose } from '../api/requestRoomClose';
import { requestRoomMessageExport } from '../api/requestRoomMessageExport';
import {
  CHAT_NOTICE_MESSAGE_KEYS,
  CHAT_STATUS_MESSAGE_KEYS,
} from '../chat.constants';
import { leaveLobbyNotice, loadRoomOwnerKey, saveBlobAsFile } from '../chat.utils';
import { useChatRoomConnection } from '../hooks/useChatRoomConnection';
import { ChatMessageInput } from './ChatMessageInput';
import { ChatMessageList } from './ChatMessageList';
import styles from './ChatRoom.module.css';

type ChatRoomPanelProps = {
  roomId: string;
  nickname: string;
};

export function ChatRoomPanel({ roomId, nickname }: ChatRoomPanelProps) {
  const t = useTranslations();
  const router = useRouter();
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [isOwnerRequestPending, setIsOwnerRequestPending] = useState(false);
  const [hasOwnerRequestFailed, setHasOwnerRequestFailed] = useState(false);
  const {
    messages,
    firstItemIndex,
    isHistoryReady,
    loadOlderMessages,
    pendingCount,
    status,
    isOwner,
    notice,
    sendMessage,
  } = useChatRoomConnection({ roomId, nickname, isPaused: !isAtBottom });

  // 페이지를 떠나면 훅 정리 단계에서 소켓이 닫히고, 게이트웨이가 방에서 이 연결을 뺀다.
  const handleLeave = useCallback(() => {
    router.push('/');
  }, [router]);

  /** 방장 전용 요청(닫기, 내역 받기)을 보낸다. 열쇠를 꺼내 넘기고 실패를 표시한다. */
  const runOwnerRequest = useCallback(
    async (request: (ownerKey: string) => Promise<void>) => {
      const ownerKey = loadRoomOwnerKey(roomId);
      if (ownerKey === null) {
        return;
      }
      setHasOwnerRequestFailed(false);
      setIsOwnerRequestPending(true);
      try {
        await request(ownerKey);
      } catch {
        setHasOwnerRequestFailed(true);
      } finally {
        setIsOwnerRequestPending(false);
      }
    },
    [roomId],
  );

  const handleDownloadHistory = useCallback(() => {
    void runOwnerRequest(async (ownerKey) => {
      saveBlobAsFile(await requestRoomMessageExport(roomId, ownerKey), `chat-${roomId}.csv`);
    });
  }, [roomId, runOwnerRequest]);

  // 방장이 나가는 것은 곧 방을 닫는 것이다. 닫히면 서버가 모두에게 알리고, 이 화면도 그 알림을 받아 첫 화면으로 간다.
  const handleCloseRoom = useCallback(() => {
    void runOwnerRequest((ownerKey) => requestRoomClose(roomId, ownerKey));
  }, [roomId, runOwnerRequest]);

  // 없는 방이거나 닫힌 방이면 첫 화면으로 돌려보낸다. 뒤로 가기로 다시 오지 않게 기록을 바꿔치운다.
  const isRoomUnavailable = status === 'roomUnavailable';
  useEffect(() => {
    if (isRoomUnavailable) {
      leaveLobbyNotice('roomUnavailable');
      router.replace('/');
    }
  }, [isRoomUnavailable, router]);

  if (isRoomUnavailable) {
    return null;
  }

  return (
    <main className={styles.panel}>
      <header className={styles.header}>
        <span className={styles.roomTitle}>{roomId}</span>
        <div className={styles.headerActions}>
          {status !== 'open' && (
            <span className={styles.status}>{t(CHAT_STATUS_MESSAGE_KEYS[status])}</span>
          )}
          {isOwner ? (
            <>
              <button
                type="button"
                className={styles.leaveButton}
                disabled={isOwnerRequestPending}
                onClick={handleDownloadHistory}
              >
                {t('messages.chat.download_history')}
              </button>
              <button
                type="button"
                className={styles.leaveButton}
                disabled={isOwnerRequestPending}
                onClick={handleCloseRoom}
              >
                {t('messages.chat.close_room')}
              </button>
            </>
          ) : (
            <button type="button" className={styles.leaveButton} onClick={handleLeave}>
              {t('common.action.leave')}
            </button>
          )}
        </div>
      </header>
      <ChatMessageList
        messages={messages}
        firstItemIndex={firstItemIndex}
        isReady={isHistoryReady}
        onLoadOlder={loadOlderMessages}
        pendingCount={pendingCount}
        isAtBottom={isAtBottom}
        onAtBottomChange={setIsAtBottom}
      />
      <div className={styles.footer}>
        {notice ? (
          <p className={styles.notice} role="status">
            {t(CHAT_NOTICE_MESSAGE_KEYS[notice])}
          </p>
        ) : (
          hasOwnerRequestFailed && (
            <p className={styles.notice} role="alert">
              {t('messages.chat.request_failed')}
            </p>
          )
        )}
        <ChatMessageInput isDisabled={status !== 'open'} onSend={sendMessage} />
      </div>
    </main>
  );
}
