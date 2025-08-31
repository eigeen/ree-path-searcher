/// Creates a string from a slice of u16 values.
pub fn string_from_utf16_bytes(slice: &[u8]) -> Option<String> {
    if slice.len() < 2 || slice.len() % 2 != 0 {
        return None;
    }

    let mut u16_slice = Vec::with_capacity(slice.len() / 2);

    for chunk in slice.chunks_exact(2) {
        u16_slice.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }

    String::from_utf16(&u16_slice).ok()
}
