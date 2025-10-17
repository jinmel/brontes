//! A custom RPC client implementation for transaction tracing.
//!
//! This module provides a custom RPC client implementation for transaction
//! tracing, as the functionality needed (particularly debug_traceBlockByHash
//! and debug_traceBlockByNumber) is not currently supported by the alloy
//! provider.
//!
//! The client handles JSON-RPC communication with Ethereum nodes using the
//! callTracer format, transforming the output into a parity-compatible trace
//! format. It provides methods for tracing blocks by hash or number, and
//! includes comprehensive error handling and logging for debugging purposes.
//!
//! Note: This is a temporary solution until the alloy provider adds support for
//! these tracing methods.

use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

use reqwest::{Client, Error as ReqwestError};
use reth_primitives::B256;
use reth_rpc_types::trace::geth::CallFrame;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug)]
pub enum RpcError {
    RequestError(ReqwestError),
    JsonError(serde_json::Error),
    RpcError { code: i64, message: String },
    UnexpectedResponse(String),
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RpcError::RequestError(e) => write!(f, "Request error: {}", e),
            RpcError::JsonError(e) => write!(f, "JSON error: {}", e),
            RpcError::RpcError { code, message } => write!(f, "RPC error {}: {}", code, message),
            RpcError::UnexpectedResponse(s) => write!(f, "Unexpected response: {}", s),
        }
    }
}

impl From<ReqwestError> for RpcError {
    fn from(err: ReqwestError) -> Self {
        RpcError::RequestError(err)
    }
}

impl From<serde_json::Error> for RpcError {
    fn from(err: serde_json::Error) -> Self {
        RpcError::JsonError(err)
    }
}

impl std::error::Error for RpcError {}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method:  String,
    params:  Value,
    id:      u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    result:  Option<Value>,
    error:   Option<JsonRpcError>,
    id:      u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcError {
    code:    i64,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceOptions {
    pub tracer:        String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "tracerConfig")]
    pub tracer_config: Option<TracerConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TracerConfig {
    #[serde(rename = "withLog")]
    pub with_log: bool,
}

/// Result from debug_traceBlockByHash/Number with callTracer
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallTracerResult {
    tx_hash: B256,
    result:  CallFrame,
}

#[derive(Debug)]
pub struct RpcClient {
    endpoint: String,
    client:   Client,
    id:       AtomicU64,
}

impl Clone for RpcClient {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            client:   self.client.clone(),
            id:       AtomicU64::new(self.id.load(Ordering::SeqCst)),
        }
    }
}

impl RpcClient {
    pub fn new(url: reqwest::Url) -> Self {
        let endpoint = url.to_string();
        Self { endpoint, client: Client::new(), id: AtomicU64::new(1) }
    }

    pub async fn call<T: for<'a> Deserialize<'a>>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, RpcError> {
        tracing::debug!(target: "rpc_client", "calling method: {:?}", method);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: self.id.load(Ordering::SeqCst),
        };
        tracing::debug!(target: "rpc_client", "request: {:?}", request);
        self.id.fetch_add(1, Ordering::SeqCst);

        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await?;

        let json: JsonRpcResponse = response.json().await?;
        if let Some(error) = json.error {
            return Err(RpcError::RpcError { code: error.code, message: error.message });
        }

        if let Some(result) = json.result {
            match serde_json::from_value::<T>(result) {
                Ok(parsed_result) => Ok(parsed_result),
                Err(err) => Err(RpcError::JsonError(err)),
            }
        } else {
            Err(RpcError::UnexpectedResponse("No result in JSON-RPC response".to_string()))
        }
    }
}
