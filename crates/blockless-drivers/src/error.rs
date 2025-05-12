pub use anyhow::{Context, Error};

#[derive(Debug)]
pub enum ErrorKind {
    ConnectError,
    EofError,
    MemoryNotExport,
    BadFileDescriptor,
    DriverNotFound,
    Addrnotavail,
    DriverBadOpen,
    DriverBadParams,
    PermissionDeny,
    Unknown,
}

impl std::error::Error for ErrorKind {}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::ConnectError => write!(f, "Connect Error."),
            Self::MemoryNotExport => write!(f, "Memory not export"),
            Self::DriverNotFound => write!(f, "Driver not found."),
            Self::DriverBadOpen => write!(f, "Driver bad open."),
            Self::BadFileDescriptor => write!(f, "Bad file descriptor."),
            Self::DriverBadParams => write!(f, "Driver bad params."),
            Self::Addrnotavail => write!(f, "Address is not avail."),
            Self::Unknown => write!(f, "Unknown error."),
            Self::EofError => write!(f, "End of file error."),
            Self::PermissionDeny => write!(f, "Permision deny."),
        }
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum HttpErrorKind {
    InvalidDriver,
    InvalidHandle,
    MemoryAccessError,
    BufferTooSmall,
    HeaderNotFound,
    Utf8Error,
    DestinationNotAllowed,
    InvalidMethod,
    InvalidEncoding,
    InvalidUrl,
    RequestError,
    HeadersValidationError,
    RuntimeError,
    TooManySessions,
    PermissionDeny,
}

impl std::error::Error for HttpErrorKind {}

impl std::fmt::Display for HttpErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::InvalidDriver => write!(f, "Invalid Driver"),
            Self::InvalidHandle => write!(f, "Invalid Error"),
            Self::MemoryAccessError => write!(f, "Memory Access Error"),
            Self::BufferTooSmall => write!(f, "Buffer too small"),
            Self::HeaderNotFound => write!(f, "Header not found"),
            Self::Utf8Error => write!(f, "Utf8 error"),
            Self::DestinationNotAllowed => write!(f, "Destination not allowed"),
            Self::InvalidMethod => write!(f, "Invalid method"),
            Self::InvalidEncoding => write!(f, "Invalid encoding"),
            Self::InvalidUrl => write!(f, "Invalid url"),
            Self::RequestError => write!(f, "Request url"),
            Self::RuntimeError => write!(f, "Runtime error"),
            Self::TooManySessions => write!(f, "Too many sessions"),
            Self::PermissionDeny => write!(f, "Permission deny."),
            Self::HeadersValidationError => write!(f, "Headers are malformed."),
        }
    }
}

#[derive(Debug)]
pub enum IpfsErrorKind {
    InvalidHandle,
    Utf8Error,
    InvalidMethod,
    InvalidEncoding,
    InvalidParameter,
    RequestError,
    RuntimeError,
    TooManySessions,
    PermissionDeny,
}

impl std::error::Error for IpfsErrorKind {}

impl std::fmt::Display for IpfsErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::InvalidHandle => write!(f, "Invalid Error"),
            Self::Utf8Error => write!(f, "Utf8 error"),
            Self::InvalidMethod => write!(f, "Invalid method"),
            Self::InvalidEncoding => write!(f, "Invalid encoding"),
            Self::InvalidParameter => write!(f, "Invalid parameter"),
            Self::RequestError => write!(f, "Request url"),
            Self::RuntimeError => write!(f, "Runtime error"),
            Self::TooManySessions => write!(f, "Too many sessions"),
            Self::PermissionDeny => write!(f, "Permission deny."),
        }
    }
}

#[derive(Debug)]
pub enum S3ErrorKind {
    InvalidHandle,
    Utf8Error,
    InvalidMethod,
    InvalidEncoding,
    CredentialsError,
    RegionError,
    InvalidParameter,
    RequestError,
    RuntimeError,
    TooManySessions,
    PermissionDeny,
}

impl std::error::Error for S3ErrorKind {}

impl std::fmt::Display for S3ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::InvalidHandle => write!(f, "Invalid Error"),
            Self::Utf8Error => write!(f, "Utf8 error"),
            Self::InvalidMethod => write!(f, "Invalid method"),
            Self::InvalidEncoding => write!(f, "Invalid encoding"),
            Self::CredentialsError => write!(f, "Credentials encoding"),
            Self::RegionError => write!(f, "Region encoding"),
            Self::InvalidParameter => write!(f, "Invalid parameter"),
            Self::RequestError => write!(f, "Request url"),
            Self::RuntimeError => write!(f, "Runtime error"),
            Self::TooManySessions => write!(f, "Too many sessions"),
            Self::PermissionDeny => write!(f, "Permission deny."),
        }
    }
}

#[derive(Debug)]
pub enum BlocklessMemoryErrorKind {
    InvalidHandle,
    RuntimeError,
    InvalidParameter,
}

impl std::error::Error for BlocklessMemoryErrorKind {}

impl std::fmt::Display for BlocklessMemoryErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::RuntimeError => write!(f, "Runtime error"),
            Self::InvalidHandle => write!(f, "Invalid Error"),
            Self::InvalidParameter => write!(f, "Invalid parameter"),
        }
    }
}

#[derive(Debug)]
pub enum CgiErrorKind {
    InvalidHandle,
    RuntimeError,
    InvalidParameter,
    InvalidExtension,
}

impl std::error::Error for CgiErrorKind {}

impl std::fmt::Display for CgiErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::RuntimeError => write!(f, "Runtime error"),
            Self::InvalidHandle => write!(f, "Invalid Error"),
            Self::InvalidParameter => write!(f, "Invalid parameter"),
            Self::InvalidExtension => write!(f, "Invalid extension"),
        }
    }
}

#[derive(Debug)]
pub enum BlocklessSocketErrorKind {
    ConnectRefused,
    ParameterError,
    ConnectionReset,
    AddressInUse,
}

impl std::error::Error for BlocklessSocketErrorKind {}

impl std::fmt::Display for BlocklessSocketErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::ConnectRefused => write!(f, "Connect Refused error"),
            Self::ConnectionReset => write!(f, "Connection Reset Error"),
            Self::AddressInUse => write!(f, "Address In Use"),
            Self::ParameterError => write!(f, "Parameter Error"),
        }
    }
}

#[derive(Debug)]
pub enum LlmErrorKind {
    ModelNotSet,               // 1
    ModelNotSupported,         // 2
    ModelInitializationFailed, // 3
    ModelCompletionFailed,     // 4
    ModelOptionsNotSet,        // 5
    ModelShutdownFailed,       // 6
    Utf8Error,                 // 7
    RuntimeError,              // 8
}
