// 보관 폴더의 Parquet 파일들을 하나의 표(`chat`)로 보고 SQL을 실행한다.
//
//   npm run archive:sql -- "SELECT count(*) FROM chat"
//
// SQL을 주지 않으면 방별 요약을 보여준다.
import { existsSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { DuckDBInstance } from '@duckdb/node-api';

const DEFAULT_SQL = `
  SELECT room_id,
         count(*) AS received,
         count(*) FILTER (WHERE delivered) AS shown,
         count(*) FILTER (WHERE NOT delivered) AS dropped,
         count(DISTINCT user_id) AS people
  FROM chat
  GROUP BY room_id
  ORDER BY received DESC`;

const archiveDir = resolve(process.env.ARCHIVE_DIR ?? '');
if (!process.env.ARCHIVE_DIR) {
  console.error('ARCHIVE_DIR is not set (run through `npm run archive:sql`)');
  process.exit(1);
}
const files = existsSync(archiveDir)
  ? readdirSync(archiveDir).filter((name) => name.endsWith('.parquet'))
  : [];
if (files.length === 0) {
  console.error(`no archive files in ${archiveDir} yet`);
  process.exit(1);
}

const connection = await (await DuckDBInstance.create(':memory:')).connect();
const pattern = join(archiveDir, '*.parquet').replaceAll("'", "''");
await connection.run(`CREATE VIEW chat AS SELECT * FROM read_parquet('${pattern}')`);

const sql = process.argv.slice(2).join(' ').trim() || DEFAULT_SQL;
const result = await connection.runAndReadAll(sql);
console.log(`${files.length} file(s) in ${archiveDir}`);
console.table(result.getRowObjectsJson());
