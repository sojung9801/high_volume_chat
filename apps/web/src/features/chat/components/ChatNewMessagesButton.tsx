'use client';

import { useTranslations } from 'next-intl';

import { CHAT_MAX_RENDERED_MESSAGES } from '../chat.constants';
import styles from './ChatRoom.module.css';

type ChatNewMessagesButtonProps = {
  count: number;
  onClick: () => void;
};

export function ChatNewMessagesButton({ count, onClick }: ChatNewMessagesButtonProps) {
  const t = useTranslations();
  // 안 본 메시지는 상한까지만 모아 둔다. 상한에 닿았으면 실제로는 그보다 많이 왔을 수 있으니 "+"를 붙인다.
  const label = count >= CHAT_MAX_RENDERED_MESSAGES ? `${count}+` : String(count);

  return (
    <button type="button" className={styles.newMessagesButton} onClick={onClick}>
      {t('messages.chat.new_messages', { count: label })}
    </button>
  );
}
