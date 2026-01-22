//! Integration tests for Phase 12: Plugin Architecture
//!
//! These tests verify that plugin registration, discovery, and instantiation work correctly.

use ondeks_core::plugin::{Plugin, PluginInfo, PluginCategory, PluginError, PluginManager};
use ondeks_core::graph::{AudioNode, ProcessContext, NodeInputs, NodeOutputs, InputPort, OutputPort};
use ondeks_core::dsp::{Buffer, Sample};
use ondeks_core::ParameterId;

/// A simple test plugin that wraps a gain node.
struct TestGainPlugin {
    info: PluginInfo,
    gain: Sample,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    gain_param_id: ParameterId,
}

impl TestGainPlugin {
    fn new() -> Self {
        Self {
            info: PluginInfo {
                id: "test.gain".to_string(),
                name: "Test Gain".to_string(),
                vendor: "Test Vendor".to_string(),
                version: "1.0.0".to_string(),
                category: PluginCategory::Effect,
            },
            gain: 1.0,
            inputs: vec![InputPort::audio("in")],
            outputs: vec![OutputPort::audio("out")],
            gain_param_id: ParameterId::generate(),
        }
    }
}

impl AudioNode for TestGainPlugin {
    fn name(&self) -> &str {
        "Test Gain"
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, _context: &ProcessContext, inputs: NodeInputs, mut outputs: NodeOutputs) {
        if let (Some(input), Some(output)) = (inputs.audio(0), outputs.audio_mut(0)) {
            output.copy_from(input);
            output.apply_gain(self.gain);
        } else if let Some(output) = outputs.audio_mut(0) {
            output.silence();
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn get_parameter(&self, id: ParameterId) -> Option<Sample> {
        if id == self.gain_param_id {
            Some(self.gain)
        } else {
            None
        }
    }

    fn set_parameter(&mut self, id: ParameterId, value: Sample) {
        if id == self.gain_param_id {
            self.gain = value.max(0.0);
        }
    }
}

impl Plugin for TestGainPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn get_state(&self) -> Vec<u8> {
        // Simple state serialization: just the gain value as bytes
        self.gain.to_le_bytes().to_vec()
    }

    fn set_state(&mut self, state: &[u8]) -> Result<(), PluginError> {
        if state.len() != 4 {
            return Err(PluginError::StateError("Invalid state size".to_string()));
        }
        let bytes: [u8; 4] = [state[0], state[1], state[2], state[3]];
        self.gain = Sample::from_le_bytes(bytes);
        Ok(())
    }
}

/// A test instrument plugin.
struct TestInstrumentPlugin {
    info: PluginInfo,
    outputs: Vec<OutputPort>,
}

impl TestInstrumentPlugin {
    fn new() -> Self {
        Self {
            info: PluginInfo {
                id: "test.instrument".to_string(),
                name: "Test Instrument".to_string(),
                vendor: "Test Vendor".to_string(),
                version: "1.0.0".to_string(),
                category: PluginCategory::Instrument,
            },
            outputs: vec![OutputPort::audio("out")],
        }
    }
}

impl AudioNode for TestInstrumentPlugin {
    fn name(&self) -> &str {
        "Test Instrument"
    }

    fn inputs(&self) -> &[InputPort] {
        &[]
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, _context: &ProcessContext, _inputs: NodeInputs, mut outputs: NodeOutputs) {
        if let Some(output) = outputs.audio_mut(0) {
            output.silence();
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }
}

impl Plugin for TestInstrumentPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn get_state(&self) -> Vec<u8> {
        vec![]
    }

    fn set_state(&mut self, _state: &[u8]) -> Result<(), PluginError> {
        Ok(())
    }
}

#[test]
fn plugin_manager_new() {
    let manager = PluginManager::new();
    assert_eq!(manager.available_plugins().len(), 0);
}

#[test]
fn plugin_manager_default() {
    let manager = PluginManager::default();
    assert_eq!(manager.available_plugins().len(), 0);
}

#[test]
fn plugin_manager_register() {
    let mut manager = PluginManager::new();
    
    let info = PluginInfo {
        id: "test.gain".to_string(),
        name: "Test Gain".to_string(),
        vendor: "Test Vendor".to_string(),
        version: "1.0.0".to_string(),
        category: PluginCategory::Effect,
    };
    
    manager.register("test.gain", info.clone(), || Box::new(TestGainPlugin::new()));
    
    assert_eq!(manager.available_plugins().len(), 1);
    let registered_info = manager.get_info("test.gain").unwrap();
    assert_eq!(registered_info.id, info.id);
    assert_eq!(registered_info.name, info.name);
    assert_eq!(registered_info.vendor, info.vendor);
    assert_eq!(registered_info.version, info.version);
    assert_eq!(registered_info.category, info.category);
}

#[test]
fn plugin_manager_create() {
    let mut manager = PluginManager::new();
    
    let info = PluginInfo {
        id: "test.gain".to_string(),
        name: "Test Gain".to_string(),
        vendor: "Test Vendor".to_string(),
        version: "1.0.0".to_string(),
        category: PluginCategory::Effect,
    };
    
    manager.register("test.gain", info, || Box::new(TestGainPlugin::new()));
    
    let plugin = manager.create("test.gain").unwrap();
    assert_eq!(plugin.info().id, "test.gain");
    assert_eq!(plugin.info().name, "Test Gain");
}

#[test]
fn plugin_manager_create_not_found() {
    let manager = PluginManager::new();
    
    let result = manager.create("nonexistent");
    assert!(result.is_err());
    match result {
        Err(PluginError::NotFound(id)) => assert_eq!(id, "nonexistent"),
        _ => panic!("Expected NotFound error"),
    }
}

#[test]
fn plugin_manager_plugins_by_category() {
    let mut manager = PluginManager::new();
    
    let effect_info = PluginInfo {
        id: "test.effect".to_string(),
        name: "Test Effect".to_string(),
        vendor: "Test Vendor".to_string(),
        version: "1.0.0".to_string(),
        category: PluginCategory::Effect,
    };
    
    let instrument_info = PluginInfo {
        id: "test.instrument".to_string(),
        name: "Test Instrument".to_string(),
        vendor: "Test Vendor".to_string(),
        version: "1.0.0".to_string(),
        category: PluginCategory::Instrument,
    };
    
    manager.register("test.effect", effect_info, || Box::new(TestGainPlugin::new()));
    manager.register("test.instrument", instrument_info, || Box::new(TestInstrumentPlugin::new()));
    
    let effects = manager.plugins_by_category(PluginCategory::Effect);
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].id, "test.effect");
    
    let instruments = manager.plugins_by_category(PluginCategory::Instrument);
    assert_eq!(instruments.len(), 1);
    assert_eq!(instruments[0].id, "test.instrument");
}

