export type RoomMessage = {
  seq: number;
  userId: string;
  nickname: string;
  text: string;
  sentAtMs: number;
  isOwner: boolean;
};

export type RoomMessageCursor = {
  sentAtMs: number;
  seq: number;
};

export type RoomMessageHistoryResponse = {
  /** 오래된 것부터 최신 순. */
  messages: RoomMessage[];
  /** 더 오래된 페이지를 요청할 때 `before`로 넘길 값. 더 없으면 null. */
  olderCursor: string | null;
};

/** bigint 컬럼은 문자열로 온다. */
export type RoomMessageRow = {
  sent_at_ms: string;
  seq: string;
  user_id: string;
  nickname: string;
  body: string;
  is_owner: boolean;
};
