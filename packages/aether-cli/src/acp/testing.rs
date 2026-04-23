//! In-memory fakes for exercising relay / session-manager behaviour without a
//! live ACP transport.

use acp_utils::notifications::{
    AuthMethodsUpdatedParams, ContextClearedParams, ContextUsageParams, ElicitationParams, ElicitationResponse,
    McpNotification, SubAgentProgressParams,
};
use acp_utils::server::{AcpConnection, AcpServerError};
use agent_client_protocol::schema::{RequestPermissionRequest, RequestPermissionResponse, SessionNotification};
use futures::future::BoxFuture;
use std::collections::VecDeque;
use std::sync::Mutex;

pub(crate) struct FakeAcpConnection {
    session_notifications: Mutex<Vec<SessionNotification>>,
    context_usage: Mutex<Vec<ContextUsageParams>>,
    context_cleared: Mutex<Vec<ContextClearedParams>>,
    sub_agent_progress: Mutex<Vec<SubAgentProgressParams>>,
    auth_methods_updated: Mutex<Vec<AuthMethodsUpdatedParams>>,
    mcp_notifications: Mutex<Vec<McpNotification>>,
    elicitation_requests: Mutex<Vec<ElicitationParams>>,
    elicitation_responses: Mutex<VecDeque<Result<ElicitationResponse, AcpServerError>>>,
    permission_responses: Mutex<VecDeque<Result<RequestPermissionResponse, AcpServerError>>>,
}

impl FakeAcpConnection {
    pub fn new() -> Self {
        Self {
            session_notifications: Mutex::new(Vec::new()),
            context_usage: Mutex::new(Vec::new()),
            context_cleared: Mutex::new(Vec::new()),
            sub_agent_progress: Mutex::new(Vec::new()),
            auth_methods_updated: Mutex::new(Vec::new()),
            mcp_notifications: Mutex::new(Vec::new()),
            elicitation_requests: Mutex::new(Vec::new()),
            elicitation_responses: Mutex::new(VecDeque::new()),
            permission_responses: Mutex::new(VecDeque::new()),
        }
    }

    pub fn session_notifications(&self) -> Vec<SessionNotification> {
        self.session_notifications.lock().unwrap().clone()
    }

    pub fn mcp_notifications(&self) -> Vec<McpNotification> {
        self.mcp_notifications.lock().unwrap().clone()
    }

    pub fn elicitation_requests(&self) -> Vec<ElicitationParams> {
        self.elicitation_requests.lock().unwrap().clone()
    }

    pub fn queue_elicitation_response(&self, response: Result<ElicitationResponse, AcpServerError>) {
        self.elicitation_responses.lock().unwrap().push_back(response);
    }
}

impl AcpConnection for FakeAcpConnection {
    fn send_session_notification(&self, notification: SessionNotification) -> Result<(), AcpServerError> {
        self.session_notifications.lock().unwrap().push(notification);
        Ok(())
    }

    fn send_context_usage(&self, params: ContextUsageParams) -> Result<(), AcpServerError> {
        self.context_usage.lock().unwrap().push(params);
        Ok(())
    }

    fn send_context_cleared(&self, params: ContextClearedParams) -> Result<(), AcpServerError> {
        self.context_cleared.lock().unwrap().push(params);
        Ok(())
    }

    fn send_auth_methods_updated(&self, params: AuthMethodsUpdatedParams) -> Result<(), AcpServerError> {
        self.auth_methods_updated.lock().unwrap().push(params);
        Ok(())
    }

    fn send_sub_agent_progress(&self, params: SubAgentProgressParams) -> Result<(), AcpServerError> {
        self.sub_agent_progress.lock().unwrap().push(params);
        Ok(())
    }

    fn send_mcp_notification(&self, notification: McpNotification) -> Result<(), AcpServerError> {
        self.mcp_notifications.lock().unwrap().push(notification);
        Ok(())
    }

    fn request_elicitation(
        &self,
        params: ElicitationParams,
    ) -> BoxFuture<'_, Result<ElicitationResponse, AcpServerError>> {
        self.elicitation_requests.lock().unwrap().push(params);
        let response = self
            .elicitation_responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(AcpServerError::ConnectionUnavailable));
        Box::pin(async move { response })
    }

    fn request_permission(
        &self,
        _request: RequestPermissionRequest,
    ) -> BoxFuture<'_, Result<RequestPermissionResponse, AcpServerError>> {
        let response =
            self.permission_responses.lock().unwrap().pop_front().unwrap_or(Err(AcpServerError::ConnectionUnavailable));
        Box::pin(async move { response })
    }
}
