/** 화면(메모리)에 유지하는 최대 메시지 수. 넘으면 오래된 것부터 버린다. */
export const CHAT_MAX_RENDERED_MESSAGES = 300;

/** 서버 SESSION_MAX_TEXT_CHARS와 맞춘다. */
export const CHAT_MAX_TEXT_LENGTH = 300;

/** apps/api NICKNAME_MAX_LENGTH와 맞춘다. */
export const CHAT_NICKNAME_MAX_LENGTH = 20;

export const CHAT_ROOM_ID_PATTERN = '[A-Za-z0-9_\\-]{1,64}';

export const CHAT_RECONNECT_BASE_DELAY_MS = 500;
export const CHAT_RECONNECT_MAX_DELAY_MS = 15_000;

/** 바닥에서 이 픽셀 이내면 "바닥에 있음"으로 본다. */
export const CHAT_AT_BOTTOM_THRESHOLD_PX = 48;

/** 서버 안내 문구(rate limit 등)를 띄워 두는 시간. */
export const CHAT_NOTICE_DURATION_MS = 3000;

/** 앞에 과거 메시지를 계속 붙여도 0 아래로 내려가지 않을 만큼 큰 시작 번호. */
export const CHAT_INITIAL_FIRST_ITEM_INDEX = 1_000_000_000;

export const CHAT_STATUS_MESSAGE_KEYS = {
  connecting: 'messages.chat.status_connecting',
  reconnecting: 'messages.chat.status_reconnecting',
} as const;

export const CHAT_NOTICE_MESSAGE_KEYS = {
  rateLimited: 'messages.chat.rate_limited',
  invalidMessage: 'messages.chat.invalid_message',
} as const;

export const CHAT_ENTRY_NOTICE_MESSAGE_KEYS = {
  roomAlreadyExists: 'messages.chat.room_already_exists',
  createRoomFailed: 'messages.chat.create_room_failed',
  roomNotFound: 'messages.chat.room_not_found',
  roomUnavailable: 'messages.chat.room_unavailable',
} as const;

/** 방장 열쇠를 브라우저에 보관할 때 쓰는 이름의 앞부분. 뒤에 방 ID가 붙는다. */
export const CHAT_OWNER_KEY_STORAGE_PREFIX = 'chat.ownerKey.';

/** 진행 중인 방 목록을 다시 불러오는 간격. */
export const CHAT_OPEN_ROOMS_REFRESH_MS = 5000;

/** 랜덤 닉네임 재료. "형용사 + 동물 + 숫자"로 조합한다 (예: 용감한수달482). */
export const CHAT_NICKNAME_ADJECTIVES = [
  '용감한', '졸린', '신난', '느긋한', '배고픈', '수줍은', '씩씩한', '엉뚱한', '다정한', '날쌘',
] as const;

export const CHAT_NICKNAME_ANIMALS = [
  '수달', '고양이', '너구리', '펭귄', '다람쥐', '여우', '판다', '고래', '부엉이', '토끼',
] as const;

/** 닉네임 끝에 붙는 숫자의 범위 (0 이상 이 값 미만). 같은 조합이 겹칠 확률을 낮춘다. */
export const CHAT_NICKNAME_NUMBER_RANGE = 1000;

export const HTTP_STATUS_NOT_FOUND = 404;
export const HTTP_STATUS_CONFLICT = 409;

/**
 * 첫 화면에 한 번만 보여줄 안내를 잠깐 맡겨 두는 이름.
 * 방에서 내보내질 때 적어 두고, 첫 화면이 읽자마자 지운다. 주소에 싣지 않으므로 새로고침하면 사라진다.
 */
export const CHAT_LOBBY_NOTICE_STORAGE_KEY = 'chat.lobbyNotice';

/** 닉네임을 브라우저에 보관할 때 쓰는 이름. 첫 화면으로 돌아와도 같은 닉네임을 쓰기 위해서다. */
export const CHAT_NICKNAME_STORAGE_KEY = 'chat.nickname';

/** 맨 위에서 이 픽셀 이내면 "맨 위에 있음"으로 보고 이전 메시지를 요청한다. */
export const CHAT_AT_TOP_THRESHOLD_PX = 1;

/** 이전 메시지가 붙은 뒤, 여전히 맨 위인지 다시 재기까지 기다리는 시간. */
export const CHAT_LOAD_OLDER_RECHECK_MS = 150;
