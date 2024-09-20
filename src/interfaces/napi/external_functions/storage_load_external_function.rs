use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction};
use wasmer::RuntimeError;

use crate::interfaces::napi::external_functions::GenericExternalFunction;
use crate::interfaces::napi::thread_safe_js_import_response::ThreadSafeJsImportResponse;
use crate::interfaces::ExternalFunction;

#[derive(Clone)]
pub struct StorageLoadExternalFunction {
    pub external_function: GenericExternalFunction,
}

impl StorageLoadExternalFunction {
    pub fn new(
        tsfn: ThreadsafeFunction<ThreadSafeJsImportResponse, ErrorStrategy::CalleeHandled>,
    ) -> Self {
        Self {
            external_function: GenericExternalFunction::new(tsfn),
        }
    }
}

impl ExternalFunction for StorageLoadExternalFunction {
    fn execute(&self, id: u64, data: &[u8]) -> Result<Vec<u8>, RuntimeError> {
        //let time = chrono::offset::Local::now();
        let resp = self.external_function.execute(id, data);

        //log_time_diff(&time, "GenericExternalFunction::load");

        resp
    }
}
