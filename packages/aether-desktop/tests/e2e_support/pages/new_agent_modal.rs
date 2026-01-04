//! Page object for the new agent modal component.

use fantoccini::{Client, Locator, elements::Element, error::CmdError};

use crate::helpers::{by_testid, get_attr, is_enabled};

/// Page object for interacting with the new agent modal.
pub struct NewAgentModalPage<'a> {
    client: &'a Client,
}

impl<'a> NewAgentModalPage<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn get_initial_message_input(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "initial-message-input").await
    }

    pub async fn get_server_dropdown(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "server-dropdown").await
    }

    pub async fn get_use_docker_checkbox(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "use-docker-checkbox").await
    }

    pub async fn get_create_button(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "create-agent-button").await
    }

    pub async fn get_cancel_button(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "cancel-agent-button").await
    }

    pub async fn type_initial_message(&self, message: &str) -> Result<(), CmdError> {
        let input = self.get_initial_message_input().await?;
        input.clear().await?;
        input.send_keys(message).await
    }

    pub async fn select_server(&self, server_name: &str) -> Result<(), CmdError> {
        let dropdown = self.get_server_dropdown().await?;
        dropdown.select_by_value(server_name).await.map_err(|_| {
            CmdError::NotW3C(serde_json::json!({"message": "Failed to select option"}))
        })
    }

    pub async fn toggle_docker(&self, enabled: bool) -> Result<(), CmdError> {
        let checkbox = self.get_use_docker_checkbox().await?;
        let is_checked = checkbox.is_selected().await?;
        if is_checked != enabled {
            checkbox.click().await?;
        }
        Ok(())
    }

    pub async fn click_create(&self) -> Result<(), CmdError> {
        let button = self.get_create_button().await?;
        button.click().await
    }

    pub async fn click_cancel(&self) -> Result<(), CmdError> {
        let button = self.get_cancel_button().await?;
        button.click().await
    }

    pub async fn create_agent(
        &self,
        message: &str,
        server_name: Option<&str>,
    ) -> Result<(), CmdError> {
        self.type_initial_message(message).await?;
        if let Some(server) = server_name {
            self.select_server(server).await?;
        }
        self.click_create().await
    }

    /// Check if the initial message input is visible.
    pub async fn is_visible(&self) -> bool {
        match self.get_initial_message_input().await {
            Ok(el) => el.is_displayed().await.unwrap_or(false),
            Err(_) => false,
        }
    }

    /// Get the current value of the initial message input.
    pub async fn get_initial_message_value(&self) -> Result<String, CmdError> {
        let input = self.get_initial_message_input().await?;
        get_attr(&input, "value")
            .await
            .map(|v| v.unwrap_or_default())
    }

    /// Get the number of options in the server dropdown.
    pub async fn get_server_option_count(&self) -> Result<usize, CmdError> {
        let _dropdown = self.get_server_dropdown().await?;
        let options = self
            .client
            .find_all(Locator::Css("[data-testid=\"server-dropdown\"] option"))
            .await?;
        Ok(options.len())
    }

    /// Check if the create button is enabled.
    pub async fn is_create_button_enabled(&self) -> Result<bool, CmdError> {
        let button = self.get_create_button().await?;
        is_enabled(&button).await
    }
}
