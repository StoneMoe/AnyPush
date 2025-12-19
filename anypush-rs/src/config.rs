use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Working directory for relative paths
    #[serde(default = "default_workdir")]
    pub workdir: String,

    /// Inbound handlers
    #[serde(default)]
    pub inbound: Vec<InboundConfig>,

    /// Outbound handlers
    #[serde(default)]
    pub outbound: Vec<OutboundConfig>,

    /// Routes connecting inbounds to pipelines
    #[serde(default)]
    pub routes: Vec<RouteConfig>,
}

fn default_workdir() -> String {
    ".".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InboundConfig {
    #[serde(rename = "smtp")]
    Smtp(SmtpInboundConfig),
    #[serde(rename = "tcp")]
    Tcp(TcpInboundConfig),
    #[serde(rename = "http")]
    Http(HttpInboundConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpInboundConfig {
    pub name: String,
    #[serde(default = "default_host")]
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpInboundConfig {
    pub name: String,
    #[serde(default = "default_host")]
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub connection_mode: ConnectionMode,
    #[serde(default = "default_outbound_timeout")]
    pub outbound_timeout_ms: u64,
    #[serde(default = "default_buffer_max")]
    pub buffer_max_bytes: usize,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_outbound_timeout() -> u64 {
    10000
}

fn default_buffer_max() -> usize {
    1024 * 1024 // 1MB
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionMode {
    #[default]
    WaitForOutbound,
    BufferUntilReady,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpInboundConfig {
    pub name: String,
    #[serde(default = "default_host")]
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutboundConfig {
    #[serde(rename = "http")]
    Http(HttpOutboundConfig),
    #[serde(rename = "tcp")]
    Tcp(TcpOutboundConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpOutboundConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

fn default_method() -> String {
    "POST".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpOutboundConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
}

/// Route configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    /// Optional condition for this route
    #[serde(rename = "when")]
    pub when_clause: Option<WhenClause>,

    /// Pipeline steps (goto with inline steps)
    #[serde(default)]
    pub goto: Option<Vec<StepConfig>>,

    /// Single action shortcut (when only one action is needed)
    #[serde(default)]
    pub action: Option<String>,

    /// Additional action parameters (flattened)
    #[serde(flatten)]
    pub action_params: HashMap<String, serde_yaml::Value>,
}

/// When clause for conditional execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhenClause {
    /// Inbound name(s) to match
    #[serde(default)]
    pub from: Option<FromMatcher>,

    /// Expression to evaluate
    #[serde(default)]
    pub expr: Option<String>,
}

/// From matcher - can be single string or list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FromMatcher {
    Single(String),
    Multiple(Vec<String>),
}

impl FromMatcher {
    pub fn matches(&self, inbound_name: &str) -> bool {
        match self {
            FromMatcher::Single(name) => name == inbound_name,
            FromMatcher::Multiple(names) => names.iter().any(|n| n == inbound_name),
        }
    }
}

/// Pipeline step configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepConfig {
    /// Action type
    pub action: Option<String>,

    /// Goto sub-pipeline
    pub goto: Option<Vec<StepConfig>>,

    /// When clause for conditional execution
    #[serde(rename = "when")]
    pub when_clause: Option<WhenClause>,

    /// Additional parameters (flattened)
    #[serde(flatten)]
    pub params: HashMap<String, serde_yaml::Value>,
}

/// Load configuration from file
pub fn load_config(path: &Path, workdir_override: Option<&Path>) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    let mut config: Config =
        serde_yaml::from_str(&content).with_context(|| "Failed to parse YAML config")?;

    // Apply workdir override if provided
    if let Some(wd) = workdir_override {
        config.workdir = wd.to_string_lossy().to_string();
    }

    Ok(config)
}

/// Get inbound name from config
impl InboundConfig {
    pub fn name(&self) -> &str {
        match self {
            InboundConfig::Smtp(c) => &c.name,
            InboundConfig::Tcp(c) => &c.name,
            InboundConfig::Http(c) => &c.name,
        }
    }
}

/// Get outbound name from config
impl OutboundConfig {
    pub fn name(&self) -> &str {
        match self {
            OutboundConfig::Http(c) => &c.name,
            OutboundConfig::Tcp(c) => &c.name,
        }
    }
}
