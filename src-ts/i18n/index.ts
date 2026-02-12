import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { en } from './locales/en.js';
import { zhCN } from './locales/zh-CN.js';

i18n
  .use(initReactI18next)
  .init({
    resources: {
      en: {
        translation: en
      },
      'zh-CN': {
        translation: zhCN
      }
    },
    lng: 'zh-CN', // Set default to Chinese
    fallbackLng: 'en',
    interpolation: {
      escapeValue: false
    }
  });

export default i18n;
