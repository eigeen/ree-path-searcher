pub struct FileContext {
    pub file_size: u64,
    pub file_hash: Option<u64>,
    pub data: Vec<u8>,
}

pub trait Filter {
    fn should_skip_file(
        &self,
        context: &FileContext,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
}

pub struct DefaultFilter;

impl Filter for DefaultFilter {
    fn should_skip_file(
        &self,
        context: &FileContext,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if context.data.len() < 8 {
            return Ok(false);
        }

        let magic = &context.data[0..8];
        Ok(magic == b"PAKFILE\0")
    }
}
