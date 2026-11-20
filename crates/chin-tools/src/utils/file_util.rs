use tokio::{
    fs::File,
    io::{self, BufWriter},
};
use tokio_util::io::ReaderStream;

use crate::anyhow::AResult;
use crate::anyhow::EResult;

use futures::{Stream, TryStreamExt};

use bytes::Bytes;
use std::path::Path;
use std::path::PathBuf;

pub async fn stream_to_file_async<S>(stream: S, save_file: &PathBuf) -> EResult
where
    S: Stream<Item = AResult<Bytes>>,
{
    async {
        let body_with_io_error =
            stream.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err));
        let body_reader = tokio_util::io::StreamReader::new(body_with_io_error);
        futures::pin_mut!(body_reader);

        let mut file = BufWriter::new(File::create(save_file).await?);

        tokio::io::copy(&mut body_reader, &mut file).await?;

        Ok::<_, io::Error>(())
    }
    .await?;

    Ok(())
}

pub async fn file_to_stream_async(filepath: &impl AsRef<Path>) -> AResult<ReaderStream<File>> {
    use crate::log_and_err;

    let file = match tokio::fs::File::open(&filepath).await {
        Ok(file) => file,
        Err(err) => return crate::log_and_err!("Read file error: {}", err),
    };

    Ok(ReaderStream::new(file))
}
