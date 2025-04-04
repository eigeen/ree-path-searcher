/// Creates a string from a slice of u16 values.
pub fn string_from_utf16_bytes(slice: &[u8]) -> Option<String> {
    if slice.len() < 2 {
        return None;
    }
    let u16_slice = slice
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect::<Vec<u16>>();

    String::from_utf16(&u16_slice).ok()
}
