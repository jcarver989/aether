//! Page object for the prompt input component.

use fantoccini::{Client, elements::Element, error::CmdError};

use crate::helpers::by_testid;

/// Page object for interacting with the prompt input.
pub struct PromptInputPage<'a> {
    client: &'a Client,
}

impl<'a> PromptInputPage<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn get_prompt_input(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "prompt-input").await
    }

    pub async fn get_submit_button(&self) -> Result<Element, CmdError> {
        by_testid(self.client, "submit-button").await
    }

    pub async fn type_message(&self, message: &str) -> Result<(), CmdError> {
        let input = self.get_prompt_input().await?;
        input.clear().await?;
        input.send_keys(message).await
    }

    pub async fn submit_message(&self) -> Result<(), CmdError> {
        let button = self.get_submit_button().await?;
        button.click().await
    }

    pub async fn type_and_submit(&self, message: &str) -> Result<(), CmdError> {
        self.type_message(message).await?;
        self.submit_message().await
    }

    pub async fn clear_input(&self) -> Result<(), CmdError> {
        let input = self.get_prompt_input().await?;
        input.clear().await
    }
}