#[test]
fn plugin_info_access() {
    let plugin = TestGainPlugin::new();
    let info = plugin.info();
    
    assert_eq!(info.id, "test.gain");
    assert_eq!(info.name, "Test Gain");
    assert_eq!(info.vendor, "Test Vendor");
    assert_eq!(info.version, "1.0.0");
    assert_eq!(info.category, PluginCategory::Effect);
}

#[test]
fn plugin_category_variants() {
    assert_eq!(PluginCategory::Instrument, PluginCategory::Instrument);
    assert_eq!(PluginCategory::Effect, PluginCategory::Effect);
    assert_eq!(PluginCategory::MidiEffect, PluginCategory::MidiEffect);
    assert_eq!(PluginCategory::Utility, PluginCategory::Utility);
    
    assert_ne!(PluginCategory::Instrument, PluginCategory::Effect);
    assert_ne!(PluginCategory::Effect, PluginCategory::MidiEffect);
}

#[test]
fn plugin_state_serialization() {
    let mut plugin = TestGainPlugin::new();
    plugin.gain = 0.5;
    
    let state = plugin.get_state();
    assert_eq!(state.len(), 4);
    
    let mut restored = TestGainPlugin::new();
    restored.set_state(&state).unwrap();
    assert_eq!(restored.gain, 0.5);
}

#[test]
fn plugin_state_deserialization_error() {
    let mut plugin = TestGainPlugin::new();
    
    let invalid_state = vec![1, 2, 3]; // Wrong size
    let result = plugin.set_state(&invalid_state);
    assert!(result.is_err());
    match result.unwrap_err() {
        PluginError::StateError(_) => {},
        _ => panic!("Expected StateError"),
    }
}

#[test]
fn plugin_implements_audio_node() {
    let mut plugin = TestGainPlugin::new();
    
    // Verify it implements AudioNode
    assert_eq!(plugin.name(), "Test Gain");
    assert_eq!(plugin.inputs().len(), 1);
    assert_eq!(plugin.outputs().len(), 1);
    
    // Test processing
    let context = ProcessContext::new(4, 44100, 120.0);
    let input = Buffer::from_samples(vec![0.5, -0.5, 0.25, 0.0]);
    let mut output = Buffer::allocate(4);
    
    let inputs = NodeInputs {
        audio: &[&input],
        midi: &[],
        controls: &[],
    };
    let output_ref: &mut Buffer = &mut output;
    let outputs = NodeOutputs {
        audio: &mut [output_ref],
        midi: &mut vec![],
    };
    
    plugin.process(&context, inputs, outputs);
    
    // Should pass through with unity gain
    assert_eq!(output.as_slice(), input.as_slice());
}

#[test]
fn plugin_manager_multiple_plugins() {
    let mut manager = PluginManager::new();
    
    manager.register(
        "test.gain",
        PluginInfo {
            id: "test.gain".to_string(),
            name: "Test Gain".to_string(),
            vendor: "Test Vendor".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
        },
        || Box::new(TestGainPlugin::new()),
    );
    
    manager.register(
        "test.instrument",
        PluginInfo {
            id: "test.instrument".to_string(),
            name: "Test Instrument".to_string(),
            vendor: "Test Vendor".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Instrument,
        },
        || Box::new(TestInstrumentPlugin::new()),
    );
    
    assert_eq!(manager.available_plugins().len(), 2);
    
    let gain_plugin = manager.create("test.gain").unwrap();
    assert_eq!(gain_plugin.info().category, PluginCategory::Effect);
    
    let instrument_plugin = manager.create("test.instrument").unwrap();
    assert_eq!(instrument_plugin.info().category, PluginCategory::Instrument);
}

#[test]
fn plugin_manager_get_info() {
    let mut manager = PluginManager::new();
    
    let info = PluginInfo {
        id: "test.gain".to_string(),
        name: "Test Gain".to_string(),
        vendor: "Test Vendor".to_string(),
        version: "1.0.0".to_string(),
        category: PluginCategory::Effect,
    };
    
    manager.register("test.gain", info.clone(), || Box::new(TestGainPlugin::new()));
    
    let retrieved = manager.get_info("test.gain").unwrap();
    assert_eq!(retrieved.id, info.id);
    assert_eq!(retrieved.name, info.name);
    
    assert!(manager.get_info("nonexistent").is_none());
}
