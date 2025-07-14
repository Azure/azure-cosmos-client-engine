// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::{borrow::Cow, fmt::Display};

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// Indicates that the query plan or other information gathered from the gateway is invalid.
    ///
    /// This error is not recoverable and indicates a bug in the gateway.
    InvalidGatewayResponse,

    /// Indicates a deserialization failure, the details of which should be available in [`Error::source`](std::error::Error::source).
    ///
    /// This error is not recoverable and indicates a bug in the Gateway, as it should not be possible to receive a response that cannot be deserialized.
    DeserializationError,

    /// Indicates that a call specified a partition key range ID that is not known to the query pipeline.
    ///
    /// The error is not recoverable and indicates a bug in the language binding or backend, since it should not be possible to specify a partition key range ID that is not known.
    UnknownPartitionKeyRange,

    /// Indicates an internal error in the query pipeline.
    ///
    /// This error is not recoverable, and indicates a bug in the client engine. We return this error only to allow the calling SDK to log the error and report it to the user.
    InternalError,

    /// Indicates that the query plan requires features that are not supported by the query engine.
    ///
    /// This error is not recoverable, and should be very rare (or even impossible).
    /// The [`SUPPORTED_FEATURES`](crate::query::SUPPORTED_FEATURES) constant reports the features supported by the engine, and the language binding must provide that information to the gateway when generating a query plan.
    /// The gateway will return an error if the query requires features not listed in the supported features.
    /// We provide this error to cover cases where the language binding is incorrectly reporting the supported features, or edge cases where the engine is not correctly reporting the features it supports.
    UnsupportedQueryPlan,

    /// Indicates that a string parameter is not valid UTF-8.
    ///
    /// This error indicates either a bug in the language binding, or invalid data returned by the backend.
    InvalidUtf8String,

    /// Indicates that one of the provided arguments was null.
    ArgumentNull,

    /// Indicates that a Python error occurred. The source of the error will be the original Python error.
    PythonError,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ErrorKind::InvalidGatewayResponse => write!(f, "invalid data recieved from gateway"),
            ErrorKind::DeserializationError => write!(f, "deserialization error"),
            ErrorKind::UnknownPartitionKeyRange => write!(f, "unknown partition key range"),
            ErrorKind::InternalError => write!(f, "internal client engine error"),
            ErrorKind::UnsupportedQueryPlan => write!(f, "unsupported query plan"),
            ErrorKind::InvalidUtf8String => write!(f, "invalid UTF-8 string"),
            ErrorKind::ArgumentNull => write!(f, "provided argument was null"),
            ErrorKind::PythonError => write!(f, "python error"),
        }
    }
}

impl ErrorKind {
    pub fn with_source(self, source: impl std::error::Error + Send + Sync + 'static) -> Error {
        Error::from(self).with_source(source)
    }

    pub fn with_message(self, message: impl Into<Cow<'static, str>>) -> Error {
        Error::from(self).with_message(message)
    }
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
    message: Option<Cow<'static, str>>,
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self {
            kind,
            source: None,
            message: None,
        }
    }
}

impl Error {
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_message(mut self, message: impl Into<Cow<'static, str>>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn into_source(self) -> Option<Box<dyn std::error::Error + Send + Sync>> {
        self.source
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.message {
            Some(message) => write!(f, "{message}"),
            None => write!(f, "{}", self.kind),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        let source = self.source.as_ref()?;
        Some(&**source)
    }
}

#[cfg(feature = "python_conversions")]
impl From<pyo3::PyErr> for Error {
    fn from(err: pyo3::PyErr) -> Self {
        ErrorKind::PythonError.with_source(err)
    }
}

#[cfg(feature = "python_conversions")]
impl From<Error> for pyo3::PyErr {
    fn from(err: Error) -> Self {
        use std::error::Error;
        if err.kind() == ErrorKind::PythonError {
            if err.source().is_none() {
                return pyo3::PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string());
            }
            let err = err.into_source().expect("we just checked that it was Some");
            let err = err
                .downcast::<pyo3::PyErr>()
                .expect("PythonError's source must be a PyErr");
            *err
        } else {
            pyo3::PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string())
        }
    }
}
