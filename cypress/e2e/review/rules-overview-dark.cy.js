describe('Review Evidence - Rules Overview Black/Matte Theme', () => {
  it('captures the main rule-based routing overview page in dark theme', () => {
    cy.visit('/dashboard/routing/rules', {
      onBeforeLoad(win) {
        win.localStorage.setItem('theme', 'dark')
        win.localStorage.setItem('merchant-id', 'review_merchant')
      },
    })

    cy.contains('h1', 'Rule-Based Routing', { timeout: 20000 }).should('be.visible')
    cy.get('html').should('have.class', 'dark')

    cy.screenshot('review_plan_18-rules-overview-black-matte', {
      capture: 'fullPage',
      overwrite: true,
    })
  })
})
