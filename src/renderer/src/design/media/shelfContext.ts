import { createContext, useContext } from 'react'

export interface ShelfLayout {
  /** The shelf shows poster cards as 16:9 cards (Settings > Appearance > Video shelves). */
  postersAsLandscape: boolean
}

export const ShelfLayoutContext = createContext<ShelfLayout>({ postersAsLandscape: false })

export function useShelfLayout(): ShelfLayout {
  return useContext(ShelfLayoutContext)
}
