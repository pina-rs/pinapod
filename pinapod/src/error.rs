//! The error type shared by every `PinaPod` read, validation, and update.

/// The error type returned by every `PinaPod` read, validation, and update.
///
/// The enum is `non_exhaustive` so new validation variants can be added in minor
/// releases. Downstream code that inspects variants must keep a wildcard arm.
///
/// <!-- {=podErrorContract|trim|linePrefix:"/// ":true} -->
/// | Variant               | Meaning                                                                      |
/// | --------------------- | ---------------------------------------------------------------------------- |
/// | `BufferTooSmall`      | The supplied slice cannot contain the required header, value, or active tail |
/// | `Overflow`            | A requested write exceeds a field capacity or checked arithmetic fails       |
/// | `InvalidBool`         | A stored boolean byte is not zero or one                                     |
/// | `InvalidTag`          | A stored option tag is not zero or one                                       |
/// | `InvalidDiscriminant` | A stored enum value has no declared variant                                  |
/// | `InvalidLength`       | A stored length exceeds capacity or violates the read contract               |
/// | `InvalidUtf8`         | Active string bytes are not UTF-8                                            |<!-- {/podErrorContract} -->
///
/// <!-- {=podErrorProgramErrorMapping|trim|linePrefix:"/// ":true} -->
/// The `solana-program-error` feature maps a small buffer to `ProgramError::AccountDataTooSmall`.
///
/// It maps every invalid representation and every write overflow to `ProgramError::InvalidAccountData`.<!-- {/podErrorProgramErrorMapping} -->
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PinaPodError {
    /// The supplied slice cannot contain the required header, value, or active tail.
    ///
    /// A fixed exact read of a longer slice reports [`Self::InvalidLength`] instead,
    /// because the slice has the wrong size rather than too little data.
    BufferTooSmall,
    /// A requested write exceeds a field capacity, or the checked arithmetic that
    /// calculates the new length fails.
    ///
    /// The destination keeps its previous contents, so a rejected write is a no-op.
    Overflow,
    /// A stored boolean byte is not zero or one.
    InvalidBool,
    /// A stored option tag is not zero or one.
    InvalidTag,
    /// A stored enum value has no declared variant.
    InvalidDiscriminant,
    /// A stored length exceeds the field capacity or violates the read contract.
    ///
    /// A reader returns this for a forged prefix, for a compact allocation whose size
    /// breaks the schema's size or tail-granularity rule, and for an eight-byte prefix
    /// that does not fit `usize` on a 32-bit target.
    InvalidLength,
    /// Active string bytes are not UTF-8.
    InvalidUtf8,
}

impl core::fmt::Display for PinaPodError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BufferTooSmall => write!(f, "buffer too small"),
            Self::Overflow => write!(f, "field value exceeds max capacity"),
            Self::InvalidBool => write!(f, "invalid bool: byte must be 0 or 1"),
            Self::InvalidTag => write!(f, "invalid option tag: prefix must encode 0 or 1"),
            Self::InvalidDiscriminant => write!(f, "invalid enum discriminant"),
            Self::InvalidLength => write!(f, "stored length exceeds capacity"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in string field"),
        }
    }
}

impl core::error::Error for PinaPodError {}

#[cfg(feature = "solana-program-error")]
impl From<PinaPodError> for solana_program_error::ProgramError {
    fn from(e: PinaPodError) -> Self {
        match e {
            PinaPodError::BufferTooSmall => solana_program_error::ProgramError::AccountDataTooSmall,
            PinaPodError::InvalidLength
            | PinaPodError::InvalidBool
            | PinaPodError::InvalidTag
            | PinaPodError::InvalidDiscriminant
            | PinaPodError::InvalidUtf8
            | PinaPodError::Overflow => solana_program_error::ProgramError::InvalidAccountData,
        }
    }
}
