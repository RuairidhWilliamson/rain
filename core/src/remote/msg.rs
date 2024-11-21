use std::time::SystemTime;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::config::Config;

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestHeader {
    pub config: Config,
    pub modified_time: SystemTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseWrapper<R> {
    Response(R),
    RestartPls(RestartReason),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RestartReason {
    RainBinaryChanged,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Info(info::InfoRequest),
    Shutdown(shutdown::ShutdownRequest),
}

pub trait RequestTrait: Into<Request> + private::Sealed {
    type Response: std::fmt::Debug + Serialize + DeserializeOwned;
}

mod private {
    pub trait Sealed {}
}

pub mod info {
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct InfoRequest;

    impl From<InfoRequest> for super::Request {
        fn from(req: InfoRequest) -> Self {
            Self::Info(req)
        }
    }

    impl super::private::Sealed for InfoRequest {}

    impl super::RequestTrait for InfoRequest {
        type Response = InfoResponse;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct InfoResponse {
        pub pid: u32,
    }
}

pub mod shutdown {
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct ShutdownRequest;

    impl From<ShutdownRequest> for super::Request {
        fn from(req: ShutdownRequest) -> Self {
            Self::Shutdown(req)
        }
    }

    impl super::private::Sealed for ShutdownRequest {}

    impl super::RequestTrait for ShutdownRequest {
        type Response = Goodbye;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Goodbye;
}
