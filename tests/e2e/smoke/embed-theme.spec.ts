import { test, expect, type Page, type Frame } from '@playwright/test'

const branded = {
  primary: '#7138a8',
  background: '#f2f0f7',
  surface: '#ffffff',
  primaryButtonBackground: '#24639a',
  primaryButtonText: '#ffffff',
  primaryButtonHover: '#194b76',
  secondaryButtonBackground: '#e8edf5',
  secondaryButtonText: '#20314f',
  secondaryButtonHover: '#d5ddea',
  fontFamily: 'Arial, sans-serif',
  fontSize: '16px',
  headingFontSize: '26px',
  radius: '6px',
}

type ThemeHost = Window & {
  messages: Array<{ type: string; frameId: string; revision?: number }>
  theme: Record<string, string> | null
}

async function host(page: Page, tokens: Record<string, string> | null = branded) {
  await page.addInitScript(() => {
    localStorage.setItem('theme', 'dark')
    localStorage.setItem(
      'auth-store',
      JSON.stringify({
        state: {
          token: 'theme-test-token',
          user: {
            userId: 'hs_theme_test',
            merchantId: 'theme-test',
            role: 'admin',
            isRedirectSession: true,
          },
          merchants: [],
        },
        version: 0,
      }),
    )
  })
  await page.route('**/decision-engine-api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
    const body = path.endsWith('/auth/me')
      ? {
          user_id: 'hs_theme_test',
          merchant_id: 'theme-test',
          role: 'admin',
          email: 'theme@example.test',
          email_verified: true,
          merchants: [],
        }
      : path.includes('/routing/list/')
        ? []
        : path.endsWith('/features')
          ? { features: [] }
          : {}
    await route.fulfill({ json: body })
  })
  await page.route('**/__embed-theme-host', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `
    <!doctype html><html><body style="margin:0">
    <script>
      window.messages = [];
      window.theme = ${JSON.stringify(tokens)};
      addEventListener('message', event => {
        if (event.origin !== location.origin || event.source !== document.querySelector('iframe').contentWindow) return;
        messages.push(event.data);
        if (event.data.type === 'de:theme-ready' && theme) event.source.postMessage({
          type: 'de:theme-update', version: 1, frameId: event.data.frameId, revision: 1, tokens: theme
        }, location.origin);
      });
    </script>
    <iframe title="Decision Engine" src="/routing/rules?embed=1" style="width:100vw;height:100vh;border:0"></iframe>
    </body></html>`,
    }),
  )
  await page.goto('/__embed-theme-host')
  const iframe = page.frameLocator('iframe')
  await expect(iframe.getByRole('heading', { name: 'Rule-Based Routing' })).toBeVisible()
  return iframe
}

async function send(page: Page, tokens: unknown, overrides: Record<string, unknown> = {}) {
  await page.evaluate(
    ({ tokens, overrides }) => {
      const host = window as unknown as ThemeHost
      const ready = host.messages
        .slice()
        .reverse()
        .find((message) => message.type === 'de:theme-ready')!
      document.querySelector('iframe')!.contentWindow!.postMessage(
        {
          type: 'de:theme-update',
          version: 1,
          frameId: ready.frameId,
          revision: 2,
          tokens,
          ...overrides,
        },
        location.origin,
      )
    },
    { tokens, overrides },
  )
}

async function appFrame(page: Page): Promise<Frame> {
  return (await page.locator('iframe').elementHandle())!.contentFrame().then((frame) => frame!)
}

test('applies host branding and live changes without losing an unsaved rule', async ({ page }) => {
  const frame = await host(page)
  const create = frame.getByRole('button', { name: 'Create Rule', exact: true })
  await expect(create).toHaveCSS('background-color', 'rgb(36, 99, 154)')
  await create.hover()
  await expect(create).toHaveCSS('background-color', 'rgb(25, 75, 118)')
  await expect(create).toHaveCSS('border-radius', '6px')
  await expect(frame.locator('html')).not.toHaveClass(/dark/)
  await expect(frame.getByRole('heading', { name: 'Rule-Based Routing' })).toHaveCSS(
    'font-size',
    '26px',
  )
  await expect(frame.locator('body')).toHaveCSS('background-color', 'rgb(242, 240, 247)')
  await create.click()
  const name = frame.getByPlaceholder('my-rule')
  await name.fill('unsaved branded rule')
  await expect(name).toHaveCSS('border-color', 'rgb(113, 56, 168)')
  const url = (await appFrame(page)).url()
  await send(page, { ...branded, primaryButtonBackground: '#842d4b' })
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as unknown as ThemeHost).messages.some(
          (message) => message.type === 'de:theme-applied' && message.revision === 2,
        ),
      ),
    )
    .toBe(true)
  await expect(name).toHaveValue('unsaved branded rule')
  expect((await appFrame(page)).url()).toBe(url)
  expect(await page.evaluate(() => localStorage.getItem('theme'))).toBe('dark')
})

