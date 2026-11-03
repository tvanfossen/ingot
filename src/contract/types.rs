//! The frozen TIV 1.0 codes (`dmt_types.h`) for the schema's enums, contract versions, and the
//! well-known role table (R-WK).

use super::schema::{KeyType, Kind, Persist, PrincipalFlag, Role};

/// `DMT_KF_*` key flags.
pub const KF_SECRET: u16 = 0x0001;
pub const KF_SURVIVES_RESET: u16 = 0x0002;
pub const KF_ALIAS: u16 = 0x0004;
pub const KF_ENUM: u16 = 0x0008;
pub const KF_CMD: u16 = 0x0010;
pub const KF_CMD_RESULT: u16 = 0x0020;
pub const KF_CMD_DEDUPE: u16 = 0x0040;
pub const KF_MFG: u16 = 0x0080;

/// `DMT_X_*` transport mask bits.
pub const X_UNIX: u8 = 0x01;
pub const X_INPROC: u8 = 0x02;
pub const X_UART: u8 = 0x04;

/// `DMT_*_HEADER_BYTES` and `DMT_DEDUPE_ENTRY_BYTES`.
pub const CMD_HEADER_BYTES: u32 = 8;
pub const RESULT_HEADER_BYTES: u32 = 12;
pub const DEDUPE_ENTRY_BYTES: u32 = 64;

/// The command owner that means "the core answers it"; reserved, never a principal name.
pub const OWNER_CORE: &str = "core";
/// `DMT_OWNER_CORE`, its `owner_principal` value.
pub const OWNER_CORE_INDEX: u16 = 0xFFFF;

/// A contract version within which minors are additive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
}

impl Version {
    /// Parse `"major.minor"`.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-002
    pub fn parse(text: &str) -> Result<Self, String> {
        let parsed = text
            .split_once('.')
            .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)));
        match parsed {
            Some((major, minor)) => Ok(Self { major, minor }),
            None => Err(format!("'{text}' is not a \"major.minor\" version")),
        }
    }

    /// The minor as seen from contract major `major`: this minor in the same major, 0 for an
    /// older major (a major bump restarts the count).
    ///
    /// @version 1
    /// @req REQ-ING-TIV-003
    pub fn minor_in(self, major: u16) -> u16 {
        if self.major == major {
            self.minor
        } else {
            0
        }
    }
}

impl std::fmt::Display for Version {
    /// Format as `"major.minor"`.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-002
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl KeyType {
    /// `DMT_T_*` code; also bits 6..3 of the key ID.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-003
    pub fn code(self) -> u8 {
        match self {
            KeyType::Bool => 0,
            KeyType::Uint8 => 1,
            KeyType::Uint16 => 2,
            KeyType::Uint32 => 3,
            KeyType::Int8 => 4,
            KeyType::Int16 => 5,
            KeyType::Int32 => 6,
            KeyType::Float32 => 7,
            KeyType::String => 8,
            KeyType::Binary => 9,
            KeyType::Struct => 10,
        }
    }

    /// Size in bytes of a scalar, `None` for string, binary and struct (sized by max_size).
    ///
    /// @version 1
    /// @req REQ-ING-TIV-003
    pub fn scalar_size(self) -> Option<u32> {
        match self {
            KeyType::Bool | KeyType::Uint8 | KeyType::Int8 => Some(1),
            KeyType::Uint16 | KeyType::Int16 => Some(2),
            KeyType::Uint32 | KeyType::Int32 | KeyType::Float32 => Some(4),
            KeyType::String | KeyType::Binary | KeyType::Struct => None,
        }
    }

    /// Name used in the model and in contract.json.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-009
    pub fn name(self) -> &'static str {
        match self {
            KeyType::Bool => "bool",
            KeyType::Uint8 => "uint8",
            KeyType::Int8 => "int8",
            KeyType::Uint16 => "uint16",
            KeyType::Int16 => "int16",
            KeyType::Uint32 => "uint32",
            KeyType::Int32 => "int32",
            KeyType::Float32 => "float32",
            KeyType::String => "string",
            KeyType::Binary => "binary",
            KeyType::Struct => "struct",
        }
    }
}

impl Persist {
    /// `DMT_P_*` code.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-003
    pub fn code(self) -> u8 {
        match self {
            Persist::None => 0,
            Persist::Deferred => 1,
            Persist::Durable => 2,
        }
    }
}

impl Role {
    /// `DMT_R_*` code.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-003
    pub fn code(self) -> u8 {
        match self {
            Role::Settings => 0,
            Role::State => 1,
            Role::Command => 2,
            Role::Config => 3,
            Role::Diag => 4,
        }
    }
}

impl Kind {
    /// `DMT_PK_*` code.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-006
    pub fn code(self) -> u8 {
        match self {
            Kind::Unix => 0,
            Kind::Inproc => 1,
            Kind::Uart => 2,
        }
    }
}

impl PrincipalFlag {
    /// `DMT_PF_*` bit.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-006
    pub fn bit(self) -> u8 {
        match self {
            PrincipalFlag::WatchdogRequired => 0x01,
            PrincipalFlag::TraceReader => 0x02,
            PrincipalFlag::TokenIssuer => 0x04,
        }
    }
}

