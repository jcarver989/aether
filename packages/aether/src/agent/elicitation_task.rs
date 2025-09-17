use crate::agent::AgentMessage;
use crate::mcp::ElicitationRequest;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct ElicitationTask {
    tx: mpsc::Sender<AgentMessage>,
    elicitation_request: ElicitationRequest,
}

impl ElicitationTask {
    pub fn new(tx: mpsc::Sender<AgentMessage>, elicitation_request: ElicitationRequest) -> Self {
        Self {
            tx,
            elicitation_request,
        }
    }

    pub async fn run(self) {
        let request_id = Uuid::new_v4().to_string();
        let _ = self
            .tx
            .send(AgentMessage::ElicitationRequest {
                request_id,
                request: self.elicitation_request.request,
                response_sender: self.elicitation_request.response_sender,
            })
            .await;
    }
}
