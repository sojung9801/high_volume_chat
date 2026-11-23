import { memo } from 'react';
import { useLocale, useTranslations } from 'next-intl';

import type { ChatMessageItem } from '../chat.types';
import { formatMessageTime } from '../chat.utils';
import styles from './ChatRoom.module.css';

type ChatMessageRowProps = {
  message: ChatMessageItem;
};

export const ChatMessageRow = memo(function ChatMessageRow({ message }: ChatMessageRowProps) {
  const t = useTranslations();
  const locale = useLocale();

  return (
    <div className={styles.row}>
      <time className={styles.time} dateTime={new Date(message.sentAtMs).toISOString()}>
        {formatMessageTime(message.sentAtMs, locale)}
      </time>
      {message.isOwner && <span className={styles.ownerBadge}>{t('messages.chat.owner')}</span>}
      <span className={styles.nickname}>{message.nickname}</span>
      <span>{message.text}</span>
    </div>
  );
});
