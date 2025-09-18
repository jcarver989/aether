#[derive(Debug)]
pub enum UiEvent {
    TextChunk {
        content: String,
        model_name: String,
        is_first_chunk: bool,
    },
    TextComplete,
    ToolStarted {
        id: String,
        name: String,
        model_name: String,
    },
    ToolCompleted {
        name: String,
        model_name: String,
        arguments: Option<String>,
        result: Option<String>,
    },
    Error {
        message: String,
    },
    Cancelled {
        message: String,
    },
    ElicitationRequest {
        request: aether::CreateElicitationRequestParam,
        response_sender: tokio::sync::oneshot::Sender<aether::CreateElicitationResult>,
    },
}
