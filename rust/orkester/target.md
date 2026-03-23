Voici une note concise, orientée implémentation.

````markdown
# Message HUB — Implementation Requirements

## Goal

Implement a generic in-process `MessageHub` in Rust.

The hub receives `Envelope` messages, queues them, routes them according to rules, and dispatches them to one or more targets.

The implementation must be:
- robust
- fast
- non-crashing on bad input
- easy to debug
- extensible for future filters/targets/security

---

## Core architecture

The hub is composed of these parts:

1. **Waiting Queue**
   - Entry point for incoming messages
   - Handles backpressure
   - Stores validated `Envelope`s waiting for routing

2. **Router**
   - Reads messages from the Waiting Queue
   - Applies routing rules
   - Sends each message to zero, one, or many Dispatching Queues

3. **Dispatching Queues**
   - One queue per dispatcher / route destination class
   - Decouple routing from dispatching

4. **Dispatcher**
   - Reads from one Dispatching Queue
   - Delivers messages to the actual target
   - Different dispatchers may exist:
     - local/in-memory
     - cross-thread
     - cross-process
     - network

5. **RoutesBuilder**
   - Builds the routing table from configuration
   - Converts config rules into executable route rules

---

## SDK / Plugin integration requirements

The HUB is **not** a separate messaging system.

It must be implemented **directly on top of the existing plugin SDK and ABI model**.

### Core principle

Components do not communicate directly with each other.

At runtime:
- the **host** creates and owns components
- a component can only:
  - handle a host request and return a response
  - send a request to the host and get a response
- therefore, **inter-component communication is host-mediated**

The HUB is the reusable host-side system that performs this mediation.

---

## Required integration model

The HUB must run **inside the host**, between:
- incoming component-to-host requests
- host-to-component dispatch

So the flow is:

- Component A sends a request to Host using the `orkester/Enveloppe:1.0` format
- The Host push it to the HUB
- HUB routes it
- HUB delivers it to Component B
- Component B returns a reponse (probably as an Enveloppe) to the host
- The host push it to the HUB
- HUB returns that response back through the chain to Component A

### Important

The SDK messaging model is:

```rust
handle(Request) -> Response
```

---

## High-level API

Expose one main object:

```rust
pub struct MessageHub { ... }
````

Minimum public API:

```rust
impl MessageHub {
    pub fn new(config: HubConfig) -> Result<Self, HubError>;
    pub fn start(&self) -> Result<(), HubError>;
    pub fn stop(&self) -> Result<(), HubError>;
    pub fn submit(&self, envelope: Envelope) -> Result<(), HubError>;
}
```

Optional but recommended:

```rust
impl MessageHub {
    pub fn try_submit(&self, envelope: Envelope) -> Result<(), SubmitError>;
    pub fn stats(&self) -> HubStats;
    pub fn update_routes(&self, config: RouteConfig) -> Result<(), HubError>;
}
```

---

## Base ABI reference

Base incoming message ABI:

```rust
#[repr(C)]
pub struct AbiMessage {
    pub id: u64,
    pub format: *const u8,
    pub format_len: u32,
    pub payload: *const u8,
    pub payload_len: u32,
}
```

This ABI is **not enough** for routing by itself.

The hub works with a higher-level `Envelope`.

---

## Envelope

Use a stable Rust-native structure inside the hub.

```rust
pub struct Envelope {
    pub id: u64,
    pub owner: Option<String>,
    pub kind: String
    pub format: Arc<str>,
    pub payload: Arc<[u8]>,
}
```

### Rules

* `owner` is required for future security integration
* `format` is routing-visible metadata
* `payload` is opaque to the hub core
* avoid copying payload bytes when possible
* use shared ownership (`Arc`) for fan-out routing

---

## Queue requirements

### Waiting Queue

* bounded queue
* must be the main backpressure point
* must not grow unbounded
* if full:

  * return an explicit error
  * do not block forever unless a blocking mode is intentionally implemented

* bounded capacity from config

### Dispatching Queues

* bounded too
* each dispatcher has its own queue
* a slow dispatcher must not block all others

---

## Backpressure requirements

Backpressure must be handled at least at Waiting Queue level.

Supported behaviors:

* `Reject` → submission fails immediately
* `Block` → submission waits until space is available
* `DropNewest` → optional
* `DropOldest` → optional

Config example:

```rust
pub enum BackpressurePolicy {
    Reject,
    Block,
    DropNewest,
    DropOldest,
}
```

---

## Routing model

A route rule has:

* a list of filters
* a list of targets

A message matches a rule if **any filter** match.

If a rule matches:

* the message is sent to **all rule targets**

A message may match several rules.

So the final fan-out is:

* zero target
* one target
* many targets

---

## Routing config model

```rust
pub struct RouteConfig {
    pub rules: Vec<RouteRuleConfig>,
}

