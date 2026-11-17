import {
  Clock,
  Clapperboard,
  Dices,
  Disc3,
  Flame,
  Folder,
  Heart,
  History,
  Library,
  ListMusic,
  Music2,
  Play,
  Radio,
  Sparkles,
  Tags,
  Tv,
  User,
  Film,
  type LucideIcon,
} from 'lucide-react'
import type { TFunction } from 'i18next'
import type { HomeRow, HomeScope, RowType } from './layout'

const ICONS: Record<HomeScope, Partial<Record<RowType, LucideIcon>>> = {
  video: {
    hero: Clapperboard,
    continue: Play,
    nextUp: Tv,
    latest: Library,
    favourites: Heart,
    genre: Tags,
    collection: Folder,
    because: Sparkles,
    released: Flame,
    random: Dices,
  },
  music: {
    recentlyPlayed: History,
    recentlyAdded: Clock,
    favAlbums: Heart,
    favArtists: User,
    favSongs: Music2,
    playlists: ListMusic,
    instantMix: Radio,
    genre: Tags,
    artist: Disc3,
    randomAlbums: Dices,
  },
}

export const rowIcon = (scope: HomeScope, type: RowType): LucideIcon =>
  ICONS[scope][type] ?? (scope === 'video' ? Film : Music2)

/** Name of a row type in the "Add a row" list. */
export const rowName = (t: TFunction, scope: HomeScope, type: RowType): string =>
  t(`rows.${scope}.${type}.name`)

export const rowDescription = (t: TFunction, scope: HomeScope, type: RowType): string =>
  t(`rows.${scope}.${type}.description`)

/** Title of a row on the home (and in the editor): the genre, library, artist... is part of it. */
export const rowTitle = (t: TFunction, scope: HomeScope, row: HomeRow): string =>
  t(`rows.${scope}.${row.type}.title`, { name: row.label ?? row.param ?? '' })
