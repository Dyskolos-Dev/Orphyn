import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router'
import type { ServerAccount } from '@shared/jellyfin'
import '@renderer/i18n'

const account: ServerAccount = {
  id: 'acc-1',
  name: 'Home Jellyfin',
  url: 'http://nas:8096',
  serverId: 'srv',
  serverName: 'Home Jellyfin',
  version: '10.10.3',
  userId: 'u1',
  userName: 'alice',
  addedAt: '',
  lastUsedAt: '',
  hasToken: true,
}

const mocks = vi.hoisted(() => ({
  probeServer: vi.fn(),
  connectServer: vi.fn(),
  activate: vi.fn(),
  refreshAccounts: vi.fn(),
  rename: vi.fn(),
  state: { status: 'no-server' as string, account: null as ServerAccount | null },
}))

vi.mock('@renderer/jellyfin', () => {
  const useSession = Object.assign(
    (selector: (s: unknown) => unknown) => selector(useSession.getState()),
    {
      getState: () => ({
        status: mocks.state.status,
        activate: mocks.activate,
        refreshAccounts: mocks.refreshAccounts,
        init: vi.fn(),
        accounts: [],
        account: mocks.state.account,
        needsLogin: {},
      }),
    },
  )
  return {
    probeServer: mocks.probeServer,
    connectServer: mocks.connectServer,
    useSession,
  }
})

import { ConnectFlow } from './ConnectFlow'
import { ServerGuard } from './ServerGuard'

beforeEach(() => {
  vi.clearAllMocks()
  mocks.state.status = 'no-server'
  mocks.state.account = null
  mocks.activate.mockResolvedValue(undefined)
  mocks.refreshAccounts.mockResolvedValue(undefined)
  mocks.rename.mockImplementation((_id: string, name: string) =>
    Promise.resolve({ ...account, name }),
  )
  Object.defineProperty(window, 'jellyra', {
    configurable: true,
    value: { accounts: { rename: mocks.rename } },
  })
})

afterEach(() => {
  Object.defineProperty(window, 'jellyra', { configurable: true, value: undefined })
})

