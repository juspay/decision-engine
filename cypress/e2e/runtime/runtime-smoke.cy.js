describe('Runtime Surface Smoke', () => {
  beforeEach(() => {
    cy.waitForRuntimeSurface()
  })

  it('serves docs and exposes the expected runtime context', () => {
    cy.runtimeContext().then((context) => {
      expect(context.runtimeMode).to.be.oneOf(['source', 'docker', 'manual'])
      expect(context.apiBaseUrl).to.be.a('string').and.not.be.empty
      expect(context.uiBaseUrl).to.be.a('string').and.not.be.empty
      expect(context.docsBaseUrl).to.be.a('string').and.not.be.empty
    })

    cy.fetchDocsPage('/introduction').then((response) => {
      expect(response.status).to.eq(200)
      expect(response.body).to.include('Decision Engine')
    })

    cy.fetchDocsPage('/api-reference').then((response) => {
      expect(response.status).to.eq(200)
      expect(response.body).to.include('Decision Engine')
    })

    cy.fetchDocsPage('/api-reference/endpoint/healthCheck').then((response) => {
      expect(response.status).to.eq(200)
      expect(response.body.toLowerCase()).to.include('health')
    })
  })
})