test('rejects wrong sender, origin, version, frame and stale revision', async ({ page }) => {
  const frame = await host(page)
  const create = frame.getByRole('button', { name: 'Create Rule', exact: true })
  for (const overrides of [
    { version: 2 },
    { frameId: 'old-frame' },
    { revision: 0 },
    { revision: -1 },
  ]) {
    await send(page, { primaryButtonBackground: '#ff0000' }, overrides)
  }
  const child = await appFrame(page)
  const ready = await page.evaluate(
    () =>
      (window as unknown as ThemeHost).messages.find(
        (message) => message.type === 'de:theme-ready',
      )!,
  )
  await child.evaluate(({ frameId }) => {
    const data = {
      type: 'de:theme-update',
      version: 1,
      frameId,
      revision: 9,
      tokens: { primaryButtonBackground: '#ff0000' },
    }
    window.postMessage(data, location.origin)
    window.dispatchEvent(
      new MessageEvent('message', { data, origin: 'https://untrusted.example', source: parent }),
    )
  }, ready)
  await send(page, branded, { revision: 3 })
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as unknown as ThemeHost).messages
          .filter((message) => message.type === 'de:theme-applied')
          .map((message) => message.revision),
      ),
    )
    .toEqual([1, 3])
  await expect(create).toHaveCSS('background-color', 'rgb(36, 99, 154)')
})

test('partial and invalid tokens reset old overrides and cannot inject CSS', async ({ page }) => {
  const frame = await host(page)
  await send(page, {
    primary: '#287040',
    primaryButtonBackground: 'red; background: url(https://untrusted.example)',
    fontFamily: 'url(https://untrusted.example)',
    radius: '999px',
  })
  const create = frame.getByRole('button', { name: 'Create Rule', exact: true })
  await expect(create).toHaveCSS('background-color', 'rgb(40, 112, 64)')
  await expect(create).toHaveCSS('border-radius', '9999px')
  await expect(frame.locator('body')).toHaveCSS('background-color', 'rgb(255, 255, 255)')
  await send(page, {}, { revision: 3 })
  await expect(create).toHaveCSS('background-color', 'rgb(12, 105, 238)')
  expect(await frame.locator('html').getAttribute('style')).not.toContain('untrusted')
})

test('missing host falls back, then accepts a late theme and survives iframe reload', async ({
  page,
}) => {
  const frame = await host(page, null)
  const create = frame.getByRole('button', { name: 'Create Rule', exact: true })
  await expect(create).toHaveCSS('background-color', 'rgb(12, 105, 238)')
  await send(page, branded)
  await expect(create).toHaveCSS('background-color', 'rgb(36, 99, 154)')
  await page.evaluate((tokens) => {
    ;(window as unknown as ThemeHost).theme = tokens
    document.querySelector('iframe')!.contentWindow!.location.reload()
  }, branded)
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          new Set(
            (window as unknown as ThemeHost).messages
              .filter((message) => message.type === 'de:theme-ready')
              .map((message) => message.frameId),
          ).size,
      ),
    )
    .toBe(2)
  await expect(create).toHaveCSS('background-color', 'rgb(36, 99, 154)')
})

test('standalone page ignores theme messages and retains its saved dark preference', async ({
  page,
}) => {
  await host(page)
  await page.goto('/routing/rules')
  await expect(page.locator('html')).toHaveClass(/dark/)
  await page.evaluate(() =>
    window.postMessage(
      {
        type: 'de:theme-update',
        version: 1,
        frameId: 'standalone',
        revision: 1,
        tokens: { primary: '#ff0000' },
      },
      location.origin,
    ),
  )
  expect(await page.locator('html').getAttribute('style')).toBeNull()
  expect(await page.evaluate(() => localStorage.getItem('theme'))).toBe('dark')
})

test('themes stay local to each frame and clear when the session is rejected', async ({
  page,
  context,
}) => {
  const first = await host(page)
  const otherPage = await context.newPage()
  const second = await host(otherPage, { ...branded, primaryButtonBackground: '#842d4b' })
  await expect(first.getByRole('button', { name: 'Create Rule', exact: true })).toHaveCSS(
    'background-color',
    'rgb(36, 99, 154)',
  )
  await expect(second.getByRole('button', { name: 'Create Rule', exact: true })).toHaveCSS(
    'background-color',
    'rgb(132, 45, 75)',
  )
  await page.route('**/decision-engine-api/auth/me', (route) =>
    route.fulfill({ status: 401, json: { error: 'expired' } }),
  )
  await (await appFrame(page)).goto('/routing/rules?embed=1')
  await expect(first.getByText('Refreshing your routing session')).toBeVisible()
  await expect.poll(() => first.locator('html').getAttribute('style')).toBe('')
  await expect(second.getByRole('button', { name: 'Create Rule', exact: true })).toHaveCSS(
    'background-color',
    'rgb(132, 45, 75)',
  )
})
