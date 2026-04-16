describe('Volume Split Routing Form', () => {
  it('submits gateway_id values when creating a volume split rule', { tags: ['@volume-split', '@routing', '@frontend'] }, () => {
    const merchantId = `merc_volume_${Date.now()}`

    cy.intercept('POST', '**/merchant-account/create', {
      statusCode: 200,
      body: {},
    }).as('createMerchant')

    cy.intercept('POST', `**/routing/list/active/${merchantId}`, {
      statusCode: 200,
      body: [],
    }).as('listActiveRouting')

    cy.intercept('POST', `**/routing/list/${merchantId}`, {
      statusCode: 200,
      body: [],
    }).as('listRouting')

    cy.intercept('POST', '**/routing/create', (req) => {
      expect(req.body.algorithm.type).to.eq('volume_split')
      expect(req.body.algorithm.data).to.have.length(2)
      expect(req.body.algorithm.data[0].output.gateway_name).to.eq('stripe')
      expect(req.body.algorithm.data[0].output.gateway_id).to.eq('gw_stripe_01')
      expect(req.body.algorithm.data[1].output.gateway_name).to.eq('adyen')
      expect(req.body.algorithm.data[1].output.gateway_id).to.eq('gw_adyen_02')

      req.reply({
        statusCode: 200,
        body: {
          id: 'routing_algo_1',
        },
      })
    }).as('createRoutingRule')

    cy.visit('/routing/volume')

    cy.get('input[placeholder="Set Merchant ID"]').type(`${merchantId}{enter}`)
    cy.wait('@createMerchant')
    cy.wait('@listActiveRouting')
    cy.wait('@listRouting')

    cy.get('input[placeholder="e.g. ab-test-split"]').type('volume-split-with-gateway-id')

    cy.get('input[placeholder="e.g. stripe"]').eq(0).clear().type('stripe')
    cy.get('input[placeholder="optional gateway_id"]').eq(0).clear().type('gw_stripe_01')
    cy.get('input[type="number"]').eq(0).clear().type('60')

    cy.get('input[placeholder="e.g. stripe"]').eq(1).clear().type('adyen')
    cy.get('input[placeholder="optional gateway_id"]').eq(1).clear().type('gw_adyen_02')
    cy.get('input[type="number"]').eq(1).clear().type('40')

    cy.contains('button', 'Create Rule').click()

    cy.wait('@createRoutingRule')
    cy.contains('Rule "volume-split-with-gateway-id" created successfully').should('be.visible')
  })
})
