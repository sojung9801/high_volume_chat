'use client';

import { create, fromBinary, toBinary } from '@bufbuild/protobuf';
import { useCallback, useEffect, useRef, useState } from 'react';

import { ClientFrameSchema, ErrorCode, ServerFrameSchema } from '@/gen/chat/v1/chat_pb';

import { requestRoomJoinToken } from '../api/requestRoomJoinToken';
import { requestRoomMessageHistory } from '../api/requestRoomMessageHistory';
import {
  CHAT_INITIAL_FIRST_ITEM_INDEX,
  CHAT_MAX_RENDERED_MESSAGES,
  CHAT_NOTICE_DURATION_MS,
} from '../chat.constants';
import type {
  ChatConnectionStatus,
  ChatMessageItem,
  ChatMessageListState,
  ChatRoomConnection,
  ChatRoomConnectionParams,
  ChatServerNotice,
} from '../chat.types';
import {
  appendNewerMessages,
  appendWithCap,
  buildGatewaySocketUrl,
  formatHistoryCursor,
  getReconnectDelayMs,
  historyToChatMessageItem,
  loadRoomOwnerKey,
  keepNewerThan,
  prependOlderMessages,
  toChatMessageItem,
} from '../chat.utils';

const EMPTY_MESSAGE_LIST: ChatMessageListState = {
  items: [],
  firstItemIndex: CHAT_INITIAL_FIRST_ITEM_INDEX,
};

/**
 * 방 WebSocket 연결과 렌더링 속도 제어를 맡는다.
 *
 * 수신 메시지는 state가 아니라 ref 버퍼에 쌓고, 프레임당 최대 한 번만
 * state로 옮긴다. 메시지가 몰려도 렌더 횟수는 화면 주사율을 넘지 않는다.
 *
 * 접속할 때마다 저장된 최근 메시지를 먼저 받아 실시간 수신분 앞에 깐다.
 * 처음 입장이면 직전 대화가 보이고, 재접속이면 끊긴 사이가 메워진다.
 */
