use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::{Arc, Mutex};
use wasmer::{AsStoreRef, FunctionEnvMut, RuntimeError, StoreMut};

use crate::domain::assembly_script::AssemblyScript;
use crate::interfaces::ExternalFunction;

use super::{CustomEnv, InstanceWrapper, CALL_COST};

static mut _RUNTIME_CACHE: Option<Arc<Mutex<HashMap<Vec<u8>, InstanceWrapper>>>> = None;

pub fn __instantiate_cache() {
    unsafe {
        _RUNTIME_CACHE = Some(Arc::new(Mutex::new(
            HashMap::<Vec<u8>, InstanceWrapper>::new(),
        )));
    }
}

//stub
pub fn __request_load(mut context: FunctionEnvMut<CustomEnv>) -> anyhow::Result<u32> {
    Ok(0)
}

//stub
pub fn __load(
    mut context: FunctionEnvMut<CustomEnv>,
    key: u32,
    ptr_start: u32,
) -> anyhow::Result<()> {
    Ok(())
}

pub fn read_cache(key: &Vec<u8>) -> anyhow::Result<InstanceWrapper> {
    unsafe {
        match _RUNTIME_CACHE.as_ref() {
            Some(_cache) => {
                let cache = _cache
                    .lock()
                    .map_err(|_e| anyhow::anyhow!("failed to lock cache"))?
                    .clone();
                let instance = cache
                    .get(key)
                    .ok_or(anyhow::anyhow!("contract not found in cache"))?;
                Ok(instance.clone())
            }
            None => Err(anyhow::anyhow!("cache uninitialized")),
        }
    }
}

pub fn __call(
    mut context: FunctionEnvMut<CustomEnv>,
    _address: u32,
    _calldata: u32,
) -> Result<u32, RuntimeError> {
    let (env, mut store) = context.data_and_store_mut();
    let instance = &env
        .instance
        .clone()
        .ok_or(RuntimeError::new("Instance not found"))?;
    instance.use_gas(&mut store, CALL_COST);
    let address = instance
        .read_arraybuffer(&store, _address)
        .map_err(|_e| RuntimeError::new("Error reading arraybuffer"))?;
    let contract_instance = read_cache(&address).ok();

    let calldata = instance
        .read_arraybuffer(&store, _calldata)
        .map_err(|_e| RuntimeError::new("Error reading arraybuffer"))?;

    //TODO: result returns the same format as env.call_other_contract_external does:
    //the first 8 bytes for the cost, the rest as the data
    //replicates older behavior if it doesnt find the contract instance in the cache
    //add contract to cache if used
    let v: Vec<u8> = vec![0; 10];
    let result = match contract_instance {
        Some(contract) => &v,
        None => &env.call_other_contract_external.execute(&calldata)?,
    };

    let call_execution_cost_bytes = &result[0..8];
    let response = &result[8..];

    let value = AssemblyScript::write_buffer(&mut store, &instance, &response, 13, 0)
        .map_err(|_e| RuntimeError::new("error writing buffer"))?;
    let call_execution_cost = u64::from_le_bytes(call_execution_cost_bytes.try_into().unwrap());

    instance.use_gas(&mut store, call_execution_cost);
    Ok(value as u32)
}
