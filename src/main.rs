use waku_test_automation::{WakuTestFramework, WakuNodeConfig, create_test_message};
use anyhow::Result;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    info!("Starting Waku Test Automation Framework");
    
    let framework = WakuTestFramework::new()?;
    
    match run_basic_test(&framework).await {
        Ok(_) => info!("Basic test completed successfully"),
        Err(e) => error!("Basic test failed: {}", e),
    }
    
    Ok(())
}

async fn run_basic_test(framework: &WakuTestFramework) -> Result<()> {
    let config = WakuNodeConfig::default();
    let node = framework.start_waku_node(config).await?;
    
    // Get node info
    let node_info = framework.get_node_info(&node).await?;
    info!("Node ENR: {}", node_info.enr_uri);
    
    // Subscribe and publish
    let topic = "/my-app/2/chatroom-1/proto";
    framework.subscribe_to_topic(&node, topic).await?;
    
    let message = create_test_message("Test message", topic);
    framework.publish_message(&node, &message).await?;
    
    // Cleanup
    framework.cleanup_node(&node).await?;
    
    Ok(())
}