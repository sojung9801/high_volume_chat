export type RoomJoinTokenClaims = {
  userId: string;
  roomId: string;
  nickname: string;
  isOwner: boolean;
};

export type RoomJoinTokenResponse = {
  token: string;
  gatewayWsUrl: string;
  expiresInSec: number;
  isOwner: boolean;
};

export type RoomStats = {
  roomId: string;
  connections: number;
  lastSeq: number;
  sampledOutMessages: number;
  laggedFrames: number;
};

export type RoomCreateResponse = {
  roomId: string;
  /** 방장 열쇠. 이 응답에서 딱 한 번만 내려준다. 서버에는 해시만 남는다. */
  ownerKey: string;
};

export type OpenRoom = {
  roomId: string;
  /** ISO 8601 시각. */
  createdAt: string;
};

export type OpenRoomListResponse = {
  rooms: OpenRoom[];
};

export type OpenRoomRow = {
  room_id: string;
  created_at: Date;
};
