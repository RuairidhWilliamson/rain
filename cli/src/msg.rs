use std::time::SystemTime;

use rain_core::config::Config;

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
    RestartRequest,
}

impl From<Response> for ResponseWrapper {
    fn from(response: Response) -> Self {
        Self::Response(response)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Success,
    Failure,
}
