use std::time::SystemTime;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestHeader {
    pub config: Config,
    pub modified_time: SystemTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseWrapper<R> {
    Response(R),
    RestartPls(RestartReason),
    ServerPanic,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RestartReason {
    RainBinaryChanged,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Run(run::RunRequest),
    Info(info::InfoRequest),
    Shutdown(shutdown::ShutdownRequest),
    Clean(clean::CleanRequest),
}

pub trait RequestTrait: Into<Request> + private::Sealed {
    type Response: std::fmt::Debug + Serialize + DeserializeOwned;
}

mod private {
    pub trait Sealed {}
}

pub mod run {
    use std::path::PathBuf;

    use rain_lang::error::OwnedResolvedError;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct RunRequest {
        pub root: PathBuf,
        pub target: String,
    }

    impl From<RunRequest> for super::Request {
        fn from(req: RunRequest) -> Self {
            Self::Run(req)
        }
    }

    impl super::private::Sealed for RunRequest {}

    impl super::RequestTrait for RunRequest {
        type Response = RunResponse;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct RunResponse {
        pub prints: Vec<String>,
        pub output: Result<String, OwnedResolvedError>,
    }
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
        pub start_time: chrono::DateTime<chrono::Utc>,
        pub config: crate::config::Config,
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

pub mod clean {

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct CleanRequest;

    impl From<CleanRequest> for super::Request {
        fn from(req: CleanRequest) -> Self {
            Self::Clean(req)
        }
    }

    impl super::private::Sealed for CleanRequest {}

    impl super::RequestTrait for CleanRequest {
        type Response = Cleaned;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Cleaned;
}
