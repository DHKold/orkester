pub const ABI_VERSION: u32 = 1;

// Core envelope flags
pub const FLAG_NONE: u32 = 0;
pub const FLAG_RESPONSE: u32 = 1 << 0;
pub const FLAG_ERROR: u32 = 1 << 1;
pub const FLAG_ONE_WAY: u32 = 1 << 2;

// Core payload kinds
pub const TYPE_INVALID: u32 = 0;
pub const TYPE_COMPONENT: u32 = 1;
pub const TYPE_BYTES: u32 = 2;
pub const TYPE_UTF8: u32 = 3;
pub const TYPE_JSON: u32 = 4;