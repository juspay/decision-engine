# Decision Engine Cypress Testing Framework

This directory contains comprehensive end-to-end tests for the Decision Engine routing flows using Cypress. The testing framework is designed to be generic and easily extensible for testing different routing algorithms and scenarios.

## 📁 Directory Structure

```
cypress/
├── e2e/
│   └── routing-flows/
│       ├── gateway-latency-scoring.cy.js    # Gateway latency scoring tests
│       ├── success-rate-routing.cy.js       # Success rate routing tests
├── support/
│   ├── commands.js                          # Custom Cypress commands
│   ├── e2e.js                              # Global test configuration
│   └── test-data-factory.js               # Test data generation utilities
└── README.md                               # This file
```

## 🚀 Getting Started

### Prerequisites

1. **Node.js** (v16 or higher)
2. **Decision Engine Service** running on `http://localhost:8082`
3. **Database** (MySQL/PostgreSQL) properly configured
4. **Redis** instance running

### Installation

1. Install dependencies:
```bash
npm install
```

2. Verify Cypress installation:
```bash
npx cypress verify
```

### Running Tests

#### Interactive Mode (Cypress Test Runner)
```bash
npm run cypress:open
```

#### Headless Mode (CI/CD)
```bash
npm run cypress:run
```

#### Specific Test Suites
```bash
# Gateway latency scoring tests
npm run test:gateway-latency

# Success rate routing tests
npm run test:success-rate

# All routing flow tests
npm run test
```

#### Running Tests with Tags
```bash
# Run only smoke tests
npx cypress run --env grepTags="@smoke"

# Run only gateway latency tests
npx cypress run --env grepTags="@gateway-latency"

# Run performance tests
npx cypress run --env grepTags="@performance"
```

## 🛠️ Custom Commands

The framework provides several custom Cypress commands for easy test creation:

### Merchant Management
```javascript
cy.createMerchantAccount(merchantId)
```

### Rule Configuration
```javascript
cy.createRoutingRule(merchantId, ruleConfig)
cy.createSuccessRateRule(merchantId, options)
cy.createPaymentLatencyRule(merchantId, options)
```

### Gateway Operations
```javascript
cy.decideGateway(decisionRequest)
cy.updateGatewayScore(scoreUpdate)
```

### Complete Flows
```javascript
cy.completeGatewayLatencyFlow(options)
```

### Utility Commands
```javascript
cy.waitForService()
cy.cleanupTestData(merchantId)
```

## 📊 Test Data Factory

The `TestDataFactory` class provides utilities for generating test data:

```javascript
const TestDataFactory = require('../support/test-data-factory')

// Generate unique IDs
const merchantId = TestDataFactory.generateMerchantId()
const paymentId = TestDataFactory.generatePaymentId()

// Get rule configurations
const srRule = TestDataFactory.getSuccessRateRuleConfig({
  successRate: 0.8,
  latencyThreshold: 100
})

// Get test scenarios
const latencyScenarios = TestDataFactory.getLatencyTestScenarios()
const paymentMethods = TestDataFactory.getPaymentMethodTestCases()
```

## ⚙️ Configuration

### Environment Variables

Configure the testing environment in `cypress.config.js`:

```javascript
env: {
  API_BASE_URL: 'http://localhost:8082',
  DEFAULT_GATEWAYS: ['GatewayA', 'GatewayB', 'GatewayC'],
  DEFAULT_AMOUNT: 100.50,
  DEFAULT_CURRENCY: 'USD'
}
```

### Override via Command Line
```bash
npx cypress run --env API_BASE_URL=http://localhost:8080
```

## 🏷️ Test Tags

Tests are organized using tags for easy filtering:

- `@smoke` - Critical path tests
- `@gateway-latency` - Gateway latency specific tests
- `@success-rate` - Success rate routing tests
- `@payment-latency` - Payment latency routing tests
- `@performance` - Performance related tests
- `@edge-cases` - Edge case scenarios
- `@routing` - General routing tests

