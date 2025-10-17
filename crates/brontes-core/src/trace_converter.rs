//! Trace conversion utilities for transforming Geth's callTracer output to parity format.
//!
//! This module handles the transformation of Geth's debug_trace callTracer output
//! into a parity-compatible trace format that is used throughout the Brontes codebase.

use alloy_primitives::{Log, LogData, U256};
use brontes_types::structured_trace::{TransactionTraceWithLogs, TxTrace};
use reth_primitives::{Address, U64};
use reth_rpc_types::trace::{
    geth::CallFrame,
    parity::{
        Action, CallAction, CallOutput, CallType, CreateAction, CreateOutput, TraceOutput,
        TransactionTrace,
    },
};
use brontes_types::timeboost_tx::TimeboostTransactionReceipt;

/// Transforms Geth's callTracer output into the parity trace format
pub fn transform_call_frame_to_parity_trace(
    frame: CallFrame,
    receipt: &TimeboostTransactionReceipt
) -> TxTrace {
    let mut traces = Vec::new();
    let mut trace_idx = 0u64;

    // Process the call frame recursively
    flatten_call_frame(frame, vec![], &mut traces, &mut trace_idx, None);

    TxTrace {
      block_number: receipt.inner.block_number.unwrap(),
      trace: traces,
      tx_hash: receipt.inner.transaction_hash,
      gas_used: receipt.inner.gas_used,
      effective_price: receipt.inner.effective_gas_price,
      tx_index: receipt.inner.transaction_index.unwrap(),
      timeboosted: receipt.timeboosted,
      is_success: receipt.inner.inner.is_success(),
    }
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

    let call_type_str = frame.typ.to_uppercase();
    let is_delegate_call = call_type_str == "DELEGATECALL";
    let is_create = call_type_str == "CREATE" || call_type_str == "CREATE2";

    let value = frame.value.unwrap_or(U256::ZERO);

    // Convert logs from CallLogFrame to alloy Log
    let logs: Vec<Log> = frame
        .logs
        .iter()
        .filter_map(|log| {
            // Only process logs that have all required fields
            match (&log.address, &log.topics, &log.data) {
                (Some(address), Some(topics), Some(data)) => Some(Log {
                    address: *address,
                    data:    LogData::new_unchecked(topics.clone(), data.clone()),
                }),
                _ => None,
            }
        })
        .collect();

    // Build the action
    let action = if is_create {
        Action::Create(CreateAction {
            from: Address::from_slice(frame.from.as_slice()),
            value,
            gas: U64::from(frame.gas.to::<u64>()),
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
            from: Address::from_slice(frame.from.as_slice()),
            to: frame
                .to
                .map(|addr| Address::from_slice(addr.as_slice()))
                .unwrap_or_default(),
            value,
            gas: U64::from(frame.gas.to::<u64>()),
            input: frame.input.clone(),
            call_type,
        })
    };

    // Build the result
    let result = if frame.error.is_none() {
        if is_create {
            Some(TraceOutput::Create(CreateOutput {
                gas_used: U64::from(frame.gas_used.to::<u64>()),
                code:     frame.output.clone().unwrap_or_default(),
                address:  frame
                    .to
                    .map(|addr| Address::from_slice(addr.as_slice()))
                    .unwrap_or_default(),
            }))
        } else {
            Some(TraceOutput::Call(CallOutput {
                gas_used: U64::from(frame.gas_used.to::<u64>()),
                output:   frame.output.clone().unwrap_or_default(),
            }))
        }
    } else {
        None
    };

    let num_children = frame.calls.len();

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
        found_msg_sender.unwrap_or_else(|| Address::from_slice(frame.from.as_slice()))
    } else {
        Address::from_slice(frame.from.as_slice())
    };

    traces.push(TransactionTraceWithLogs {
        trace,
        logs,
        msg_sender,
        trace_idx: current_trace_idx,
        decoded_data: None,
    });

    // Process child calls
    for (idx, child_frame) in frame.calls.into_iter().enumerate() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use brontes_types::structured_trace::TxTrace;

    /// Helper function to validate that transformed TxTrace matches expected TxTrace
    fn assert_traces_equal(
        transformed: &TxTrace,
        expected: &TxTrace,
    ) {
        // Compare top-level TxTrace fields
        assert_eq!(
            transformed.block_number,
            expected.block_number,
            "block_number mismatch"
        );
        assert_eq!(
            transformed.tx_hash,
            expected.tx_hash,
            "tx_hash mismatch"
        );
        assert_eq!(
            transformed.gas_used,
            expected.gas_used,
            "gas_used mismatch"
        );
        assert_eq!(
            transformed.effective_price,
            expected.effective_price,
            "effective_price mismatch"
        );
        assert_eq!(
            transformed.tx_index,
            expected.tx_index,
            "tx_index mismatch"
        );
        assert_eq!(
            transformed.timeboosted,
            expected.timeboosted,
            "timeboosted mismatch"
        );
        assert_eq!(
            transformed.is_success,
            expected.is_success,
            "is_success mismatch"
        );

        // Compare traces array
        assert_eq!(
            transformed.trace.len(),
            expected.trace.len(),
            "Trace count mismatch: transformed={}, expected={}",
            transformed.trace.len(),
            expected.trace.len()
        );

        for (idx, (transformed_trace, expected_trace)) in transformed.trace
            .iter()
            .zip(expected.trace.iter())
            .enumerate()
        {
            // Check trace_idx
            assert_eq!(
                transformed_trace.trace_idx, expected_trace.trace_idx,
                "Trace {} trace_idx mismatch",
                idx
            );

            // Check trace_address
            assert_eq!(
                transformed_trace.trace.trace_address, expected_trace.trace.trace_address,
                "Trace {} trace_address mismatch",
                idx
            );

            // Check subtraces count
            assert_eq!(
                transformed_trace.trace.subtraces, expected_trace.trace.subtraces,
                "Trace {} subtraces mismatch",
                idx
            );

            // Check msg_sender
            assert_eq!(
                transformed_trace.msg_sender, expected_trace.msg_sender,
                "Trace {} msg_sender mismatch: transformed={:?}, expected={:?}",
                idx, transformed_trace.msg_sender, expected_trace.msg_sender
            );

            // Check action types match
            match (&transformed_trace.trace.action, &expected_trace.trace.action) {
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
                        idx, transformed_trace.trace.action, expected_trace.trace.action
                    );
                }
            }

            // Check result field
            match (&transformed_trace.trace.result, &expected_trace.trace.result) {
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
                        idx, transformed_trace.trace.result, expected_trace.trace.result
                    );
                }
            }

            // Check error field
            assert_eq!(
                transformed_trace.trace.error, expected_trace.trace.error,
                "Trace {} error mismatch",
                idx
            );

            // Check logs count
            assert_eq!(
                transformed_trace.logs.len(),
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
        let parity_json = include_str!("testdata/parity.json");
        let receipt_json = include_str!("testdata/receipt.json");

        let call_frame: CallFrame =
            serde_json::from_str(call_json).expect("Failed to deserialize callTracer output (call.json)");
        let expected: TxTrace =
            serde_json::from_str(parity_json).expect("Failed to deserialize brontesTracer output (parity.json)");
        let receipt: TimeboostTransactionReceipt = serde_json::from_str(receipt_json)
            .expect("Failed to deserialize receipt (receipt.json)");

        // Transform callTracer to our format
        let transformed_traces = transform_call_frame_to_parity_trace(call_frame, &receipt);

        // Use helper function to validate all traces
        assert_traces_equal(&transformed_traces, &expected);

        // Verify expected trace count
        assert_eq!(transformed_traces.trace.len(), 65);
    }

    #[test]
    fn test_transform_call_frame_to_traces_second_example() {
        // Load second test case with different transaction structure
        let call_json = include_str!("testdata/call2.json");
        let parity_json = include_str!("testdata/parity2.json");
        let receipt_json = include_str!("testdata/receipt2.json");

        let call_frame: CallFrame = serde_json::from_str(call_json)
            .expect("Failed to deserialize callTracer output (call2.json)");
        let expected: TxTrace = serde_json::from_str(parity_json)
            .expect("Failed to deserialize brontesTracer output (brontes2.json)");
        let receipt: TimeboostTransactionReceipt = serde_json::from_str(receipt_json)
            .expect("Failed to deserialize receipt (receipt2.json)");

        // Transform callTracer to our format
        let transformed_traces = transform_call_frame_to_parity_trace(call_frame, &receipt);

        // Use helper function to validate all traces
        assert_traces_equal(&transformed_traces, &expected);

        // Verify expected trace count
        assert_eq!(transformed_traces.trace.len(), 114);
    }
}

