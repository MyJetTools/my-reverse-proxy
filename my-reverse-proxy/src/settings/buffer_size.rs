/// Parses a human-friendly buffer-size value: a raw byte count, or a number
/// suffixed with `Kb` / `Mb` (e.g. `512Kb`, `1Mb`).
pub fn parse_buffer_size(value: &str) -> Result<usize, String> {
    let on_err = |err: std::num::ParseIntError| {
        format!("Can not parse buffer size value: '{}'. Error: {}", value, err)
    };

    if let Some(kb) = value.strip_suffix("Kb") {
        return kb.trim().parse::<usize>().map(|v| v * 1024).map_err(on_err);
    }

    if let Some(mb) = value.strip_suffix("Mb") {
        return mb
            .trim()
            .parse::<usize>()
            .map(|v| v * 1024 * 1024)
            .map_err(on_err);
    }

    value.parse::<usize>().map_err(on_err)
}
