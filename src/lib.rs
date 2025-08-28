use anyhow::{Context, Result};
use bollard::{Docker, container::{CreateContainerOptions, Config, StartContainerOptions, ListContainersOptions}, network::{CreateNetworkOptions, ConnectNetworkOptions}, models::{Ipam, IpamConfig, HostConfig, EndpointSettings, PortBinding}};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct WakuNode {
    pub container_id: String,
    pub name: String,
    pub rest_port: u16,
    pub tcp_port: u16,
    pub websocket_port: u16,
    pub discv5_port: u16,
    pub external_ip: String,
    pub enr_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NodeInfo {
    #[serde(rename = "enrUri")]
    pub enr_uri: String,
    #[serde(rename = "listenAddresses")]
    pub listen_addresses: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct Message {
    pub payload: String,
    #[serde(rename = "contentTopic")]
    pub content_topic: String,
    pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
pub struct ReceivedMessage {
    pub payload: String,
    #[serde(rename = "contentTopic")]
    pub content_topic: String,
    pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
pub struct PeerInfo {
    #[serde(rename = "peerID")]
    pub peer_id: String,
    pub multiaddr: String,
    pub connected: bool,
}

pub struct WakuTestFramework {
    docker: Docker,
    client: Client,
    network_name: String,
}

impl WakuTestFramework {
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker daemon")?;
        
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            docker,
            client,
            network_name: "waku".to_string(),
        })
    }

    pub async fn setup_network(&self) -> Result<()> {
        info!("Creating Docker network: {}", self.network_name);
        
        let config = CreateNetworkOptions {
            name: self.network_name.clone(),
            driver: "bridge".to_string(),
            ipam: Ipam {
                driver: Some("default".to_string()),
                config: Some(vec![IpamConfig {
                    subnet: Some("172.18.0.0/16".to_string()),
                    gateway: Some("172.18.0.1".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            },
            ..Default::default()
        };

        match self.docker.create_network(config).await {
            Ok(_) => info!("Network created successfully"),
            Err(e) if e.to_string().contains("already exists") => {
                info!("Network already exists, continuing");
            }
            Err(e) => return Err(e.into()),
        }

        Ok(())
    }

    pub async fn start_waku_node(&self, node_config: WakuNodeConfig) -> Result<WakuNode> {
        info!("Starting Waku node: {}", node_config.name);

        let port_bindings = create_port_bindings(&node_config);
        let cmd = create_waku_command(&node_config);

        let config = Config {
            image: Some("wakuorg/nwaku:v0.24.0".to_string()),
            cmd: Some(cmd),
            exposed_ports: Some(create_exposed_ports(&node_config)),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),
                ..Default::default()
            }),
            ..Default::default()
        };

        let container = self.docker
            .create_container(Some(CreateContainerOptions {
                name: node_config.name.clone(),
                platform: None,
            }), config)
            .await
            .context("Failed to create container")?;

        self.docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .context("Failed to start container")?;

        // Wait for container to be ready
        sleep(Duration::from_secs(5)).await;

        let mut node = WakuNode {
            container_id: container.id,
            name: node_config.name,
            rest_port: node_config.rest_port,
            tcp_port: node_config.tcp_port,
            websocket_port: node_config.websocket_port,
            discv5_port: node_config.discv5_port,
            external_ip: node_config.external_ip,
            enr_uri: None,
        };

        // Get node info and ENR URI
        node.enr_uri = Some(self.get_node_info(&node).await?.enr_uri);
        
        Ok(node)
    }

    pub async fn connect_to_network(&self, node: &WakuNode) -> Result<()> {
        info!("Connecting node {} to network {}", node.name, self.network_name);

        let config = ConnectNetworkOptions {
            container: node.container_id.clone(),
            endpoint_config: EndpointSettings {
                ip_address: Some(node.external_ip.clone()),
                ..Default::default()
            },
        };

        self.docker
            .connect_network(&self.network_name, config)
            .await
            .context("Failed to connect container to network")?;

        Ok(())
    }

    pub async fn get_node_info(&self, node: &WakuNode) -> Result<NodeInfo> {
        let url = format!("http://127.0.0.1:{}/debug/v1/info", node.rest_port);
        
        for attempt in 1..=5 {
            match self.client.get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    // Try parsing as direct NodeInfo first, then as wrapped response
                    let text = response.text().await
                        .context("Failed to get response text")?;
                    
                    // Try direct parsing first
                    if let Ok(node_info) = serde_json::from_str::<NodeInfo>(&text) {
                        return Ok(node_info);
                    }
                    
                    // Try wrapped response format
                    if let Ok(api_response) = serde_json::from_str::<ApiResponse<NodeInfo>>(&text) {
                        return Ok(api_response.data);
                    }
                    
                    // Log the actual response for debugging
                    warn!("Unable to parse node info response: {}", text);
                    return Err(anyhow::anyhow!("Failed to parse node info response"));
                }
                Ok(response) => {
                    warn!("Node info request failed with status: {}", response.status());
                }
                Err(e) => {
                    warn!("Node info request failed (attempt {}): {}", attempt, e);
                }
            }
            
            if attempt < 5 {
                sleep(Duration::from_secs(2)).await;
            }
        }
        
        Err(anyhow::anyhow!("Failed to get node info after 5 attempts"))
    }

    pub async fn subscribe_to_topic(&self, node: &WakuNode, topic: &str) -> Result<()> {
        let url = format!("http://127.0.0.1:{}/relay/v1/auto/subscriptions", node.rest_port);
        let payload = json!([topic]);

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .context("Failed to send subscription request")?;

        if response.status().is_success() {
            info!("Successfully subscribed node {} to topic {}", node.name, topic);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Subscription failed with status: {}", response.status()))
        }
    }

    pub async fn publish_message(&self, node: &WakuNode, message: &Message) -> Result<()> {
        let url = format!("http://127.0.0.1:{}/relay/v1/auto/messages", node.rest_port);

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(message)
            .send()
            .await
            .context("Failed to send publish request")?;

        if response.status().is_success() {
            info!("Successfully published message from node {}", node.name);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Message publication failed with status: {}", response.status()))
        }
    }

    pub async fn get_messages(&self, node: &WakuNode, topic: &str) -> Result<Vec<ReceivedMessage>> {
        let encoded_topic = urlencoding::encode(topic);
        let url = format!("http://127.0.0.1:{}/relay/v1/auto/messages/{}", 
                         node.rest_port, encoded_topic);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to get messages")?;

        if response.status().is_success() {
            let messages: Vec<ReceivedMessage> = response.json().await
                .context("Failed to parse messages response")?;
            Ok(messages)
        } else {
            Ok(vec![])
        }
    }

    pub async fn get_peers(&self, node: &WakuNode) -> Result<Vec<PeerInfo>> {
        let url = format!("http://127.0.0.1:{}/admin/v1/peers", node.rest_port);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to get peers")?;

        if response.status().is_success() {
            let peers: Vec<PeerInfo> = response.json().await
                .context("Failed to parse peers response")?;
            Ok(peers)
        } else {
            Ok(vec![])
        }
    }

    pub async fn wait_for_peer_connection(&self, node: &WakuNode, timeout_secs: u64) -> Result<bool> {
        let start = std::time::Instant::now();
        
        while start.elapsed().as_secs() < timeout_secs {
            if let Ok(peers) = self.get_peers(node).await {
                if !peers.is_empty() && peers.iter().any(|p| p.connected) {
                    return Ok(true);
                }
            }
            sleep(Duration::from_secs(5)).await;
        }
        
        Ok(false)
    }

    pub async fn cleanup_existing_containers(&self) -> Result<()> {
        use bollard::container::ListContainersOptions;
        
        let options = Some(ListContainersOptions::<String> {
            all: true,
            filters: {
                let mut filters = std::collections::HashMap::new();
                filters.insert("name".to_string(), vec!["waku-node".to_string()]);
                filters
            },
            ..Default::default()
        });

        let containers = self.docker.list_containers(options).await?;
        
        for container in containers {
            if let Some(id) = container.id {
                info!("Cleaning up existing container: {}", id);
                let _ = self.docker.stop_container(&id, None).await;
                let _ = self.docker.remove_container(&id, None).await;
            }
        }
        
        Ok(())
    }

    pub async fn cleanup_node(&self, node: &WakuNode) -> Result<()> {
        info!("Cleaning up node: {}", node.name);
        
        // Stop and remove container
        if let Err(e) = self.docker.stop_container(&node.container_id, None).await {
            warn!("Failed to stop container {}: {}", node.container_id, e);
        }
        
        if let Err(e) = self.docker.remove_container(&node.container_id, None).await {
            warn!("Failed to remove container {}: {}", node.container_id, e);
        }
        
        Ok(())
    }

    pub async fn cleanup_network(&self) -> Result<()> {
        info!("Cleaning up network: {}", self.network_name);
        
        if let Err(e) = self.docker.remove_network(&self.network_name).await {
            warn!("Failed to remove network {}: {}", self.network_name, e);
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WakuNodeConfig {
    pub name: String,
    pub rest_port: u16,
    pub tcp_port: u16,
    pub websocket_port: u16,
    pub discv5_port: u16,
    pub external_ip: String,
    pub bootstrap_node: Option<String>,
}

impl Default for WakuNodeConfig {
    fn default() -> Self {
        Self {
            name: "waku-node".to_string(),
            rest_port: 22161, // Changed from 21161 to avoid conflicts
            tcp_port: 22162,
            websocket_port: 22163,
            discv5_port: 22164,
            external_ip: "172.18.111.226".to_string(),
            bootstrap_node: None,
        }
    }
}

fn create_port_bindings(config: &WakuNodeConfig) -> HashMap<String, Option<Vec<PortBinding>>> {
    let mut bindings = HashMap::new();
    
    bindings.insert(
        format!("{}/tcp", config.rest_port),
        Some(vec![PortBinding {
            host_ip: None,
            host_port: Some(config.rest_port.to_string()),
        }])
    );
    
    bindings.insert(
        format!("{}/tcp", config.tcp_port),
        Some(vec![PortBinding {
            host_ip: None,
            host_port: Some(config.tcp_port.to_string()),
        }])
    );
    
    bindings.insert(
        format!("{}/tcp", config.websocket_port),
        Some(vec![PortBinding {
            host_ip: None,
            host_port: Some(config.websocket_port.to_string()),
        }])
    );
    
    bindings.insert(
        format!("{}/udp", config.discv5_port),
        Some(vec![PortBinding {
            host_ip: None,
            host_port: Some(config.discv5_port.to_string()),
        }])
    );
    
    bindings
}

fn create_exposed_ports(config: &WakuNodeConfig) -> HashMap<String, HashMap<(), ()>> {
    let mut ports = HashMap::new();
    ports.insert(format!("{}/tcp", config.rest_port), HashMap::new());
    ports.insert(format!("{}/tcp", config.tcp_port), HashMap::new());
    ports.insert(format!("{}/tcp", config.websocket_port), HashMap::new());
    ports.insert(format!("{}/udp", config.discv5_port), HashMap::new());
    ports
}

fn create_waku_command(config: &WakuNodeConfig) -> Vec<String> {
    let mut cmd = vec![
        "--listen-address=0.0.0.0".to_string(),
        "--rest=true".to_string(),
        "--rest-admin=true".to_string(),
        "--websocket-support=true".to_string(),
        "--log-level=TRACE".to_string(),
        "--rest-relay-cache-capacity=100".to_string(),
        format!("--websocket-port={}", config.websocket_port),
        format!("--rest-port={}", config.rest_port),
        format!("--tcp-port={}", config.tcp_port),
        format!("--discv5-udp-port={}", config.discv5_port),
        "--rest-address=0.0.0.0".to_string(),
        format!("--nat=extip:{}", config.external_ip),
        "--peer-exchange=true".to_string(),
        "--discv5-discovery=true".to_string(),
        "--relay=true".to_string(),
    ];
    
    if let Some(bootstrap) = &config.bootstrap_node {
        cmd.push(format!("--discv5-bootstrap-node={}", bootstrap));
    }
    
    cmd
}

pub fn create_test_message(content: &str, topic: &str) -> Message {
    use base64::{Engine, engine::general_purpose};
    
    Message {
        payload: general_purpose::STANDARD.encode(content),
        content_topic: topic.to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    }
}