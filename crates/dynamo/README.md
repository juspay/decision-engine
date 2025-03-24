## Dynamo: Dynamic Routing

This library is a dynamic routing system that can be used to route payments to different
processors. The system is designed to be able to route payments based on the success rate of
the processors.

This success rate is calculated based on some predefined parameters. It counts the number of
successful payments and the number of failed payments for a given set of parameters. Then it
calculates the success rate based on the number of successful payments and the total number of
payments over a given window of time.

The system, primarily uses the following formula to calculate the success rate:

$$
success\ rate
    = \sum_{i=0}^{n-1} (
        (\frac{success\ count[i]}
              {total\ count[i]}) *
        (\frac{i + 1}
              {\sum_{k=1}^{n} k}))
    * 100
$$
