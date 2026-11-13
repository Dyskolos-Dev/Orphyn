// Design system barrel (owner: design agent). Import from '@renderer/design'.
// Rules and catalogue: docs/DESIGN.md.

// Theme
export { ThemeProvider, useTheme } from './theme/ThemeProvider'
export type { ThemeProviderProps, UseThemeResult } from './theme/ThemeProvider'
export {
  ACCENT_PRESETS,
  THEME_MODES,
  THEME_STORAGE_KEYS,
  themeStore,
  type AccentColor,
  type AccentPreset,
  type ResolvedTheme,
  type ThemeMode,
  type ThemeState,
} from './theme/theme'

// Platform / window
export { formatShortcut, getPlatform, modifierLabel, useWindowState } from './platform'
export type { Platform, UseWindowStateResult } from './platform'

// Motion presets
export { durations, easings, springs } from './motion'

// Primitives
export { Material, Text } from './primitives'
export type { MaterialProps, MaterialThickness, TextProps, TextStyle } from './primitives'

// Controls
export { Button } from './controls/Button'
export type { ButtonProps, ButtonSize, ButtonVariant } from './controls/Button'
export { IconButton } from './controls/IconButton'
export type { IconButtonProps, IconButtonSize, IconButtonVariant } from './controls/IconButton'
export { SegmentedControl } from './controls/SegmentedControl'
export type { SegmentedControlProps, SegmentedOption } from './controls/SegmentedControl'
export { Toggle } from './controls/Toggle'
export type { ToggleProps } from './controls/Toggle'
export { Slider } from './controls/Slider'
export type { SliderProps } from './controls/Slider'
export { SearchField, TextField } from './controls/TextField'
export type { SearchFieldProps, TextFieldProps } from './controls/TextField'

// Navigation
export { Sidebar, SidebarItem, SidebarSection } from './navigation/Sidebar'
export type { SidebarItemProps, SidebarProps, SidebarSectionProps } from './navigation/Sidebar'
export { Toolbar } from './navigation/Toolbar'
export type { ToolbarProps } from './navigation/Toolbar'

// Media
export { Artwork, initials } from './media/Artwork'
export type { ArtworkProps, ArtworkRatio } from './media/Artwork'
export { LandscapeCard, PosterCard, RoundCard, SquareCard } from './media/cards'
export type {
  CardBaseProps,
  LandscapeCardProps,
  PosterCardProps,
  RoundCardProps,
  SquareCardProps,
} from './media/cards'
export { Shelf } from './media/Shelf'
export type { ShelfItemKind, ShelfProps } from './media/Shelf'
export { Grid } from './media/Grid'
export type { GridProps } from './media/Grid'
export { HeroBanner } from './media/HeroBanner'
export type { HeroBannerProps } from './media/HeroBanner'

// Lists
export { List, ListRow } from './lists/ListRow'
export type { ListProps, ListRowProps } from './lists/ListRow'
export { formatDuration, TrackList, TrackRow } from './lists/TrackRow'
export type { TrackListProps, TrackRowProps } from './lists/TrackRow'

// Overlays
export { Modal, Sheet } from './overlays/Sheet'
export type { SheetProps } from './overlays/Sheet'
export { Popover } from './overlays/Popover'
export type { PopoverAnchor, PopoverPlacement, PopoverProps } from './overlays/Popover'
export { ContextMenu, Menu, MenuButton } from './overlays/Menu'
export type {
  ContextMenuProps,
  MenuActionItem,
  MenuButtonProps,
  MenuHeaderItem,
  MenuItem,
  MenuProps,
  MenuSeparatorItem,
} from './overlays/Menu'
export { dismissToast, toast, ToastViewport, useToast } from './overlays/Toast'
export type { ToastOptions } from './overlays/Toast'

// Feedback
export { Spinner } from './feedback/Spinner'
export type { SpinnerProps } from './feedback/Spinner'
export { Skeleton } from './feedback/Skeleton'
export type { SkeletonProps } from './feedback/Skeleton'
export { EmptyState } from './feedback/EmptyState'
export type { EmptyStateProps } from './feedback/EmptyState'
export { ProgressRing } from './feedback/ProgressRing'
export type { ProgressRingProps } from './feedback/ProgressRing'
