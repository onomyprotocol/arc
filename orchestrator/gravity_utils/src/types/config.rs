//! contains configuration structs that need to be accessed across crates.
use serde::{Deserialize, Serialize};

/// Global configuration struct for Gravity bridge tools
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default, Clone)]
pub struct GravityBridgeToolsConfig {
    pub relayer: RelayerConfig,
    pub orchestrator: OrchestratorConfig,
    pub metrics: MetricsConfig,
}

/// Toml serializable configuration struct for Gravity bridge tools
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default, Clone)]
pub struct TomlGravityBridgeToolsConfig {
    #[serde(default = "TomlRelayerConfig::default")]
    pub relayer: TomlRelayerConfig,
    #[serde(default = "OrchestratorConfig::default")]
    pub orchestrator: OrchestratorConfig,
    #[serde(default = "MetricsConfig::default")]
    pub metrics: MetricsConfig,
}

impl From<TomlGravityBridgeToolsConfig> for GravityBridgeToolsConfig {
    fn from(input: TomlGravityBridgeToolsConfig) -> Self {
        GravityBridgeToolsConfig {
            relayer: input.relayer.into(),
            orchestrator: input.orchestrator,
            metrics: input.metrics,
        }
    }
}

/// Relayer configuration options
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct RelayerConfig {
    pub valset_relaying_mode: ValsetRelayingMode,
    pub batch_request_mode: BatchRequestMode,
    pub batch_relaying_mode: BatchRelayingMode,
    pub logic_call_market_enabled: bool,
    /// the speed at which the relayer loop runs, in seconds
    /// higher values reduce the chances of money lost to a collision
    pub relayer_loop_speed: u64,
}

/// Relayer configuration that's is more easily parsable with toml
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct TomlRelayerConfig {
    #[serde(default = "default_valset_relaying_mode")]
    pub valset_relaying_mode: TomlValsetRelayingMode,
    #[serde(default = "default_batch_request_mode")]
    pub batch_request_mode: BatchRequestMode,
    #[serde(default = "default_batch_relaying_mode")]
    pub batch_relaying_mode: TomlBatchRelayingMode,
    #[serde(default = "default_logic_call_market_enabled")]
    pub logic_call_market_enabled: bool,
    #[serde(default = "default_relayer_loop_speed")]
    pub relayer_loop_speed: u64,
}

impl From<TomlRelayerConfig> for RelayerConfig {
    fn from(input: TomlRelayerConfig) -> Self {
        RelayerConfig {
            valset_relaying_mode: input.valset_relaying_mode.into(),
            batch_relaying_mode: input.batch_relaying_mode.into(),
            batch_request_mode: input.batch_request_mode,
            logic_call_market_enabled: input.logic_call_market_enabled,
            relayer_loop_speed: input.relayer_loop_speed,
        }
    }
}

/// The various possible modes for relaying validator set updates
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub enum ValsetRelayingMode {
    /// Relay validator sets when continued operation of the chain
    /// requires it, this will cost some ETH
    Altruistic,
    /// Relay every validator set update, mostly for developer use
    EveryValset,
}

/// A version of valset relaying mode that's easy to serialize as toml
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct TomlValsetRelayingMode {
    mode: String,
}

impl From<TomlValsetRelayingMode> for ValsetRelayingMode {
    fn from(input: TomlValsetRelayingMode) -> Self {
        match input.mode.to_uppercase().as_str() {
            "EVERYVALSET" => ValsetRelayingMode::EveryValset,
            "ALTRUISTIC" => ValsetRelayingMode::Altruistic,
            _ => panic!("Invalid TomlValsetRelayingMode"),
        }
    }
}

/// The various possible modes for automatic requests of batches
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub enum BatchRequestMode {
    /// Every possible valid batch should be requested
    EveryBatch,
    /// Does not automatically request batches
    None,
}

/// The various possible modes for batch relaying
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum BatchRelayingMode {
    /// Every possible batch is relayed
    EveryBatch,
    /// Does not automatically request batches
    None,
}

/// A version of BatchRelaying mode that is easy to serialize as toml
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct TomlBatchRelayingMode {
    mode: String,
}

impl From<TomlBatchRelayingMode> for BatchRelayingMode {
    fn from(input: TomlBatchRelayingMode) -> Self {
        match input.mode.to_uppercase().as_str() {
            "EVERYBATCH" => BatchRelayingMode::EveryBatch,
            _ => panic!("Bad TomlBatchRelayingMode"),
        }
    }
}

fn default_batch_relaying_mode() -> TomlBatchRelayingMode {
    TomlBatchRelayingMode {
        mode: "EveryBatch".to_string(),
    }
}

fn default_logic_call_market_enabled() -> bool {
    true
}

fn default_valset_relaying_mode() -> TomlValsetRelayingMode {
    TomlValsetRelayingMode {
        mode: "Altruistic".to_string(),
    }
}

fn default_batch_request_mode() -> BatchRequestMode {
    BatchRequestMode::EveryBatch
}

fn default_relayer_loop_speed() -> u64 {
    600
}

impl Default for RelayerConfig {
    fn default() -> Self {
        RelayerConfig {
            // FIXME check if needed
            valset_relaying_mode: default_valset_relaying_mode().into(),
            batch_request_mode: default_batch_request_mode(),
            batch_relaying_mode: default_batch_relaying_mode().into(),
            logic_call_market_enabled: default_logic_call_market_enabled(),
            relayer_loop_speed: default_relayer_loop_speed(),
        }
    }
}

impl Default for TomlRelayerConfig {
    fn default() -> Self {
        TomlRelayerConfig {
            valset_relaying_mode: default_valset_relaying_mode(),
            batch_request_mode: default_batch_request_mode(),
            batch_relaying_mode: default_batch_relaying_mode(),
            logic_call_market_enabled: default_logic_call_market_enabled(),
            relayer_loop_speed: default_relayer_loop_speed(),
        }
    }
}

/// Orchestrator configuration options
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub struct OrchestratorConfig {
    /// If this Orchestrator should run an integrated relayer or not
    #[serde(default = "default_relayer_enabled")]
    pub relayer_enabled: bool,
}

fn default_relayer_enabled() -> bool {
    false
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        OrchestratorConfig {
            relayer_enabled: default_relayer_enabled(),
        }
    }
}

/// Metrics server configuration options
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct MetricsConfig {
    /// If this Orchestrator should run an integrated metrics server or not
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
    /// Bind to specified ip:port
    #[serde(default = "default_metrics_bind")]
    pub metrics_bind: String,
}

fn default_metrics_enabled() -> bool {
    false
}

fn default_metrics_bind() -> String {
    "127.0.0.1:6631".to_string()
}

impl Default for MetricsConfig {
    fn default() -> Self {
        MetricsConfig {
            metrics_enabled: default_metrics_enabled(),
            metrics_bind: default_metrics_bind(),
        }
    }
}
