import { expect, type Page, type Locator } from '@playwright/test'
import { RoutingListPage } from './routing-list'

/**
 * Page object for the Euclid rule builder — Playwright port of cypress/support/euclid-helpers.js.
 * The selectors are carried over verbatim (data-cy hooks, placeholders, the portal-rendered
 * SearchableSelect); only the transport changes from `cy.*` to Playwright locators. Cypress's
 * `.within()` scoping becomes chained locators, and the `{withinSubject: null}` portal trick becomes
 * a plain page-level locator (the dropdown renders at the document root).
 */
export class EuclidRuleBuilder extends RoutingListPage {
  constructor(page: Page) {
    super(page)
  }

  /** Navigate to the rule builder and wait for the routing-key fetch to settle. */
  async goto(path = '/routing/rules/new'): Promise<void> {
    await this.page.goto(path)
    await this.waitUntilReady()
  }

  /** Navigate to the rules list (the table of saved rules), which has no routing-key gate. */
  async gotoList(): Promise<void> {
    await this.page.goto('/routing/rules')
  }


  /**
   * The builder renders a "Loading routing keys from backend..." placeholder until GET
   * /config/routing-keys resolves. Every condition interaction depends on those keys, so each spec
   * gates on this first.
   */
  async waitUntilReady(): Promise<void> {
    await expect(this.page.getByText('Loading routing keys from backend...')).toHaveCount(0, {
      timeout: 15_000,
    })
  }

  async addRuleBlock(): Promise<void> {
    await this.page.getByRole('button', { name: 'Add Rule', exact: true }).click()
  }

  /** The nth nested (OR) branch inside a rule block — the sky-bordered, indented group. */
  nestedBranch(blockIndex: number, index: number): Locator {
    return this.ruleBlock(blockIndex).locator('.border-l-2.border-sky-200').nth(index)
  }

  /** All nested branches in a rule block, for count assertions. */
  nestedBranches(blockIndex: number): Locator {
    return this.ruleBlock(blockIndex).locator('.border-l-2.border-sky-200')
  }

  async addNestedBranch(blockIndex: number): Promise<void> {
    await this.ruleBlock(blockIndex).getByRole('button', { name: 'Add nested branch' }).click()
  }

  /** Scope to the nth rule block (0-indexed). */
  ruleBlock(index = 0): Locator {
    return this.page
      .locator('.rounded-xl.overflow-hidden')
      .filter({ has: this.page.locator('input[placeholder="Rule name"]') })
      .nth(index)
  }

  /** THEN section of the nth rule block, regardless of current output type. */
  thenSection(blockIndex = 0): Locator {
    return this.ruleBlock(blockIndex).locator('[data-cy="then-section"]')
  }

  async setRuleName(index: number, name: string): Promise<void> {
    await this.page.locator('input[placeholder="Rule name"]').nth(index).fill(name)
  }

  /** typeLabel: 'Priority' | 'Volume Split' | 'Split + Priority' */
  async switchOutputType(blockIndex: number, typeLabel: string): Promise<void> {
    await this.thenSection(blockIndex).getByRole('button', { name: typeLabel, exact: true }).click()
  }

  async addGatewayToBlock(blockIndex: number, gatewayName: string, gatewayId = ''): Promise<void> {
    const then = this.thenSection(blockIndex)
    await then.locator('input[placeholder="Gateway name"]').fill(gatewayName)
    if (gatewayId) await then.locator('input[placeholder="Gateway ID (optional)"]').fill(gatewayId)
    await then.getByRole('button', { name: 'Add', exact: true }).click()
  }

  async addVolumeSplitEntry(blockIndex: number, split: number, gatewayName: string, gatewayId = ''): Promise<void> {
    const then = this.thenSection(blockIndex)
    await then.locator('input[placeholder="Split %"]').fill(String(split))
    await then.locator('input[placeholder="Gateway name"]').fill(gatewayName)
    if (gatewayId) await then.locator('input[placeholder="Gateway ID (optional)"]').fill(gatewayId)
    await then.getByRole('button', { name: 'Add', exact: true }).click()
  }

