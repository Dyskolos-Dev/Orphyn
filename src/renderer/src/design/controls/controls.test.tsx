import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Play } from 'lucide-react'
import { useState } from 'react'
import '@renderer/i18n'
import { Button } from './Button'
import { IconButton } from './IconButton'
import { SegmentedControl } from './SegmentedControl'
import { Slider } from './Slider'
import { SearchField, TextField } from './TextField'
import { Toggle } from './Toggle'

describe('Button', () => {
  it('is a type=button that fires onClick', async () => {
    const onClick = vi.fn()
    render(
      <Button variant="tinted" size="large" icon={<Play />} onClick={onClick}>
        Play
      </Button>,
    )
    const btn = screen.getByRole('button', { name: 'Play' })
    expect(btn).toHaveAttribute('type', 'button')
    expect(btn).toHaveClass('jr-button--tinted', 'jr-button--large')
    await userEvent.click(btn)
    expect(onClick).toHaveBeenCalledOnce()
  })

  it('is busy and disabled while loading', async () => {
    const onClick = vi.fn()
    render(
      <Button loading onClick={onClick}>
        Save
      </Button>,
    )
    const btn = screen.getByRole('button', { name: /save/i })
    expect(btn).toBeDisabled()
    expect(btn).toHaveAttribute('aria-busy', 'true')
    await userEvent.click(btn)
    expect(onClick).not.toHaveBeenCalled()
  })
})

describe('IconButton', () => {
  it('uses the label as accessible name and tooltip, and exposes pressed state', () => {
    render(<IconButton icon={<Play />} label="Shuffle" pressed />)
    const btn = screen.getByRole('button', { name: 'Shuffle' })
    expect(btn).toHaveAttribute('title', 'Shuffle')
    expect(btn).toHaveAttribute('aria-pressed', 'true')
  })
})

function Segmented() {
  const [v, setV] = useState<'all' | 'movies' | 'shows'>('all')
  return (
    <SegmentedControl
      aria-label="Filter"
      value={v}
      onChange={setV}
      options={[
        { value: 'all', label: 'All' },
        { value: 'movies', label: 'Movies' },
        { value: 'shows', label: 'Shows', disabled: true },
      ]}
    />
  )
}

describe('SegmentedControl', () => {
  it('is a radio group with a roving tab stop and arrow-key selection', async () => {
    const user = userEvent.setup()
    render(<Segmented />)
    const all = screen.getByRole('radio', { name: 'All' })
    const movies = screen.getByRole('radio', { name: 'Movies' })
    expect(screen.getByRole('radiogroup', { name: 'Filter' })).toBeInTheDocument()
    expect(all).toHaveAttribute('aria-checked', 'true')
    expect(movies).toHaveAttribute('tabindex', '-1')

    await user.tab()
    expect(all).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(movies).toHaveFocus()
    expect(movies).toHaveAttribute('aria-checked', 'true')
    // "Shows" is disabled, so ArrowRight wraps to "All".
    await user.keyboard('{ArrowRight}')
    expect(all).toHaveAttribute('aria-checked', 'true')
    await user.keyboard('{End}')
    expect(movies).toHaveAttribute('aria-checked', 'true')
    await user.click(all)
    expect(all).toHaveAttribute('aria-checked', 'true')
  })
})

describe('Toggle', () => {
  it('is a labelled switch toggled by click and keyboard', async () => {
    const user = userEvent.setup()
    function T() {
      const [on, setOn] = useState(false)
      return <Toggle checked={on} onChange={setOn} label="Autoplay" description="Next episode" />
    }
    render(<T />)
    const sw = screen.getByRole('switch', { name: 'Autoplay' })
    expect(sw).toHaveAccessibleDescription('Next episode')
    expect(sw).toHaveAttribute('aria-checked', 'false')
    await user.click(sw)
    expect(sw).toHaveAttribute('aria-checked', 'true')
    sw.focus()
    await user.keyboard(' ')
    expect(sw).toHaveAttribute('aria-checked', 'false')
    // Clicking the visible label toggles too.
    await user.click(screen.getByText('Autoplay'))
    expect(sw).toHaveAttribute('aria-checked', 'true')
  })
})

