pub fn take_chars(s: &str, n: usize) -> &str {
    // 找到前 n 个字符的总字节数（即第 n 个字符的起始字节索引）
    let byte_end = s
        .char_indices()
        .nth(n) // 第 n 个字符的 (byte_idx, char)
        .map(|(idx, _)| idx) // 取其字节索引
        .unwrap_or_else(|| s.len()); // 若不足 n 个字符，则取到末尾
    &s[..byte_end]
}
