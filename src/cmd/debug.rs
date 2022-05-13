use crate::utils::{
    resp_ok,
    resp_err,
};
use crate::{Connection, Parse};
use crate::config::LOGGER;
use crate::tikv::{
    start_profiler,
    stop_profiler,
};
use slog::debug;

#[derive(Debug)]
pub struct Debug {
    subcommand: String,
}

impl Debug {
    pub fn new(subcommand: impl ToString) -> Debug {
        Debug {
            subcommand: subcommand.to_string(),
        }
    }

    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Debug> {
        let subcommand = parse.next_string()?;

        Ok(Debug::new(subcommand))
    }

    pub(crate) async fn apply(self, dst: &mut Connection) -> crate::Result<()> {
        let response = match self.subcommand.to_lowercase().as_str() {
            "profiler_start" => {
                start_profiler();
                resp_ok()
            },
            "profiler_stop" => {
                stop_profiler();
                resp_ok()
            },
            _ => {
                resp_err("not supported debug subcommand")
            }
        };

        debug!(LOGGER, "res, {} -> {}, {:?}", dst.local_addr(), dst.peer_addr(), response);

        dst.write_frame(&response).await?;

        Ok(())
    }

}