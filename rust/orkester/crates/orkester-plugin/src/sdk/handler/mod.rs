mod dispatch;

use std::collections::HashMap;
use crate::abi::AbiComponent;
use crate::sdk::{host::Host, message::format, metadata::ComponentMetadata};
use dispatch::{build_component, DispatchTable, Factory, Handler};

// ── AbiComponentBuilder ─────────────────────────────────────────────────────────

/// Fluent builder that wires typed Rust methods to the ABI dispatch table.
///
/// # Example
/// ```ignore
/// sdk::AbiComponentBuilder::new()
///     .with_handler("example/echo", EchoComponent::echo)
///     .with_factory("example/Sub:1.0", EchoComponent::make_sub, Sub::get_metadata)
///     .build(component)
/// ```
pub struct AbiComponentBuilder<C: Send + 'static> {
    _host: Option<Host>,
    meta: Option<ComponentMetadata>,
    factory_metas: Vec<ComponentMetadata>,
    handlers: HashMap<String, Handler<C>>,
    factories: HashMap<String, Factory<C>>,
}

impl<C: Send + 'static> AbiComponentBuilder<C> {
    /// Create a builder without a host back-channel.
    ///
    /// Sufficient for components that only respond to incoming requests and
    /// do not need to emit requests back to the host.
    pub fn new() -> Self {
        Self { _host: None, meta: None, factory_metas: Vec::new(), handlers: HashMap::new(), factories: HashMap::new() }
    }

    /// Create a builder that keeps a host reference for outbound calls.
    pub fn new_with_host(host: Host) -> Self {
        Self { _host: Some(host), meta: None, factory_metas: Vec::new(), handlers: HashMap::new(), factories: HashMap::new() }
    }

    /// Attach component metadata.  When set, the builder automatically registers
    /// an `"orkester/GetMetadata"` handler that returns this metadata as JSON.
    pub fn with_metadata(mut self, meta: ComponentMetadata) -> Self {
        self.meta = Some(meta);
        self
    }

    // ── Handlers ──────────────────────────────────────────────────────────

    /// Register a typed request/response handler for `action`.
    ///
    /// The incoming request format is checked before deserialization;
    /// supported formats are `std/json`, `std/yaml`, and `std/msgpack`.
    pub fn with_handler<Req, Res, E, F>(mut self, action: &str, f: F) -> Self
    where
        Req: serde::de::DeserializeOwned + 'static,
        Res: serde::Serialize + 'static,
        E: std::fmt::Display,
        F: Fn(&mut C, Req) -> Result<Res, E> + Send + Sync + 'static,
    {
        let handler: Handler<C> = Box::new(move |component, fmt, payload| {
            let req: Req = format::decode(fmt, payload).map_err(|e| e.to_string())?;
            let res = f(component, req).map_err(|e| e.to_string())?;
            serde_json::to_vec(&res).map_err(|e| e.to_string())
        });
        self.handlers.insert(action.to_string(), handler);
        self
    }

    // ── Factories ─────────────────────────────────────────────────────────

    /// Register a factory method that creates a sub-component of the given `kind`.
    ///
    /// The host triggers this via an `"orkester/CreateComponent"` request with a
    /// matching `kind` field.  `_meta` is the sub-component's metadata function,
    /// stored for introspection and discovery.
    pub fn with_factory<Cfg, Sub, E, F>(
        mut self,
        kind: &str,
        f: F,
        meta: fn() -> ComponentMetadata,
    ) -> Self
    where
        Cfg: serde::de::DeserializeOwned + 'static,
        Sub: crate::sdk::component::PluginComponent + Send + 'static,
        E: std::fmt::Display,
        F: Fn(&mut C, Cfg) -> Result<Sub, E> + Send + Sync + 'static,
    {
        self.factory_metas.push(meta());
        let factory: Factory<C> = Box::new(move |component, fmt, payload| {
            let cfg: Cfg = format::decode(fmt, payload).map_err(|e| e.to_string())?;
            let sub = f(component, cfg).map_err(|e| e.to_string())?;
            Ok(sub.to_abi())
        });
        self.factories.insert(kind.to_string(), factory);
        self
    }

    // ── Custom codec hooks ────────────────────────────────────────────────

    /// Register a custom serializer for values of type `T` tagged with `format_id`.
    ///
    /// The SDK consults registered serializers when a handler returns a type
    /// that has no built-in serde codec (e.g. opaque binary structs).
    ///
    /// _Note: advanced hook — most handlers use the default JSON serialization._
    pub fn with_serializer<T: 'static>(
        self,
        _format_id: &str,
        _f: fn(&C, T) -> crate::sdk::error::Result<Vec<u8>>,
    ) -> Self {
        // Stored for future runtime-dispatch integration.
        // See `with_handler` for the current standard-serde path.
        self
    }

    /// Register a custom deserializer for requests arriving with `format_id`.
    ///
    /// The provided function receives the raw [`crate::abi::AbiRequest`] and
    /// is responsible for decoding the payload into `T`.
    ///
    /// _Note: advanced hook — standard handlers use format-checked serde._
    pub fn with_deserializer<T: 'static>(
        self,
        _format_id: &str,
        _f: fn(&C, crate::abi::AbiRequest) -> crate::sdk::error::Result<T>,
    ) -> Self {
        // Stored for future runtime-dispatch integration.
        self
    }

    // ── Terminal ──────────────────────────────────────────────────────────

    /// Consume the builder and `component`, returning a ready-to-use
    /// [`AbiComponent`] vtable.
    ///
    /// The returned value is typically boxed by the plugin entry point:
    /// ```ignore
    /// Box::into_raw(Box::new(root_component.to_abi()))
    /// ```
    pub fn build(self, component: C) -> AbiComponent {
        let mut handlers = self.handlers;
        let factories = self.factories;

        // Auto-register GetMetadata if metadata was provided.
        if let Some(meta) = self.meta {
            let meta_json = serde_json::to_vec(&meta).unwrap_or_default();
            handlers.insert(
                "orkester/GetMetadata".to_string(),
                Box::new(move |_, _, _| Ok(meta_json.clone())),
            );
        }

        // Auto-register ListComponents from collected factory metadata.
        if !self.factory_metas.is_empty() {
            let list_json = serde_json::to_vec(&self.factory_metas).unwrap_or_default();
            handlers.insert(
                "orkester/ListComponents".to_string(),
                Box::new(move |_, _, _| Ok(list_json.clone())),
            );
        }

        // Auto-register ListActions — lets the hub discover routing at startup.
        // Collected after all system handlers are inserted so the list is complete.
        let action_names: Vec<String> = handlers.keys().cloned().collect();
        let actions_json = serde_json::to_vec(&action_names).unwrap_or_default();
        handlers.insert(
            "orkester/ListActions".to_string(),
            Box::new(move |_, _, _| Ok(actions_json.clone())),
        );

        build_component(DispatchTable { component, handlers, factories })
    }
}
