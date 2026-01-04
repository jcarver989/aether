//! Page object for the agent view component.

use fantoccini::{Client, Locator, elements::Element, error::CmdError};

use crate::helpers::{by_testid, by_testid_prefix};

/// Page object for interacting with the agent view.
pub struct AgentViewPage<'a> {
    client: &'a Client,
}

impl<'a> AgentViewPage<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn get_message_list(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "message-list").await
    }

    pub async fn get_agent_status(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "agent-status").await
    }

    pub async fn get_view_tabs(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "view-tabs").await
    }

    pub async fn get_message(&self, message_id: &str) -> Result<Element, CmdError> {
        by_testid(self.client, &format!("message-{}", message_id)).await
    }

    pub async fn get_messages(&self) -> Result<Vec<Element>, CmdError> {
        by_testid_prefix(self.client, "message-").await
    }

    pub async fn get_status_text(&self) -> Result<String, CmdError> {
        let element = self.get_agent_status().await?;
        element.text().await
    }

    pub async fn has_messages(&self) -> Result<bool, CmdError> {
        let messages = self.get_messages().await?;
        Ok(!messages.is_empty())
    }

    pub async fn get_message_count(&self) -> Result<usize, CmdError> {
        let messages = self.get_messages().await?;
        Ok(messages.len())
    }

    /// Check if the empty state message is visible.
    pub async fn has_empty_state(&self) -> bool {
        match self
            .client
            .find(Locator::XPath(
                "//*[contains(text(), 'Send a message to start the conversation')]",
            ))
            .await
        {
            Ok(el) => el.is_displayed().await.unwrap_or(false),
            Err(_) => false,
        }
    }
}