describe('Slider', () => {
  function Volume({ onEnd }: { onEnd?: (v: number) => void }) {
    const [v, setV] = useState(0.5)
    return (
      <Slider
        aria-label="Volume"
        value={v}
        step={0.1}
        onChange={setV}
        onChangeEnd={onEnd}
        formatValue={(x) => `${Math.round(x * 100)}%`}
      />
    )
  }

  it('exposes slider semantics and moves with the keyboard', async () => {
    const user = userEvent.setup()
    const onEnd = vi.fn()
    render(<Volume onEnd={onEnd} />)
    const slider = screen.getByRole('slider', { name: 'Volume' })
    expect(slider).toHaveAttribute('aria-valuenow', '0.5')
    expect(slider).toHaveAttribute('aria-valuetext', '50%')
    slider.focus()
    await user.keyboard('{ArrowRight}')
    expect(slider).toHaveAttribute('aria-valuenow', '0.6')
    expect(onEnd).toHaveBeenLastCalledWith(0.6)
    await user.keyboard('{ArrowLeft}{ArrowLeft}')
    expect(slider).toHaveAttribute('aria-valuenow', '0.4')
    await user.keyboard('{End}')
    expect(slider).toHaveAttribute('aria-valuenow', '1')
    await user.keyboard('{ArrowRight}')
    expect(slider).toHaveAttribute('aria-valuenow', '1')
    await user.keyboard('{Home}')
    expect(slider).toHaveAttribute('aria-valuenow', '0')
  })

  it('scrubs with the pointer and commits on release', () => {
    const onEnd = vi.fn()
    const onChange = vi.fn()
    const onStart = vi.fn()
    render(
      <Slider
        aria-label="Seek"
        value={0}
        min={0}
        max={200}
        onChange={onChange}
        onChangeStart={onStart}
        onChangeEnd={onEnd}
      />,
    )
    const slider = screen.getByRole('slider', { name: 'Seek' })
    slider.getBoundingClientRect = () =>
      ({ left: 100, width: 400, top: 0, height: 20, right: 500, bottom: 20 }) as DOMRect
    fireEvent.pointerDown(slider, { button: 0, clientX: 200, pointerId: 1 })
    expect(onStart).toHaveBeenCalled()
    expect(onChange).toHaveBeenLastCalledWith(50)
    fireEvent.pointerMove(slider, { clientX: 400, pointerId: 1 })
    expect(onChange).toHaveBeenLastCalledWith(150)
    fireEvent.pointerMove(slider, { clientX: 900, pointerId: 1 })
    expect(onChange).toHaveBeenLastCalledWith(200)
    fireEvent.pointerUp(slider, { clientX: 300, pointerId: 1 })
    expect(onEnd).toHaveBeenCalledWith(100)
  })
})

describe('TextField', () => {
  it('links label, hint and error', () => {
    render(<TextField label="Server address" hint="Include the port" error="Unreachable" />)
    const input = screen.getByLabelText('Server address')
    expect(input).toHaveAttribute('aria-invalid', 'true')
    expect(input).toHaveAccessibleDescription('Unreachable')
    expect(screen.getByRole('alert')).toHaveTextContent('Unreachable')
  })
})

describe('SearchField', () => {
  function S({ onSubmit }: { onSubmit?: (v: string) => void }) {
    const [q, setQ] = useState('')
    return <SearchField aria-label="Search" value={q} onChange={setQ} onSubmit={onSubmit} />
  }

  it('types, submits on Enter, clears with the button and Escape', async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn()
    render(<S onSubmit={onSubmit} />)
    const box = screen.getByRole('searchbox', { name: 'Search' })
    await user.type(box, 'daft punk{Enter}')
    expect(onSubmit).toHaveBeenCalledWith('daft punk')
    await user.click(screen.getByRole('button', { name: 'Clear' }))
    expect(box).toHaveValue('')
    expect(box).toHaveFocus()
    await user.type(box, 'abc')
    await user.keyboard('{Escape}')
    expect(box).toHaveValue('')
    expect(box).toHaveFocus()
    await user.keyboard('{Escape}')
    expect(box).not.toHaveFocus()
  })
})
