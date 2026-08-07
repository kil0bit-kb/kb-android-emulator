import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

const modules = import.meta.glob('./localization/*.json', { eager: true });
const resources = {};

for (const path in modules) {
  const match = path.match(/\/([^/]+)\.json$/);
  if (match) {
    const lang = match[1];
    const moduleContent = modules[path].default || modules[path];
    resources[lang] = { translation: moduleContent };
  }
}

i18n
  .use(initReactI18next)
  .init({
    resources,
    lng: localStorage.getItem('app_lang') || 'en',
    fallbackLng: 'en',
    interpolation: {
      escapeValue: false
    }
  });

const setDirection = (lang: string) => {
  document.documentElement.dir = lang === 'ar' ? 'rtl' : 'ltr';
  document.documentElement.lang = lang;
};

setDirection(i18n.language);

i18n.on('languageChanged', (lang) => {
  setDirection(lang);
});

export default i18n;
