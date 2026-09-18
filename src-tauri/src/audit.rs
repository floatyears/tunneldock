use serde_json::Value;

const RESULT_SUMMARY_CHAR_LIMIT: usize = 500;

#[derive(Debug, PartialEq)]
pub enum RpcAuditEvent {
    ToolStarted {
        call_id: String,
        tool_name: String,
        args_json: String,
        input_tokens: u64,
    },
    ToolFinished {
        call_id: String,
        result_summary: String,
        is_error: bool,
        output_tokens: u64,
    },
}

pub fn parse_rpc_audit_event(line: &str) -> Option<RpcAuditEvent> {
    let value: Value = serde_json::from_str(line).ok()?;
    let event_type = value.get("type")?.as_str()?;

    match event_type {
        "tool_execution_start" => {
            let call_id = value.get("toolCallId")?.as_str()?.to_string();
            let tool_name = value.get("toolName")?.as_str()?.to_string();
            let args_json = value
                .get("args")
                .cloned()
                .unwrap_or_else(|| Value::Object(Default::default()))
                .to_string();

            Some(RpcAuditEvent::ToolStarted {
                call_id,
                tool_name,
                input_tokens: count_tokens(&args_json),
                args_json,
            })
        }
        "tool_execution_end" => {
            let call_id = value.get("toolCallId")?.as_str()?.to_string();
            let result = value.get("result").cloned().unwrap_or(Value::Null);
            let result_json = result.to_string();

            Some(RpcAuditEvent::ToolFinished {
                call_id,
                result_summary: summarize_result(&result, &result_json),
                is_error: value
                    .get("isError")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                output_tokens: count_tokens(&result_json),
            })
        }
        _ => None,
    }
}

fn count_tokens(text: &str) -> u64 {
    tiktoken_rs::o200k_base_singleton()
        .encode_ordinary(text)
        .len() as u64
}

fn summarize_result(result: &Value, fallback_json: &str) -> String {
    let text = result
        .get("content")
        .and_then(Value::as_array)
        .map(|content| {
            content
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| fallback_json.to_string());

    truncate_chars(&text, RESULT_SUMMARY_CHAR_LIMIT)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut truncated: String = text.chars().take(max_chars).collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::{parse_rpc_audit_event, RpcAuditEvent};

    #[test]
    fn parses_tool_start_with_real_arguments() {
        let event = parse_rpc_audit_event(
            r#"{"type":"tool_execution_start","toolCallId":"call_123","toolName":"read","args":{"path":"README.md"}}"#,
        )
        .expect("tool start should be audited");

        assert_eq!(
            event,
            RpcAuditEvent::ToolStarted {
                call_id: "call_123".to_string(),
                tool_name: "read".to_string(),
                args_json: r#"{"path":"README.md"}"#.to_string(),
                input_tokens: 6,
            }
        );
    }

    #[test]
    fn parses_successful_tool_end_and_counts_the_full_result() {
        let full_output = "a".repeat(5_000);
        let line = serde_json::json!({
            "type": "tool_execution_end",
            "toolCallId": "call_123",
            "toolName": "read",
            "result": {
                "content": [{"type": "text", "text": full_output}],
                "details": {"truncation": null}
            },
            "isError": false
        })
        .to_string();

        let event = parse_rpc_audit_event(&line).expect("tool end should be audited");
        let RpcAuditEvent::ToolFinished {
            call_id,
            result_summary,
            is_error,
            output_tokens,
        } = event
        else {
            panic!("expected a completed tool event");
        };

        assert_eq!(call_id, "call_123");
        assert!(!is_error);
        assert!(result_summary.ends_with('…'));
        assert!(result_summary.chars().count() <= 501);
        assert!(
            output_tokens > 500,
            "must count the untruncated result payload"
        );
    }

    #[test]
    fn preserves_error_status_from_tool_end() {
        let event = parse_rpc_audit_event(
            r#"{"type":"tool_execution_end","toolCallId":"call_bad","toolName":"bash","result":{"content":[{"type":"text","text":"permission denied"}]},"isError":true}"#,
        )
        .expect("failed tool end should still be audited");

        assert!(matches!(
            event,
            RpcAuditEvent::ToolFinished {
                call_id,
                is_error: true,
                ..
            } if call_id == "call_bad"
        ));
    }

    #[test]
    fn ignores_non_tool_rpc_lines() {
        assert_eq!(
            parse_rpc_audit_event(r#"{"type":"message_update","usage":{"input":0}}"#),
            None
        );
        assert_eq!(parse_rpc_audit_event("not-json"), None);
    }
}
