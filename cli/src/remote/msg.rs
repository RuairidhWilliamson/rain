use std::{path::PathBuf, time::SystemTime};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use rain_core::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestWrapper {
    pub header: RequestHeader,
    pub request: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestHeader {
    pub config: Config,
    pub modified_time: SystemTime,
    pub exe: PathBuf,
}

// Message from the server to the client
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    ServerPanic,
    RestartPls(RestartReason),
    Intermediate(Vec<u8>),
    Response(Vec<u8>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RestartReason {
    RainBinaryChanged,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Run(run::RunRequest),
    Info(info::InfoRequest),
    Inspect(inspect::InspectRequest),
    Shutdown(shutdown::ShutdownRequest),
    Clean(clean::CleanRequest),
    Prune(prune::PruneRequest),
}

pub trait RequestTrait: Into<Request> + private::Sealed {
    type Intermediate: std::fmt::Debug + Serialize + DeserializeOwned;
    type Response: std::fmt::Debug + Serialize + DeserializeOwned;
}

mod private {
    pub trait Sealed {}
}

pub mod run {
    use std::{path::PathBuf, time::Duration};

    use rain_core::CoreError;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct RunRequest {
        pub root: PathBuf,
        pub target: String,
        pub args: Vec<String>,
        pub resolve: bool,
        pub offline: bool,
        pub host_override: Option<String>,
    }

    impl From<RunRequest> for super::Request {
        fn from(req: RunRequest) -> Self {
            Self::Run(req)
        }
    }

    impl super::private::Sealed for RunRequest {}

    impl super::RequestTrait for RunRequest {
        type Intermediate = RunProgress;
        type Response = RunResponse;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub enum RunProgress {
        Print(String),
        EnterCall(String),
        ExitCall(String),
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct RunResponse {
        pub output: Result<String, CoreError>,
        pub elapsed: Duration,
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
        type Intermediate = ();
        type Response = InfoResponse;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct InfoResponse {
        pub pid: u32,
        pub start_time: chrono::DateTime<chrono::Utc>,
        pub config: rain_core::config::Config,
        pub stats: Stats,
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Stats {
        pub requests_received: usize,
        pub responses_sent: usize,
        pub cache_size: usize,
    }
}

pub mod inspect {
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct InspectRequest;

    impl From<InspectRequest> for super::Request {
        fn from(req: InspectRequest) -> Self {
            Self::Inspect(req)
        }
    }

    impl super::private::Sealed for InspectRequest {}

    impl super::RequestTrait for InspectRequest {
        type Intermediate = ();
        type Response = InspectResponse;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct InspectResponse {
        pub cache_size: usize,
        pub entries: Vec<String>,
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
        type Intermediate = ();
        type Response = Goodbye;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Goodbye;
}

pub mod clean {
    use std::{collections::HashMap, path::PathBuf};

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct CleanRequest;

    impl From<CleanRequest> for super::Request {
        fn from(req: CleanRequest) -> Self {
            Self::Clean(req)
        }
    }

    impl super::private::Sealed for CleanRequest {}

    impl super::RequestTrait for CleanRequest {
        type Intermediate = ();
        type Response = Cleaned;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Cleaned(pub HashMap<PathBuf, u64>);
}

pub mod prune {
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct PruneRequest;

    impl From<PruneRequest> for super::Request {
        fn from(req: PruneRequest) -> Self {
            Self::Prune(req)
        }
    }

    impl super::private::Sealed for PruneRequest {}

    impl super::RequestTrait for PruneRequest {
        type Intermediate = ();
        type Response = Pruned;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Pruned(pub u64);
}
