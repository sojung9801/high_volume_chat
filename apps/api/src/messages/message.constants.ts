/** 커서 형식: `<sentAtMs>_<seq>`. 이 메시지보다 오래된 것을 달라는 뜻이다. */
export const MESSAGE_CURSOR_PATTERN = /^(\d{1,16})_(\d{1,16})$/;

export const MESSAGE_CURSOR_SEPARATOR = '_';

/** 내보내기 때 DB에서 한 번에 읽는 행 수. 전부 메모리에 올리지 않고 이만큼씩 흘려보낸다. */
export const EXPORT_CURSOR_ROWS = 1000;

export const EXPORT_CSV_HEADER = 'sent_at,nickname,is_owner,text';

/** 엑셀이 UTF-8로 읽도록 파일 맨 앞에 붙이는 표식. 없으면 한글이 깨진다. */
export const CSV_UTF8_BOM = '\uFEFF';

export const CSV_NEEDS_QUOTES_PATTERN = /[",\r\n]/;

/** 스프레드시트가 수식으로 실행할 수 있는 첫 글자. */
export const CSV_FORMULA_PREFIX_PATTERN = /^[=+\-@\t\r]/;
