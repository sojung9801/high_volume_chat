-- 게이트웨이가 시청자에게 내보낸 메시지만 저장한다. 샘플링으로 버려진 것은 들어오지 않는다.
--
-- seq는 방이 살아 있는 동안만 이어지는 번호라, 방이 비었다가 다시 생기면 1부터 다시 센다.
-- 그래서 정렬과 식별은 (sent_at_ms, seq) 쌍으로 한다.
CREATE TABLE IF NOT EXISTS chat_messages (
    room_id    text   NOT NULL,
    sent_at_ms bigint NOT NULL,
    seq        bigint NOT NULL,
    user_id    text   NOT NULL,
    nickname   text   NOT NULL,
    body       text   NOT NULL,
    is_owner   boolean NOT NULL DEFAULT false,
    PRIMARY KEY (room_id, sent_at_ms, seq)
);

-- 이미 만들어진 DB에 뒤늦게 추가된 칸.
ALTER TABLE chat_messages ADD COLUMN IF NOT EXISTS is_owner boolean NOT NULL DEFAULT false;

-- 사람이 조회할 때 쓰는 뷰. 시각을 읽을 수 있는 형태로 보여준다.
CREATE OR REPLACE VIEW chat_messages_readable AS
SELECT
    room_id,
    to_timestamp(sent_at_ms / 1000.0) AS sent_at,
    seq,
    nickname,
    body,
    user_id,
    is_owner
FROM chat_messages;

-- 개설된 방. room_id가 기본 키라서 같은 ID로는 두 번 만들 수 없다 (중복 확인을 DB가 한다).
CREATE TABLE IF NOT EXISTS rooms (
    room_id        text        PRIMARY KEY,
    -- 방장만 아는 비밀 키의 해시. 원문은 저장하지 않는다.
    owner_key_hash text        NOT NULL,
    created_at     timestamptz NOT NULL DEFAULT now(),
    -- 방장이 방을 닫은 시각. 열려 있는 동안은 NULL.
    closed_at      timestamptz
);
