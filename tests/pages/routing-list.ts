import { expect, type Page, type Locator } from '@playwright/test'

/**
 * Shared behaviour of the routing list tables (`/routing/rules`, `/routing/volume`).
 *
 * Both render rows with the same `data-testid`/`data-rule-name` hooks and the same ⋮ action menu,
 * so the row and menu handling lives here rather than being copied per page object.
 */
export class RoutingListPage {
  constructor(protected readonly page: Page) {}

  /** A row in the rules list, addressed by the rule's name rather than its DOM shape. */
  ruleRow(name: string): Locator {
    return this.page.locator(`[data-testid="rule-row"][data-rule-name="${name}"]`)
  }

  /** Open a row's action menu. The panel is portalled to <body>, so it is NOT inside the row. */
  async openRuleMenu(name: string): Promise<void> {
    const trigger = this.ruleRow(name).getByRole('button', { name: 'Rule actions' })
    // The menu is anchored to the trigger's rect and closes itself on any scroll or resize, so a
    // list re-render arriving just after the click can dismiss it. Re-open until it stays up.
    await expect(async () => {
      await trigger.click()
      await expect(this.page.getByRole('menu')).toBeVisible({ timeout: 1_000 })
    }).toPass({ timeout: 15_000 })
  }

  /**
   * An item in the currently open row menu. Exact matching matters here — a substring match on
   * "Activate" would also hit "Deactivate".
   */
  menuItem(action: string): Locator {
    return this.page.getByRole('menuitem', { name: action, exact: true })
  }

  /** Open a row's menu and run one action. */
  async ruleAction(name: string, action: string): Promise<void> {
    const item = this.menuItem(action)
    // Same reason as openRuleMenu: if the menu is dismissed between opening it and clicking the
    // item, re-open and try again rather than waiting out the action timeout on a gone element.
    await expect(async () => {
      if (!(await item.isVisible())) await this.openRuleMenu(name)
      await item.click({ timeout: 2_000 })
    }).toPass({ timeout: 20_000 })
  }
}
