use flume::Sender;
use log::debug;

use crate::{ActorSqlError, EResult, Result, model::*};

#[derive(Clone)]
pub struct ActorSqliteTxClient {
    pub(super) inner: flume::Sender<RspWrapper<TxCmdReq, TxCmdRsp>>,
}

pub struct ActorSqliteConnClient {
    pub(super) inner: Sender<RspWrapper<ConnCmdReq, ConnCmdRsp>>,
}

impl ActorSqliteConnClient {
    async fn inner(&self, req: ConnCmdReq) -> Result<ConnCmdRsp> {
        let (otx, orx) = oneshot::channel();
        self.inner.send(RspWrapper { command: req, otx })?;
        orx.await?
    }

    pub async fn execute<S: Into<String>>(&self, sql: S, params: SqlValueVec) -> Result<usize> {
        match self
            .inner(ConnCmdReq::Command(CmdReq::Exec {
                sql: sql.into(),
                params,
            }))
            .await?
        {
            ConnCmdRsp::Cmd(CmdResult::Exec(count)) => Ok(count),
            _ => Err(ActorSqlError::RusqliteBuildError(
                "affected size".to_owned(),
            )),
        }
    }

    pub async fn query<S: Into<String>>(
        &self,
        sql: S,
        params: SqlValueVec,
    ) -> Result<Vec<ActorSqliteRow>> {
        match self
            .inner(ConnCmdReq::Command(CmdReq::QueryMap {
                sql: sql.into(),
                params,
            }))
            .await?
        {
            ConnCmdRsp::Cmd(CmdResult::QueryMap(res)) => Ok(res),
            _ => Err(ActorSqlError::RusqliteBuildError(
                "not query result".to_owned(),
            )),
        }
    }

    pub async fn transaction(&mut self) -> Result<ActorSqliteTxClient> {
        match self.inner(ConnCmdReq::Transaction).await? {
            ConnCmdRsp::Tx(tx) => Ok(ActorSqliteTxClient { inner: tx }),
            _ => Err(ActorSqlError::RusqliteBuildError(
                "unable to create tx client".to_owned(),
            )),
        }
    }
}

impl ActorSqliteTxClient {
    async fn inner(&self, command: TxCmdReq) -> Result<TxCmdRsp> {
        let (otx, orx) = oneshot::channel();
        debug!("begin to send tx cmd {command:?}");
        self.inner.send(RspWrapper { command, otx })?;
        orx.await?
    }

    pub async fn execute(&self, sql: String, params: SqlValueVec) -> Result<usize> {
        match self
            .inner(TxCmdReq::Command(CmdReq::Exec { sql, params }))
            .await?
        {
            TxCmdRsp::Cmd(CmdResult::Exec(res)) => Ok(res),
            _ => Err("affected size".into()),
        }
    }

    pub async fn query(&self, sql: String, params: SqlValueVec) -> Result<Vec<ActorSqliteRow>> {
        match self
            .inner(TxCmdReq::Command(CmdReq::QueryMap { sql, params }))
            .await?
        {
            TxCmdRsp::Cmd(CmdResult::QueryMap(res)) => Ok(res),
            _ => Err("not query result".into()),
        }
    }

    pub async fn commit(&self) -> EResult {
        match self.inner(TxCmdReq::Commit).await? {
            TxCmdRsp::Committed => Ok(()),
            _ => Err("fail to commit".into()),
        }
    }

    pub async fn rollback(&self) -> EResult {
        match self.inner(TxCmdReq::Rollback).await? {
            TxCmdRsp::Rollbacked => Ok(()),
            _ => Err("fail to rollback".into()),
        }
    }
}
