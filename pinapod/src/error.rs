#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinaPodError {
    BufferTooSmall,
    Overflow,
    InvalidBool,
    InvalidTag,
    InvalidDiscriminant,
    InvalidLength,
    InvalidUtf8,
}

impl core::fmt::Display for PinaPodError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BufferTooSmall => write!(f, "buffer too small"),
            Self::Overflow => write!(f, "field value exceeds max capacity"),
            Self::InvalidBool => write!(f, "invalid bool: byte must be 0 or 1"),
            Self::InvalidTag => write!(f, "invalid option tag: byte must be 0 or 1"),
            Self::InvalidDiscriminant => write!(f, "invalid enum discriminant"),
            Self::InvalidLength => write!(f, "stored length exceeds capacity"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in string field"),
        }
    }
}

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
