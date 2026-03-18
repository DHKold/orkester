use libloading::{Library, Symbol};
use orkester_plugin::abi::*;
use std::{ffi::c_void, path::Path};

type PluginInit = unsafe extern "C" fn(*const AbiHostApi) -> *mut c_void;
type PluginCall = unsafe extern "C" fn(AbiCallContext, AbiMessage, *mut AbiOwnedMessage) -> i32;
type PluginFree = unsafe extern "C" fn(*mut AbiOwnedMessage);

fn main() {
    let lib_path = find_plugin();

    println!("Loading: {:?}", lib_path);

    unsafe {
        let lib = Library::new(lib_path).unwrap();

        let plugin_init: Symbol<PluginInit> = lib.get(b"plugin_init").unwrap();
        let plugin_call: Symbol<PluginCall> = lib.get(b"plugin_call").unwrap();
        let plugin_free: Symbol<PluginFree> = lib.get(b"plugin_free").unwrap();

        let plugin_init_fn: PluginInit = *plugin_init;
        let plugin_call_fn: PluginCall = *plugin_call;
        let plugin_free_fn: PluginFree = *plugin_free;

        let host_api = make_host_api();

        let runtime = plugin_init_fn(&host_api);
        assert!(!runtime.is_null());

        let ctx_root = AbiCallContext {
            host: &host_api,
            component: std::ptr::null_mut(),
        };

        let metadata = call_json(plugin_call_fn, plugin_free_fn, ctx_root, 1000);
        println!("Metadata: {}", metadata);

        let list = call_json(plugin_call_fn, plugin_free_fn, ctx_root, 1001);
        println!("Components: {}", list);

        let create_req = serde_json::json!({
            "component_id": "echo",
            "config": {}
        });

        let component = call_create_component(plugin_call_fn, plugin_free_fn, ctx_root, create_req);
        println!("Component handle: {:?}", component);

        let ctx_component = AbiCallContext {
            host: &host_api,
            component,
        };

        let response = call_utf8(plugin_call_fn, plugin_free_fn, ctx_component, b"hello world");
        println!("Response: {}", response);
    }
}

fn find_plugin() -> &'static Path {
    #[cfg(target_os = "windows")]
    {
        Path::new("target/debug/orkester_plugin_test.dll")
    }

    #[cfg(target_os = "linux")]
    {
        Path::new("target/debug/liborkester_plugin_test.so")
    }

    #[cfg(target_os = "macos")]
    {
        Path::new("target/debug/liborkester_plugin_test.dylib")
    }
}

fn make_host_api() -> AbiHostApi {
    unsafe extern "C" fn call_host(
        _ctx: *mut c_void,
        _req: AbiMessage,
        out: *mut AbiOwnedMessage,
    ) -> i32 {
        if !out.is_null() {
            unsafe {
                *out = AbiOwnedMessage::empty();
            }
        }
        0
    }

    unsafe extern "C" fn free_host_message(_ctx: *mut c_void, msg: *mut AbiOwnedMessage) {
        if !msg.is_null() {
            unsafe {
                *msg = AbiOwnedMessage::empty();
            }
        }
    }

    AbiHostApi {
        abi_version: ABI_VERSION,
        host_ctx: std::ptr::null_mut(),
        call_host,
        free_host_message,
    }
}

unsafe fn call_json(
    call: PluginCall,
    free: PluginFree,
    ctx: AbiCallContext,
    type_id: u32,
) -> String {
    let req = AbiMessage::empty(1, type_id, 0);
    let mut out = AbiOwnedMessage::empty();

    let rc = unsafe { call(ctx, req, &mut out) };
    assert_eq!(rc, 0);

    let bytes = unsafe { std::slice::from_raw_parts(out.payload, out.len as usize) };
    let s = String::from_utf8_lossy(bytes).to_string();

    unsafe { free(&mut out) };
    s
}

unsafe fn call_create_component(
    call: PluginCall,
    free: PluginFree,
    ctx: AbiCallContext,
    json: serde_json::Value,
) -> *mut c_void {
    let payload = serde_json::to_vec(&json).unwrap();

    let req = AbiMessage {
        id: 1,
        type_id: 1002,
        flags: 0,
        payload: payload.as_ptr(),
        len: payload.len() as u32,
    };

    let mut out = AbiOwnedMessage::empty();

    let rc = unsafe { call(ctx, req, &mut out) };
    assert_eq!(rc, 0);
    assert_eq!(out.type_id, orkester_plugin::sdk::protocol::constants::MSG_TYPE_POINTER);

    let ptr_bytes = unsafe { std::slice::from_raw_parts(out.payload, out.len as usize) };
    let mut arr = [0u8; std::mem::size_of::<usize>()];
    arr.copy_from_slice(ptr_bytes);

    let ptr = usize::from_ne_bytes(arr) as *mut c_void;

    unsafe { free(&mut out) };
    ptr
}

unsafe fn call_utf8(
    call: PluginCall,
    free: PluginFree,
    ctx: AbiCallContext,
    input: &[u8],
) -> String {
    let req = AbiMessage {
        id: 1,
        type_id: orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING,
        flags: 0,
        payload: input.as_ptr(),
        len: input.len() as u32,
    };

    let mut out = AbiOwnedMessage::empty();

    let rc = unsafe { call(ctx, req, &mut out) };
    assert_eq!(rc, 0);

    let bytes = unsafe { std::slice::from_raw_parts(out.payload, out.len as usize) };
    let s = String::from_utf8_lossy(bytes).to_string();

    unsafe { free(&mut out) };
    s
}