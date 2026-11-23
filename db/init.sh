#!/bin/sh
# 컨테이너 최초 기동 때 한 번 실행된다. 개발용 DB와 테스트용 DB에 같은 스키마를 만든다.
set -eu

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" -f /schema/schema.sql
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" -c "CREATE DATABASE ${POSTGRES_DB}_test"
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "${POSTGRES_DB}_test" -f /schema/schema.sql
