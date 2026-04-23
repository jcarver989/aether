use acp_utils::notifications::McpRequest;
use agent_client_protocol::schema::{
    AuthenticateRequest, CancelNotification, ClientRequest, InitializeRequest, ListSessionsRequest, LoadSessionRequest,
    NewSessionRequest, PromptRequest, SetSessionConfigOptionRequest, SetSessionModeRequest,
};
use agent_client_protocol::{self as acp, Agent, Builder, Client, HandleDispatchFrom, NullRun};
use std::sync::Arc;

use super::session_manager::SessionManager;

/// Wire every inbound ACP request and notification we care about into the
/// builder. Each handler is a thin wrapper that forwards the call to
/// [`SessionManager`].
///
/// A generic `forward` helper was evaluated and rejected: `on_receive_request`
/// takes an `AsyncFnMut` plus a companion dispatch value produced by the
/// `acp::on_receive_request!()` macro at the call site, and `SessionManager`'s
/// async methods have type `for<'a> fn(&'a SessionManager, Req) -> impl Future<..>`.
/// Threading those through a single higher-ranked helper costs more clarity
/// than the explicit repetition below saves.
pub(crate) fn configure_agent_builder(
    manager: Arc<SessionManager>,
) -> Builder<Agent, impl HandleDispatchFrom<Client>, NullRun> {
    Agent
        .builder()
        .on_receive_request(
            {
                let mgr = manager.clone();
                async move |req: InitializeRequest, responder, _cx| {
                    responder.respond_with_result(mgr.initialize(req).await)
                }
            },
            acp::on_receive_request!(),
        )
        .on_receive_request(
            {
                let mgr = manager.clone();
                async move |req: AuthenticateRequest, responder, _cx| {
                    responder.respond_with_result(mgr.authenticate(req).await)
                }
            },
            acp::on_receive_request!(),
        )
        .on_receive_request(
            {
                let mgr = manager.clone();
                async move |req: NewSessionRequest, responder, _cx| {
                    responder.respond_with_result(mgr.new_session(req).await)
                }
            },
            acp::on_receive_request!(),
        )
        .on_receive_request(
            {
                let mgr = manager.clone();
                async move |req: ListSessionsRequest, responder, _cx| {
                    responder.respond_with_result(mgr.list_sessions(&req))
                }
            },
            acp::on_receive_request!(),
        )
        .on_receive_request(
            {
                let mgr = manager.clone();
                async move |req: LoadSessionRequest, responder, _cx| {
                    responder.respond_with_result(mgr.load_session(req).await)
                }
            },
            acp::on_receive_request!(),
        )
        .on_receive_request(
            {
                let mgr = manager.clone();
                async move |req: PromptRequest, responder, _cx| responder.respond_with_result(mgr.prompt(req).await)
            },
            acp::on_receive_request!(),
        )
        .on_receive_request(
            {
                let mgr = manager.clone();
                async move |req: SetSessionConfigOptionRequest, responder, _cx| {
                    responder.respond_with_result(mgr.set_session_config_option(req).await)
                }
            },
            acp::on_receive_request!(),
        )
        .on_receive_request(
            async move |_: SetSessionModeRequest, responder, _cx| {
                responder.respond_with_error(acp::Error::method_not_found())
            },
            acp::on_receive_request!(),
        )
        .on_receive_request(
            async move |_req: ClientRequest, responder, _cx| {
                responder.respond_with_error(acp::Error::method_not_found())
            },
            acp::on_receive_request!(),
        )
        .on_receive_notification(
            {
                let mgr = manager.clone();
                async move |notif: CancelNotification, _cx| {
                    let _ = mgr.cancel(notif).await;
                    Ok(())
                }
            },
            acp::on_receive_notification!(),
        )
        .on_receive_notification(
            {
                async move |req: McpRequest, _cx| {
                    let _ = manager.on_mcp_request(req).await;
                    Ok(())
                }
            },
            acp::on_receive_notification!(),
        )
}
