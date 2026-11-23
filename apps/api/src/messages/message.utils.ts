import {
  CSV_FORMULA_PREFIX_PATTERN,
  CSV_NEEDS_QUOTES_PATTERN,
  MESSAGE_CURSOR_PATTERN,
  MESSAGE_CURSOR_SEPARATOR,
} from './message.constants.ts';
import type { RoomMessage, RoomMessageCursor, RoomMessageRow } from './message.types.ts';

export function parseMessageCursor(raw: string): RoomMessageCursor | null {
  const match = MESSAGE_CURSOR_PATTERN.exec(raw);
  const sentAtMs = match?.[1];
  const seq = match?.[2];
  if (sentAtMs === undefined || seq === undefined) {
    return null;
  }
  return { sentAtMs: Number(sentAtMs), seq: Number(seq) };
}

export function formatMessageCursor(message: RoomMessage): string {
  return `${message.sentAtMs}${MESSAGE_CURSOR_SEPARATOR}${message.seq}`;
}

export function toRoomMessage(row: RoomMessageRow): RoomMessage {
  return {
    seq: Number(row.seq),
    userId: row.user_id,
    nickname: row.nickname,
    text: row.body,
    sentAtMs: Number(row.sent_at_ms),
    isOwner: row.is_owner,
  };
}

/**
 * CSV 칸 하나로 바꾼다.
 * - 쉼표, 따옴표, 줄바꿈이 있으면 따옴표로 감싼다.
 * - `=`, `+`, `-`, `@`로 시작하면 앞에 작은따옴표를 붙인다. 채팅은 남이 쓴 글이라
 *   그대로 두면 스프레드시트에서 수식으로 실행될 수 있다.
 */
export function toCsvCell(value: string): string {
  const safe = CSV_FORMULA_PREFIX_PATTERN.test(value) ? `'${value}` : value;
  return CSV_NEEDS_QUOTES_PATTERN.test(safe) ? `"${safe.replaceAll('"', '""')}"` : safe;
}

export function toCsvLine(message: RoomMessage): string {
  return [
    new Date(message.sentAtMs).toISOString(),
    toCsvCell(message.nickname),
    String(message.isOwner),
    toCsvCell(message.text),
  ].join(',');
}
