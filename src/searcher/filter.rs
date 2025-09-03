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
            // Skip too small files
            return Ok(true);
        }

        if check_skip_format(context.data[0..8].try_into().unwrap()) {
            return Ok(true);
        }

        Ok(false)
    }
}

fn check_skip_format(magic: [u8; 8]) -> bool {
    let magic_lower = u32::from_le_bytes(magic[0..4].try_into().unwrap());
    let magic_upper = u32::from_le_bytes(magic[4..8].try_into().unwrap());

    match magic_lower {
        0x584554 => return true,   // tex
        0x44484B42 => return true, // bnk
        0x4B504B41 => return true, // pck
        _ => {}
    };
    #[allow(clippy::single_match)]
    match magic_upper {
        0x47534D47 => return true, // msg
        _ => {}
    };

    false
}
