import { type Page, type Locator } from '@playwright/test'
import { RoutingListPage } from './routing-list'

/**
 * Page object for the volume split pages — the list at /routing/volume and the builder at
 * /routing/volume/new and /routing/volume/:id/edit. Row and ⋮ menu handling comes from
 * `RoutingListPage`, shared with the rule-based list.
 */
export class VolumeSplitBuilder extends RoutingListPage {
  constructor(page: Page) {
    super(page)
  }

  /** Navigate to the builder. */
  async goto(path = '/routing/volume/new'): Promise<void> {
    await this.page.goto(path)
  }

  /** Navigate to the list of saved volume split rules. */
  async gotoList(): Promise<void> {
    await this.page.goto('/routing/volume')
  }

  /** The nth gateway row in the builder's distribution table. */
  gatewayRow(index: number): Locator {
    return this.page.locator('input[placeholder="e.g. stripe"]').nth(index)
  }

  /** Fill one gateway row. The last row's split is auto-computed, so it takes no percentage. */
  async setGateway(index: number, name: string, gatewayId = '', split?: number): Promise<void> {
    await this.page.locator('input[placeholder="e.g. stripe"]').nth(index).fill(name)
    if (gatewayId) {
      await this.page.locator('input[placeholder="optional gateway_id"]').nth(index).fill(gatewayId)
    }
    if (split !== undefined) {
      await this.page.getByLabel(`${name} split percentage`).fill(String(split))
    }
  }

  async addGateway(): Promise<void> {
    await this.page.getByRole('button', { name: 'Add Gateway' }).click()
  }
}
