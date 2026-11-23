'use client';

import { useState, type FormEvent } from 'react';
import { useTranslations } from 'next-intl';

import { CHAT_MAX_TEXT_LENGTH } from '../chat.constants';
import styles from './ChatRoom.module.css';

type ChatMessageInputProps = {
  isDisabled: boolean;
  onSend: (text: string) => boolean;
};

export function ChatMessageInput({ isDisabled, onSend }: ChatMessageInputProps) {
  const t = useTranslations();
  const [text, setText] = useState('');

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const trimmed = text.trim();
    if (trimmed !== '' && onSend(trimmed)) {
      setText('');
    }
  };

  return (
    <form className={styles.inputForm} onSubmit={handleSubmit}>
      <input
        className={styles.textInput}
        value={text}
        maxLength={CHAT_MAX_TEXT_LENGTH}
        placeholder={t('messages.chat.placeholder')}
        aria-label={t('messages.chat.placeholder')}
        onChange={(event) => setText(event.target.value)}
      />
      <button type="submit" className={styles.submitButton} disabled={isDisabled}>
        {t('common.action.send')}
      </button>
    </form>
  );
}
