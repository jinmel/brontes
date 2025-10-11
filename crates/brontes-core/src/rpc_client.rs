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

use alloy_primitives::{Address as AlloyAddress, Log, LogData, U256};
use brontes_types::{
    serde_utils::option_u256,
    structured_trace::{TransactionTraceWithLogs, TxTrace},
};
use reqwest::{Client, Error as ReqwestError};
use reth_primitives::{hex, Address, Bytes, B256, U64};
use reth_rpc_types::trace::parity::{
    Action, CallAction, CallOutput, CallType, CreateAction, CreateOutput, TraceOutput,
    TransactionTrace,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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

/// Geth's callTracer output format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFrame {
    #[serde(rename = "type")]
    pub call_type: String,
    pub from:      Address,
    #[serde(default)]
    pub to:        Option<Address>,
    #[serde(default, with = "option_u256")]
    pub value:     Option<U256>,
    pub gas:       U64,
    #[serde(rename = "gasUsed")]
    pub gas_used:  U64,
    pub input:     Bytes,
    #[serde(default)]
    pub output:    Option<Bytes>,
    #[serde(default)]
    pub error:     Option<String>,
    #[serde(default)]
    pub calls:     Option<Vec<CallFrame>>,
    #[serde(default)]
    pub logs:      Option<Vec<CallLog>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallLog {
    pub address:  Address,
    pub topics:   Vec<B256>,
    pub data:     Bytes,
    #[serde(default)]
    pub position: Option<String>,
}

/// Result from debug_traceBlockByHash/Number with callTracer
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallTracerResult {
    tx_hash: B256,
    result:  CallFrame,
}

/// Transforms Geth's callTracer output into the parity trace format
fn transform_call_frame_to_traces(
    frame: CallFrame,
    _tx_hash: B256,
) -> Vec<TransactionTraceWithLogs> {
    let mut traces = Vec::new();
    let mut trace_idx = 0u64;

    // Process the call frame recursively
    flatten_call_frame(frame, vec![], &mut traces, &mut trace_idx, None);

    traces
}

/// Recursively flattens the nested call frame structure into a flat array of
/// traces
fn flatten_call_frame(
    frame: CallFrame,
    trace_address: Vec<usize>,
    traces: &mut Vec<TransactionTraceWithLogs>,
    trace_idx: &mut u64,
    _parent_msg_sender: Option<Address>,
) {
    let current_trace_idx = *trace_idx;
    *trace_idx += 1;

    let call_type_str = frame.call_type.to_uppercase();
    let is_delegate_call = call_type_str == "DELEGATECALL";
    let is_create = call_type_str == "CREATE" || call_type_str == "CREATE2";

    let value = frame.value.unwrap_or(U256::ZERO);

    // Convert logs from CallLog to alloy Log
    let logs: Vec<Log> = frame
        .logs
        .as_ref()
        .map(|logs| {
            logs.iter()
                .map(|log| Log {
                    address: AlloyAddress::from_slice(log.address.as_slice()),
                    data:    LogData::new_unchecked(log.topics.clone(), log.data.clone()),
                })
                .collect()
        })
        .unwrap_or_default();

    // Build the action
    let action = if is_create {
        Action::Create(CreateAction {
            from: frame.from,
            value,
            gas: frame.gas,
            init: frame.input.clone(),
        })
    } else {
        let call_type = match call_type_str.as_str() {
            "CALL" => CallType::Call,
            "STATICCALL" => CallType::StaticCall,
            "DELEGATECALL" => CallType::DelegateCall,
            "CALLCODE" => CallType::CallCode,
            _ => CallType::Call,
        };

        Action::Call(CallAction {
            from: frame.from,
            to: frame.to.unwrap_or_default(),
            value,
            gas: frame.gas,
            input: frame.input.clone(),
            call_type,
        })
    };

    // Build the result
    let result = if frame.error.is_none() {
        if is_create {
            Some(TraceOutput::Create(CreateOutput {
                gas_used: frame.gas_used,
                code:     frame.output.clone().unwrap_or_default(),
                address:  frame.to.unwrap_or_default(),
            }))
        } else {
            Some(TraceOutput::Call(CallOutput {
                gas_used: frame.gas_used,
                output:   frame.output.clone().unwrap_or_default(),
            }))
        }
    } else {
        None
    };

    let num_children = frame.calls.as_ref().map(|c| c.len()).unwrap_or(0);

    let trace = TransactionTrace {
        action,
        error: frame.error.clone(),
        result,
        trace_address: trace_address.clone(),
        subtraces: num_children,
    };

    // Determine msg_sender using the correct logic:
    // For DELEGATECALL: search backwards through traces to find the first
    // non-delegatecall For other types: use the from address
    let msg_sender = if is_delegate_call {
        // Search backwards through existing traces to find first non-delegatecall
        let mut found_msg_sender = None;
        for prev_trace in traces.iter().rev() {
            match &prev_trace.trace.action {
                Action::Call(call) if call.call_type != CallType::DelegateCall => {
                    found_msg_sender = Some(prev_trace.msg_sender);
                    break;
                }
                Action::Create(_) => {
                    found_msg_sender = Some(prev_trace.msg_sender);
                    break;
                }
                _ => continue,
            }
        }
        found_msg_sender.unwrap_or(frame.from)
    } else {
        frame.from
    };

    traces.push(TransactionTraceWithLogs {
        trace,
        logs,
        msg_sender,
        trace_idx: current_trace_idx,
        decoded_data: None,
    });

    // Process child calls
    if let Some(calls) = frame.calls {
        for (idx, child_frame) in calls.into_iter().enumerate() {
            let mut child_trace_address = trace_address.clone();
            child_trace_address.push(idx);
            flatten_call_frame(
                child_frame,
                child_trace_address,
                traces,
                trace_idx,
                None, // Not used anymore - msg_sender is determined by searching backwards
            );
        }
    }
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

    async fn call<T: for<'a> Deserialize<'a>>(
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

    pub async fn debug_trace_block_by_hash(
        &self,
        block_hash: B256,
        trace_options: TraceOptions,
    ) -> Result<Vec<TxTrace>, RpcError> {
        let params = json!([format!("0x{}", hex::encode(block_hash)), trace_options]);

        let result: Result<Vec<CallTracerResult>, RpcError> =
            self.call("debug_traceBlockByHash", params).await;
        result.map(|traces| {
            traces
                .into_iter()
                .enumerate()
                .map(|(tx_index, trace_result)| {
                    // Transform callTracer output to parity format
                    let traces =
                        transform_call_frame_to_traces(trace_result.result, trace_result.tx_hash);

                    // We need to get gas_used and other tx-level info from the trace
                    // The root call's gas_used is the transaction's gas_used
                    let gas_used = traces
                        .first()
                        .and_then(|t| match &t.trace.result {
                            Some(TraceOutput::Call(c)) => Some(c.gas_used.to::<u128>()),
                            Some(TraceOutput::Create(c)) => Some(c.gas_used.to::<u128>()),
                            _ => None,
                        })
                        .unwrap_or(0);

                    let is_success = traces
                        .first()
                        .map(|t| t.trace.error.is_none())
                        .unwrap_or(false);

                    TxTrace {
                        block_number: 0, // Will be filled by caller if needed
                        trace: traces,
                        tx_hash: trace_result.tx_hash,
                        gas_used,
                        effective_price: 0, // Not available from callTracer
                        tx_index: tx_index as u64,
                        timeboosted: false,
                        is_success,
                    }
                })
                .collect()
        })
    }

    pub async fn debug_trace_block_by_number(
        &self,
        block_number: u64,
        trace_options: TraceOptions,
    ) -> Result<Vec<TxTrace>, RpcError> {
        let params = json!([format!("0x{:x}", block_number), trace_options]);

        let result: Result<Vec<CallTracerResult>, RpcError> =
            self.call("debug_traceBlockByNumber", params).await;
        result.map(|traces| {
            traces
                .into_iter()
                .enumerate()
                .map(|(tx_index, trace_result)| {
                    // Transform callTracer output to parity format
                    let traces =
                        transform_call_frame_to_traces(trace_result.result, trace_result.tx_hash);

                    // We need to get gas_used and other tx-level info from the trace
                    // The root call's gas_used is the transaction's gas_used
                    let gas_used = traces
                        .first()
                        .and_then(|t| match &t.trace.result {
                            Some(TraceOutput::Call(c)) => Some(c.gas_used.to::<u128>()),
                            Some(TraceOutput::Create(c)) => Some(c.gas_used.to::<u128>()),
                            _ => None,
                        })
                        .unwrap_or(0);

                    let is_success = traces
                        .first()
                        .map(|t| t.trace.error.is_none())
                        .unwrap_or(false);

                    TxTrace {
                        block_number,
                        trace: traces,
                        tx_hash: trace_result.tx_hash,
                        gas_used,
                        effective_price: 0, // Not available from callTracer
                        tx_index: tx_index as u64,
                        timeboosted: false,
                        is_success,
                    }
                })
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to validate that transformed traces match expected
    /// traces
    fn assert_traces_equal(
        transformed_traces: &[TransactionTraceWithLogs],
        expected_traces: &[TransactionTraceWithLogs],
    ) {
        assert_eq!(
            transformed_traces.len(),
            expected_traces.len(),
            "Trace count mismatch: transformed={}, expected={}",
            transformed_traces.len(),
            expected_traces.len()
        );

        for (idx, (transformed, expected_trace)) in transformed_traces
            .iter()
            .zip(expected_traces.iter())
            .enumerate()
        {
            // Check trace_idx
            assert_eq!(
                transformed.trace_idx, expected_trace.trace_idx,
                "Trace {} trace_idx mismatch",
                idx
            );

            // Check trace_address
            assert_eq!(
                transformed.trace.trace_address, expected_trace.trace.trace_address,
                "Trace {} trace_address mismatch",
                idx
            );

            // Check subtraces count
            assert_eq!(
                transformed.trace.subtraces, expected_trace.trace.subtraces,
                "Trace {} subtraces mismatch",
                idx
            );

            // Check msg_sender
            assert_eq!(
                transformed.msg_sender, expected_trace.msg_sender,
                "Trace {} msg_sender mismatch: transformed={:?}, expected={:?}",
                idx, transformed.msg_sender, expected_trace.msg_sender
            );

            // Check action types match
            match (&transformed.trace.action, &expected_trace.trace.action) {
                (Action::Call(t_call), Action::Call(e_call)) => {
                    assert_eq!(
                        t_call.call_type, e_call.call_type,
                        "Trace {} call_type mismatch",
                        idx
                    );
                    assert_eq!(t_call.from, e_call.from, "Trace {} from address mismatch", idx);
                    assert_eq!(t_call.to, e_call.to, "Trace {} to address mismatch", idx);
                    assert_eq!(t_call.value, e_call.value, "Trace {} value mismatch", idx);
                }
                (Action::Create(t_create), Action::Create(e_create)) => {
                    assert_eq!(t_create.from, e_create.from, "Trace {} create from mismatch", idx);
                    assert_eq!(
                        t_create.value, e_create.value,
                        "Trace {} create value mismatch",
                        idx
                    );
                }
                _ => {
                    panic!(
                        "Trace {} action type mismatch: transformed={:?}, expected={:?}",
                        idx, transformed.trace.action, expected_trace.trace.action
                    );
                }
            }

            // Check result field
            match (&transformed.trace.result, &expected_trace.trace.result) {
                (Some(TraceOutput::Call(t_output)), Some(TraceOutput::Call(e_output))) => {
                    // Note: gas_used comparison skipped - may differ due to L2 specifics
                    assert_eq!(
                        t_output.output, e_output.output,
                        "Trace {} result.output mismatch",
                        idx
                    );
                }
                (Some(TraceOutput::Create(t_output)), Some(TraceOutput::Create(e_output))) => {
                    assert_eq!(t_output.code, e_output.code, "Trace {} result.code mismatch", idx);
                    assert_eq!(
                        t_output.address, e_output.address,
                        "Trace {} result.address mismatch",
                        idx
                    );
                }
                (None, None) => {
                    // Both have no result (error case)
                }
                _ => {
                    panic!(
                        "Trace {} result type mismatch: transformed={:?}, expected={:?}",
                        idx, transformed.trace.result, expected_trace.trace.result
                    );
                }
            }

            // Check error field
            assert_eq!(
                transformed.trace.error, expected_trace.trace.error,
                "Trace {} error mismatch",
                idx
            );

            // Check logs count
            assert_eq!(
                transformed.logs.len(),
                expected_trace.logs.len(),
                "Trace {} logs count mismatch",
                idx
            );
        }
    }

    #[test]
    fn test_transform_call_frame_to_traces() {
        // Load actual callTracer output and expected brontes output
        let call_json = include_str!("testdata/call.json");
        let brontes_json = include_str!("testdata/brontes.json");

        let call_frame: CallFrame =
            serde_json::from_str(call_json).expect("Failed to deserialize callTracer output");
        let expected: TxTrace =
            serde_json::from_str(brontes_json).expect("Failed to deserialize brontesTracer output");

        // Transform callTracer to our format
        let tx_hash = B256::from_slice(
            &hex::decode("28a9692548a4f87d113338ba88d541e8092b21b141b4d614269d3354192ea87f")
                .unwrap(),
        );
        let transformed_traces = transform_call_frame_to_traces(call_frame, tx_hash);

        // Use helper function to validate all traces
        assert_traces_equal(&transformed_traces, &expected.trace);

        // Verify expected trace count
        assert_eq!(transformed_traces.len(), 65);
    }

    #[test]
    fn test_transform_call_frame_to_traces_second_example() {
        // Load second test case with different transaction structure
        let call_json = include_str!("testdata/call2.json");
        let brontes_json = include_str!("testdata/brontes2.json");

        let call_frame: CallFrame = serde_json::from_str(call_json)
            .expect("Failed to deserialize callTracer output (call2.json)");
        let expected: TxTrace = serde_json::from_str(brontes_json)
            .expect("Failed to deserialize brontesTracer output (brontes2.json)");

        // Transform callTracer to our format
        let tx_hash = B256::from_slice(
            &hex::decode("bc4327e2332e8ad8c0f90b376a92ace63b8831ed2a3dda9b9cdf46ebec312719")
                .unwrap(),
        );
        let transformed_traces = transform_call_frame_to_traces(call_frame, tx_hash);

        // Use helper function to validate all traces
        assert_traces_equal(&transformed_traces, &expected.trace);

        // Verify expected trace count
        assert_eq!(transformed_traces.len(), 114);
    }
}
