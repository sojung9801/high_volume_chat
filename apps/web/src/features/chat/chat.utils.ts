import type { ChatMessage } from '@/gen/chat/v1/chat_pb';

import {
  CHAT_LOBBY_NOTICE_STORAGE_KEY,
  CHAT_NICKNAME_ADJECTIVES,
  CHAT_NICKNAME_ANIMALS,
  CHAT_NICKNAME_NUMBER_RANGE,
  CHAT_NICKNAME_STORAGE_KEY,
  CHAT_OWNER_KEY_STORAGE_PREFIX,
  CHAT_RECONNECT_BASE_DELAY_MS,
  CHAT_RECONNECT_MAX_DELAY_MS,
} from './chat.constants';
import type {
  ChatHistoryMessage,
  ChatMessageItem,
  ChatMessageListState,
  ChatRoomEntryNotice,
} from './chat.types';

function buildMessageKey(sentAtMs: number, seq: number): string {
  return `${sentAtMs}-${seq}`;
}

export function toChatMessageItem(message: ChatMessage): ChatMessageItem {
  const seq = Number(message.seq);
  const sentAtMs = Number(message.sentAtMs);
  return {
    key: buildMessageKey(sentAtMs, seq),
    seq,
    userId: message.userId,
    nickname: message.nickname,
    text: message.text,
    sentAtMs,
    isOwner: message.isOwner,
  };
}

export function historyToChatMessageItem(message: ChatHistoryMessage): ChatMessageItem {
  return { ...message, key: buildMessageKey(message.sentAtMs, message.seq) };
}

/** 관리 API의 `before` 커서. 이 메시지보다 오래된 것을 요청한다. */
export function formatHistoryCursor(message: ChatMessageItem): string {
  return `${message.sentAtMs}_${message.seq}`;
}

/** 서버 저장 순서와 같은 기준 (보낸 시각, seq)로 a가 b보다 나중인지 본다. */
export function isNewerMessage(a: ChatMessageItem, b: ChatMessageItem): boolean {
  return a.sentAtMs !== b.sentAtMs ? a.sentAtMs > b.sentAtMs : a.seq > b.seq;
}

/** `after`보다 나중 것만 남긴다. 실시간 수신분과 저장분이 겹칠 때 중복을 걸러낸다. */
export function keepNewerThan(
  messages: readonly ChatMessageItem[],
  after: ChatMessageItem | undefined,
): ChatMessageItem[] {
  return after === undefined
    ? messages.slice()
    : messages.filter((message) => isNewerMessage(message, after));
}

/** 새 메시지를 뒤에 붙인다. 이미 가진 것은 건너뛰고, 상한을 넘으면 오래된 것부터 버린다. */
export function appendNewerMessages(
  state: ChatMessageListState,
  incoming: readonly ChatMessageItem[],
  cap: number,
): ChatMessageListState {
  const newer = keepNewerThan(incoming, state.items.at(-1));
  if (newer.length === 0) {
    return state;
  }
  return { ...state, items: appendWithCap(state.items, newer, cap) };
}

/** 과거 메시지를 앞에 붙인다. 읽던 위치가 유지되도록 첫 항목 번호를 그만큼 줄인다. */
export function prependOlderMessages(
  state: ChatMessageListState,
  older: readonly ChatMessageItem[],
): ChatMessageListState {
  const first = state.items[0];
  const missing =
    first === undefined ? older.slice() : older.filter((message) => isNewerMessage(first, message));
  if (missing.length === 0) {
    return state;
  }
  return {
    items: missing.concat(state.items),
    firstItemIndex: state.firstItemIndex - missing.length,
  };
}

/** 뒤에 붙이고, 상한을 넘으면 앞(오래된 것)에서 잘라낸다. */
export function appendWithCap<T>(current: readonly T[], incoming: readonly T[], cap: number): T[] {
  const merged = current.concat(incoming);
  return merged.length > cap ? merged.slice(merged.length - cap) : merged;
}

/**
 * 지수 백오프 + 지터. 게이트웨이가 재시작될 때 모든 클라이언트가
 * 같은 순간에 재접속하는 것을 막는다.
 */
