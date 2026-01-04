//! Page object for the sidebar component.

use fantoccini::{Client, elements::Element, error::CmdError};

use crate::helpers::{by_testid, by_testid_prefix};

/// Page object for interacting with the sidebar.
pub struct SidebarPage<'a> {
    client: &'a Client,
}

impl<'a> SidebarPage<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn get_new_agent_button(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "new-agent-button").await
    }

    pub async fn get_settings_button(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "settings-button").await
    }

    pub async fn get_no_agents_message(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "no-agents-message").await
    }

    pub async fn get_agent_item(&self, agent_id: &str) -> Result<Element, CmdError> {
        by_testid(self.client, &format!("agent-item-{}", agent_id)).await
    }

    pub async fn get_agent_items(&self) -> Result<Vec<Element>, CmdError> {
        by_testid_prefix(self.client, "agent-item-").await
    }

    pub async fn click_new_agent(&self) -> Result<(), CmdError> {
        let button = self.get_new_agent_button().await?;
        button.click().await
    }

    pub async fn click_settings(&self) -> Result<(), CmdError> {
        let button = self.get_settings_button().await?;
        button.click().await
    }

    pub async fn select_agent(&self, agent_id: &str) -> Result<(), CmdError> {
        let item = self.get_agent_item(agent_id).await?;
        item.click().await
    }

    /// Click the first agent item in the list.
    pub async fn click_first_agent(&self) -> Result<(), CmdError> {
        let items = self.get_agent_items().await?;
        if let Some(first) = items.first() {
            first.click().await
        } else {
            Err(CmdError::NotW3C(
                serde_json::json!({"message": "No agent items found"}),
            ))
        }
    }
}
