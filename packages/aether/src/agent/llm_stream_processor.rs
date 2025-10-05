use crate::llm::Context;
use crate::llm::ModelProvider;
use crate::types::LlmResponse;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Encapsulates LLM stream processing: spawning, channels, and message handling
pub struct LlmStreamProcessor {
    response_rx: mpsc::Receiver<LlmResponse>,
    handle: JoinHandle<()>,
    complete: bool,
}

impl LlmStreamProcessor {
    /// Create a new LLM stream processor
    pub fn new<T: ModelProvider + 'static>(llm: Arc<T>, context: Arc<Context>) -> Self {
        let (response_tx, response_rx) = mpsc::channel(100);

        let handle = tokio::spawn(async move {
            let response_stream = llm.stream_response(&context);
            futures::pin_mut!(response_stream);

            while let Some(event) = response_stream.next().await {
                match event {
                    Ok(response) => {
                        let is_done = matches!(response, LlmResponse::Done);
                        if response_tx.send(response).await.is_err() {
                            break;
                        }
                        if is_done {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = response_tx
                            .send(LlmResponse::Error {
                                message: e.to_string(),
                            })
                            .await;
                        break;
                    }
                }
            }
        });

        Self {
            response_rx,
            handle,
            complete: false,
        }
    }

    /// Receive the next response from the LLM stream
    pub async fn recv_response(&mut self) -> Option<LlmResponse> {
        if self.complete {
            return None;
        }

        match self.response_rx.recv().await {
            Some(response) => Some(response),
            None => {
                self.complete = true;
                None
            }
        }
    }

    /// Check if the stream has completed
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Shutdown the processor, awaiting task completion
    pub async fn shutdown(mut self) {
        // Drain any remaining messages
        while self.response_rx.recv().await.is_some() {}

        // Wait for task to complete
        let _ = self.handle.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{Context, LlmError};
    use crate::types::ChatMessage;
    use crate::types::IsoString;
    use crate::types::LlmResponse;
    use futures::stream;

    // Mock ModelProvider for testing
    struct MockProvider {
        responses: Vec<LlmResponse>,
    }

    impl crate::llm::ModelProvider for MockProvider {
        fn stream_response(&self, _context: &Context) -> crate::llm::LlmResponseStream {
            let responses = self.responses.clone();
            Box::pin(stream::iter(
                responses.into_iter().map(|r| Ok::<_, LlmError>(r)),
            ))
        }

        fn display_name(&self) -> String {
            "mock".to_string()
        }
    }

    #[tokio::test]
    async fn test_processor_receives_responses() {
        let provider = MockProvider {
            responses: vec![
                LlmResponse::Start {
                    message_id: "msg1".to_string(),
                },
                LlmResponse::Text {
                    chunk: "Hello".to_string(),
                },
                LlmResponse::Done,
            ],
        };

        let context = Context::new(
            vec![ChatMessage::User {
                content: "test".to_string(),
                timestamp: IsoString::now(),
            }],
            Vec::new(),
        );

        let mut processor = LlmStreamProcessor::new(Arc::new(provider), Arc::new(context));

        // Should receive Start
        let resp = processor.recv_response().await;
        assert!(matches!(resp, Some(LlmResponse::Start { .. })));

        // Should receive Text
        let resp = processor.recv_response().await;
        assert!(matches!(resp, Some(LlmResponse::Text { .. })));

        // Should receive Done
        let resp = processor.recv_response().await;
        assert!(matches!(resp, Some(LlmResponse::Done)));

        // Stream should complete
        let resp = processor.recv_response().await;
        assert!(resp.is_none());
        assert!(processor.is_complete());

        processor.shutdown().await;
    }

    #[tokio::test]
    async fn test_processor_shutdown() {
        let provider = MockProvider {
            responses: vec![LlmResponse::Done],
        };

        let context = Context::new(vec![], Vec::new());
        let processor = LlmStreamProcessor::new(Arc::new(provider), Arc::new(context));

        // Should cleanly shutdown
        processor.shutdown().await;
    }
}
