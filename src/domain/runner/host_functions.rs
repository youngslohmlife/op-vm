use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasmer::{AsStoreMut, FunctionEnvMut, RuntimeError};

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

pub fn __write_to_cache(
    key: &Vec<u8>,
    _instance: &InstanceWrapper,
    store: &mut impl AsStoreMut,
) -> anyhow::Result<()> {
    let mut cache = get_cache()?;
    let instance = _instance.clone();
    instance.prep_for_cache(store)?;
    cache.insert(key.clone(), instance);
    Ok(())
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

pub fn get_cache() -> anyhow::Result<HashMap<Vec<u8>, InstanceWrapper>> {
    unsafe {
        match _RUNTIME_CACHE.as_ref() {
            Some(_cache) => {
                let cache = _cache
                    .lock()
                    .map_err(|_e| anyhow::anyhow!("failed to get cache lock"))?
                    .clone();
                Ok(cache)
            }
            None => Err(anyhow::anyhow!("cache uninitialized")),
        }
    }
}

pub fn read_cache(key: &Vec<u8>) -> anyhow::Result<InstanceWrapper> {
    let cache = get_cache()?;
    let instance = cache
        .get(key)
        .ok_or(anyhow::anyhow!("contract not found in cache"))?;
    Ok(instance.clone())
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

    let v: Vec<u8> = vec![0; 10];
    let result = match contract_instance {
        Some(contract) => {
            //@TODO write this out properly
            let ctr = contract.call(&mut store, "call", &[]);
            &v
        }
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
