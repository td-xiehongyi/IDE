pub const DEFAULT_CHUNK_CHARACTERS: usize = 8_000;
pub const DEFAULT_CHUNK_OVERLAP: usize = 400;

pub fn chunk_text(
    text: &str,
    chunk_characters: usize,
    overlap: usize,
) -> Result<Vec<String>, String> {
    if chunk_characters == 0 || overlap >= chunk_characters {
        return Err("分段大小必须大于重叠大小".into());
    }
    let characters: Vec<char> = text.chars().collect();
    if characters.is_empty() {
        return Ok(Vec::new());
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < characters.len() {
        let end = (start + chunk_characters).min(characters.len());
        chunks.push(characters[start..end].iter().collect());
        if end == characters.len() {
            break;
        }
        start = end - overlap;
    }
    Ok(chunks)
}
