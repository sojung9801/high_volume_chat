export type ChatMessageItem = {
  /** `sentAtMs-seq`. 리스트 key로 쓴다. */
  key: string;
  /** 방이 살아 있는 동안만 이어지는 번호. 방이 다시 생기면 1부터 다시 센다. */
  seq: number;
  userId: string;
  nickname: string;
  text: string;
  sentAtMs: number;
  /** 방장이 보낸 메시지인지. */
  isOwner: boolean;
};

/** `roomUnavailable`: 방이 없거나 닫혔다. 다시 시도하지 않는다. */
export type ChatConnectionStatus = 'connecting' | 'open' | 'reconnecting' | 'roomUnavailable';

export type ChatServerNotice = 'rateLimited' | 'invalidMessage';

export type ChatRoomJoinToken = {
  token: string;
  gatewayWsUrl: string;
  expiresInSec: number;
  /** 보낸 방장 열쇠가 맞았는지. 서버가 판단한 값이다. */
  isOwner: boolean;
};

/** 입장 토큰 요청 결과. 방이 없거나 닫혔으면 토큰 대신 그 사실을 받는다. */
export type ChatRoomJoinTokenResult =
  | { status: 'issued'; joinToken: ChatRoomJoinToken }
  | { status: 'roomNotFound' };

export type ChatRoomConnectionParams = {
  roomId: string;
  nickname: string;
  /** true면 새 메시지를 화면에 붙이지 않고 쌓아만 둔다 (사용자가 위로 스크롤한 상태). */
  isPaused: boolean;
};

/** 관리 API가 돌려주는 저장된 메시지. */
export type ChatHistoryMessage = {
  seq: number;
  userId: string;
  nickname: string;
  text: string;
  sentAtMs: number;
  isOwner: boolean;
};

export type ChatRoomMessageHistory = {
  /** 오래된 것부터 최신 순. */
  messages: ChatHistoryMessage[];
  olderCursor: string | null;
};

export type ChatMessageListState = {
  items: ChatMessageItem[];
  /** 가상화 리스트의 첫 항목 번호. 앞에 과거 메시지를 붙일 때마다 그만큼 줄인다. */
  firstItemIndex: number;
};

export type ChatRoomConnection = {
  messages: ChatMessageItem[];
  firstItemIndex: number;
  /** 첫 접속의 저장 메시지까지 반영됐는지. 그 전에는 목록을 그리지 않는다. */
  isHistoryReady: boolean;
  /** 지금 가진 것보다 오래된 메시지를 한 페이지 불러와 앞에 붙인다. */
  loadOlderMessages: () => void;
  /** 일시정지 중 쌓인 미표시 메시지 수. */
  pendingCount: number;
  status: ChatConnectionStatus;
  /** 이 접속이 방장으로 들어왔는지. */
  isOwner: boolean;
  notice: ChatServerNotice | null;
  sendMessage: (text: string) => boolean;
};

/** 방 만들기 결과. 성공하면 방장 열쇠를 받는다. */
export type ChatRoomCreationResult =
  | { status: 'created'; ownerKey: string }
  | { status: 'alreadyExists' };

export type ChatRoomEntryNotice =
  | 'roomAlreadyExists'
  | 'createRoomFailed'
  | 'roomNotFound'
  | 'roomUnavailable';

export type ChatOpenRoom = {
  roomId: string;
  /** ISO 8601 시각. */
  createdAt: string;
};
