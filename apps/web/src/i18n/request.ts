import { getRequestConfig } from 'next-intl/server';

import messages from '../../messages/ko.json';
import { DEFAULT_LOCALE } from './i18n.constants';

export default getRequestConfig(async () => ({
  locale: DEFAULT_LOCALE,
  messages,
}));