export function useChatRoomConnection({
  roomId,
  nickname,
  isPaused,
}: ChatRoomConnectionParams): ChatRoomConnection {
  const [list, setList] = useState<ChatMessageListState>(EMPTY_MESSAGE_LIST);
  /**
   * 목록의 최신 값. 화면 상태(`list`)는 다음 렌더에야 바뀌지만 이 값은 바로 바뀐다.
   * 이전 메시지를 요청할 때 "지금 가진 가장 오래된 글"을 여기서 읽는다. 렌더를 기다리는 값을
   * 읽으면 빠르게 스크롤할 때 방금 받은 페이지를 한 번 더 요청하게 되고, 그 결과는 전부
   * 이미 가진 것이라 목록이 안 바뀌어서 스크롤이 맨 위에 멈춘 채 더 불러오지 않는다.
   */
  const listRef = useRef<ChatMessageListState>(EMPTY_MESSAGE_LIST);

  const updateList = useCallback(
    (update: (current: ChatMessageListState) => ChatMessageListState) => {
      const next = update(listRef.current);
      if (next !== listRef.current) {
        listRef.current = next;
        setList(next);
      }
    },
    [],
  );
  const [pendingCount, setPendingCount] = useState(0);
  const [isHistoryReady, setIsHistoryReady] = useState(false);
  const [isOwner, setIsOwner] = useState(false);
  const [status, setStatus] = useState<ChatConnectionStatus>('connecting');
  const [notice, setNotice] = useState<ChatServerNotice | null>(null);

  const socketRef = useRef<WebSocket | null>(null);
  const bufferRef = useRef<ChatMessageItem[]>([]);
  const frameRequestRef = useRef<number | null>(null);
  const isPausedRef = useRef(isPaused);
  /** 이번 접속의 저장 메시지를 받았는지. 받기 전에는 화면에 붙이지 않는다. */
  const isHistorySettledRef = useRef(false);
  const olderRequestRef = useRef<AbortController | null>(null);

  const flushBufferedMessages = useCallback(() => {
    frameRequestRef.current = null;
    if (!isHistorySettledRef.current) {
      return;
    }
    // 저장 메시지가 목록에 들어가는 것과 같은 렌더에서 목록을 띄운다.
    setIsHistoryReady(true);

    const buffered = bufferRef.current;
    if (isPausedRef.current) {
      setPendingCount(buffered.length);
      return;
    }

    setPendingCount(0);
    if (buffered.length === 0) {
      return;
    }
    bufferRef.current = [];
    updateList((current) => appendNewerMessages(current, buffered, CHAT_MAX_RENDERED_MESSAGES));
  }, [updateList]);

  const scheduleFlush = useCallback(() => {
    if (frameRequestRef.current === null) {
      frameRequestRef.current = requestAnimationFrame(flushBufferedMessages);
    }
  }, [flushBufferedMessages]);

  useEffect(() => {
    isPausedRef.current = isPaused;
    if (!isPaused) {
      scheduleFlush();
    }
  }, [isPaused, scheduleFlush]);

  useEffect(() => {
    const abortController = new AbortController();
    let reconnectAttempt = 0;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    /** 방이 없거나 닫혔다. 한번 true가 되면 다시 접속하지 않는다. */
    let isRoomGone = false;

    const markRoomUnavailable = () => {
      isRoomGone = true;
      setStatus('roomUnavailable');
    };

    const scheduleReconnect = () => {
      if (abortController.signal.aborted || isRoomGone) {
        return;
      }
      setStatus('reconnecting');
      reconnectTimer = setTimeout(connect, getReconnectDelayMs(reconnectAttempt));
      reconnectAttempt += 1;
    };

    // 소켓이 열린 뒤에 받는다. 그래야 저장분과 실시간 수신분 사이에 빈틈이 없다 (겹치는 것은 걸러낸다).
    const loadRecentHistory = async () => {
      try {
        const history = await requestRoomMessageHistory(roomId, null, abortController.signal);
        const stored = history.messages.map(historyToChatMessageItem);
        bufferRef.current = appendWithCap(
          stored,
          keepNewerThan(bufferRef.current, stored.at(-1)),
          CHAT_MAX_RENDERED_MESSAGES,
        );
      } catch {
        // 저장소를 못 읽어도 실시간 채팅은 그대로 보여준다.
      }
      if (abortController.signal.aborted) {
        return;
      }
      isHistorySettledRef.current = true;
      scheduleFlush();
    };

    const connect = async () => {
      isHistorySettledRef.current = false;
      let socket: WebSocket;
      try {
        // 토큰 수명이 짧아서 접속할 때마다 새로 받는다.
        const result = await requestRoomJoinToken(
          roomId,
          nickname,
          loadRoomOwnerKey(roomId),
          abortController.signal,
        );
        if (result.status === 'refused') {
          // 없는 방, 닫힌 방, 받아들여지지 않는 요청이다. 재접속을 예약하지 않고 여기서 끝낸다.
          markRoomUnavailable();
          return;
        }
        setIsOwner(result.joinToken.isOwner);
        const { gatewayWsUrl, token } = result.joinToken;
        socket = new WebSocket(buildGatewaySocketUrl(gatewayWsUrl, token));
      } catch {
        scheduleReconnect();
        return;
      }

      socket.binaryType = 'arraybuffer';
      socketRef.current = socket;

      socket.onopen = () => {
        reconnectAttempt = 0;
        setStatus('open');
        void loadRecentHistory();
      };

      socket.onmessage = (event: MessageEvent<ArrayBuffer>) => {
        const frame = fromBinary(ServerFrameSchema, new Uint8Array(event.data));

        if (frame.payload.case === 'batch') {
          // 탭이 백그라운드라 flush가 멈춰도 버퍼가 끝없이 자라지 않게 여기서도 상한을 건다.
          bufferRef.current = appendWithCap(
            bufferRef.current,
            frame.payload.value.messages.map(toChatMessageItem),
            CHAT_MAX_RENDERED_MESSAGES,
          );
          scheduleFlush();
        } else if (frame.payload.case === 'roomClosed') {
          // 방장이 방을 닫았다. 곧 서버가 연결을 끊는데, 그때 재접속하지 않게 표시해 둔다.
          markRoomUnavailable();
        } else if (frame.payload.case === 'error') {
          setNotice(
            frame.payload.value.code === ErrorCode.RATE_LIMITED ? 'rateLimited' : 'invalidMessage',
          );
        }
      };

      socket.onclose = () => {
        if (socketRef.current === socket) {
          socketRef.current = null;
        }
        scheduleReconnect();
      };
    };

    void connect();

    return () => {
      abortController.abort();
      if (reconnectTimer !== null) {
        clearTimeout(reconnectTimer);
      }
      socketRef.current?.close();
      socketRef.current = null;
      olderRequestRef.current?.abort();
      olderRequestRef.current = null;
      if (frameRequestRef.current !== null) {
        cancelAnimationFrame(frameRequestRef.current);
        frameRequestRef.current = null;
      }
    };
  }, [roomId, nickname, scheduleFlush]);

  useEffect(() => {
    if (notice === null) {
      return;
    }
    const timer = setTimeout(() => setNotice(null), CHAT_NOTICE_DURATION_MS);
    return () => clearTimeout(timer);
  }, [notice]);

  const sendMessage = useCallback((text: string): boolean => {
    const socket = socketRef.current;
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      return false;
    }

    setNotice(null);
    socket.send(
      toBinary(
        ClientFrameSchema,
        create(ClientFrameSchema, { payload: { case: 'sendMessage', value: { text } } }),
      ),
    );
    return true;
  }, []);

  const loadOlderMessages = useCallback(() => {
    const oldest = listRef.current.items[0];
    if (oldest === undefined || olderRequestRef.current !== null) {
      return;
    }

    const request = new AbortController();
    olderRequestRef.current = request;
    requestRoomMessageHistory(roomId, formatHistoryCursor(oldest), request.signal)
      .then((history) => {
        const older = history.messages.map(historyToChatMessageItem);
        updateList((current) => prependOlderMessages(current, older));
      })
      .catch(() => {
        // 실패하면 그대로 둔다. 다시 맨 위로 올리면 재시도된다.
      })
      .finally(() => {
        if (olderRequestRef.current === request) {
          olderRequestRef.current = null;
        }
      });
  }, [roomId, updateList]);

  return {
    messages: list.items,
    firstItemIndex: list.firstItemIndex,
    isHistoryReady,
    loadOlderMessages,
    pendingCount,
    status,
    isOwner,
    notice,
    sendMessage,
  };
}
