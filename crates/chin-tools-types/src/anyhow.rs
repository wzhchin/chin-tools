pub type AResult<T> = anyhow::Result<T>;
pub type AError = anyhow::Error;
pub type EResult = anyhow::Result<()>;

pub use anyhow::Context as AnyhowContext;
pub use anyhow::anyhow as aanyhow;

#[macro_export]
macro_rules! log_and_err {
    ($fmt:expr $(, $arg:expr)*) => {{
        let msg = format!(concat!(" [{}:{}]", $fmt), file!(), line!(), $($arg)*);
        log::error!("{}", msg);
        Err($crate::anyhow::aanyhow!(msg))
    }};
}

#[macro_export]
macro_rules! eanyhow {
    ($msg:literal $(,)?) => {
        Err($crate::anyhow::aanyhow!($msg))
    };
    ($err:expr $(,)?) => {
        Err($crate::anyhow::aanyhow!($err))
    };
    ($fmt:expr, $($arg:tt)*) => {
        Err($crate::anyhow::aanyhow!($fmt, $($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::AResult;

    #[test]
    fn test() {
        let res: AResult<()> = crate::eanyhow!("what");
        assert!(res.is_err())
    }
}
