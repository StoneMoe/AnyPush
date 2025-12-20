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

    /// Processing pipeline (flat array of steps)
    #[serde(default)]
    pub pipeline: Vec<StepConfig>,
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
    #[serde(rename = "s3")]
    S3(S3OutboundConfig),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3OutboundConfig {
    pub name: String,
    pub bucket: String,
    /// Key template (supports {{id}}, {{from}}, {{subject}}, {{timestamp}})
    #[serde(default = "default_s3_key")]
    pub key: String,
    /// Optional region override
    #[serde(default)]
    pub region: Option<String>,
    /// Optional endpoint override (for S3-compatible services)
    #[serde(default)]
    pub endpoint: Option<String>,
}

fn default_s3_key() -> String {
    "{{id}}.json".to_string()
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

    // Validate configuration
    config.validate()?;

    Ok(config)
}

impl Config {
    /// Validate the configuration for common issues
    pub fn validate(&self) -> Result<()> {
        use std::collections::HashSet;

        // Collect all inbound names and check for duplicates
        let mut inbound_names: HashSet<&str> = HashSet::new();
        for inbound in &self.inbound {
            let name = inbound.name();
            if !inbound_names.insert(name) {
                return Err(anyhow::anyhow!(
                    "Duplicate inbound handler name: '{}'. Each handler must have a unique name.",
                    name
                ));
            }
        }

        // Collect all outbound names and check for duplicates
        let mut outbound_names: HashSet<&str> = HashSet::new();
        for outbound in &self.outbound {
            let name = outbound.name();
            if !outbound_names.insert(name) {
                return Err(anyhow::anyhow!(
                    "Duplicate outbound handler name: '{}'. Each handler must have a unique name.",
                    name
                ));
            }
        }

        // Validate pipeline steps
        self.validate_steps(&self.pipeline, &inbound_names, &outbound_names)?;

        Ok(())
    }

    fn validate_steps(
        &self,
        steps: &[StepConfig],
        inbound_names: &std::collections::HashSet<&str>,
        outbound_names: &std::collections::HashSet<&str>,
    ) -> Result<()> {
        for step in steps {
            // Validate when.from references valid inbound names
            if let Some(ref when) = step.when_clause {
                if let Some(ref from) = when.from {
                    let names_to_check: Vec<&str> = match from {
                        FromMatcher::Single(name) => vec![name.as_str()],
                        FromMatcher::Multiple(names) => names.iter().map(|s| s.as_str()).collect(),
                    };
                    for name in names_to_check {
                        if !inbound_names.contains(name) {
                            return Err(anyhow::anyhow!(
                                "Invalid inbound reference in 'when.from': '{}'. Available inbounds: {:?}",
                                name,
                                inbound_names.iter().collect::<Vec<_>>()
                            ));
                        }
                    }
                }
            }

            // Validate route action targets valid outbounds
            if let Some(ref action) = step.action {
                if action == "route" {
                    if let Some(to) = step.params.get("to") {
                        let targets: Vec<String> = match to {
                            serde_yaml::Value::String(s) => vec![s.clone()],
                            serde_yaml::Value::Sequence(seq) => seq
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect(),
                            _ => vec![],
                        };
                        for target in targets {
                            if !outbound_names.contains(target.as_str()) {
                                return Err(anyhow::anyhow!(
                                    "Invalid outbound reference in 'route.to': '{}'. Available outbounds: {:?}",
                                    target,
                                    outbound_names.iter().collect::<Vec<_>>()
                                ));
                            }
                        }
                    }
                }
            }

            // Recursively validate nested goto steps
            if let Some(ref goto_steps) = step.goto {
                self.validate_steps(goto_steps, inbound_names, outbound_names)?;
            }
        }

        Ok(())
    }
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
            OutboundConfig::S3(c) => &c.name,
        }
    }
}