export function getReconnectDelayMs(attempt: number): number {
  const exponential = Math.min(
    CHAT_RECONNECT_BASE_DELAY_MS * 2 ** attempt,
    CHAT_RECONNECT_MAX_DELAY_MS,
  );
  return exponential * (0.5 + Math.random() * 0.5);
}

export function buildGatewaySocketUrl(gatewayWsUrl: string, token: string): string {
  const url = new URL(gatewayWsUrl);
  url.searchParams.set('token', token);
  return url.toString();
}

export function getApiBaseUrl(): string {
  const apiBaseUrl = process.env.NEXT_PUBLIC_API_URL;
  if (!apiBaseUrl) {
    throw new Error('NEXT_PUBLIC_API_URL is not set');
  }
  return apiBaseUrl;
}

/**
 * 방장 열쇠를 이 브라우저에 보관한다. 새로고침해도 방장으로 남기 위해서다.
 * 사생활 보호 모드처럼 저장이 막힌 환경이면 `false`를 돌려준다.
 */
export function saveRoomOwnerKey(roomId: string, ownerKey: string): boolean {
  try {
    localStorage.setItem(`${CHAT_OWNER_KEY_STORAGE_PREFIX}${roomId}`, ownerKey);
    return true;
  } catch {
    return false;
  }
}

function pickRandom<T>(items: readonly T[]): T {
  return items[Math.floor(Math.random() * items.length)] as T;
}

/** "형용사 + 동물 + 숫자" 닉네임을 만든다. 가장 긴 조합도 닉네임 길이 제한 안에 들어온다. */
export function generateRandomNickname(): string {
  const number = Math.floor(Math.random() * CHAT_NICKNAME_NUMBER_RANGE);
  return `${pickRandom(CHAT_NICKNAME_ADJECTIVES)}${pickRandom(CHAT_NICKNAME_ANIMALS)}${number}`;
}

/** 이 브라우저가 가진 방장 열쇠. 방장이 아니거나 저장소를 못 읽으면 `null`. */
export function loadRoomOwnerKey(roomId: string): string | null {
  try {
    return localStorage.getItem(`${CHAT_OWNER_KEY_STORAGE_PREFIX}${roomId}`);
  } catch {
    return null;
  }
}

/** 받은 데이터를 파일로 내려받게 한다. */
export function saveBlobAsFile(blob: Blob, fileName: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = fileName;
  link.click();
  URL.revokeObjectURL(url);
}

/** 이 브라우저에서 마지막으로 쓴 닉네임. 없거나 저장소를 못 읽으면 `null`. */
export function loadSavedNickname(): string | null {
  try {
    return localStorage.getItem(CHAT_NICKNAME_STORAGE_KEY);
  } catch {
    return null;
  }
}

export function saveNickname(nickname: string): void {
  try {
    localStorage.setItem(CHAT_NICKNAME_STORAGE_KEY, nickname);
  } catch {
    // 저장이 막힌 환경이면 다음에 새 닉네임을 받는다. 입장에는 지장이 없다.
  }
}

/** 첫 화면에 한 번 보여줄 안내를 맡긴다. 탭을 닫으면 사라지는 저장소(sessionStorage)를 쓴다. */
export function leaveLobbyNotice(notice: ChatRoomEntryNotice): void {
  try {
    sessionStorage.setItem(CHAT_LOBBY_NOTICE_STORAGE_KEY, notice);
  } catch {
    // 저장이 막힌 환경이면 안내 없이 첫 화면으로 간다.
  }
}

/** 맡겨 둔 안내를 꺼내고 지운다. 그래서 딱 한 번만 보인다. */
export function takeLobbyNotice(): ChatRoomEntryNotice | null {
  try {
    const notice = sessionStorage.getItem(CHAT_LOBBY_NOTICE_STORAGE_KEY);
    sessionStorage.removeItem(CHAT_LOBBY_NOTICE_STORAGE_KEY);
    return notice === 'roomUnavailable' ? notice : null;
  } catch {
    return null;
  }
}
