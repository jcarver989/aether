//! End-to-end tests using fantoccini (WebDriver client) with Chrome.
//!
//! # Prerequisites
//!
//! Install chromedriver: `brew install chromedriver`
//!
//! # Running Tests
//!
//! From the `packages/aether-desktop` directory:
//! ```bash
//! just e2e              # Run all tests (headless, parallel)
//! just e2e-visible      # Run with visible browser for debugging
//! just e2e test_name    # Run a specific test
//! ```
//!
//! The justfile handles starting dx serve and cleaning up automatically.
//! Each test spawns its own chromedriver instance on a dynamic port.

#[path = "e2e_support/pages/mod.rs"]
mod pages;

#[path = "e2e_support/helpers.rs"]
mod helpers;

use fantoccini::Locator;
use helpers::{TestHarness, assert_not_visible, assert_visible, require_agents};
use std::time::Duration;

#[tokio::test]
async fn test_app_loads_successfully() {
    let harness = TestHarness::new().await.expect("setup failed");

    let title = harness.client().title().await.expect("Failed to get title");
    assert!(
        title.to_lowercase().contains("aether"),
        "Expected title to contain 'aether', got: {title}"
    );

    harness.close().await.ok();
}

#[tokio::test]
async fn test_sidebar_is_visible() {
    let harness = TestHarness::new().await.expect("setup failed");

    assert_visible(&harness.client(), "new-agent-button").await;
    assert_visible(&harness.client(), "settings-button").await;

    harness.close().await.ok();
}

#[tokio::test]
async fn test_empty_state_shows_when_no_agents() {
    let harness = TestHarness::new().await.expect("setup failed");

    assert_visible(&harness.client(), "no-agents-message").await;

    let message = harness
        .sidebar()
        .get_no_agents_message()
        .await
        .expect("Failed to find no agents message");
    let text = message.text().await.expect("Failed to get text");
    assert_eq!(text, "No agents yet. Create one to get started.");

    harness.close().await.ok();
}

#[tokio::test]
async fn test_new_agent_button_is_clickable() {
    let harness = TestHarness::new().await.expect("setup failed");

    // Check button is enabled
    let button = harness
        .sidebar()
        .get_new_agent_button()
        .await
        .expect("Failed to find new agent button");
    assert!(
        button.attr("disabled").await.expect("attr check").is_none(),
        "New agent button should be enabled"
    );

    // Click and verify modal opens
    harness.sidebar().click_new_agent().await.expect("click");
    assert_visible(&harness.client(), "initial-message-input").await;

    harness.close().await.ok();
}

#[tokio::test]
async fn test_main_content_area_is_visible() {
    let harness = TestHarness::new().await.expect("setup failed");

    let main_content = harness
        .client()
        .find(Locator::Css(".flex-1"))
        .await
        .expect("Failed to find main content area");
    assert!(
        main_content.is_displayed().await.expect("visibility check"),
        "Main content area should be visible"
    );

    harness.close().await.ok();
}

#[tokio::test]
async fn test_can_open_new_agent_form() {
    let harness = TestHarness::new().await.expect("setup failed");

    harness.sidebar().click_new_agent().await.expect("click");

    assert_visible(&harness.client(), "initial-message-input").await;
    assert_visible(&harness.client(), "server-dropdown").await;
    assert_visible(&harness.client(), "create-agent-button").await;
    assert_visible(&harness.client(), "cancel-agent-button").await;

    harness.close().await.ok();
}

#[tokio::test]
async fn test_can_cancel_new_agent_form() {
    let harness = TestHarness::new().await.expect("setup failed");

    harness.sidebar().click_new_agent().await.expect("click");
    assert_visible(&harness.client(), "initial-message-input").await;

    harness.modal().click_cancel().await.expect("cancel");
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert_not_visible(&harness.client(), "initial-message-input").await;

    harness.close().await.ok();
}

#[tokio::test]
#[ignore = "Text input not working in web mode - needs investigation"]
async fn test_can_type_initial_message() {
    let harness = TestHarness::new().await.expect("setup failed");

    harness.sidebar().click_new_agent().await.expect("click");

    let test_message = "Hello, I would like to create an agent";
    harness
        .modal()
        .type_initial_message(test_message)
        .await
        .expect("type");

    let value = harness
        .modal()
        .get_initial_message_value()
        .await
        .expect("get value");
    assert_eq!(value, test_message);

    harness.close().await.ok();
}

#[tokio::test]
async fn test_can_select_server_from_dropdown() {
    let harness = TestHarness::new().await.expect("setup failed");

    harness.sidebar().click_new_agent().await.expect("click");

    let count = harness
        .modal()
        .get_server_option_count()
        .await
        .expect("option count");
    assert!(count > 0, "Server dropdown should have at least one option");

    harness.close().await.ok();
}

#[tokio::test]
async fn test_create_button_disabled_when_empty() {
    let harness = TestHarness::new().await.expect("setup failed");

    harness.sidebar().click_new_agent().await.expect("click");

    let enabled = harness
        .modal()
        .is_create_button_enabled()
        .await
        .expect("button state");
    assert!(!enabled, "Create button should be disabled when empty");

    harness.close().await.ok();
}

#[tokio::test]
#[ignore = "Depends on text input which is not working in web mode"]
async fn test_create_button_enabled_when_typed() {
    let harness = TestHarness::new().await.expect("setup failed");

    harness.sidebar().click_new_agent().await.expect("click");
    harness
        .modal()
        .type_initial_message("Hello, agent!")
        .await
        .expect("type");

    let enabled = harness
        .modal()
        .is_create_button_enabled()
        .await
        .expect("button state");
    assert!(
        enabled,
        "Create button should be enabled when message typed"
    );

    harness.close().await.ok();
}

#[tokio::test]
async fn test_can_select_agent_from_sidebar() {
    let harness = TestHarness::new().await.expect("setup failed");

    if require_agents(&harness).await.is_none() {
        harness.close().await.ok();
        return;
    }

    harness
        .sidebar()
        .click_first_agent()
        .await
        .expect("click agent");

    assert_visible(&harness.client(), "message-list").await;

    harness.close().await.ok();
}

#[tokio::test]
async fn test_message_list_empty_for_new_agent() {
    let harness = TestHarness::new().await.expect("setup failed");

    if require_agents(&harness).await.is_none() {
        harness.close().await.ok();
        return;
    }

    harness
        .sidebar()
        .click_first_agent()
        .await
        .expect("click agent");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let message_count = harness
        .agent_view()
        .get_message_count()
        .await
        .expect("message count");
    let has_empty_state = harness.agent_view().has_empty_state().await;

    assert!(
        message_count == 0 || has_empty_state,
        "New agent should have empty message list or show empty state"
    );

    harness.close().await.ok();
}
