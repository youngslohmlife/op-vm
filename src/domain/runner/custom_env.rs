use std::sync::Arc;

use crate::domain::runner::bitcoin_network::BitcoinNetwork;
use crate::domain::runner::{AbortData, InstanceWrapper};
use crate::interfaces::napi::js_contract_manager::Functions;

pub struct CustomEnv {
    pub instance: Option<InstanceWrapper>,
    pub network: BitcoinNetwork,
    pub abort_data: Option<AbortData>,
    functions: Arc<Functions>,
}

impl CustomEnv {
    pub fn new(network: BitcoinNetwork, functions: Arc<Functions>) -> anyhow::Result<Self> {
        Ok(Self {
            instance: None,
            network,
            abort_data: None,
            functions,
        })
    }

    pub fn get_functions(&self) -> Functions {
        Arc::try_unwrap(self.functions.clone()).unwrap()
    }
}
