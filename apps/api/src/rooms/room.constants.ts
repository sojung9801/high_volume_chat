/** 게이트웨이(gateway/src/auth.rs)의 JOIN_TOKEN_AUDIENCE와 같아야 한다. */
export const JOIN_TOKEN_AUDIENCE = 'chat-gateway';

/** 토큰이 URL 쿼리에 실리므로 수명을 짧게 둔다. 접속 직전에 발급받아 바로 쓴다. */
export const JOIN_TOKEN_TTL_SEC = 60;

export const ROOM_ID_PATTERN = /^[A-Za-z0-9_-]{1,64}$/;

export const NICKNAME_MAX_LENGTH = 20;

export const GATEWAY_REQUEST_TIMEOUT_MS = 2000;

/** 방장 열쇠 길이. 추측할 수 없을 만큼 길게 둔다. */
export const OWNER_KEY_BYTES = 32;

/** 요청으로 받는 방장 열쇠의 최대 길이. 정상 열쇠(43자)보다 넉넉하게 둔다. */
export const OWNER_KEY_MAX_LENGTH = 128;

/** 방 목록에 한 번에 내려주는 최대 개수. */
export const OPEN_ROOM_LIST_LIMIT = 50;

/** 유효기간이 지난 방을 찾아 닫는 주기. */
export const ROOM_EXPIRY_SWEEP_INTERVAL_MS = 60_000;
