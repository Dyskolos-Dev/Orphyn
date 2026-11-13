import { useTranslation } from 'react-i18next'

/** Returns `(seconds) => "1 h 12 min" / "48 min"`, in the UI language. */
export function useFormatTotal(): (seconds: number) => string {
  const { t } = useTranslation('music')
  return (seconds) => {
    const minutes = Math.max(1, Math.round(seconds / 60))
    return minutes >= 60
      ? t('album.durationHours', { hours: Math.floor(minutes / 60), minutes: minutes % 60 })
      : t('album.durationMinutes', { minutes })
  }
}
