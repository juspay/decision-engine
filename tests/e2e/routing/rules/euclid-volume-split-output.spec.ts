import { test, expect } from '../../../fixtures/test'
import { EuclidRuleBuilder } from '../../../pages/euclid-page'

/**
 * Port of cypress/e2e/ui/euclid-rules-volume-split.cy.js.
 *
 * The volume-split OUTPUT MODE inside the rule builder at /routing/rules — distinct from the standalone
 * /routing/volume page, which volume-split-page.spec.ts covers. Hence the `-output` suffix.
 *
 * The behaviour that matters is the 100% guard: a split that doesn't total 100 is a misconfigured rule
 * that would silently drop traffic, so the editor has to say so before the rule can be created.
 * Pure UI, so this runs on the worker-scoped merchant.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Volume split output', () => {
  let euclid: EuclidRuleBuilder

  test.beforeEach(async ({ sharedPage }) => {
    euclid = new EuclidRuleBuilder(sharedPage)
    await euclid.goto('/routing/rules/new')
    await euclid.addRuleBlock()
    await euclid.switchOutputType(0, 'Volume Split')
  })

  test('switches the THEN section to volume split mode', async () => {
    const then = euclid.thenSection(0)

    await expect(then.getByPlaceholder('Split %')).toBeVisible()
    await expect(then.getByPlaceholder('Gateway name')).toBeVisible()
    // Priority numbering disappears in split mode.
    await expect(then.getByText('1.', { exact: true })).toHaveCount(0)
  })

  test('adds a volume split entry and shows split % with gateway name', async () => {
    await euclid.addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')

    const then = euclid.thenSection(0)
    await expect(then.getByText('60%').first()).toBeVisible()
    await expect(then.getByText('stripe').first()).toBeVisible()
  })

  test('shows a running total after adding entries', async () => {
    await euclid.addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')

    await expect(euclid.thenSection(0).getByText('Total: 60%')).toBeVisible()
  })

  test('shows a warning when the total is not 100%', async () => {
    await euclid.addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')

    await expect(euclid.thenSection(0).getByText('must equal 100%')).toBeVisible()
  })

  test('shows a success indicator when the total reaches exactly 100%', async () => {
    await euclid.addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')
    await euclid.addVolumeSplitEntry(0, 40, 'adyen', 'mca_adyen')

    const then = euclid.thenSection(0)
    await expect(then.getByText('Total: 100%')).toBeVisible()
    await expect(then.getByText('✓')).toBeVisible()
    await expect(then.getByText('must equal 100%')).toHaveCount(0)
  })

  test('removes a split entry via its delete button', async ({ sharedPage }) => {
    await euclid.addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')
    await euclid.addVolumeSplitEntry(0, 40, 'adyen', 'mca_adyen')

    const then = euclid.thenSection(0)
    // Targeted by accessible name rather than button order: a row also carries an edit control,
    // so "the first button in the row" is not the delete one.
    await then.getByRole('button', { name: 'Remove stripe' }).click()

    await expect(then.getByText('stripe')).toHaveCount(0)
    await expect(then.getByText('Total: 40%')).toBeVisible()
  })

  test('switching back to Priority mode hides the split editor', async () => {
    await euclid.addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')

    await euclid.switchOutputType(0, 'Priority')

    const then = euclid.thenSection(0)
    await expect(then.getByPlaceholder('Split %')).toHaveCount(0)
    await expect(then.getByPlaceholder('Gateway name')).toBeVisible()
  })
})
