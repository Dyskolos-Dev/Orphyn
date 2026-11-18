import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useSession } from '@renderer/jellyfin'
import { createMixSource } from './api'
import { mixesStorageKey, readMixes, writeMixes } from './cache'
import { generateMixes } from './generate'
import { loadTopYearMix } from './topYear'
import type { Mix } from './types'

/** The user's mixes of the day: read from the cache of the day, else generated and saved. */
export async function loadMixes(
  accountId: string,
  userId: string,
  now = Date.now(),
): Promise<Mix[]> {
  const key = mixesStorageKey(accountId, userId)
  const cached = readMixes(key, now)
  if (cached) return cached
  const mixes = await generateMixes(createMixSource(), now)
  writeMixes(key, mixes, now)
  return mixes
}

export function useMixes() {
  const account = useSession((s) => s.account)
  const api = useSession((s) => s.api)
  const qc = useQueryClient()
  return useQuery({
    queryKey: ['jf', account?.id ?? '', 'mixes', account?.userId ?? ''],
    queryFn: async () => {
      const accountId = account?.id ?? ''
      const userId = account?.userId ?? ''
      const [daily, top] = await Promise.all([
        loadMixes(accountId, userId),
        loadTopYearMix(qc, accountId, userId),
      ])
      return top ? [...daily, top] : daily
    },
    enabled: !!account && !!api,
    // Generated once a day (the day cache answers repeat loads), never refetched on focus.
    staleTime: 60 * 60_000,
    refetchOnWindowFocus: false,
  })
}

/** Display name and subtitle of a mix in the current language. */
export function useMixLabels() {
  const { t } = useTranslation('mixes')
  return (mix: Mix) => ({
    name:
      mix.kind === 'forYou'
        ? t('names.forYou', { n: mix.n })
        : mix.kind === 'genre'
          ? t('names.genre', { genre: mix.genre })
          : mix.kind === 'topYear'
            ? t('names.topYear', { year: mix.year })
            : t(`names.${mix.kind}`),
    subtitle: mix.artists.length
      ? t('with', { artists: mix.artists.slice(0, 3).join(', ') })
      : t(`hints.${mix.kind}`),
  })
}
