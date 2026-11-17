import {
  Disc3,
  Download,
  Film,
  Heart,
  Home,
  ListMusic,
  ListPlus,
  LogOut,
  Moon,
  Music,
  Palette,
  Pause,
  PictureInPicture2,
  Play,
  RefreshCw,
  Repeat,
  Search,
  Server,
  Settings,
  Shuffle,
  SkipBack,
  SkipForward,
  Star,
  Sun,
  Tv,
  User,
  Disc,
  type LucideIcon,
} from 'lucide-react'
import { createCommandRegistry, type Command, type PaletteContext } from './registry'

const hasTrack = (ctx: PaletteContext) => ctx.player.hasTrack

function page(id: string, icon: LucideIcon, to: string, keywords: string[] = []): Command {
  return {
    id: `go-${id}`,
    group: 'navigate',
    labelKey: `go-${id}`,
    icon,
    keywords,
    run: (ctx) => ctx.navigate(to),
  }
}

/** Playback and "now playing" commands, available while a track is loaded. */
const playback: Command[] = [
  {
    id: 'play-pause',
    group: 'playback',
    labelKey: (ctx) => (ctx.player.playing ? 'pause' : 'play'),
    icon: Play,
    iconFor: (ctx) => (ctx.player.playing ? Pause : Play),
    keywords: ['play', 'pause', 'lecture', 'toggle'],
    shortcut: 'Space',
    available: hasTrack,
    run: (ctx) => ctx.player.toggle(),
  },
  {
    id: 'next',
    group: 'playback',
    labelKey: 'next',
    icon: SkipForward,
    keywords: ['skip', 'suivant'],
    available: hasTrack,
    run: (ctx) => ctx.player.next(),
  },
  {
    id: 'previous',
    group: 'playback',
    labelKey: 'previous',
    icon: SkipBack,
    keywords: ['back', 'precedent'],
    available: hasTrack,
    run: (ctx) => ctx.player.previous(),
  },
  {
    id: 'shuffle',
    group: 'playback',
    labelKey: (ctx) => (ctx.player.shuffle ? 'shuffle-off' : 'shuffle-on'),
    icon: Shuffle,
    keywords: ['random', 'aleatoire'],
    available: hasTrack,
    run: (ctx) => ctx.player.toggleShuffle(),
  },
  {
    id: 'repeat',
    group: 'playback',
    labelKey: (ctx) => `repeat-${ctx.player.repeat}`,
    icon: Repeat,
    keywords: ['loop', 'boucle'],
    available: hasTrack,
    run: (ctx) => ctx.player.cycleRepeat(),
  },
  {
    id: 'now-playing',
    group: 'playback',
    labelKey: 'now-playing',
    icon: Disc3,
    keywords: ['player', 'lecteur', 'lyrics', 'paroles', 'queue', 'file'],
    available: hasTrack,
    run: (ctx) => ctx.openNowPlaying(),
  },
  {
    id: 'pip',
    group: 'playback',
    labelKey: 'pip',
    icon: PictureInPicture2,
    keywords: ['picture in picture', 'pip', 'mini player', 'mini lecteur', 'fenetre flottante'],
    available: (ctx) => ctx.openPip !== undefined,
    run: (ctx) => ctx.openPip?.(),
  },
  {
    id: 'favorite',
    group: 'playback',
    labelKey: (ctx) => (ctx.favorite.value ? 'unfavorite' : 'favorite'),
    icon: Heart,
    keywords: ['like', 'heart', 'coeur', 'favori', 'favourite'],
    available: (ctx) => ctx.favorite.available,
    run: (ctx) => ctx.favorite.toggle(),
  },
  {
    id: 'add-to-playlist',
    group: 'playback',
    labelKey: 'add-to-playlist',
    icon: ListPlus,
    keywords: ['playlist', 'liste de lecture', 'ajouter'],
    available: hasTrack,
    run: (ctx) => ctx.addToPlaylist(),
  },
]

