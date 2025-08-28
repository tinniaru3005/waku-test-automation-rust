// tests/integration_tests.rs
use waku_test_automation::{WakuTestFramework, WakuNodeConfig, create_test_message};
use std::time::Duration;

const TEST_TOPIC: &str = "/my-app/2/chatroom-1/proto";
const TEST_MESSAGE: &str = "Relay works!!";

#[tokio::test]
async fn test_suite_1_basic_node_operation() {
    // Initialize tracing (ignore if already initialized)
    let _ = tracing_subscriber::fmt::try_init();
    
    let framework = WakuTestFramework::new()
        .expect("Failed to create test framework");

    // Clean up any existing containers
    framework.cleanup_existing_containers()
        .await
        .expect("Failed to cleanup existing containers");

    let config = WakuNodeConfig::default();
    
    // Start the node
    let node = framework.start_waku_node(config)
        .await
        .expect("Failed to start Waku node");

    // Verify node information
    let node_info = framework.get_node_info(&node)
        .await
        .expect("Failed to get node info");
    
    assert!(!node_info.enr_uri.is_empty(), "ENR URI should not be empty");
    assert!(!node_info.listen_addresses.is_empty(), "Listen addresses should not be empty");

    // Subscribe to topic
    framework.subscribe_to_topic(&node, TEST_TOPIC)
        .await
        .expect("Failed to subscribe to topic");

    // Publish message
    let message = create_test_message(TEST_MESSAGE, TEST_TOPIC);
    framework.publish_message(&node, &message)
        .await
        .expect("Failed to publish message");

    // Wait a bit for message to be processed
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Confirm message publication
    let received_messages = framework.get_messages(&node, TEST_TOPIC)
        .await
        .expect("Failed to get messages");

    assert!(!received_messages.is_empty(), "Should have received at least one message");
    
    let decoded_payload = {
        use base64::{Engine, engine::general_purpose};
        general_purpose::STANDARD.decode(&received_messages[0].payload)
            .expect("Failed to decode message payload")
    };
    let payload_text = String::from_utf8(decoded_payload)
        .expect("Failed to convert payload to string");
    
    assert_eq!(payload_text, TEST_MESSAGE, "Message content should match");
    assert_eq!(received_messages[0].content_topic, TEST_TOPIC, "Topic should match");

    // Cleanup
    framework.cleanup_node(&node).await.expect("Failed to cleanup node");
    
    println!("✅ Test Suite 1: Basic Node Operation - PASSED");
}

#[tokio::test]
async fn test_suite_2_inter_node_communication() {
    // Initialize tracing (ignore if already initialized)
    let _ = tracing_subscriber::fmt::try_init();
    
    let framework = WakuTestFramework::new()
        .expect("Failed to create test framework");

    // Clean up any existing containers and network
    framework.cleanup_existing_containers()
        .await
        .expect("Failed to cleanup existing containers");
    
    let _ = framework.cleanup_network().await; // Ignore errors if network doesn't exist

    // Setup network
    framework.setup_network()
        .await
        .expect("Failed to setup network");

    // Start first node
    let config1 = WakuNodeConfig {
        name: "waku-node-1".to_string(),
        rest_port: 23161, // Different ports to avoid conflicts
        tcp_port: 23162,
        websocket_port: 23163,
        discv5_port: 23164,
        external_ip: "172.18.111.226".to_string(),
        bootstrap_node: None,
    };

    let mut node1 = framework.start_waku_node(config1)
        .await
        .expect("Failed to start node1");

    // Get ENR URI from node1
    let node1_info = framework.get_node_info(&node1)
        .await
        .expect("Failed to get node1 info");
    node1.enr_uri = Some(node1_info.enr_uri.clone());

    // Connect node1 to network
    framework.connect_to_network(&node1)
        .await
        .expect("Failed to connect node1 to network");

    // Subscribe node1 to topic
    framework.subscribe_to_topic(&node1, TEST_TOPIC)
        .await
        .expect("Failed to subscribe node1 to topic");

    // Start second node with bootstrap
    let config2 = WakuNodeConfig {
        name: "waku-node-2".to_string(),
        rest_port: 23171, // Different ports
        tcp_port: 23172,
        websocket_port: 23173,
        discv5_port: 23174,
        external_ip: "172.18.111.227".to_string(),
        bootstrap_node: Some(node1_info.enr_uri),
    };

    let node2 = framework.start_waku_node(config2)
        .await
        .expect("Failed to start node2");

    // Connect node2 to network
    framework.connect_to_network(&node2)
        .await
        .expect("Failed to connect node2 to network");

    // Subscribe node2 to topic
    framework.subscribe_to_topic(&node2, TEST_TOPIC)
        .await
        .expect("Failed to subscribe node2 to topic");

    // Wait for nodes to discover each other
    let connected = framework.wait_for_peer_connection(&node2, 30)
        .await
        .expect("Failed to check peer connections");
    
    assert!(connected, "Nodes should be connected to each other");

    // Publish message from node1
    let message = create_test_message("Inter-node communication works!", TEST_TOPIC);
    framework.publish_message(&node1, &message)
        .await
        .expect("Failed to publish message from node1");

    // Wait for message propagation
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Verify node2 received the message
    let received_messages = framework.get_messages(&node2, TEST_TOPIC)
        .await
        .expect("Failed to get messages from node2");

    assert!(!received_messages.is_empty(), "Node2 should have received messages");
    
    let decoded_payload = {
        use base64::{Engine, engine::general_purpose};
        general_purpose::STANDARD.decode(&received_messages[0].payload)
            .expect("Failed to decode message payload")
    };
    let payload_text = String::from_utf8(decoded_payload)
        .expect("Failed to convert payload to string");
    
    assert_eq!(payload_text, "Inter-node communication works!", 
              "Message content should match");

    // Cleanup
    framework.cleanup_node(&node1).await.expect("Failed to cleanup node1");
    framework.cleanup_node(&node2).await.expect("Failed to cleanup node2");
    framework.cleanup_network().await.expect("Failed to cleanup network");
    
    println!("✅ Test Suite 2: Inter-Node Communication - PASSED");
}