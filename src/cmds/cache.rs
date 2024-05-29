use crate::cli::cache::CacheOptions;
use crate::config::{Config, ConfigProperties};
use crate::Result;
use std::fmt;
use std::sync::Arc;

pub fn execute(options: CacheOptions, config: Arc<Config>) -> Result<()> {
    match options {
        CacheOptions::Info => {
            let size = get_cache_directory_size(&config)?;
            println!("Location: {}", config.cache_location());
            println!("Size: {}", BytesToHumanReadable::from(size));
        }
    }
    Ok(())
}

struct BytesToHumanReadable(u64);

impl From<u64> for BytesToHumanReadable {
    fn from(size: u64) -> Self {
        BytesToHumanReadable(size)
    }
}

impl fmt::Display for BytesToHumanReadable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let size = self.0;
        let suffixes = ["B", "KB", "MB", "GB"];
        let suffix_len = suffixes.len();
        let mut size = size as f64;
        let mut i = 0;
        while size >= 1024.0 && i < suffix_len - 1 {
            size /= 1024.0;
            i += 1;
        }
        write!(f, "{:.2} {}", size, suffixes[i])
    }
}

fn get_cache_directory_size<D: ConfigProperties>(config: &D) -> Result<u64> {
    let path = config.cache_location();
    let mut size = 0;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        size += metadata.len();
    }
    Ok(size)
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::config::ConfigProperties;
    use std::fs::File;
    use std::io::Write;
    use tempfile::{tempdir, TempDir};

    #[test]
    fn test_bytes_display() {
        let test_table = vec![
            (0, "0.00 B"),
            (1024, "1.00 KB"),
            (1024 * 1024, "1.00 MB"),
            (1024 * 1024 * 1024, "1.00 GB"),
        ];
        for (size, expected) in test_table {
            let actual = BytesToHumanReadable::from(size).to_string();
            assert_eq!(expected, actual);
        }
    }

    struct ConfigMock {
        tmp_dir: String,
    }

    impl ConfigMock {
        fn new(tmp_dir: &TempDir) -> Self {
            Self {
                tmp_dir: tmp_dir.path().to_str().unwrap().to_string(),
            }
        }
    }

    impl ConfigProperties for ConfigMock {
        fn cache_location(&self) -> &str {
            &self.tmp_dir
        }

        fn api_token(&self) -> &str {
            todo!()
        }
    }

    #[test]
    fn test_get_size_of_cached_data() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_file");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(&[0; 10]).unwrap();
        let size = get_cache_directory_size(&ConfigMock::new(&dir)).unwrap();
        assert_eq!(size, 10);
    }
}
