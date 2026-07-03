import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'

export function Settings({ 
  emojisEnabled, 
  setEmojisEnabled, 
  toast 
}) {
  const { t, i18n } = useTranslation()
  const currentLang = i18n.language || 'en'
  const setLang = (newLang) => {
    i18n.changeLanguage(newLang)
    localStorage.setItem('app_lang', newLang)
  }

  const languages = Object.keys(i18n.store?.data || {}).map(langCode => {
    const translation = i18n.store.data[langCode]?.translation || {}
    return {
      code: langCode,
      name: translation.lang_name || langCode.toUpperCase()
    }
  })
  const handleSave = () => {
    toast('💾 ' + t('ready') + '! Settings updated.', 'info')
  }

  return (
    <div className="page fade-in">
      <div className="page-header">
        <h1 className="page-title">{emojisEnabled ? '🛠️ ' : ''}{t('settings_title')}</h1>
        <p className="page-subtitle">{t('settings_subtitle')}</p>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 20, maxWidth: 640 }}>
        {/* Localization & Interface Options */}
        <div className="card">
          <div className="card-title">{t('settings_section_interface')}</div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16, marginTop: 12 }}>
            <div className="form-group">
              <label className="form-label">{t('settings_lang_label')}</label>
              <select 
                className="form-select" 
                value={currentLang} 
                onChange={e => setLang(e.target.value)}
              >
                {languages.map(l => (
                  <option key={l.code} value={l.code}>{l.name}</option>
                ))}
              </select>
            </div>

            <div className="divider" style={{ margin: '8px 0' }} />

            <div className="flex items-center justify-between">
              <div style={{ flex: 1, paddingRight: 16 }}>
                <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 2 }}>
                  {t('settings_emoji_label')}
                </div>
                <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>
                  {t('settings_emoji_desc')}
                </div>
              </div>
              <div 
                className={`toggle ${emojisEnabled ? 'on' : ''}`} 
                onClick={() => setEmojisEnabled(!emojisEnabled)} 
              />
            </div>
          </div>
        </div>

        <button 
          className="btn btn-primary" 
          onClick={handleSave}
          style={{ alignSelf: 'flex-start', marginTop: 8 }}
        >
          {t('settings_save_btn')}
        </button>
      </div>
    </div>
  )
}
