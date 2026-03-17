use orkester_plugin::{Plugin, Component, Host, PluginError, Message, OwnedMessage};

struct TestPlugin;

impl Plugin for TestPlugin {
    fn load(host: Host) -> Result<Self, PluginError> {
        // do something with host, e.g. call host API to get config, etc.
        Ok(Self)
    }

    fn call(&mut self, req: Message) -> Result<OwnedMessage, PluginError> {
        // metadata, etc. (JSON/whatever) decided by caller
        Ok(OwnedMessage::utf8("ok"))
    }

    fn unload(&mut self) -> Result<(), PluginError> {
        // cleanup, etc.
        Ok(())
    }
}

struct TestComponent;

impl Component for TestComponent {
    fn handle(&mut self, _host: Host, _req: Message) -> Result<OwnedMessage, PluginError> {
        Ok(OwnedMessage::utf8("ok"))
    }
}