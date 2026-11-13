// App shell public API (owner: shell agent). Features import from '@renderer/shell'.
export { AppShell } from './AppShell'
export { ShellPage } from './ShellPage'
export type { ShellPageProps } from './ShellPage'
export {
  ShellImmersive,
  ShellNowPlayingSlot,
  ShellServerSwitcherSlot,
  useShellImmersive,
} from './context'
export { getShellExtensions, registerShellExtension, useShellExtensions } from './registry'
export type { ShellExtension, ShellSidebarEntry, ShellSidebarSection } from './registry'
