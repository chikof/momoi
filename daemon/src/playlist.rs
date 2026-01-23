use anyhow::Result;
use glob::glob;
use rand::rng;
use rand::seq::SliceRandom;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Playlist state for wallpaper rotation
#[derive(Debug, Clone)]
pub struct PlaylistState {
    /// List of wallpaper paths in the playlist
    wallpapers: Vec<PathBuf>,

    /// Current index in the playlist
    current_index: usize,

    /// Shuffle order (indices into wallpapers vec)
    shuffle_order: Vec<usize>,

    /// Whether shuffle is enabled
    shuffle: bool,

    /// Last rotation time
    last_rotation: Instant,

    /// Rotation interval
    interval: Duration,

    /// Output name this playlist is for (None = global)
    output_name: Option<String>,
}

impl PlaylistState {
    /// Create a new playlist from sources
    pub fn new(
        sources: &[String],
        extensions: &[String],
        interval_secs: u64,
        shuffle: bool,
        output_name: Option<String>,
    ) -> Result<Self> {
        let wallpapers = Self::load_wallpapers_from_sources(sources, extensions)?;

        if wallpapers.is_empty() {
            anyhow::bail!("No wallpapers found in playlist sources");
        }

        let mut state = Self {
            wallpapers: wallpapers.clone(),
            current_index: 0,
            shuffle_order: Vec::new(),
            shuffle,
            last_rotation: Instant::now(),
            interval: Duration::from_secs(interval_secs),
            output_name,
        };

        if shuffle {
            state.generate_shuffle_order();
        }

        log::info!(
            "Created playlist with {} wallpapers (shuffle: {}, interval: {}s){}",
            state.wallpapers.len(),
            shuffle,
            interval_secs,
            state
                .output_name
                .as_ref()
                .map(|n| format!(" for output {}", n))
                .unwrap_or_default()
        );

        Ok(state)
    }

    /// Load wallpapers from source paths (files or glob patterns)
    fn load_wallpapers_from_sources(
        sources: &[String],
        extensions: &[String],
    ) -> Result<Vec<PathBuf>> {
        let mut wallpapers = Vec::new();

        for source in sources {
            let expanded_source = shellexpand::tilde(source);

            // Check if it's a direct file
            let source_path = Path::new(expanded_source.as_ref());
            if source_path.is_file() {
                if Self::has_valid_extension(source_path, extensions) {
                    wallpapers.push(source_path.to_path_buf());
                }
                continue;
            }

            // Check if it's a directory
            if source_path.is_dir() {
                // Scan directory for wallpapers
                for ext in extensions {
                    let pattern = format!("{}/*.{}", expanded_source, ext);
                    if let Ok(entries) = glob(&pattern) {
                        for entry in entries.flatten() {
                            if entry.is_file() {
                                wallpapers.push(entry);
                            }
                        }
                    }
                }
                continue;
            }

            // Try as glob pattern
            match glob(&expanded_source) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        if entry.is_file() && Self::has_valid_extension(&entry, extensions) {
                            wallpapers.push(entry);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to glob pattern '{}': {}", source, e);
                }
            }
        }

        // Remove duplicates
        wallpapers.sort();
        wallpapers.dedup();

        Ok(wallpapers)
    }

    /// Check if a file has a valid extension
    fn has_valid_extension(path: &Path, extensions: &[String]) -> bool {
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                return extensions.iter().any(|e| e.eq_ignore_ascii_case(ext_str));
            }
        }
        false
    }

    /// Generate a new shuffle order
    fn generate_shuffle_order(&mut self) {
        let mut rng = rng();
        self.shuffle_order = (0..self.wallpapers.len()).collect();
        self.shuffle_order.shuffle(&mut rng);
        self.current_index = 0;
        log::debug!("Generated new shuffle order");
    }

    /// Get the current wallpaper path
    pub fn current(&self) -> Option<&Path> {
        if self.wallpapers.is_empty() {
            return None;
        }

        let index = if self.shuffle {
            self.shuffle_order
                .get(self.current_index)
                .copied()
                .unwrap_or(0)
        } else {
            self.current_index
        };

        self.wallpapers.get(index).map(|p| p.as_path())
    }

    /// Move to the next wallpaper
    pub fn next(&mut self) -> Option<&Path> {
        if self.wallpapers.is_empty() {
            return None;
        }

        self.current_index = (self.current_index + 1) % self.wallpapers.len();

        // If we've completed a shuffle cycle, regenerate the order
        if self.shuffle && self.current_index == 0 {
            self.generate_shuffle_order();
        }

        self.last_rotation = Instant::now();
        self.current()
    }

    /// Move to the previous wallpaper
    pub fn prev(&mut self) -> Option<&Path> {
        if self.wallpapers.is_empty() {
            return None;
        }

        if self.current_index == 0 {
            self.current_index = self.wallpapers.len() - 1;
        } else {
            self.current_index -= 1;
        }

        self.last_rotation = Instant::now();
        self.current()
    }

    /// Check if it's time to rotate to the next wallpaper
    pub fn should_rotate(&self) -> bool {
        self.last_rotation.elapsed() >= self.interval
    }

    /// Toggle shuffle mode
    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
        if self.shuffle {
            self.generate_shuffle_order();
        }
        log::info!(
            "Shuffle {}",
            if self.shuffle { "enabled" } else { "disabled" }
        );
    }

    /// Get the number of wallpapers in the playlist
    pub fn len(&self) -> usize {
        self.wallpapers.len()
    }

    /// Check if the playlist is empty
    pub fn is_empty(&self) -> bool {
        self.wallpapers.is_empty()
    }

    /// Get the current index
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Get the output name this playlist is for
    pub fn output_name(&self) -> Option<&str> {
        self.output_name.as_deref()
    }

    /// Reset the rotation timer
    pub fn reset_timer(&mut self) {
        self.last_rotation = Instant::now();
    }

    /// Get time until next rotation
    pub fn time_until_rotation(&self) -> Duration {
        let elapsed = self.last_rotation.elapsed();
        if elapsed >= self.interval {
            Duration::from_secs(0)
        } else {
            self.interval - elapsed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_valid_extension() {
        let extensions = vec!["jpg".to_string(), "png".to_string()];

        assert!(PlaylistState::has_valid_extension(
            Path::new("test.jpg"),
            &extensions
        ));
        assert!(PlaylistState::has_valid_extension(
            Path::new("test.PNG"),
            &extensions
        ));
        assert!(!PlaylistState::has_valid_extension(
            Path::new("test.txt"),
            &extensions
        ));
    }

    #[test]
    fn test_playlist_navigation() {
        // Create a playlist with manual wallpapers for testing
        let mut playlist = PlaylistState {
            wallpapers: vec![
                PathBuf::from("/tmp/1.jpg"),
                PathBuf::from("/tmp/2.jpg"),
                PathBuf::from("/tmp/3.jpg"),
            ],
            current_index: 0,
            shuffle_order: Vec::new(),
            shuffle: false,
            last_rotation: Instant::now(),
            interval: Duration::from_secs(300),
            output_name: None,
        };

        assert_eq!(playlist.current(), Some(Path::new("/tmp/1.jpg")));

        playlist.next();
        assert_eq!(playlist.current(), Some(Path::new("/tmp/2.jpg")));

        playlist.next();
        assert_eq!(playlist.current(), Some(Path::new("/tmp/3.jpg")));

        playlist.next();
        assert_eq!(playlist.current(), Some(Path::new("/tmp/1.jpg")));

        playlist.prev();
        assert_eq!(playlist.current(), Some(Path::new("/tmp/3.jpg")));
    }
}
