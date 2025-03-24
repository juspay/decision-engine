# Open Router by Juspay

The Open Router is an intelligent payment routing system that helps in choosing the most optimal payment gateway in real-time for every transaction based on success rate, latency and other business requirements. It is a fully modular service that can work with any orchestrator and third party vaults. 

## [Features]()  
### [1.1 Eligibility Check](#11-eligibility-check)  
### [1.2 Rule-Based Ordering](#12-rule-based-ordering)  
### [1.3 Dynamic Gateway Ordering](#13-dynamic-gateway-ordering)  
### [1.4 Downtime Detection](#14-downtime-detection)  

## Getting Started
### 2.1 Local Setup
### 2.2 Docker (coming soon)

## [Contributor Guidelines](#contributor-guidelines)  

## [Product Roadmap]()


## 🚀 Features

### ✅ Eligibility Check
Eligibility Check ensures that only the eligible gateways are considered for processing the transaction. This significantly reduces the risk of payment failures, such as declined transactions or processing errors, leading to a smoother and more efficient payment experience for merchants and customers.

### 📌 Rule-Based Ordering
Rule-based ordering relies on predefined rules set by merchants to determine the preferred payment gateway for each transaction. The routing path for each transaction is defined by these sets of rules, making the process highly predictable.

Effective for business commitments: Rule-based ordering is beneficial for businesses because it ensures they meet specific commercial obligations to designated gateways. This approach maintains minimum transaction volumes and efficiently directs traffic to particular payment gateways (PGs).
"If-Else" Logic: The logic behind rule-based ordering often resembles a series of nested "if-else" statements, where each condition triggers a specific routing decision.

#### Examples:
Rule 1: If the transaction currency is USD and the payment instrument is a Card, route it to Gateway A.
Rule 2: If the payment instrument is a Card and card type is CREDIT issued by Bank X, route 90% of traffic to Gateway B and 10% of traffic to Gateway C.
Rule 3: If the payment method is Net Banking using Bank Y, route it to Gateway C.
Rule 4: If the payment instrument is Wallet, route it to Gateway D.
Rule 5: If none of the above rules matches, route it to Gateway E as a default.

### 🔄 Dynamic gateway ordering
It is an alternative to the rule-based ordering described above. Unlike rule-based ordering, dynamic ordering inputs real-time success rates into consideration for each combination of payment instruments, type of transaction, Network, Platform, transaction origin country, etc., without extra manual effort. Merchants can extend criteria to additional fields based on their requirements.

Dynamic Gateway Ordering leverages advanced concepts of Reinforcement Learning and Statistical Distribution, enabling real-time success rate optimization by routing the transaction to the most optimal PG.

The problem of selecting the best Gateway can be mapped to a Non-stationary Multi-Armed Bandit (MAB) problem with Delayed Feedback, where each Gateway is an "arm" with fluctuating success rates and varying latency for success and failure. The approach used to solve this problem is driven by explore-exploit strategy. This method takes a two-pronged approach:

Exploration: We continuously evaluate all gateways by sending a small percentage of traffic to ensure up-to-date performance data.

Exploitation: We continuously route most traffic to the best-performing Gateway to maximize the overall success rate.

The algorithm uses a sliding window technique to assess the success rates of each Gateway's last few transactions. This ensures that, without downtime, the highest-ranked Gateway is chosen for transaction routing.

### ⚠️ Downtime Detection

The downtime detection mechanism uses a "reward" and "penalize" feedback loop inspired by the Proportional–integral–derivative (PID) controller to maintain health scores of underlying payment gateways.

If the score for any gateway drops below the merchant-configured threshold, the gateway is classified as "down".

In case of any downtime, if a merchant uses rule-based ordering, the gateways are re-ordered on the basis of this downtime detection mechanism.

In case of Dynamic Ordering, the cost of exploration becomes high when the payment gateway faces downtime. Exploration is therefore stopped for that Gateway for a specific time interval, aka cool-off period. After this cool-off period, the routing system re-evaluates the Gateway for further exploration by allowing gateways to process a limited number of transactions for exploration purposes. If the underlying issue persists even after routing these small number of transactions, the Gateway is classified "down" rapidly.

### Setup

Install Nix 

```bash
curl --proto '=https' --tlsv1.2 -sSf -L \
  https://juspay.github.io/nixone/setup | sh -s
```


### Development

Enter development shell by running:
```bash
nix develop
```
Hot reload with `ghcid` on code change for `euler-router`:
```bash
ghcid -c cabal repl euler-router-library
```
Similarly, you can hot reload on other local packages, for example:
```bash
ghcid -c cabal repl types

```

If you are facing issues with the setup, run [nix-health](https://crates.io/crates/nix_health) to verify everything is green:
```bash
cd euler-router
nix --accept-flake-config run github:juspay/nix-browser#nix-health .
```

### Build

Build the executable by running:
```bash
nix build
```

### Build the docker image
```bash
nix build .#dockerImage
```

### Starting MySQL, Redis Servers
```bash
nix run .#services
```

## FAQ
### Locally link euler dependencies
Create `cabal.project.local` by running:
```bash
echo 'packages:
  ../euler-hs
  ../euler-db
  ../euler-webservice' > cabal.project.local
```
**Note: modify the path for dependencies if needed**

### Override dependencies

#### Add input

##### Public repositories
```nix
# github
repo = {
  url = "github:<owner>/<repo>/<branch/commit/tag>";
  # If the repo doesn't have a flake.nix/ you don't want to use it
  flake = false;
};
```
Refer here for more URL syntax: https://nixos.org/manual/nix/unstable/command-ref/new-cli/nix3-flake.html#url-like-syntax

#### Use input added above to override the haskell package set

Head over to `nix/haskell-project.nix` and follow the instructions here: https://community.flake.parts/haskell-flake/dependency#source

#### Update the lock file

Update all inputs:
```sh
nix flake update
```
Update specific input:
```sh
nix flake lock --update-input euler-webservice
```

### How do I append flase-positive leaks to .gitleaksignore

```sh
nix run .#gitleaks | awk '/Fingerprint/ { sub("Fingerprint: ", ""); print }' >> .gitleaksignore

```

## Getting started

## Product Roadmap

🗺️ Our Roadmap typically pans out over a 3-month period and we establish topics we work on upfront.

Before the beginning of every quarter we come together to develop the next roadmap based on our core values, previous roadmap, findings over the previous quarter, what we heard from the community as feature requests.

👂And as always, we listen to your feedback and adapt our plans if needed.

Visit our [Current Roadmap]()

## Contributing
We welcome contributions from everyone! Here's how you can help:
 <br> <br> 
<b>For code contribution:<b>
1. Fork the repository
2. Create your feature branch: git checkout -b feature/amazing-feature
3. Commit your changes: git commit -m 'Add some amazing feature'
4. Push to your branch: git push origin feature/amazing-feature
5. Open a Pull Request

<b>For knowledge contribution:<b>
1. Start a new discussion thread [here](https://github.com/juspay/open-router/discussions/new?category=ideas)
2. Our team will collaborate with you to raise a Pull Request
 
See CONTRIBUTING.md for detailed guidelines.

