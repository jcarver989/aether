//! Test helpers for e2e tests.

use fantoccini::{Client, Locator, elements::Element, error::CmdError};
use std::time::Duration;

use crate::pages::{AgentViewPage, NewAgentModalPage, SidebarPage};

pub const BASE_URL: &str = "http://localhost:8080";

/// Test harness that handles setup/teardown and provides access to page objects.
pub struct TestHarness {
    pub client: Client,
}

/// Error type for test harness setup failures.
#[derive(Debug)]
pub enum SetupError {
    Session(fantoccini::error::NewSessionError),
    Navigation(CmdError),
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetupError::Session(e) => write!(f, "Failed to create WebDriver session: {e}"),
            SetupError::Navigation(e) => write!(f, "Failed to navigate to app: {e}"),
        }
    }
}

impl std::error::Error for SetupError {}

impl From<fantoccini::error::NewSessionError> for SetupError {
    fn from(e: fantoccini::error::NewSessionError) -> Self {
        SetupError::Session(e)
    }
}

impl From<CmdError> for SetupError {
    fn from(e: CmdError) -> Self {
        SetupError::Navigation(e)
    }
}

impl TestHarness {
    /// Create a new test harness, connecting to WebDriver and navigating to the app.
    pub async fn new() -> Result<Self, SetupError> {
        // Try to close any stale session from a previous failed test
        if let Ok(stale_client) = create_client().await {
            let _ = stale_client.close().await;
        }

        let client = create_client().await?;
        goto_app(&client).await?;
        Ok(Self { client })
    }

    /// Get a SidebarPage for interacting with the sidebar.
    pub fn sidebar(&self) -> SidebarPage<'_> {
        SidebarPage::new(&self.client)
    }

    /// Get a NewAgentModalPage for interacting with the new agent modal.
    pub fn modal(&self) -> NewAgentModalPage<'_> {
        NewAgentModalPage::new(&self.client)
    }

    /// Get an AgentViewPage for interacting with the agent view.
    pub fn agent_view(&self) -> AgentViewPage<'_> {
        AgentViewPage::new(&self.client)
    }

    /// Close the WebDriver session. Call this at the end of each test.
    pub async fn close(self) -> Result<(), CmdError> {
        self.client.close().await
    }
}

/// Assert that an element with the given testid is visible.
pub async fn assert_visible(client: &Client, testid: &str) {
    let element = by_testid(client, testid)
        .await
        .unwrap_or_else(|_| panic!("Element [data-testid=\"{testid}\"] not found"));
    assert!(
        element.is_displayed().await.unwrap_or(false),
        "Element [data-testid=\"{testid}\"] should be visible"
    );
}

/// Assert that an element with the given testid is NOT visible (or doesn't exist).
pub async fn assert_not_visible(client: &Client, testid: &str) {
    assert!(
        !is_visible(client, testid).await,
        "Element [data-testid=\"{testid}\"] should not be visible"
    );
}

/// Check if agents exist, returning None if no agents (for skipping tests).
pub async fn require_agents(harness: &TestHarness) -> Option<()> {
    let items = harness.sidebar().get_agent_items().await.ok()?;
    if items.is_empty() {
        println!("Skipping: no agents exist");
        return None;
    }
    Some(())
}

/// Wait for Dioxus to hydrate the page.
pub async fn wait_for_hydration(client: &Client) -> Result<(), CmdError> {
    client.wait().for_element(Locator::Css("body")).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    Ok(())
}

/// Find element by data-testid attribute.
pub async fn by_testid(client: &Client, testid: &str) -> Result<Element, CmdError> {
    client
        .find(Locator::Css(&format!("[data-testid=\"{}\"]", testid)))
        .await
}

/// Check if element with testid exists and is displayed.
pub async fn is_visible(client: &Client, testid: &str) -> bool {
    match by_testid(client, testid).await {
        Ok(el) => el.is_displayed().await.unwrap_or(false),
        Err(_) => false,
    }
}

/// Find elements matching a CSS selector prefix for testid.
pub async fn by_testid_prefix(client: &Client, prefix: &str) -> Result<Vec<Element>, CmdError> {
    client
        .find_all(Locator::Css(&format!("[data-testid^=\"{}\"]", prefix)))
        .await
}

/// Get attribute value of an element.
pub async fn get_attr(element: &Element, attr: &str) -> Result<Option<String>, CmdError> {
    element.attr(attr).await
}

/// Check if an element is enabled (not disabled).
pub async fn is_enabled(element: &Element) -> Result<bool, CmdError> {
    let disabled = element.attr("disabled").await?;
    Ok(disabled.is_none())
}

/// Create a new WebDriver client connected to geckodriver.
pub async fn create_client() -> Result<Client, fantoccini::error::NewSessionError> {
    fantoccini::ClientBuilder::native()
        .connect("http://localhost:4444")
        .await
}

/// Navigate to the app and wait for hydration.
pub async fn goto_app(client: &Client) -> Result<(), CmdError> {
    client.goto(BASE_URL).await?;
    wait_for_hydration(client).await
}
