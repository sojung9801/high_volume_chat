'use client';

import { useTranslations } from 'next-intl';

import styles from './ChatRoom.module.css';

type ChatScrollToBottomButtonProps = {
  onClick: () => void;
};

/** 위로 올려 지난 글을 보는 중에 가장 최근 글로 바로 내려가는 버튼. */
export function ChatScrollToBottomButton({ onClick }: ChatScrollToBottomButtonProps) {
  const t = useTranslations();

  return (
    <button type="button" className={styles.newMessagesButton} onClick={onClick}>
      {t('messages.chat.scroll_to_bottom')}
    </button>
  );
}
