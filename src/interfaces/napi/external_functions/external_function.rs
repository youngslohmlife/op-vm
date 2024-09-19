use wasmer::RuntimeError;

pub trait ExternalFunction {
    fn execute(&self, id: u64, data: &[u8]) -> Result<Vec<u8>, RuntimeError>;
}
