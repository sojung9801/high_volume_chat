import { z } from 'zod';

import {
  NICKNAME_WITHOUT_CONTROL_CHARS_PATTERN,
  NICKNAME_MAX_LENGTH,
  OWNER_KEY_MAX_LENGTH,
  ROOM_ID_PATTERN,
} from './room.constants.ts';

export const roomIdSchema = z.string().regex(ROOM_ID_PATTERN);

export const roomJoinRequestSchema = z.object({
  // 제어 문자(NUL 등)는 받지 않는다. 화면에 안 보이고 DB에 저장할 수 없는 것도 있다.
  nickname: z
    .string()
    .trim()
    .min(1)
    .max(NICKNAME_MAX_LENGTH)
    .regex(NICKNAME_WITHOUT_CONTROL_CHARS_PATTERN),
  /** 방장만 보낸다. 맞으면 방장으로, 없거나 틀리면 일반 참여자로 입장한다. */
  ownerKey: z.string().min(1).max(OWNER_KEY_MAX_LENGTH).optional(),
});

/** 방장만 할 수 있는 요청(방 닫기, 내역 받기)의 본문. */
export const roomOwnerRequestSchema = z.object({
  ownerKey: z.string().min(1).max(OWNER_KEY_MAX_LENGTH),
});

export const roomStatsListSchema = z.array(
  z.object({
    roomId: z.string(),
    connections: z.number(),
    lastSeq: z.number(),
    sampledOutMessages: z.number(),
    laggedFrames: z.number(),
  }),
);

export const roomCreateRequestSchema = z.object({
  roomId: roomIdSchema,
});
