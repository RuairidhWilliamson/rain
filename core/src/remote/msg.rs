use std::{fmt::Debug, time::SystemTime};

use crate::config::Config;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestHeader {
    pub config: Config,
    pub modified_time: SystemTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Info,
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseWrapper {
    Response(Response),
    RestartPls(RestartReason),
}

impl From<Response> for ResponseWrapper {
    fn from(resp: Response) -> Self {
        Self::Response(resp)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RestartReason {
    RainBinaryChanged,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Info(ServerInfo),
    Goodbye,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub pid: u32,
}
