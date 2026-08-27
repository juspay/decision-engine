import { test, expect, factory } from '../../fixtures/test'
import { VolumeSplitBuilder } from '../../pages/volume-page'
import { expectApiCall } from '../../helpers/network'

/**
 * The volume split pages: the list table at /routing/volume and the builder at /routing/volume/new
 * and /:id/edit — the same split the rule-based pages use.
 *
 * Every test here mutates merchant state (creating, activating or deleting a rule), so they use the
 * per-test `merchant` fixture rather than the worker-scoped one.
 */
test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Volume Split', () => {
  let volume: VolumeSplitBuilder
  let ruleName: string

  test.beforeEach(async ({ authedPage }) => {
    ruleName = factory.ruleName('vol_rule')
    volume = new VolumeSplitBuilder(authedPage)
  })

  test.describe('Builder', () => {
    test.beforeEach(async () => {
      await volume.goto()
    })

    test('shows the builder form and its live distribution rail', async ({ authedPage }) => {
      await expect(authedPage.getByRole('heading', { name: 'Create Volume Split Rule' })).toBeVisible()
      await expect(authedPage.getByPlaceholder('e.g. ab-test-split')).toBeVisible()
      await expect(authedPage.getByText('Rule details')).toBeVisible()
    })

    test('auto-computes the last split so the total stays 100', async ({ authedPage }) => {
      await volume.setGateway(0, 'stripe', 'mca_stripe', 70)

      // The final row absorbs the remainder rather than being typed in.
      await expect(authedPage.getByText('Total: 100%')).toBeVisible()
    })

    test('creates a rule and returns to the list', async ({ authedPage }) => {
      await authedPage.getByPlaceholder('e.g. ab-test-split').fill(ruleName)
      await volume.setGateway(0, 'stripe', 'mca_stripe')
      await volume.setGateway(1, 'adyen', 'mca_adyen')

      const call = expectApiCall(authedPage, '/routing/create')
      await authedPage.getByRole('button', { name: 'Create Rule' }).click()
      const { status, requestBody, body } = await call
      expect(status, `POST /routing/create failed: ${JSON.stringify(body)}`).toBe(200)
      expect(requestBody.algorithm.type).toBe('volume_split')
      expect(requestBody.algorithm.data).toHaveLength(2)

      await expect(authedPage.getByRole('heading', { name: 'Volume Split Routing' })).toBeVisible()
      await expect(volume.ruleRow(ruleName)).toBeVisible()
    })

    test('blocks a rule with no name', async ({ authedPage }) => {
      await volume.setGateway(0, 'stripe')
      await volume.setGateway(1, 'adyen')

      await authedPage.getByRole('button', { name: 'Create Rule' }).click()

      await expect(authedPage.getByText('Enter a rule name')).toBeVisible()
    })

    test('returns to the list via Cancel', async ({ authedPage }) => {
      await authedPage.getByRole('button', { name: 'Cancel' }).click()

      await expect(authedPage.getByRole('heading', { name: 'Volume Split Routing' })).toBeVisible()
    })
  })

  test.describe('Rules list', () => {
    test.beforeEach(async ({ authedPage }) => {
      // Seed through the UI so the row under test matches what the builder produces.
      await volume.goto()
      await authedPage.getByPlaceholder('e.g. ab-test-split').fill(ruleName)
      await volume.setGateway(0, 'stripe', 'mca_stripe')
      await volume.setGateway(1, 'adyen', 'mca_adyen')
      const call = expectApiCall(authedPage, '/routing/create')
      await authedPage.getByRole('button', { name: 'Create Rule' }).click()
      expect((await call).status).toBe(200)
      await expect(volume.ruleRow(ruleName)).toBeVisible()
    })

    test('shows the split distribution in the row', async () => {
      await expect(volume.ruleRow(ruleName).getByText(/stripe \d+% \/ adyen \d+%/)).toBeVisible()
      await expect(volume.ruleRow(ruleName).getByText('Inactive')).toBeVisible()
    })

    test('expands the row to the distribution breakdown', async ({ authedPage }) => {
      await volume.ruleRow(ruleName).click()

      await expect(authedPage.getByText('mca_stripe')).toBeVisible()
    })

    test('activates and deactivates the rule', async ({ authedPage }) => {
      await volume.ruleAction(ruleName, 'Activate')
      await expect(authedPage.getByText('Rule activated.')).toBeVisible()
      await expect(volume.ruleRow(ruleName).getByText('Active', { exact: true })).toBeVisible()

      await volume.ruleAction(ruleName, 'Deactivate')
      await expect(authedPage.getByText('Deactivate this rule?')).toBeVisible()
      await authedPage.locator('.fixed.inset-0').getByRole('button', { name: 'Deactivate' }).click()

      await expect(authedPage.getByText('Rule deactivated.')).toBeVisible()
      await expect(volume.ruleRow(ruleName).getByText('Inactive')).toBeVisible()
    })

    test('blocks Edit and Delete while the rule is active', async ({ authedPage }) => {
      await volume.ruleAction(ruleName, 'Activate')
      await expect(authedPage.getByText('Rule activated.')).toBeVisible()

      // /routing/update and /routing/delete both reject an active algorithm.
      await volume.openRuleMenu(ruleName)
      await expect(volume.menuItem('Edit')).toBeDisabled()
      await expect(volume.menuItem('Delete')).toBeDisabled()
    })

    test('edits an inactive rule and sends the update to the backend', async ({ authedPage }) => {
      const renamed = `${ruleName}-edited`

      await volume.ruleAction(ruleName, 'Edit')
      await expect(authedPage.getByRole('heading', { name: 'Edit Volume Split Rule' })).toBeVisible()
      await expect(authedPage.getByPlaceholder('e.g. ab-test-split')).toHaveValue(ruleName)

      await authedPage.getByPlaceholder('e.g. ab-test-split').fill(renamed)
      const call = expectApiCall(authedPage, '/routing/update')
      await authedPage.getByRole('button', { name: 'Save Changes' }).click()
      const { status, requestBody, body } = await call
      expect(status, `POST /routing/update failed: ${JSON.stringify(body)}`).toBe(200)
      expect(requestBody.name).toBe(renamed)
      expect(requestBody.algorithm.type).toBe('volume_split')

      await expect(volume.ruleRow(renamed)).toBeVisible()
    })

    test('duplicates a rule into the builder', async ({ authedPage }) => {
      await volume.ruleAction(ruleName, 'Duplicate')

      await expect(authedPage.getByRole('heading', { name: 'Create Volume Split Rule' })).toBeVisible()
      await expect(authedPage.getByPlaceholder('e.g. ab-test-split')).toHaveValue(`copy-of-${ruleName}`)
    })

    test('deletes an inactive rule', async ({ authedPage }) => {
      await volume.ruleAction(ruleName, 'Delete')

      await expect(authedPage.getByText('Delete this rule?')).toBeVisible()
      await authedPage.locator('.fixed.inset-0').getByRole('button', { name: 'Delete' }).click()

      await expect(volume.ruleRow(ruleName)).toHaveCount(0)
    })

    test('filters the list by status from the column header', async ({ authedPage }) => {
      await authedPage.getByLabel('Filter by status').click()
      await authedPage.getByRole('button', { name: 'Active', exact: true }).click()

      await expect(volume.ruleRow(ruleName)).toHaveCount(0)
      await expect(authedPage.getByText('No rules match these filters.')).toBeVisible()

      await authedPage.getByRole('button', { name: 'Clear filters' }).click()

      await expect(volume.ruleRow(ruleName)).toBeVisible()
    })
  })
})
