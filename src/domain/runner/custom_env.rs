use crate::domain::runner::{AbortData, InstanceWrapper};
use crate::domain::runner::bitcoin_network::BitcoinNetwork;

pub struct CustomEnv {
    pub instance: Option<InstanceWrapper>,
    pub network: BitcoinNetwork,
    pub abort_data: Option<AbortData>
}

impl CustomEnv {
    pub fn new(
        network: BitcoinNetwork,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            instance: None,
            network,
            abort_data: None
        })
    }
}