const navigation: Command[] = [
  page('home', Home, '/video', ['accueil', 'watch', 'video']),
  page('movies', Film, '/video/library/movies', ['films', 'cinema']),
  page('series', Tv, '/video/library/series', ['tv', 'shows', 'series tv', 'emissions']),
  page('music', Music, '/music', ['musique']),
  page('albums', Disc, '/music/albums'),
  page('artists', User, '/music/artists', ['artistes']),
  page('playlists', ListMusic, '/music/playlists', ['listes de lecture']),
  page('songs', Music, '/music/songs', ['titres', 'tracks', 'morceaux', 'chansons']),
  page('favorites', Star, '/music/favorites', ['favoris', 'likes', 'liked']),
  page('search', Search, '/search', ['recherche', 'find', 'chercher']),
  page('settings', Settings, '/settings', ['preferences', 'reglages', 'options', 'parametres']),
]

const actions: Command[] = [
  {
    id: 'refresh',
    group: 'actions',
    labelKey: 'refresh',
    icon: RefreshCw,
    keywords: ['reload', 'recharger', 'actualiser', 'sync'],
    shortcut: 'R',
    run: (ctx) => ctx.refreshLibrary(),
  },
  {
    id: 'check-updates',
    group: 'actions',
    labelKey: 'check-updates',
    icon: Download,
    keywords: ['update', 'upgrade', 'mise a jour', 'version'],
    available: (ctx) => ctx.checkForUpdates !== undefined,
    run: (ctx) => ctx.checkForUpdates?.(),
  },
  {
    id: 'sign-out',
    group: 'actions',
    labelKey: 'sign-out',
    icon: LogOut,
    keywords: ['logout', 'log out', 'deconnexion', 'se deconnecter'],
    available: (ctx) => ctx.signedIn,
    run: (ctx) => ctx.signOut(),
  },
]

const appearance: Command[] = [
  {
    id: 'toggle-theme',
    group: 'appearance',
    labelKey: (ctx) => (ctx.theme.resolved === 'dark' ? 'theme-light' : 'theme-dark'),
    icon: Moon,
    iconFor: (ctx) => (ctx.theme.resolved === 'dark' ? Sun : Moon),
    keywords: ['dark', 'light', 'sombre', 'clair', 'theme', 'mode', 'apparence'],
    run: (ctx) => ctx.theme.setMode(ctx.theme.resolved === 'dark' ? 'light' : 'dark'),
  },
]

/** One "Interface style: X" command per style, except the one in use. */
function styleCommands(ctx: PaletteContext): Command[] {
  return ctx.interfaceStyle.styles
    .filter((s) => s !== ctx.interfaceStyle.current)
    .map((style) => ({
      id: `style-${style}`,
      group: 'appearance' as const,
      labelKey: `style-${style}`,
      icon: Palette,
      keywords: ['style', 'interface', 'apparence', 'skin'],
      run: (c: PaletteContext) => c.interfaceStyle.set(style),
    }))
}

/** One "switch to <server>" command per account that is not the active one. */
function serverCommands(ctx: PaletteContext): Command[] {
  return ctx.accounts
    .filter((a) => !a.active)
    .map((a) => ({
      id: `server-${a.id}`,
      group: 'servers' as const,
      title: a.name,
      icon: Server,
      keywords: ['switch server', 'server', 'changer de serveur', 'serveur', 'compte', 'account'],
      run: (c: PaletteContext) => c.switchServer(a.id),
    }))
}

/** Registry the palette lists; other features can `paletteRegistry.register(...)` their own. */
export const paletteRegistry = createCommandRegistry()

export const BUILTIN_COMMANDS = [
  ...playback,
  ...navigation,
  ...actions,
  ...appearance,
  styleCommands,
  serverCommands,
] as const

paletteRegistry.register(...BUILTIN_COMMANDS)
