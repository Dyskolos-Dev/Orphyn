import logo from '../assets/orphyn-logo.png'

/** The official Orphyn logo (decorative: the app name is always rendered next to it). */
export function BrandLogo({ size = 64, className }: { size?: number; className?: string }) {
  return (
    <img
      src={logo}
      width={size}
      height={size}
      alt=""
      aria-hidden="true"
      draggable={false}
      className={className}
      style={{ width: size, height: size, display: 'block', userSelect: 'none' }}
    />
  )
}