describe('ConnectFlow', () => {
  it('walks address → credentials → name and activates the server', async () => {
    mocks.probeServer.mockResolvedValue({
      ok: true,
      info: {
        url: 'http://nas:8096',
        serverId: 'srv',
        serverName: 'Home Jellyfin',
        version: '10.10.3',
      },
    })
    mocks.connectServer.mockResolvedValue({ ok: true, account })
    const onDone = vi.fn()
    const user = userEvent.setup()
    render(<ConnectFlow onDone={onDone} />)

    await user.type(screen.getByLabelText('Server address'), 'nas')
    await user.click(screen.getByRole('button', { name: 'Connect' }))
    expect(mocks.probeServer).toHaveBeenCalledWith('nas')
    expect(await screen.findByText('Home Jellyfin · Jellyfin 10.10.3')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Continue' }))

    await user.type(await screen.findByLabelText('Username'), 'alice')
    const password = screen.getByLabelText('Password')
    await user.type(password, 'secret{Enter}')
    expect(mocks.connectServer).toHaveBeenCalledWith({
      url: 'http://nas:8096',
      username: 'alice',
      password: 'secret',
    })

    const nameField = await screen.findByLabelText('Server name')
    expect(nameField).toHaveValue('Home Jellyfin')
    await user.clear(nameField)
    await user.type(nameField, 'Salon')
    await user.click(screen.getByRole('button', { name: 'Start Watching' }))

    await waitFor(() => expect(onDone).toHaveBeenCalled())
    expect(mocks.rename).toHaveBeenCalledWith('acc-1', 'Salon')
    expect(mocks.activate).toHaveBeenCalledWith(expect.objectContaining({ name: 'Salon' }))
  })

  it('switches to the new token at once when signing in again to the active server', async () => {
    mocks.state.account = account
    mocks.probeServer.mockResolvedValue({
      ok: true,
      info: {
        url: 'http://nas:8096',
        serverId: 'srv',
        serverName: 'Home Jellyfin',
        version: '10.11.10',
      },
    })
    mocks.connectServer.mockResolvedValue({ ok: true, account })
    const user = userEvent.setup()
    render(<ConnectFlow onDone={vi.fn()} />)
    await user.type(screen.getByLabelText('Server address'), 'nas{Enter}')
    await user.click(await screen.findByRole('button', { name: 'Continue' }))
    await user.type(await screen.findByLabelText('Username'), 'alice')
    await user.type(screen.getByLabelText('Password'), 'secret{Enter}')
    // Before the naming step is finished (the user may close the sheet there).
    await screen.findByLabelText('Server name')
    expect(mocks.activate).toHaveBeenCalledWith(account)
  })

  it.each([
    ['unreachable', 'Jellyra could not reach this server. Check the address and your connection.'],
    ['not_jellyfin', 'This address answered, but it is not a Jellyfin server.'],
  ])('shows an inline %s error', async (kind, text) => {
    mocks.probeServer.mockResolvedValue({ ok: false, error: { kind, message: 'x' } })
    const user = userEvent.setup()
    render(<ConnectFlow onDone={vi.fn()} />)
    await user.type(screen.getByLabelText('Server address'), 'nope{Enter}')
    expect(await screen.findByRole('alert')).toHaveTextContent(text)
  })

  it('explains a too-old server with its version', async () => {
    mocks.probeServer.mockResolvedValue({
      ok: false,
      error: { kind: 'version_too_old', version: '10.7.0', message: 'x' },
    })
    const user = userEvent.setup()
    render(<ConnectFlow onDone={vi.fn()} />)
    await user.type(screen.getByLabelText('Server address'), 'old{Enter}')
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'This server runs Jellyfin 10.7.0. Jellyra needs version 10.9 or newer.',
    )
  })

  it('reports bad credentials, wipes the password and can toggle its visibility', async () => {
    mocks.probeServer.mockResolvedValue({
      ok: true,
      info: { url: 'http://nas:8096', serverId: 'srv', serverName: 'Home', version: '10.10.3' },
    })
    mocks.connectServer.mockResolvedValue({
      ok: false,
      error: { kind: 'invalid_credentials', message: 'x' },
    })
    const user = userEvent.setup()
    render(<ConnectFlow onDone={vi.fn()} />)
    await user.type(screen.getByLabelText('Server address'), 'nas{Enter}')
    await screen.findByText('Server found')
    await user.click(screen.getByRole('button', { name: 'Continue' }))

    await user.type(await screen.findByLabelText('Username'), 'alice')
    const password = screen.getByLabelText('Password')
    expect(password).toHaveAttribute('type', 'password')
    await user.type(password, 'wrong')
    await user.click(screen.getByRole('button', { name: 'Show password' }))
    expect(password).toHaveAttribute('type', 'text')
    await user.click(password)
    await user.keyboard('{Enter}')

    expect(await screen.findByRole('alert')).toHaveTextContent('Wrong username or password.')
    await waitFor(() => expect(screen.getByLabelText('Password')).toHaveValue(''))
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
    expect(mocks.activate).not.toHaveBeenCalled()
  })
})

describe('ServerGuard', () => {
  function renderAt(path: string) {
    return render(
      <MemoryRouter initialEntries={[path]}>
        <ServerGuard>
          <Routes>
            <Route path="/onboarding" element={<p>onboarding</p>} />
            <Route path="/video" element={<p>video</p>} />
            <Route path="/settings" element={<p>settings</p>} />
          </Routes>
        </ServerGuard>
      </MemoryRouter>,
    )
  }

  it('sends every route to onboarding when no server is saved', () => {
    mocks.state.status = 'no-server'
    renderAt('/video')
    expect(screen.getByText('onboarding')).toBeInTheDocument()
  })

  it('leaves onboarding for /video once a server exists', () => {
    mocks.state.status = 'ready'
    renderAt('/onboarding')
    expect(screen.getByText('video')).toBeInTheDocument()
  })

  it('sends an expired session to Settings', () => {
    mocks.state.status = 'needs-login'
    renderAt('/video')
    expect(screen.getByText('settings')).toBeInTheDocument()
  })

  it('does nothing outside Electron', () => {
    Object.defineProperty(window, 'jellyra', { configurable: true, value: undefined })
    mocks.state.status = 'no-server'
    renderAt('/video')
    expect(screen.getByText('video')).toBeInTheDocument()
  })
})
