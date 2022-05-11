use std::sync::Arc;

use crate::cmd::{Parse};
use crate::tikv::errors::AsyncResult;
use crate::tikv::list::ListCommandCtx;
use crate::{Connection, Frame};
use crate::config::{is_use_txn_api};
use crate::utils::{resp_err, resp_invalid_arguments};

use tikv_client::Transaction;
use tokio::sync::Mutex;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct Ltrim {
    key: String,
    start: i64,
    end: i64,
    valid: bool,
}

impl Ltrim {
    pub fn new(key: &str, start: i64, end: i64) -> Ltrim {
        Ltrim {
            key: key.to_owned(),
            start: start,
            end: end,
            valid: true,
        }
    }

    pub fn new_invalid() -> Ltrim {
        Ltrim {
            key: "".to_owned(),
            start: 0,
            end: 0,
            valid: false,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Ltrim> {
        let key = parse.next_string()?;
        let start = parse.next_int()?;
        let end = parse.next_int()?;

        Ok(Ltrim { key, start, end, valid: true })
    }

    pub(crate) fn parse_argv(argv: &Vec<String>) -> crate::Result<Ltrim> {
        if argv.len() != 3 {
            return Ok(Ltrim::new_invalid());
        }
        let key = &argv[0];
        let start;
        let end;
        match argv[1].parse::<i64>() {
            Ok(v) => start = v,
            Err(_) => return Ok(Ltrim::new_invalid()),
        }

        match argv[2].parse::<i64>() {
            Ok(v) => end = v,
            Err(_) => return Ok(Ltrim::new_invalid()),
        }
        Ok(Ltrim::new(key, start, end))
    }

    #[instrument(skip(self, dst))]
    pub(crate) async fn apply(self, dst: &mut Connection) -> crate::Result<()> {
        let response = self.ltrim(None).await?;
        debug!(?response);
        dst.write_frame(&response).await?;

        Ok(())
    }

    pub async fn ltrim(&self, txn: Option<Arc<Mutex<Transaction>>>) -> AsyncResult<Frame> {
        if !self.valid {
            return Ok(resp_invalid_arguments());
        }
        if is_use_txn_api() {
            ListCommandCtx::new(txn).do_async_txnkv_ltrim(&self.key, self.start, self.end).await
        } else {
            Ok(resp_err("not supported yet"))
        }
    }
}