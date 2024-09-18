use std::collections::HashMap;

use anyhow::anyhow;
use bytes::Bytes;
use napi::bindgen_prelude::{AsyncTask, BigInt, Buffer, Undefined};
use napi::{Env, Error, JsFunction, JsNumber};

use crate::interfaces::napi::bitcoin_network_request::BitcoinNetworkRequest;
use crate::interfaces::napi::contract::JsContractParameter;
use crate::interfaces::napi::js_contract::JsContract;
use crate::interfaces::{AbortDataResponse, ContractCallTask};

macro_rules! create_tsfn {
    ($id:ident) => {
        $id.create_threadsafe_function(10, |ctx| Ok(vec![ctx.value]))?
    };
}

macro_rules! abort_tsfn {
    ($id:expr, $env:expr) => {
        if !$id.aborted() {
            $id.clone().abort()?;
        }

        $id.unref(&$env)
            .map_err(|e| Error::from_reason(format!("{:?}", e)))?;
    };
}

#[napi(js_name = "ContractManager")]
pub struct ContractManager {
    contracts: HashMap<u64, JsContract>,
    contract_cache: HashMap<String, Bytes>,
    next_id: u64,
    storage_load_tsfn: ThreadsafeFunction<ThreadSafeJsImportResponse, ErrorStrategy::CalleeHandled>,
    storage_store_tsfn:
        ThreadsafeFunction<ThreadSafeJsImportResponse, ErrorStrategy::CalleeHandled>,
    call_other_contract_tsfn:
        ThreadsafeFunction<ThreadSafeJsImportResponse, ErrorStrategy::CalleeHandled>,
    deploy_from_address_tsfn:
        ThreadsafeFunction<ThreadSafeJsImportResponse, ErrorStrategy::CalleeHandled>,
    console_log_tsfn: ThreadsafeFunction<ThreadSafeJsImportResponse, ErrorStrategy::CalleeHandled>
}


