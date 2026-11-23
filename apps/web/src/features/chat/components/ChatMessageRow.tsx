import { memo } from 'react';
import { useTranslations } from 'next-intl';

import type { ChatMessageItem } from '../chat.types';
import styles from './ChatRoom.module.css';

type ChatMessageRowProps = {
  message: ChatMessageItem;
};

export const ChatMessageRow = memo(function ChatMessageRow({ message }: ChatMessageRowProps) {
  const t = useTranslations();

  return (
    <div className={styles.row}>
      {message.isOwner && <span className={styles.ownerBadge}>{t('messages.chat.owner')}</span>}
      <span className={styles.nickname}>{message.nickname}</span>
      <span>{message.text}</span>
    </div>
  );
});
