//! Terminal width utilities for responsive output formatting.
//!
//! Provides functions to get terminal width and clip/compress content
//! to fit within the available space without wrapping.

use crossterm::terminal;

/// Right margin to leave for visual clarity and elegance.
const RIGHT_MARGIN: usize = 4;

/// Minimum usable terminal width (below this, we don't compress further).
const MIN_WIDTH: usize = 40;

/// Default terminal width when size cannot be determined.
const DEFAULT_WIDTH: usize = 80;

/// Get the usable terminal width (total width minus right margin).
/// 
/// Returns the terminal width minus a 4-character right margin for clarity.
/// Falls back to 80 columns (76 usable) if terminal size cannot be determined.
/// Enforces a minimum usable width of 40 characters.
pub fn get_terminal_width() -> usize {
    let width = terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(DEFAULT_WIDTH);
    
    // Subtract margin, but ensure minimum usable width
    width.saturating_sub(RIGHT_MARGIN).max(MIN_WIDTH)
}

/// Clip a line to fit within the given width, adding ellipsis if truncated.
/// 
/// Uses UTF-8 safe character counting to avoid panics on multi-byte characters.
pub fn clip_line(line: &str, max_width: usize) -> String {
    let char_count = line.chars().count();
    
    if char_count <= max_width {
        return line.to_string();
    }
    
    // Need to truncate: leave room for "…" (1 char)
    let truncate_at = max_width.saturating_sub(1);
    let truncated: String = line.chars().take(truncate_at).collect();
    format!("{}…", truncated)
}

/// Compress a file path to fit within the given width.
/// 
/// Preserves the filename and as much of the path as possible.
/// Truncates parent directories from the left, replacing with "…".
/// 
/// Examples:
/// - Full: `/Users/dhanji/src/g3/crates/g3-cli/src/ui_writer_impl.rs`
/// - Compressed: `…g3-cli/src/ui_writer_impl.rs`
/// - More compressed: `…/ui_writer_impl.rs`
pub fn compress_path(path: &str, max_width: usize) -> String {
    let char_count = path.chars().count();
    
    if char_count <= max_width {
        return path.to_string();
    }
    
    // Extract filename (last component)
    let filename = path.rsplit('/').next().unwrap_or(path);
    let filename_len = filename.chars().count();
    
    // If filename alone is too long, truncate it
    if filename_len + 1 >= max_width {
        // Just show truncated filename with ellipsis
        return clip_line(filename, max_width);
    }
    
    // Try to fit as much of the path as possible
    // Format: "…<partial_path>/<filename>"
    let available_for_path = max_width.saturating_sub(filename_len + 2); // 1 for "…", 1 for "/"
    
    if available_for_path == 0 {
        return format!("…/{}", filename);
    }
    
    // Get the directory part (everything before filename)
    let dir_part = if let Some(pos) = path.rfind('/') {
        &path[..pos]
    } else {
        return path.to_string(); // No directory separator
    };
    
    // Take characters from the end of the directory path
    let dir_chars: Vec<char> = dir_part.chars().collect();
    let dir_len = dir_chars.len();
    
    if dir_len <= available_for_path {
        return path.to_string(); // Shouldn't happen, but safety check
    }
    
    // Take the last `available_for_path` characters from the directory
    let start_idx = dir_len.saturating_sub(available_for_path);
    let partial_dir: String = dir_chars[start_idx..].iter().collect();
    
    format!("…{}/{}", partial_dir, filename)
}

/// Compress a shell command to fit within the given width.
/// 
/// Preserves the command name and as much of the arguments as possible.
/// Truncates from the right, adding "…" at the end.
pub fn compress_command(command: &str, max_width: usize) -> String {
    clip_line(command, max_width)
}

/// Calculate available width for content after accounting for a prefix.
/// 
/// This is useful for tool output lines that have a fixed prefix like "│ ".
#[allow(dead_code)] // Utility function for future use
pub fn available_width_after_prefix(prefix_width: usize) -> usize {
    get_terminal_width().saturating_sub(prefix_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_line_short() {
        let line = "hello world";
        assert_eq!(clip_line(line, 80), "hello world");
    }

    #[test]
    fn test_clip_line_exact() {
        let line = "hello";
        assert_eq!(clip_line(line, 5), "hello");
    }

    #[test]
    fn test_clip_line_truncate() {
        let line = "hello world this is a long line";
        assert_eq!(clip_line(line, 15), "hello world th…");
    }

    #[test]
    fn test_clip_line_unicode() {
        let line = "héllo wörld 你好";
        let clipped = clip_line(line, 10);
        assert_eq!(clipped.chars().count(), 10);
        assert!(clipped.ends_with('…'));
    }

    #[test]
    fn test_clip_line_empty() {
        assert_eq!(clip_line("", 80), "");
    }

    #[test]
    fn test_compress_path_short() {
        let path = "src/main.rs";
        assert_eq!(compress_path(path, 80), "src/main.rs");
    }

    #[test]
    fn test_compress_path_long() {
        let path = "/Users/dhanji/src/g3/crates/g3-cli/src/ui_writer_impl.rs";
        let compressed = compress_path(path, 40);
        assert!(compressed.chars().count() <= 40);
        assert!(compressed.ends_with("ui_writer_impl.rs"));
        assert!(compressed.starts_with('…'));
    }

    #[test]
    fn test_compress_path_preserves_filename() {
        let path = "/very/long/path/to/some/deeply/nested/file.rs";
        let compressed = compress_path(path, 20);
        assert!(compressed.contains("file.rs"));
    }

    #[test]
    fn test_compress_path_very_narrow() {
        let path = "/path/to/extremely_long_filename_that_exceeds_width.rs";
        let compressed = compress_path(path, 15);
        assert!(compressed.chars().count() <= 15);
        assert!(compressed.ends_with('…'));
    }

    #[test]
    fn test_compress_command_short() {
        let cmd = "ls -la";
        assert_eq!(compress_command(cmd, 80), "ls -la");
    }

    #[test]
    fn test_compress_command_long() {
        let cmd = "rg 'pattern' --type rust -l | head -20 | sort";
        let compressed = compress_command(cmd, 30);
        assert!(compressed.chars().count() <= 30);
        assert!(compressed.starts_with("rg 'pattern'"));
        assert!(compressed.ends_with('…'));
    }

    #[test]
    fn test_get_terminal_width_returns_reasonable_value() {
        let width = get_terminal_width();
        // Should be at least MIN_WIDTH
        assert!(width >= MIN_WIDTH);
        // Should be reasonable (not absurdly large)
        assert!(width < 1000);
    }

    #[test]
    fn test_available_width_after_prefix() {
        let width = available_width_after_prefix(3); // e.g., "│ "
        assert!(width >= MIN_WIDTH.saturating_sub(3));
    }
}
