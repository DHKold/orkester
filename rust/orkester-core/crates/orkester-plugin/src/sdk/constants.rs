///======================================================================================
/// Constants for protocol versions in the Orkester plugin SDK.
///======================================================================================

pub const PROTOCOL_V1: u32 = 1;

///======================================================================================
/// Constants for message types in the Orkester plugin SDK.
///======================================================================================

// Raw types
pub const MSG_TYPE_BYTES: u32           = 0000;
pub const MSG_TYPE_STRING: u32          = 0001;
pub const MSG_TYPE_POINTER: u32         = 0002;
pub const MSG_TYPE_INT: u32             = 0003;

// Structured types
pub const MSG_TYPE_JSON: u32            = 1000;
pub const MSG_TYPE_YAML: u32            = 1001;
pub const MSG_TYPE_TOML: u32            = 1002;
pub const MSG_TYPE_XML: u32             = 1003;

// Domain-specific types
pub const MSG_TYPE_PROTOBUF: u32        = 2000;
pub const MSG_TYPE_AVRO: u32            = 2001;

///======================================================================================
/// Constants for message flags in the Orkester plugin SDK.
///======================================================================================

// Generic flags
pub const FLAG_NONE: u32                = 0x0000;

// Payload encoding flags
pub const FLAG_UTF8: u32                = 1 << 0; // Indicates that the payload is UTF-8 encoded

///======================================================================================
/// Constants for component kinds in the Orkester plugin SDK.
///======================================================================================

pub const COMPONENT_KIND_PLUGIN: u32 = 1;
pub const COMPONENT_KIND_HOST: u32   = 2;

///======================================================================================
/// Constants for error codes in the Orkester plugin SDK.
///======================================================================================

pub const ERROR_NONE: u32             = 0;
pub const ERROR_INVALID_REQUEST: u32  = 1;
pub const ERROR_INTERNAL: u32         = 2;
pub const ERROR_UNSUPPORTED: u32      = 3;