## 📝 Writing New Tests

### 1. Basic Test Structure

```javascript
describe('New Routing Flow', () => {
  let testData = {}

  beforeEach(() => {
    cy.waitForService()
    testData = {
      merchantId: `merc_new_${Date.now()}`,
      paymentId: `PAY_new_${Date.now()}`
    }
  })

  afterEach(() => {
    cy.cleanupTestData(testData.merchantId)
  })

  it('should test new routing logic', { tags: ['@new-feature'] }, () => {
    // Test implementation
  })
})
```

### 2. Using Test Data Factory

```javascript
const TestDataFactory = require('../support/test-data-factory')

it('should test with generated data', () => {
  const merchantId = TestDataFactory.generateMerchantId()
  const ruleConfig = TestDataFactory.getSuccessRateRuleConfig({
    successRate: 0.9
  })
  
  cy.createMerchantAccount(merchantId)
    .then(() => cy.createRoutingRule(merchantId, ruleConfig))
    .then(() => {
      // Continue test
    })
})
```

### 3. Adding New Custom Commands

In `cypress/support/commands.js`:

```javascript
Cypress.Commands.add('newCustomCommand', (parameters) => {
  return cy.request({
    method: 'POST',
    url: `${getApiBaseUrl()}/new-endpoint`,
    body: parameters
  }).then((response) => {
    expect(response.status).to.eq(200)
    return cy.wrap(response.body)
  })
})
```

## 🔧 Extending for New Routing Logic

To add tests for a new routing algorithm:

### 1. Create New Test File
```bash
touch cypress/e2e/routing-flows/new-algorithm-routing.cy.js
```

### 2. Add Rule Configuration Helper
In `cypress/support/commands.js`:
```javascript
Cypress.Commands.add('createNewAlgorithmRule', (merchantId, options = {}) => {
  const ruleConfig = {
    type: "newAlgorithm",
    data: {
      // Algorithm specific configuration
    }
  }
  return cy.createRoutingRule(merchantId, ruleConfig)
})
```

### 3. Add Test Data Factory Methods
In `cypress/support/test-data-factory.js`:
```javascript
static getNewAlgorithmRuleConfig(options = {}) {
  return {
    type: "newAlgorithm",
    data: {
      // Default configuration
    }
  }
}
```

### 4. Update Package.json Scripts
```json
{
  "scripts": {
    "test:new-algorithm": "cypress run --spec 'cypress/e2e/routing-flows/new-algorithm-routing.cy.js'"
  }
}
```

## 🐛 Debugging

### 1. Enable Debug Logs
```bash
DEBUG=cypress:* npm run cypress:run
```

### 2. Screenshots and Videos
- Screenshots are automatically taken on test failures
- Videos can be enabled in `cypress.config.js`

### 3. Browser DevTools
When running in interactive mode, use browser DevTools to inspect network requests and responses.

## 📈 Performance Testing

The framework includes utilities for performance testing:

```javascript
const performanceConfig = TestDataFactory.getPerformanceTestConfig()
const loadTestData = TestDataFactory.generateLoadTestData(100)

// Use in tests for load testing scenarios
```

## 🚨 Best Practices

1. **Use unique test data** - Always generate unique merchant IDs and payment IDs
2. **Clean up after tests** - Use `afterEach` hooks to clean up test data
3. **Wait for service readiness** - Always call `cy.waitForService()` in `beforeEach`
4. **Use descriptive test names** - Make test purposes clear from the name
5. **Tag tests appropriately** - Use tags for easy test filtering
6. **Verify responses** - Always validate API response structure and data
7. **Log important data** - Use `cy.log()` for debugging information

## 🤝 Contributing

When adding new tests:

1. Follow the existing file structure and naming conventions
2. Add appropriate tags to new tests
3. Update this README if adding new features
4. Ensure tests are independent and can run in any order
5. Add test data factory methods for reusable test data

---

**Happy Testing! 🎉**
