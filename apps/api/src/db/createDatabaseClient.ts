import postgres from 'postgres';

import { DATABASE_CONNECT_TIMEOUT_SEC, DATABASE_MAX_CONNECTIONS } from './database.constants.ts';
import type { DatabaseClient } from './database.types.ts';

/** 연결은 첫 쿼리 때 맺는다. DB가 내려가 있어도 API는 뜨고 토큰 발급은 계속된다. */
export function createDatabaseClient(databaseUrl: string): DatabaseClient {
  return postgres(databaseUrl, {
    max: DATABASE_MAX_CONNECTIONS,
    connect_timeout: DATABASE_CONNECT_TIMEOUT_SEC,
  });
}