pub struct RouteRuleConfig {
    pub name: String,
    pub filters: Vec<FilterConfig>,
    pub targets: Vec<TargetConfig>,
}
```

### Filter config

Each filter must be extensible by `kind`.

```rust
pub struct FilterConfig {
    pub kind: String,
    pub config: serde_json::Value,
}
```

### Target config

Each target must be extensible by `kind`.

```rust
pub struct TargetConfig {
    pub kind: String,
    pub config: serde_json::Value,
}
```

---

## Initial filter kinds to implement

Implement these first:

### 1. `all`

Always matches.

```json
{ "kind": "all", "config": {} }
```

### 2. `match`

Matches if a given field is matching a given value (mode can be regex/compare/...)

```json
{ "kind": "match", "config": { "field": "owner", "value": "user#alice", "mode": "regex" } }
```

### Important

Do **not** decode payload content in the hub core.

The core must route using envelope-visible metadata only.

---

## Initial target kinds to implement

Implement these first:

### 1. `components`

Routes to local components by kind or name.

```json
{ "kind": "components", "config": { "targets": [ { "name": "logger" }, { "kind": "orkester/MetricsAggregator" } ] } }
```

Notes: there will be the need to bridge the dispatcher with a Component Registry in order for the dispatch to be done.

### 2. `drop`

Explicitly drops matched messages

```json
{ "kind": "drop", "config": {} }
```

---

## Dispatcher model

Define a dispatcher trait:

```rust
pub trait Dispatcher: Send + Sync + 'static {
    fn dispatch(&self, envelope: Envelope) -> Result<(), DispatchError>;
}
```

### Notes

* `Envelope` should be cheap to clone
* dispatchers must return errors, never panic
* dispatchers must be independently testable

## RoutesBuilder

`RoutesBuilder` converts config into executable routing objects.

```rust
pub struct RoutesBuilder { ... }
```

Responsibilities:

* validate config
* instantiate concrete filters
* resolve target names
* reject invalid rules at build time
* produce immutable routing table

Output:

```rust
pub struct RoutingTable {
    pub rules: Vec<RouteRule>,
}
```

---

## Error handling requirements

The hub must never crash because of:

* invalid config
* bad message
* unknown filter kind
* unknown target kind
* dispatcher failure

### Rules

* bad config => reject hub creation / route update
* bad message => reject submission or send to dead-letter if configured
* dispatcher error => log + count metric + continue
* panic inside dispatcher/router thread => catch at thread boundary if possible and convert to fatal worker error

### Never do

* `unwrap()` in core path
* panic on malformed runtime input

---

## Logging requirements

Log at least:

* hub start
* hub stop
* route table loaded
* message rejected by backpressure
* message matched zero rules
* dispatcher failure
* invalid config
* route update success/failure

Each log should include:

* message id if relevant
* route name if relevant
* dispatcher name if relevant

---

## Metrics requirements

Track at least:

* submitted messages count
* rejected messages count
* routed messages count
* dropped messages count
* dispatch failures count
* waiting queue length
* dispatching queue length per dispatcher

Expose through a simple `HubStats` first.

---

## Performance requirements

### Required

* avoid payload copies
* route with shared payload ownership
* avoid decoding payload in core
* avoid global lock on hot path if possible

### Recommended

* `Arc<[u8]>` for payload
* `Arc<str>` for format
* immutable routing table behind `Arc`
* one routing thread or one routing worker loop first
* one dispatcher loop per dispatching queue

### V1 guidance

Prefer a correct, measurable design over premature micro-optimization.

---

## Concurrency model

Good V1:

* one input queue
* one router worker
* N dispatching queues
* N dispatcher workers

This is simple and robust.

---

## Required tests

### Unit tests

* filter matching
* route matching
* route fan-out
* backpressure rejection
* dispatcher error propagation
* routes builder validation

### Integration tests

* submit one message -> one dispatcher receives it
* submit one message -> many dispatchers receive it
* submit one message -> zero match
* invalid route config fails at startup
* full waiting queue rejects message
* slow dispatcher does not block other dispatchers

### Resilience tests

* dispatcher returns error
* malformed message envelope
* unknown target kind in config
* unknown filter kind in config

---

## Mandatory implementation rules

* Keep payload opaque in the hub core
* Do not decode business payload for routing
* Use bounded queues
* Return explicit errors
* No panic in normal error paths
* Keep routing table immutable after build
* Support route reload through full replacement, not in-place mutation of partial state
* Prefer `Arc` over copying bytes

---

## Nice-to-have but not required for V1

* hot route reload
* dead letter storage
* async dispatcher support
* network dispatchers
* cross-process dispatchers
* authorization layer based on `owner`
* payload-aware filters in optional extension layer only

---

## Deliverable

The implementation is complete when:

* a `MessageHub` can be created from config
* messages can be submitted
* messages are routed correctly
* messages are dispatched correctly
* queue pressure is handled
* failures are logged and counted
* all tests above pass