  async addVolumeSplitPriorityRow(blockIndex: number, split: number): Promise<void> {
    const then = this.thenSection(blockIndex)
    await then.locator('input[placeholder="Split %"]').fill(String(split))
    await then.getByRole('button', { name: 'Add split', exact: true }).click()
  }

  async addGatewayToSplitRow(blockIndex: number, rowIndex: number, gatewayName: string, gatewayId = ''): Promise<void> {
    const row = this.thenSection(blockIndex)
      .locator('[class*="p-3"]')
      .filter({ has: this.page.locator('p', { hasText: 'Priority list for this split' }) })
      .nth(rowIndex)
    await row.locator('input[placeholder="Gateway name"]').fill(gatewayName)
    if (gatewayId) await row.locator('input[placeholder="Gateway ID (optional)"]').fill(gatewayId)
    await row.getByRole('button', { name: 'Add', exact: true }).click()
  }

  async addFallbackGateway(gatewayName: string, gatewayId = ''): Promise<void> {
    const section = this.page
      .locator('.rounded-xl')
      .filter({ has: this.page.locator('p', { hasText: 'Default Fallback' }) })
    await section.locator('input[placeholder="Gateway name"]').fill(gatewayName)
    if (gatewayId) await section.locator('input[placeholder="Gateway ID (optional)"]').fill(gatewayId)
    await section.getByRole('button', { name: 'Add', exact: true }).click()
  }

  /**
   * Select a routing-key (LHS) from the SearchableSelect dropdown (portal-rendered).
   *
   * `scope` restricts the index to one container — the equivalent of Cypress's `.within()`. Pass a
   * nested branch (see `nestedBranch`) when addressing a condition inside one, because page-wide
   * indices shift depending on what the other conditions look like.
   */
  async selectCondLhs(index: number, value: string, scope?: Locator): Promise<void> {
    await (scope ?? this.page).locator('[data-cy="cond-lhs"] button.cond-select').nth(index).click()
    await this.selectFromPortal(value)
  }

  /**
   * Select an enum value from the SearchableSelect value dropdown (portal-rendered).
   *
   * Numeric keys (e.g. `amount`) render a plain number input rather than a `cond-val` button, so a
   * page-wide index here is NOT the same as the matching `selectCondLhs` index. Prefer passing a
   * `scope`.
   */
  async selectCondVal(index: number, value: string, scope?: Locator): Promise<void> {
    await (scope ?? this.page).locator('[data-cy="cond-val"] button.cond-select').nth(index).click()
    await this.selectFromPortal(value)
  }

  /** Select multiple enum values in the SearchableMultiSelect (skips already-checked options). */
  async selectMultiCondVals(index: number, values: string[]): Promise<void> {
    await this.page.locator('[data-cy="cond-val"]').nth(index).click()
    const search = this.page.locator('input[placeholder="Search…"]')
    for (const value of values) {
      await search.fill(value)
      const opt = this.page.locator(`button[data-value="${value}"]:not(.cond-select)`).first()
      await opt.waitFor({ state: 'visible' })
      const cls = (await opt.getAttribute('class')) || ''
      // text-brand-600 marks the checked state — skip so we don't deselect it.
      if (!cls.includes('text-brand-600')) await opt.click({ force: true })
      await search.focus()
    }
    await this.dismissDropdown()
  }

  /**
   * Close an open SearchableSelect / SearchableMultiSelect.
   *
   * Not a body click: the dropdown is portalled to <body>, so a click at the body's centre lands on
   * whichever option happens to sit under that point and silently toggles it.
   */
  async dismissDropdown(): Promise<void> {
    await this.page.keyboard.press('Escape')
  }

  private async selectFromPortal(value: string): Promise<void> {
    const search = this.page.locator('input[placeholder="Search…"]')
    await search.waitFor({ state: 'visible' })
    await search.fill(value)
    await this.page.locator(`button[data-value="${value}"]:not(.cond-select)`).first().click()
  }
}
