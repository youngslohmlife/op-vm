use wasmer::RuntimeError;

pub trait ExternalFunction {
    fn execute(&self, id: data: &[u8]) -> Result<Vec<u8>, RuntimeError>;
}
