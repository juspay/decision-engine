# Decision Engine 
## Overview 

The Decision Engine system helps in choosing the most optimal payment gateway in real-time for every transaction based on pre-defined rules, success rate, latency and other business requirements. It is a fully modular service that can work with any orchestrator and third party vaults.

## Vision 

Build a reliable, open source payments software for the world \- which is interoperable, collaborative and community-driven.

## Features

The Decision Engine comes with the following features out-of-the box for your payment routing needs. 
✅ Eligibility Check – Ensures only eligible gateways are used, reducing payment failures and improving transaction success.

📌 Rule-Based Ordering – Routes transactions based on predefined merchant rules, ensuring predictable and obligation-driven payment processing.

🔄 Dynamic Gateway Ordering – Uses real-time success rates and ML-driven optimization to route transactions to the best-performing gateway.

⚠️ Downtime Detection – Monitors gateway health, dynamically reordering or pausing routing to prevent transaction failures during downtime.

To learn more, refer to this blog: [https://juspay.io/blog/juspay-orchestrator-and-merchant-controlled-routing-engine](https://juspay.io/blog/juspay-orchestrator-and-merchant-controlled-routing-engine)  


## Architecture 

![](https://cdn.sanity.io/images/9sed75bn/production/fd872ae5b086e7a60011ad9d4d5c7988e1084d03-1999x1167.png)  

## Try it out

You can run Decision Engine on your system using Docker compose after cloning this repository. 

```shell
git clone --depth 1 --branch latest https://github.com/juspay/decision-engine
cd decision-engine
docker compose up -d
```



## API Reference (TODO)

   

## Support, Feature Requests, Bugs 

For any support, join the conversation in [Slack](https://join.slack.com/t/hyperswitch-io/shared_invite/zt-2jqxmpsbm-WXUENx022HjNEy~Ark7Orw)
     
For new product features, enhancements, roadmap discussions, or to share queries and ideas, visit our [GitHub Discussions](https://github.com/juspay/hyperswitch/discussions)

For reporting a bug, please read the issue guidelines and search for [existing and closed issues]. If your problem or idea is not addressed yet, please [open a new issue].

[existing and closed issues]: https://github.com/juspay/decision-engine/issues
[open a new issue]: https://github.com/juspay/decision-engine/issues/new/choose
 

## Contributing

We welcome contributions from everyone\! Here's how you can help:

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

## Versioning

Check the [CHANGELOG.md](CHANGELOG.md) file for details.

## Copyright and License

This product is licensed under the [AGPL V3](LICENSE) License.
