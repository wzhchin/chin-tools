pub mod client;
mod model;
pub mod pool;
pub mod pool_config;
mod worker;

use std::error::Error;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ActorSqlError {
    #[error("BuilderSqlError")]
    BuilderSqlError,
    #[error("Literal Error {0}")]
    LiteralError(String),
    #[error("Custom Sqlite Error {0}")]
    CustomRusqliteError(rusqlite::Error),
    #[error("Sqlite Error {0}")]
    RusqliteBuildError(String),
    #[error("Actor Sqlite Error {0}")]
    ActorError(Box<dyn Error + Send + Sync>),
    #[error("Actor Sqlite Error ({1}) {0}")]
    ActorErrorWithDesc(Box<dyn Error + Send + Sync>, String),
}

type Result<T> = std::result::Result<T, ActorSqlError>;
type EResult = Result<()>;

pub use model::ActorSqliteRow;
pub use rusqlite::types::Value as RsValue;

impl From<rusqlite::Error> for ActorSqlError {
    fn from(value: rusqlite::Error) -> Self {
        Self::CustomRusqliteError(value)
    }
}

impl<'a> From<&'a str> for ActorSqlError {
    fn from(value: &'a str) -> Self {
        Self::LiteralError(value.to_owned())
    }
}

impl<T: 'static + Send + Sync> From<flume::SendError<T>> for ActorSqlError {
    fn from(value: flume::SendError<T>) -> Self {
        Self::ActorErrorWithDesc(Box::new(value), "flume send error".to_owned())
    }
}

impl From<flume::RecvError> for ActorSqlError {
    fn from(value: flume::RecvError) -> Self {
        Self::ActorErrorWithDesc(Box::new(value), "flume recv error".to_string())
    }
}

impl<T: 'static + Send + Sync> From<oneshot::SendError<T>> for ActorSqlError {
    fn from(value: oneshot::SendError<T>) -> Self {
        Self::ActorErrorWithDesc(Box::new(value), "oneshot send error".to_string())
    }
}

impl From<oneshot::RecvError> for ActorSqlError {
    fn from(value: oneshot::RecvError) -> Self {
        Self::ActorErrorWithDesc(Box::new(value), "oneshot recv error".to_string())
    }
}
