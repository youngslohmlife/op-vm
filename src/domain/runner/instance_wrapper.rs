use crate::domain::runner::MAX_MEMORY_SIZE;
use wasmer::{
    AsStoreMut, AsStoreRef, ExportError, Function, Instance, Memory, MemoryAccessError,
    MemoryError, MemoryType, MemoryView, Pages, Value,
};
use wasmer_middlewares::metering::{get_remaining_points, set_remaining_points, MeteringPoints};

#[derive(Clone)]
pub struct InstanceWrapper {
    instance: Instance,
}

impl InstanceWrapper {
    pub fn new(instance: Instance) -> Self {
        Self { instance }
    }

    pub fn call(
        &self,
        store: &mut impl AsStoreMut,
        function: &str,
        params: &[Value],
    ) -> anyhow::Result<Box<[Value]>> {
        let export = Self::get_function(&self.instance, function)?;
        let result = export.call(store, params)?;

        Ok(result)
    }

    pub fn is_out_of_memory(
        &self,
        store: &(impl AsStoreRef + ?Sized),
    ) -> Result<bool, MemoryAccessError> {
        let memory = Self::get_memory(&self.instance);
        let view = memory.view(store);
        let size = view.data_size();

        Ok(MAX_MEMORY_SIZE <= size)
    }

    pub fn read_arraybuffer_len(
        &self,
        ptr: u64,
        view: &MemoryView,
    ) -> Result<u32, MemoryAccessError> {
        if ptr < 4 {
            return Err(MemoryAccessError::HeapOutOfBounds);
        }
        let mut length_buffer: Vec<u8> = vec![0; 4];
        view.read(ptr - 4, &mut length_buffer)?;
        let length = u32::from_le_bytes(length_buffer.try_into().unwrap());
        Ok(length)
    }
    pub fn read_arraybuffer(
        &self,
        store: &(impl AsStoreRef + ?Sized),
        ptr: u64,
        memory: &str,
    ) -> Result<Vec<u8>, MemoryAccessError> {
        let memory = Self::get_memory_from(&self.instance, memory);
        let view = memory.view(store);
        let length = self.read_arraybuffer_len(ptr, &view)?;
        let mut result: Vec<u8> = vec![0; length as usize];
        view.read(ptr, &mut result)?;
        Ok(result)
    }

    pub fn read_memory(
        &self,
        store: &(impl AsStoreRef + ?Sized),
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, MemoryAccessError> {
        let memory = Self::get_memory(&self.instance);
        let view = memory.view(store);

        let mut buffer: Vec<u8> = vec![0; length as usize];
        view.read(offset, &mut buffer)?;

        Ok(buffer)
    }

    pub fn read_memory_u8(
        &self,
        store: &(impl AsStoreRef + ?Sized),
        offset: u64,
    ) -> Result<u8, MemoryAccessError> {
        let memory = Self::get_memory(&self.instance);
        let view = memory.view(store);
        view.read_u8(offset)
    }

    pub fn prep_for_cache(&self, store: &mut impl AsStoreMut) -> Result<(), MemoryError> {
        let memory = Self::get_memory(&self.instance);
        memory.reset(store)?;
        Ok(())
    }

    pub fn init_storage(&mut self, store: &mut impl AsStoreMut) -> anyhow::Result<()> {
        self.instance.exports.insert(
            "storage",
            Memory::new(store, MemoryType::new(1, None, false)).unwrap(),
        );
        Ok(())
    }

    pub fn request_storage(&self, store: &mut impl AsStoreMut, key: u64) -> anyhow::Result<u64> {
        let storage = Self::get_storage(&self.instance);
        let storage_view = storage.view(store);
        let length = self.read_arraybuffer_len(key, &storage_view)?;
        let ptr = self.get_write_offset_for_size(store, length as u64, "memory")?;
        Ok(ptr)
    }

    pub fn load_from_storage(
        &self,
        store: &mut impl AsStoreMut,
        key: u64,
        ptr_start: u64,
    ) -> anyhow::Result<()> {
        let data = self.read_arraybuffer(store, key, "storage")?;
        self.write_memory(store, ptr_start, data.as_slice())?;
        Ok(())
    }

    pub fn get_write_offset_for_size(
        &self,
        store: &mut impl AsStoreMut,
        data_size: u64,
        memory: &str,
    ) -> Result<u64, MemoryError> {
        let memory = Self::get_memory_from(&self.instance, memory);
        let size = memory.view(store).data_size();
        let total = size + data_size;
        if total > MAX_MEMORY_SIZE {
            return Err(MemoryError::Generic("could not provision".to_string()));
        }
        memory.grow_at_least(store, total)?;
        Ok(size)
    }

    pub fn write_memory(
        &self,
        store: &(impl AsStoreRef + ?Sized),
        offset: u64,
        data: &[u8],
    ) -> Result<(), MemoryAccessError> {
        let memory = Self::get_memory(&self.instance);
        let view = memory.view(store);
        view.write(offset, data)
    }

    pub fn write_storage(
        &self,
        store: &(impl AsStoreRef + ?Sized),
        offset: u64,
        data: &[u8],
    ) -> Result<(), MemoryAccessError> {
        let memory = Self::get_storage(&self.instance);
        let view = memory.view(store);
        view.write(offset, data)
    }
    pub fn use_gas(&self, store: &mut impl AsStoreMut, gas_cost: u64) {
        let gas_before = self.get_remaining_gas(store);

        let gas_after = if gas_before <= gas_cost {
            0
        } else {
            gas_before - gas_cost
        };

        self.set_remaining_gas(store, gas_after);
    }

    pub fn get_remaining_gas(&self, store: &mut impl AsStoreMut) -> u64 {
        let remaining_points = get_remaining_points(store, &self.instance);
        match remaining_points {
            MeteringPoints::Remaining(remaining) => remaining,
            MeteringPoints::Exhausted => 0,
        }
    }

    pub fn set_remaining_gas(&self, store: &mut impl AsStoreMut, gas: u64) {
        set_remaining_points(store, &self.instance, gas);
    }

    fn get_memory(instance: &Instance) -> &Memory {
        instance.exports.get_memory("memory").unwrap()
    }

    fn get_storage(instance: &Instance) -> &Memory {
        instance.exports.get_memory("storage").unwrap()
    }
    fn get_memory_from<'a>(instance: &'a Instance, ty: &str) -> &'a Memory {
        instance.exports.get_memory(ty).unwrap()
    }
    fn get_function<'a>(
        instance: &'a Instance,
        function: &str,
    ) -> Result<&'a Function, ExportError> {
        instance.exports.get_function(function)
    }
}
