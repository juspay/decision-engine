# Universal Adapter Pattern for Domain Extensions

This document outlines a systematic approach for extending the unified framework to any domain through a standardized adapter pattern, ensuring consistent architecture while accommodating domain-specific needs.

## The Universal Adapter Pattern

The universal adapter pattern provides a standardized interface between the core unified framework and domain-specific implementations. This approach enables:

1. **Consistent Architecture**: Maintain architectural integrity across all domains
2. **Domain Independence**: Core framework remains unaware of domain specifics
3. **Reusable Components**: Share common functionality across domains
4. **Predictable Extensions**: Standardized approach to adding new domains

![Universal Adapter Pattern Diagram](https://mermaid.ink/img/pako:eNp1kk1PwzAMhv9KlBMIaRu0fZ1QJw5IcOAw7UC5NF4XkTZR4oBWVf8723RlgwhK_Np-H8dJDqA9RmAwoUP0gkcFhYMoZSfFlh4iT1wLW6Yo9-KcUHNL0VG0OIKnlE7Y0yW3NFFO7qjSJAtx4j4hfRGHB7mVGvHnGvJYWGQ5KbPFUxWyKxPF9SOiSHLmJpvsnfk8pXDSw-9SvV0lnDa3c5X1hPXpEsfnaeYXTbpU7ry0OL0oyhCzXppHs2uxrR-9AkXBg8Etwi3QpjVVB9Qoz-SBUa7CiTU9o14yWTb1jy-YsXaFNJDJNngLhdmBZq7Oaz4j3fxWTx8Y5A6-1d5S1fVPdD1t9r9q-ubhVuM2FE0nV9sK2OBFhbDRME4Q1RRWr7AGM7mGxQaWg48e9mCd9AOWnj1wdrRYNFh7G2JRjnzTKvDXh_0oOA1-2Hvr-QfdWKo9)

## Core Unified Framework Components

These components remain domain-agnostic and form the foundation of the architecture:

1. **Rule Engine**: Orchestrates the evaluation of filters, comparators, and output processing
2. **Working Memory**: Stores facts and intermediate results
3. **Configuration System**: Manages algorithm definitions and settings
4. **Metrics Collection**: Records performance data across components

## Domain Adapter Interface

Each domain adapter must implement the following standard interfaces:

```java
// Core interfaces that domain adapters must implement

interface DomainEntityAdapter {
    void insertFacts(WorkingMemory memory, Object domainRequest);
    Object translateResult(WorkingMemory memory, DecisionResult result);
}

interface FilterAdapter {
    void registerFilters(RuleRegistry registry);
    List<String> getStandardFilterTypes();
}

interface ComparatorAdapter {
    void registerComparators(RuleRegistry registry);
    List<String> getStandardComparatorTypes();
}

interface OutputProcessorAdapter {
    void registerOutputProcessors(RuleRegistry registry);
    List<String> getStandardOutputProcessorTypes();
}

interface FeedbackAdapter {
    void processFeedback(Object domainFeedback);
    void updateMetrics(WorkingMemory memory);
}
```

## Domain Adapter Implementation Pattern

```java
// Example adapter implementation for the cab service domain

public class CabServiceDomainAdapter implements DomainAdapter {
    private final DomainEntityAdapter entityAdapter;
    private final FilterAdapter filterAdapter;
    private final ComparatorAdapter comparatorAdapter;
    private final OutputProcessorAdapter outputAdapter;
    private final FeedbackAdapter feedbackAdapter;
    
    public CabServiceDomainAdapter() {
        this.entityAdapter = new CabServiceEntityAdapter();
        this.filterAdapter = new CabServiceFilterAdapter();
        this.comparatorAdapter = new CabServiceComparatorAdapter();
        this.outputAdapter = new CabServiceOutputAdapter();
        this.feedbackAdapter = new CabServiceFeedbackAdapter();
    }
    
    @Override
    public void initialize(RuleRegistry registry) {
        filterAdapter.registerFilters(registry);
        comparatorAdapter.registerComparators(registry);
        outputAdapter.registerOutputProcessors(registry);
    }
    
    @Override
    public void insertFacts(WorkingMemory memory, Object request) {
        entityAdapter.insertFacts(memory, request);
    }
    
    @Override
    public Object processResult(WorkingMemory memory, DecisionResult result) {
        return entityAdapter.translateResult(memory, result);
    }
    
    @Override
    public void handleFeedback(Object feedback) {
        feedbackAdapter.processFeedback(feedback);
    }
}
```

## Configuration Schema

Domain-specific components are defined in a standard JSON schema:

```json
{
  "domain_adapter": {
    "id": "cab_service_adapter",
    "implementation": "com.framework.adapters.CabServiceDomainAdapter",
    "config": {
      "entity_mappings": {
        "ride_request": "RideRequest",
        "driver": "Driver",
        "rider": "Rider"
      },
      "fact_types": [
        "com.cabservice.facts.RideRequest",
        "com.cabservice.facts.Driver",
        "com.cabservice.facts.Rider",
        "com.cabservice.facts.Vehicle"
      ]
    }
  },
  "filter_components": [
    {
      "id": "vehicle_type_filter",
      "implementation": "com.cabservice.filters.VehicleTypeFilter",
      "config_schema": {
        "vehicle_type_mapping": "object",
        "fallback_allowed": "boolean"
      }
    },
    // Additional filter components...
  ],
  "comparator_components": [
    {
      "id": "distance_comparator",
      "implementation": "com.cabservice.comparators.DistanceComparator",
      "config_schema": {
        "use_actual_route_distance": "boolean",
        "traffic_conditions_factor": "boolean",
        "max_deviation_factor": "number"
      }
    },
    // Additional comparator components...
  ],
  "output_processor_components": [
    {
      "id": "nearest_driver_processor",
      "implementation": "com.cabservice.output.NearestDriverProcessor",
      "config_schema": {
        "timeout_seconds": "number",
        "max_driver_requests": "number",
        "sequential_offering": "boolean"
      }
    },
    // Additional output processor components...
  ]
}
```

## Universal Adapter Registry

The framework maintains a registry of domain adapters:

```java
public class DomainAdapterRegistry {
    private final Map<String, DomainAdapter> adapters = new HashMap<>();
    
    public void registerAdapter(String domainType, DomainAdapter adapter) {
        adapters.put(domainType, adapter);
        adapter.initialize(RuleRegistry.getInstance());
    }
    
    public DomainAdapter getAdapter(String domainType) {
        return adapters.get(domainType);
    }
    
    public Set<String> getSupportedDomains() {
        return adapters.keySet();
    }
}
```

## Step-by-Step Extension Methodology

To extend the framework to a new domain:

### 1. Domain Analysis

Start by analyzing the new domain to identify:

- Core entities (e.g., rides, drivers, orders, delivery partners)
- Decision factors (distance, rating, time, cost)
- Output requirements (assignments, rankings, batches)
- Feedback mechanisms (ratings, success/failure metrics)

### 2. Entity Mapping

Map domain entities to framework concepts:

| Framework Concept | Payment Domain | Cab Service | Food Delivery | New Domain |
|-------------------|---------------|-------------|---------------|------------|
| Service Providers | Gateways | Drivers | Delivery Partners | ? |
| Service Recipients | Merchants | Riders | Customers | ? |
| Service Requests | Transactions | Ride Requests | Orders | ? |
| Selection Criteria | Success Rates | Distance/Rating | Time/Rating | ? |

### 3. Component Development

Develop domain-specific components following the standard interfaces:

```java
// Example of domain-specific filter implementation
public class NewDomainSpecificFilter implements Filter {
    private final FilterConfig config;
    
    public NewDomainSpecificFilter(FilterConfig config) {
        this.config = config;
    }
    
    @Override
    public void apply(WorkingMemory memory) {
        // Domain-specific filtering logic
        List<ServiceProvider> providers = memory.getFacts(ServiceProvider.class);
        List<ServiceProvider> filtered = providers.stream()
            .filter(this::meetsFilterCriteria)
            .collect(Collectors.toList());
        
        memory.insertFact(new FilteredProviders(filtered));
    }
    
    private boolean meetsFilterCriteria(ServiceProvider provider) {
        // Domain-specific criteria evaluation
        return true; // Simplified for example
    }
}
```

### 4. Configuration Definition

Define the JSON configurations for the new domain:

```json
{
  "routing_algorithm": {
    "id": "new_domain_algorithm",
    "filters": [
      {
        "id": "domain_specific_filter",
        "config": {
          "parameter1": "value1",
          "parameter2": "value2"
        }
      }
    ],
    "comparators": [
      {
        "id": "domain_specific_comparator",
        "weight": 0.7,
        "config": {
          "parameter1": "value1"
        }
      }
    ],
    "output_processor": {
      "id": "domain_specific_processor",
      "config": {
        "parameter1": "value1"
      }
    }
  }
}
```

### 5. Adapter Implementation

Implement the domain adapter interfaces:

```java
public class NewDomainEntityAdapter implements DomainEntityAdapter {
    @Override
    public void insertFacts(WorkingMemory memory, Object domainRequest) {
        // Convert domain-specific request to facts
        NewDomainRequest request = (NewDomainRequest) domainRequest;
        
        // Insert primary request fact
        memory.insertFact(new ServiceRequest(request.getId(), request.getParameters()));
        
        // Insert available service providers
        for (Provider provider : fetchAvailableProviders(request)) {
            memory.insertFact(new ServiceProvider(provider));
        }
        
        // Insert additional context facts
        memory.insertFact(new RequestContext(request.getContext()));
    }
    
    @Override
    public Object translateResult(WorkingMemory memory, DecisionResult result) {
        // Convert framework result to domain-specific response
        NewDomainResponse response = new NewDomainResponse();
        
        // Map selected providers
        List<String> selectedProviderIds = result.getSelectedProviders();
        response.setSelectedProviders(
            selectedProviderIds.stream()
                .map(this::getProviderDetails)
                .collect(Collectors.toList())
        );
        
        return response;
    }
    
    // Helper methods...
}
```

### 6. Integration Testing

Test the new domain integration using standard scenarios:

1. Basic decision-making test
2. Filter component tests
3. Comparator ranking tests 
4. End-to-end flow tests
5. Performance benchmarks

## Reusable Cross-Domain Components

Some components have universal applicability and can be shared across domains:

### 1. Geographic Components

```json
{
  "id": "geographic_distance_calculator",
  "type": "utility",
  "implementation": "GeographicDistanceCalculator",
  "config": {
    "distance_calculation": "haversine",
    "use_road_network": true,
    "cache_results": true,
    "cache_ttl_seconds": 300
  }
}
```

### 2. Time-Based Components

```json
{
  "id": "time_of_day_filter",
  "type": "filter",
  "implementation": "TimeOfDayFilter",
  "config": {
    "time_ranges": [
      {
        "name": "morning_rush",
        "start_time": "07:00",
        "end_time": "10:00",
        "days": ["MONDAY", "TUESDAY", "WEDNESDAY", "THURSDAY", "FRIDAY"]
      },
      {
        "name": "evening_rush",
        "start_time": "16:00",
        "end_time": "19:00",
        "days": ["MONDAY", "TUESDAY", "WEDNESDAY", "THURSDAY", "FRIDAY"]
      }
    ],
    "timezone": "user_local"
  }
}
```

### 3. Rating-Based Components

```json
{
  "id": "rating_threshold_filter",
  "type": "filter",
  "implementation": "RatingThresholdFilter",
  "config": {
    "global_minimum_rating": 4.0,
    "tier_minimum_ratings": {
      "premium": 4.5,
      "standard": 4.2,
      "economy": 4.0
    }
  }
}
```

### 4. Load Balancing Components

```json
{
  "id": "load_balancer",
  "type": "preprocessor",
  "implementation": "LoadBalancer",
  "config": {
    "max_assignments_per_provider": 10,
    "provider_capacity_factor": 0.8,
    "cooldown_period_seconds": 300
  }
}
```

## Implementation Considerations

### 1. Performance Optimization

Adapt domain-specific optimizations through the adapter interface:

```java
interface PerformanceAdapter {
    void optimizeMemory(WorkingMemory memory);
    void registerCaches(CacheRegistry registry);
    List<String> getIndexableFactTypes();
}
```

### 2. Incremental Adoption

Allow partial implementation of the adapter interface:

```java
public class MinimalDomainAdapter implements DomainEntityAdapter, FilterAdapter {
    // Implements only the minimum required interfaces
    // Other functionality falls back to defaults
}
```

### 3. Versioning Support

Include version management in the adapter registry:

```java
public void registerAdapter(String domainType, String version, DomainAdapter adapter) {
    String key = String.format("%s:v%s", domainType, version);
    adapters.put(key, adapter);
}

public DomainAdapter getAdapter(String domainType, String version) {
    String key = String.format("%s:v%s", domainType, version);
    return adapters.getOrDefault(key, adapters.get(domainType + ":latest"));
}
```

### 4. Monitoring and Metrics

Standardized metrics collection across domains:

```java
interface MetricsAdapter {
    Map<String, Double> collectPerformanceMetrics();
    Map<String, Long> collectVolumeMetrics();
    List<MetricDefinition> getDomainSpecificMetrics();
}
```

## Example: E-commerce Product Recommendations

As a concrete example, here's how the framework could be extended to e-commerce product recommendations:

### Domain Mapping

| Framework Concept | E-commerce Equivalent |
|-------------------|----------------------|
| Service Providers | Products |
| Service Recipients | Shoppers |
| Service Requests | Product Search/Browse |
| Selection Criteria | Relevance, Popularity, Margin |

### Sample Configuration

```json
{
  "routing_algorithm": {
    "id": "product_recommendation_algorithm",
    "filters": [
      {
        "id": "inventory_availability_filter",
        "config": {
          "require_in_stock": true,
          "min_stock_level": 3
        }
      },
      {
        "id": "category_filter",
        "config": {
          "include_parent_categories": true,
          "include_sibling_categories": false
        }
      }
    ],
    "comparators": [
      {
        "id": "relevance_score_comparator",
        "weight": 0.5
      },
      {
        "id": "popularity_comparator",
        "weight": 0.3
      },
      {
        "id": "margin_comparator",
        "weight": 0.2
      }
    ],
    "output_processor": {
      "id": "product_recommendation_processor",
      "config": {
        "max_recommendations": 10,
        "diversity_factor": 0.3,
        "include_recently_viewed": true
      }
    }
  }
}
```

## Conclusion

The universal adapter pattern provides a standardized approach for extending the unified framework to any domain. By adhering to this pattern, organizations can:

1. **Maintain Consistency**: Ensure a consistent architecture across all domains
2. **Accelerate Development**: Reuse components and patterns across domains
3. **Simplify Integration**: Standardize how new domains integrate with the core framework
4. **Support Evolution**: Allow domains to evolve independently while preserving compatibility

This pattern transforms the unified framework from a domain-specific solution into a general-purpose decision-making platform that can be applied to any domain requiring sophisticated filtering, comparison, and selection logic.
