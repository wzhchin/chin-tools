use std::sync::Arc;

use flume::{Receiver, Sender};

use crate::{ActorSqlError, Result, model::*, pool_config::PoolConfig};

use super::client::ActorSqliteConnClient;

pub struct InnerActorSqlitePool {
    worker_tx: Sender<RspWrapper<ConnCmdReq, ConnCmdRsp>>,
    worker_rx: Receiver<RspWrapper<ConnCmdReq, ConnCmdRsp>>,
    config: PoolConfig,
}

pub type ActorSqlitePool = Arc<InnerActorSqlitePool>;

impl TryFrom<PoolConfig> for ActorSqlitePool {
    type Error = ActorSqlError;

    fn try_from(config: PoolConfig) -> Result<Self> {
        let (worker_tx, worker_rx) = flume::unbounded();

        for i in 0..config.pool_size.unwrap_or(1) {
            log::info!("creating initial worker-{i}");
            config.clone().spawn(worker_rx.clone())?;
        }
        let inner = InnerActorSqlitePool {
            worker_tx,
            worker_rx,
            config,
        };
        Ok(inner.into())
    }
}

impl InnerActorSqlitePool {
    pub fn check_size(&self) -> Result<()> {
        let full_count = self.config.pool_size.unwrap_or(1) as usize;
        loop {
            if self.worker_tx.receiver_count() < full_count {
                self.config.clone().spawn(self.worker_rx.clone())?;
            } else {
                return Ok(());
            }
        }
    }

    pub async fn get(&self) -> Result<ActorSqliteConnClient> {
        self.check_size()?;
        Ok(ActorSqliteConnClient {
            inner: self.worker_tx.clone(),
        })
    }
}
