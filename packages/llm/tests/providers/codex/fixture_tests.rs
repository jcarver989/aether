//! Fixture-driven Codex / `OpenAI` Responses streaming tests.
//!
//! Loads raw SSE bodies captured from `api.openai.com/v1/responses` and feeds
//! them through `process_response_stream`. The Codex provider and the
//! `OpenAI` Responses provider both consume the same `ResponseStreamEvent` wire
//! format, so a single fixture set covers the deserialization path for both.

use async_openai::types::responses::ResponseStreamEvent;
use llm::providers::codex::streaming::process_response_stream;
use llm::{LlmError, LlmResponse, Result, StopReason};
use tokio_stream::StreamExt;

use crate::providers::common::{assert_minimal_usage, find_usage, parse_sse_data_lines, read_fixture};

async fn parse_fixture(scenario: &str) -> Vec<LlmResponse> {
    let bytes = read_fixture("openai_responses", scenario);
    let lines = parse_sse_data_lines(&bytes);
    let events: Vec<Result<ResponseStreamEvent>> = lines
        .into_iter()
        .filter_map(|line| match serde_json::from_str::<ResponseStreamEvent>(&line) {
            Ok(event) => Some(Ok(event)),
            Err(e) => {
                // Some Responses event types may not yet be modeled in async_openai;
                // skipping unknown events keeps the test forward-compatible while
                // still exercising the events we do parse.
                eprintln!("openai_responses/{scenario}: skipping unparseable event: {e}");
                None
            }
        })
        .collect();

    let stream = tokio_stream::iter(events.into_iter().map(|r| r.map_err(|e: LlmError| e)));
    let mut processed = Box::pin(process_response_stream(stream));
    let mut out = Vec::new();
    while let Some(event) = processed.next().await {
        out.push(event.expect("stream item should not error"));
    }
    out
}

#[tokio::test]
async fn codex_responses_minimal_emits_usage() {
    let events = parse_fixture("01_minimal").await;
    let usage = find_usage(&events).expect("usage event should be present");
    assert_minimal_usage(&usage, "01_minimal");
}

#[tokio::test]
async fn codex_responses_minimal_ends_with_done() {
    let events = parse_fixture("01_minimal").await;
    let last = events.last().expect("at least one event");
    assert!(
        matches!(last, LlmResponse::Done { stop_reason: Some(StopReason::EndTurn) }),
        "last event should be Done(EndTurn), got: {last:?}"
    );
}

#[tokio::test]
async fn codex_responses_reasoning_reports_reasoning_tokens() {
    let events = parse_fixture("02_reasoning").await;
    let usage = find_usage(&events).expect("usage event should be present");
    assert_minimal_usage(&usage, "02_reasoning");
    assert!(
        usage.reasoning_tokens.unwrap_or(0) > 0,
        "02_reasoning should report reasoning_tokens > 0, got {:?}",
        usage.reasoning_tokens
    );
}
