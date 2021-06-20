use rocket_dyn_templates::Template;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Serialize, Debug, PartialEq, Eq)]
struct DirEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: String,
    pub is_symlink: bool,
    pub path: String,
    pub last_modified: Option<String>,
}

impl PartialOrd for DirEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(
            other
                .is_directory
                .cmp(&self.is_directory)
                .then(self.name.cmp(&other.name)),
        )
    }
}

impl Ord for DirEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.is_directory
            .cmp(&other.is_directory)
            .then(self.name.cmp(&other.name))
    }
}

fn format_byte_size(mut bytes: u64) -> String {
    const SUFFIXES: &str = "kMGTPE";
    if bytes < 1000 {
        format!("{} B", bytes)
    } else {
        let mut iter = SUFFIXES.bytes().peekable();
        while bytes >= 999_950 {
            bytes /= 1000;
            iter.next();
        }
        let suffix = *iter.peek().unwrap() as char;

        format!("{:.2} {}B", bytes as f64 / 1000.0, suffix)
    }
}

#[derive(Debug)]
pub struct DirectoryIndex {
    url_path: PathBuf,
    fs_path: PathBuf,
    root_dir: PathBuf,
}

impl DirectoryIndex {
    pub fn new(url_path: PathBuf, fs_path: PathBuf, root_dir: PathBuf) -> Self {
        Self {
            url_path,
            fs_path,
            root_dir,
        }
    }

    async fn list_entries(&self) -> io::Result<Vec<DirEntry>> {
        log::info!("listing directory {}", self.fs_path.display());

        let mut stream = fs::read_dir(&self.fs_path).await?;
        let mut entries = vec![];
        let formattter = timeago::Formatter::new();

        while let Ok(Some(entry)) = stream.next_entry().await {
            let file_name: String = entry.file_name().to_string_lossy().into();
            if file_name.starts_with('.') {
                continue;
            }
            let metadata = entry.metadata().await?;
            let path = entry
                .path()
                .strip_prefix(&self.root_dir)
                .map(Path::to_owned)
                .unwrap_or_else(|_| entry.path())
                .display()
                .to_string();

            let last_modified = metadata
                .accessed()
                .ok()
                .map(|time| formattter.convert(time.elapsed().expect("should not fail")));

            entries.push(DirEntry {
                name: file_name,
                path,
                is_directory: metadata.is_dir(),
                size: format_byte_size(metadata.len()),
                is_symlink: metadata.file_type().is_symlink(),
                last_modified,
            })
        }

        entries.sort();
        Ok(entries)
    }

    pub async fn render_template(&self, is_root: bool) -> io::Result<Template> {
        let entries = self.list_entries().await?;
        let mut ctx = HashMap::new();

        ctx.insert("entries", json!(entries));
        let mut base_dir = self
            .fs_path
            .strip_prefix(&self.root_dir)
            .map(Path::to_owned)
            .unwrap_or_else(|_| self.fs_path.clone());

        if !base_dir.starts_with("/") {
            base_dir = Path::new("/").join(base_dir);
        }

        let base_dir = base_dir.display().to_string();
        ctx.insert("base_dir", json!(base_dir));

        if let Some(parent) = self.url_path.parent() {
            if !is_root {
                let parent = if parent.starts_with("/") {
                    parent.to_path_buf()
                } else {
                    Path::new("/").join(parent)
                };
                log::info!("parent: {}", parent.display());
                ctx.insert("parent", json!(parent.display().to_string()));
            }
        }
        Ok(Template::render("index", ctx))
    }
}
