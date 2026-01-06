//! Page object for the sub-agent display component.

use fantoccini::{Client, elements::Element, error::CmdError};

use crate::helpers::{by_testid, by_testid_prefix};

/// Page object for interacting with sub-agent displays.
pub struct SubAgentPage<'a> {
    client: &'a Client,
}

impl<'a> SubAgentPage<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Get the sub-agent display container element.
    pub async fn get_sub_agent_display(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "sub-agent-display").await
    }

    /// Get the sub-agent header element.
    pub async fn get_sub_agent_header(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "sub-agent-header").await
    }

    /// Get the sub-agent name element.
    pub async fn get_sub_agent_name_element(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "sub-agent-name").await
    }

    /// Get the sub-agent name text.
    pub async fn get_sub_agent_name(&self) -> Result<String, CmdError> {
        let element = self.get_sub_agent_name_element().await?;
        element.text().await
    }

    /// Get the sub-agent prompt element.
    pub async fn get_sub_agent_prompt_element(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "sub-agent-prompt").await
    }

    /// Get the sub-agent prompt text.
    pub async fn get_sub_agent_prompt(&self) -> Result<String, CmdError> {
        let element = self.get_sub_agent_prompt_element().await?;
        element.text().await
    }

    /// Get the stream text element.
    pub async fn get_stream_text_element(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "sub-agent-stream-text").await
    }

    /// Get the stream text content.
    pub async fn get_stream_text(&self) -> Result<String, CmdError> {
        let element = self.get_stream_text_element().await?;
        element.text().await
    }

    /// Get all tool started elements.
    pub async fn get_tool_started_elements(&self) -> Result<Vec<Element>, CmdError> {
        by_testid_prefix(self.client, "sub-agent-tool-started-").await
    }

    /// Get all tool completed elements.
    pub async fn get_tool_completed_elements(&self) -> Result<Vec<Element>, CmdError> {
        by_testid_prefix(self.client, "sub-agent-tool-completed-").await
    }

    /// Get all tool failed elements.
    pub async fn get_tool_failed_elements(&self) -> Result<Vec<Element>, CmdError> {
        by_testid_prefix(self.client, "sub-agent-tool-failed-").await
    }

    /// Get all error elements.
    pub async fn get_error_elements(&self) -> Result<Vec<Element>, CmdError> {
        by_testid_prefix(self.client, "sub-agent-error-").await
    }

    /// Check if the sub-agent display is visible.
    pub async fn is_visible(&self) -> bool {
        match self.get_sub_agent_display().await {
            Ok(el) => el.is_displayed().await.unwrap_or(false),
            Err(_) => false,
        }
    }
}
