import i18n from 'i18next'
import { toast } from '@renderer/design'
import { universalAudioUrl, useSession } from '@renderer/jellyfin'
import { getPlaybackPreferences } from '@renderer/features/settings'
import { createJellyfinReporter, PlayerEngine } from './engine'
import { LocalAudioOutput } from './output'
import { getAudioMaxBitrate } from './quality'
import { trackTitle } from './track'

let engine: PlayerEngine | null = null

/** The one engine of the app. Created and started on first use, never stopped. */
export function startPlayerEngine(): PlayerEngine {
  if (engine) return engine
  engine = new PlayerEngine({
    output: new LocalAudioOutput(),
    resolveUrl(track) {
      const { api, account } = useSession.getState()
      if (!api || !account || !track.Id) return null
      return universalAudioUrl(api, account.userId, track.Id, { maxBitrate: getAudioMaxBitrate() })
    },
    getCrossfade() {
      const p = getPlaybackPreferences()
      return {
        enabled: p.crossfade,
        seconds: p.crossfadeSeconds,
        skipSameAlbum: p.crossfadeSkipAlbum,
      }
    },
    createReporter: createJellyfinReporter,
    onUnplayable(track) {
      toast({
        title: i18n.t('music:player.unplayable', { title: trackTitle(track) }),
        tone: 'error',
      })
    },
  })
  engine.start()
  return engine
}

export const getPlayerEngine = (): PlayerEngine | null => engine
