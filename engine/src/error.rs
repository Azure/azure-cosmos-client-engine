use std::{borrow::Cow, fmt::Display};

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq)]
pub enum ErrorKind {
    QueryPlanInvalid,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ErrorKind::QueryPlanInvalid => write!(f, "query plan is invalid"),
        }
    }
}

impl ErrorKind {
    pub fn with_source(self, source: impl std::error::Error + 'static) -> Error {
        Error::from(self).with_source(source)
    }

    pub fn with_message(self, message: impl Into<Cow<'static, str>>) -> Error {
        Error::from(self).with_message(message)
    }
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn std::error::Error>>,
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
    pub fn with_source(mut self, source: impl std::error::Error + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_message(mut self, message: impl Into<Cow<'static, str>>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.message {
            Some(message) => write!(f, "{}", message),
            None => write!(f, "{}", self.kind),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref()
    }
}
