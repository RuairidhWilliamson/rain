use std::borrow::Cow;

use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

#[derive(Debug, Default, Clone, Copy)]
struct JSONRPCVersion;

impl<'de> Deserialize<'de> for JSONRPCVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Inner<'a>(#[serde(borrow)] Cow<'a, str>);

        let Inner(ver) = Inner::deserialize(deserializer)?;

        match ver.as_ref() {
            "2.0" => Ok(JSONRPCVersion),
            _ => Err(serde::de::Error::custom(
                "expected JSON-RPC version \"2.0\"",
            )),
        }
    }
}

impl Serialize for JSONRPCVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("2.0")
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Request<T> {
    #[serde(default)]
    pub id: serde_json::Value,
    jsonrpc: JSONRPCVersion,
    pub method: String,
    pub params: Option<T>,
}

impl<T> Request<T> {
    pub fn ok_response<U>(self, resp: U) -> Response<U> {
        Response {
            id: self.id,
            jsonrpc: JSONRPCVersion,
            value: ResponseValue::Ok { result: resp },
        }
    }

    pub fn error_response(self, error: ResponseError) -> Response<()> {
        Response {
            id: self.id,
            jsonrpc: JSONRPCVersion,
            value: ResponseValue::Err { error },
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Notification<T> {
    pub method: Cow<'static, str>,
    jsonrpc: JSONRPCVersion,
    pub params: T,
}

impl<T> Notification<T> {
    pub fn new(method: impl Into<Cow<'static, str>>, params: T) -> Self {
        Self {
            method: method.into(),
            jsonrpc: JSONRPCVersion,
            params,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Response<T> {
    pub id: serde_json::Value,
    jsonrpc: JSONRPCVersion,
    #[serde(flatten)]
    pub value: ResponseValue<T>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ResponseValue<T> {
    Ok { result: T },
    Err { error: ResponseError },
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ResponseError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

pub trait Message: Serialize + DeserializeOwned {}

impl<T: Serialize + DeserializeOwned> Message for Request<T> {}
impl<T: Serialize + DeserializeOwned> Message for Response<T> {}
impl<T: Serialize + DeserializeOwned> Message for Notification<T> {}