#[napi]
impl ContractManager {
    #[napi(constructor)]
    pub fn new(
      storage_load_js_function: JsFunction
      #[napi(
        ts_arg_type = "(_: never, result: ThreadSafeJsImportResponse) => Promise<Buffer | Uint8Array>"
      )],
      storage_store_js_function: JsFunction,
      #[napi(
        ts_arg_type = "(_: never, result: ThreadSafeJsImportResponse) => Buffer | Uint8Array"
      )],
      call_other_contract_js_function: JsFunction,
      #[napi(
        ts_arg_type = "(_: never, result: ThreadSafeJsImportResponse) => Promise<Buffer | Uint8Array>"
      )],
      deploy_from_address_js_function: JsFunction,
      #[napi(
        ts_arg_type = "(_: never, result: ThreadSafeJsImportResponse) => Promise<void>"
      )],
      console_log_js_function: JsFunction
      #[napi(
        ts_arg_type = "(_: never, result: ThreadSafeJsImportResponse) => Buffer | Uint8Array"
      )]) {
        ContractManager {
            contracts: HashMap::new(),
            contract_cache: HashMap::new(),
            next_id: 1, // Start the ID counter at 1 (or 0, if preferred)
            storage_load_tsfn: create_tsfn!(storage_load_js_function),
            storage_store_tsfn: create_tsfn!(storage_store_js_function),
            call_other_contract_tsfn: create_tsfn!(call_other_contract_js_function),
            deploy_from_address_tsfn: create_tsfn!(deploy_from_address_js_function),
i           console_log_tsfn: create_tsfn!(console_log_js_function)
        }
    }
    #[napi]
    pub fn instantiate(&mut self, address: String, bytecode: Option<Buffer>, max_gas: BigInt, network: BitcoinNetworkRequest) -> Result<BigInt, Error> {
        let max_gas = max_gas.get_u64().1;

        let mut params: JsContractParameter = JsContractParameter {
            bytecode: None,
            serialized: None,
            max_gas,
            network
        };

        let mut should_cache: bool = false;
        if self.contract_cache.contains_key(&address) {
            let serialized = self.contract_cache.get(&address).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
            params.serialized = Some(serialized.clone());
        } else {
            let bytecode = bytecode.ok_or_else(|| Error::from_reason(anyhow!("Bytecode is required").to_string()))?.to_vec();

            should_cache = true;
            params.bytecode = Some(bytecode);
        }

        let js_contract: JsContract = JsContract::from(self, params)?;
        if should_cache {
            let serialized = js_contract.serialize()?;
            self.contract_cache.insert(address, serialized);
        }
        let id = self.add_contract(js_contract)?;
        Ok(BigInt::from(id))
    }

    #[napi]
    pub fn validate_bytecode(&self, bytecode: Buffer, max_gas: BigInt) -> Result<bool, Error> {
        JsContract::validate_bytecode(bytecode, max_gas)
    }

    #[napi]
    pub fn destroy(&mut self, env: Env, id: BigInt) -> Result<bool, Error> {
        //catch_unwind(|| {
        let id = id.get_u64().1;

        let contract = self.contracts.get_mut(&id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        contract.destroy(env)?;

        match self.contracts.remove(&id) {
            Some(_) => Ok(true),
            None => Ok(false),
        }
        //})
        //   .unwrap_or_else(|e| Err(Error::from_reason(format!("{:?}", e))))
    }

    #[napi]
    pub fn destroy_all(&mut self, env: Env) -> Result<(), Error> {
        //catch_unwind(|| {
        for contract in self.contracts.values_mut() {
            contract.destroy(env)?;
        }

        self.contracts.clear();
        self.contract_cache.clear();

        Ok(())
        //})
        //    .unwrap_or_else(|e| Err(Error::from_reason(format!("{:?}", e))))
    }

    // Add a JsContract to the map and return its ID
    fn add_contract(&mut self, contract: JsContract) -> Result<u64, Error> {
        if self.next_id > u64::MAX - 1 {
            self.next_id = 1;
        }

        let id = self.next_id;
        self.next_id += 1;
        contract.set_id(id);
        self.contracts.insert(id, contract);

        Ok(id)
    }

    #[napi]
    pub fn use_gas(&self, contract_id: BigInt, gas: BigInt) -> Result<(), Error> {
        let id = contract_id.get_u64().1;

        let contract = self.contracts.get(&id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        contract.use_gas(gas)
    }

    #[napi]
    pub fn write_buffer(&self, contract_id: BigInt, value: Buffer, id: i32, align: u32) -> Result<i64, Error> {
        let contract_id = contract_id.get_u64().1;

        let contract = self.contracts.get(&contract_id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        contract.write_buffer(value, id, align)
    }

    #[napi]
    pub fn get_abort_data(&self, contract_id: BigInt) -> Result<AbortDataResponse, Error> {
        let id = contract_id.get_u64().1;

        let contract = self.contracts.get(&id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        contract.get_abort_data()
    }


    #[napi]
    pub fn set_remaining_gas(&self, id: BigInt, gas: BigInt) -> Result<(), Error> {
        let id = id.get_u64().1;

        let contract = self.contracts.get(&id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        contract.set_remaining_gas(gas)
    }

    #[napi]
    pub fn get_remaining_gas(&self, id: BigInt) -> Result<BigInt, Error> {
        let id = id.get_u64().1;

        let contract = self.contracts.get(&id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        contract.get_remaining_gas()
    }

    #[napi]
    pub fn set_used_gas(&self, id: BigInt, gas: BigInt) -> Result<(), Error> {
        let id = id.get_u64().1;

        let contract = self.contracts.get(&id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        contract.set_used_gas(gas)
    }

    #[napi]
    pub fn get_used_gas(&self, id: BigInt) -> Result<BigInt, Error> {
        let id = id.get_u64().1;

        let contract = self.contracts.get(&id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        contract.get_used_gas()
    }

    #[napi]
    pub fn write_memory(&self, id: BigInt, offset: BigInt, data: Buffer) -> Result<Undefined, Error> {
        let id = id.get_u64().1;

        let contract = self.contracts.get(&id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        contract.write_memory(offset, data)
    }

    #[napi]
    pub fn read_memory(&self, id: BigInt, offset: BigInt, length: BigInt) -> Result<Buffer, Error> {
        let id = id.get_u64().1;

        let contract = self.contracts.get(&id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        contract.read_memory(offset, length)
    }

    #[napi(ts_return_type = "Promise<CallResponse>")]
    pub fn call(
        &self,
        id: BigInt,
        func_name: String,
        params: Vec<JsNumber>,
    ) -> Result<AsyncTask<ContractCallTask>, Error> {
        let id = id.get_u64().1;

        let contract = self.contracts.get(&id).ok_or_else(|| Error::from_reason(anyhow!("Contract not found").to_string()))?;
        let result = contract.call(func_name, params)?;

        Ok(result)
    }

    #[napi]
    pub fn length(&self) -> Result<BigInt, Error> {
        Ok(BigInt::from(self.contracts.len() as u64))
    }

    #[napi]
    pub fn clear(&mut self, env: Env) -> Result<(), Error> {
        for contract in self.contracts.values_mut() {
            contract.destroy(env)?;
        }

        self.contracts.clear();

        Ok(())
    }
}
