import { ChevronsUpDown } from 'lucide-react'
import { useRef, useState } from 'react'
import { Button, Menu } from '@renderer/design'

export interface ValueMenuOption<T> {
  value: T
  label: string
}

/** A gray pill showing the current value; opens a checkmarked menu (macOS pop-up button). */
export function ValueMenu<T extends string | number | null>({
  label,
  value,
  options,
  onChange,
}: {
  label: string
  value: T
  options: readonly ValueMenuOption<T>[]
  onChange(value: T): void
}) {
  const ref = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const current = options.find((o) => o.value === value)
  return (
    <>
      <Button
        ref={ref}
        variant="gray"
        size="small"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        trailingIcon={<ChevronsUpDown />}
        onClick={() => setOpen((o) => !o)}
      >
        {current?.label ?? ''}
      </Button>
      <Menu
        open={open}
        onClose={() => setOpen(false)}
        anchor={ref}
        aria-label={label}
        placement="bottom-end"
        items={options.map((o) => ({
          id: String(o.value),
          label: o.label,
          checked: o.value === value,
          onSelect: () => {
            setOpen(false)
            onChange(o.value)
          },
        }))}
      />
    </>
  )
}