/// What a well-known role requires of the key mapped to it (R-WK).
#[derive(Debug, Clone, Copy)]
pub struct WkRule {
    pub name: &'static str,
    pub required: bool,
    /// `None` for the command roles, which map a command key.
    pub ty: Option<KeyType>,
    pub persist: Persist,
    pub survives_reset: bool,
    /// Written only by the core: no principal may have write on it (R-V5).
    pub core_written: bool,
}

/// Build one R-WK row.
///
/// @version 1
/// @req REQ-ING-TIV-010
const fn wk(name: &'static str, required: bool, ty: Option<KeyType>, persist: Persist) -> WkRule {
    WkRule {
        name,
        required,
        ty,
        persist,
        survives_reset: false,
        core_written: ty.is_some(),
    }
}

/// A core-written, non-persistent diagnostic counter.
///
/// @version 1
/// @req REQ-ING-TIV-010
const fn diag(name: &'static str, ty: KeyType) -> WkRule {
    wk(name, false, Some(ty), Persist::None)
}

/// R-WK in `DMT_WK_*` order; the first `WK_REQUIRED` are required.
pub const WK_RULES: [WkRule; 26] = [
    wk("RESET_REQUEST", true, None, Persist::None),
    wk("RESET_PRE_COMMAND", true, None, Persist::None),
    wk("TRACE_ENABLE", true, None, Persist::None),
    wk("SECRETS_FALLBACK", true, Some(KeyType::Bool), Persist::None),
    WkRule {
        survives_reset: true,
        ..wk("BOOT_EPOCH", true, Some(KeyType::Uint32), Persist::Durable)
    },
    wk(
        "DIAG_CORRUPTION",
        true,
        Some(KeyType::Uint32),
        Persist::Deferred,
    ),
    wk(
        "DIAG_KEYSTORE_VERSION",
        true,
        Some(KeyType::String),
        Persist::None,
    ),
    wk(
        "DIAG_CONTRACT_VERSION",
        true,
        Some(KeyType::String),
        Persist::None,
    ),
    WkRule {
        survives_reset: true,
        core_written: false,
        ..wk("DEVICE_ID", true, Some(KeyType::String), Persist::Durable)
    },
    diag("DIAG_BYTES_WRITTEN_KIB", KeyType::Uint32),
    diag("DIAG_RECORDS_WRITTEN", KeyType::Uint32),
    diag("DIAG_FSYNC_COUNT", KeyType::Uint32),
    diag("DIAG_FSYNC_LATENCY_US", KeyType::Uint32),
    diag("DIAG_COMPACTIONS", KeyType::Uint32),
    diag("DIAG_JOURNAL_BYTES", KeyType::Uint32),
    diag("DIAG_FLASH_LIFE", KeyType::Uint8),
    diag("DIAG_OVERFLOW_RESYNCS", KeyType::Uint32),
    diag("DIAG_HANDSHAKE_REFUSALS", KeyType::Uint32),
    diag("DIAG_RIGHTS_REJECTIONS", KeyType::Uint32),
    diag("DIAG_RATE_REJECTIONS", KeyType::Uint32),
    diag("DIAG_REPLAY_SKIPPED", KeyType::Uint32),
    diag("DIAG_TORN_TAILS", KeyType::Uint32),
    diag("DIAG_ALLOC_GAPS", KeyType::Uint32),
    diag("DIAG_CONNECTED", KeyType::Uint32),
    diag("DIAG_SUBSCRIBERS", KeyType::Binary),
    diag("DIAG_TRACE_REFUSALS", KeyType::Uint32),
];

/// Index of the role named `name` in `WK_RULES`.
///
/// @version 1
/// @req REQ-ING-TIV-010
pub fn wk_index(name: &str) -> Option<usize> {
    WK_RULES.iter().position(|rule| rule.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parses_and_resolves_per_major() {
        let v = Version::parse("1.3").unwrap();
        assert_eq!(v, Version { major: 1, minor: 3 });
        assert_eq!(v.minor_in(1), 3);
        assert_eq!(v.minor_in(2), 0);
        assert_eq!(v.to_string(), "1.3");
        assert!(Version::parse("1").is_err());
        assert!(Version::parse("a.b").is_err());
    }

    #[test]
    fn codes_match_dmt_types() {
        assert_eq!(KeyType::Float32.code(), 7);
        assert_eq!(KeyType::Struct.code(), 10);
        assert_eq!(KeyType::Uint16.code(), 2);
        assert_eq!(Persist::Durable.code(), 2);
        assert_eq!(Role::Diag.code(), 4);
        assert_eq!(Kind::Uart.code(), 2);
        assert_eq!(PrincipalFlag::TokenIssuer.bit(), 4);
        assert_eq!(KeyType::Float32.scalar_size(), Some(4));
        assert_eq!(KeyType::Struct.scalar_size(), None);
    }

    #[test]
    fn wk_table_matches_r_wk() {
        assert_eq!(WK_RULES.iter().filter(|r| r.required).count(), 9);
        assert!(WK_RULES[..9].iter().all(|r| r.required));
        assert_eq!(wk_index("DEVICE_ID"), Some(8));
        assert_eq!(wk_index("DIAG_TRACE_REFUSALS"), Some(25));
        let epoch = WK_RULES[wk_index("BOOT_EPOCH").unwrap()];
        assert!(epoch.survives_reset && epoch.core_written);
        assert!(
            !WK_RULES[8].core_written,
            "DEVICE_ID is written by the MFG writer"
        );
        assert!(!WK_RULES[2].core_written, "TRACE_ENABLE is a command");
    }
}
