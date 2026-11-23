'use client';

import { useCallback, useEffect, useRef } from 'react';
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso';

import {
  CHAT_AT_BOTTOM_THRESHOLD_PX,
  CHAT_AT_TOP_THRESHOLD_PX,
  CHAT_LOAD_OLDER_RECHECK_MS,
} from '../chat.constants';
import type { ChatMessageItem } from '../chat.types';
import { ChatMessageRow } from './ChatMessageRow';
import { ChatNewMessagesButton } from './ChatNewMessagesButton';
import { ChatScrollToBottomButton } from './ChatScrollToBottomButton';
import styles from './ChatRoom.module.css';

type ChatMessageListProps = {
  messages: ChatMessageItem[];
  firstItemIndex: number;
  /** false인 동안은 목록을 띄우지 않는다. 처음 띄울 때 맨 아래에서 시작하기 위해서다. */
  isReady: boolean;
  pendingCount: number;
  /** 맨 아래에 있는지. 아니면 아래로 내려가는 버튼을 보여준다. */
  isAtBottom: boolean;
  onLoadOlder: () => void;
  onAtBottomChange: (isAtBottom: boolean) => void;
};

function computeMessageKey(_index: number, message: ChatMessageItem) {
  return message.key;
}

function renderMessageRow(_index: number, message: ChatMessageItem) {
  return <ChatMessageRow message={message} />;
}

/** 처음 띄울 때 가장 최근 메시지가 바닥에 오게 한다. */
const INITIAL_TOP_MOST_ITEM = { index: 'LAST', align: 'end' } as const;

/** 바닥에 있을 때만 새 메시지를 따라 내려간다. */
function followOutputAtBottom(isAtBottom: boolean) {
  return isAtBottom ? 'auto' : false;
}

export function ChatMessageList({
  messages,
  firstItemIndex,
  isReady,
  pendingCount,
  isAtBottom,
  onLoadOlder,
  onAtBottomChange,
}: ChatMessageListProps) {
  const virtuosoRef = useRef<VirtuosoHandle>(null);

  const scrollerRef = useRef<HTMLElement | null>(null);
  const setScroller = useCallback((element: HTMLElement | Window | null) => {
    scrollerRef.current = element instanceof HTMLElement ? element : null;
  }, []);

  // 실제 스크롤 위치가 맨 위일 때만 이전 메시지를 요청한다.
  // 목록 라이브러리가 알려주는 "맨 위" 신호는 쓰지 않는다. 그 신호는 상태가 바뀌는 순간에만 울려서,
  // 한 번 빈 결과를 받거나 불러온 뒤에도 맨 위에 머물러 있으면 다시 울리지 않는다.
  const loadOlderIfAtTop = useCallback(() => {
    const scroller = scrollerRef.current;
    if (scroller !== null && scroller.scrollTop <= CHAT_AT_TOP_THRESHOLD_PX) {
      onLoadOlder();
    }
  }, [onLoadOlder]);

  // 맨 앞 메시지가 바뀌었다 = 이전 메시지가 붙었거나 목록이 새로 채워졌다.
  // 그 뒤에도 맨 위에 있으면 (빠르게 올리는 중이거나 화면을 못 채웠을 때) 이어서 요청한다.
  // 목록이 자리를 잡을 시간을 조금 준 뒤에 위치를 잰다. 더 불러올 것이 없으면
  // 맨 앞 메시지가 안 바뀌므로 자연히 멈춘다.
  const oldestMessageKey = messages[0]?.key;
  useEffect(() => {
    if (oldestMessageKey === undefined) {
      return;
    }
    const timer = setTimeout(loadOlderIfAtTop, CHAT_LOAD_OLDER_RECHECK_MS);
    return () => clearTimeout(timer);
  }, [oldestMessageKey, loadOlderIfAtTop]);

  const scrollToLatest = useCallback(() => {
    virtuosoRef.current?.scrollToIndex({ index: 'LAST', align: 'end' });
  }, []);

  return (
    <div className={styles.listWrap}>
      {isReady && (
        <Virtuoso
          ref={virtuosoRef}
          className={styles.list}
          data={messages}
          initialTopMostItemIndex={INITIAL_TOP_MOST_ITEM}
          firstItemIndex={firstItemIndex}
          scrollerRef={setScroller}
          onScroll={loadOlderIfAtTop}
          computeItemKey={computeMessageKey}
          itemContent={renderMessageRow}
          followOutput={followOutputAtBottom}
          atBottomStateChange={onAtBottomChange}
          atBottomThreshold={CHAT_AT_BOTTOM_THRESHOLD_PX}
        />
      )}
      {/* 위로 올려 둔 동안: 새 글이 쌓였으면 개수를, 아니면 그냥 내려가는 버튼을 보여준다. */}
      {!isAtBottom &&
        (pendingCount > 0 ? (
          <ChatNewMessagesButton count={pendingCount} onClick={scrollToLatest} />
        ) : (
          <ChatScrollToBottomButton onClick={scrollToLatest} />
        ))}
    </div>
  );
}
