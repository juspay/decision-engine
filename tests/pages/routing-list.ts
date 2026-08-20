import { type Page, type Locator } from '@playwright/test'

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
    await this.ruleRow(name).getByRole('button', { name: 'Rule actions' }).click()
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
    await this.openRuleMenu(name)
    await this.menuItem(action).click()
  }
}
