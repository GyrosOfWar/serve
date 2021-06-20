use rocket::fs::NamedFile;
use rocket::Either;
use rocket::State;
use rocket_dyn_templates::Template;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Arguments {
    #[structopt(short, long, default_value = "8000")]
    port: u16,

    // TODO: basic auth
    #[structopt(default_value = ".", parse(from_os_str))]
    root_dir: PathBuf,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct DirEntry {
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

pub fn format_byte_size(mut bytes: u64) -> String {
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

#[rocket::get("/style.css")]
async fn serve_styles() -> Option<Either<NamedFile, &'static str>> {
    if cfg!(debug_assertions) {
        NamedFile::open("./templates/style.css")
            .await
            .ok()
            .map(Either::Left)
    } else {
        let css = include_str!("../templates/style.css");
        Some(Either::Right(css))
    }
}

#[rocket::get("/<url_path..>")]
async fn serve_directory(
    url_path: PathBuf,
    args: &State<Arguments>,
) -> io::Result<Either<NamedFile, Template>> {
    use tokio::fs;

    let is_root = url_path.components().count() == 0;
    let fs_path = args.root_dir.join(&url_path);

    if fs_path.is_file() {
        log::info!("serving file {}", fs_path.display());
        Ok(Either::Left(NamedFile::open(fs_path).await?))
    } else {
        log::info!("listing directory {}", fs_path.display());
        let mut ctx = HashMap::new();
        let mut stream = fs::read_dir(&fs_path).await?;

        let mut entries = vec![];
        while let Ok(Some(entry)) = stream.next_entry().await {
            let file_name: String = entry.file_name().to_string_lossy().into();
            if file_name.starts_with(".") {
                continue;
            }
            let metadata = entry.metadata().await?;
            let path = entry
                .path()
                .strip_prefix(&args.root_dir)
                .map(Path::to_owned)
                .unwrap_or_else(|_| entry.path())
                .display()
                .to_string();
            let formattter = timeago::Formatter::new();
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
        let mut base_dir = fs_path
            .strip_prefix(&args.root_dir)
            .map(Path::to_owned)
            .unwrap_or_else(|_| fs_path);

        if !base_dir.starts_with("/") {
            base_dir = Path::new("/").join(base_dir);
        }

        let base_dir = base_dir.display().to_string();

        if let Some(parent) = url_path.parent() {
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

        ctx.insert("entries", json!(entries));
        ctx.insert("base_dir", json!(base_dir));
        Ok(Either::Right(Template::render("index", ctx)))
    }
}

#[rocket::launch]
fn run() -> _ {
    use rocket::Config;

    let args = Arguments::from_args();

    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let is_debug = cfg!(debug_assertions);
    let default_config = if is_debug {
        Config::debug_default()
    } else {
        Config::release_default()
    };

    let config = Config {
        port: args.port,
        address: "0.0.0.0".parse().unwrap(),

        ..default_config
    };

    let mut builder = rocket::build().configure(config).manage(args);

    if is_debug {
        let templates = Template::custom(|engine| {
            engine
                .tera
                .add_raw_template("index", include_str!("../templates/index.html.tera"))
                .unwrap();
        });
        builder = builder.attach(templates);
    } else {
        builder = builder.attach(Template::fairing());
    }

    builder.mount("/", rocket::routes![serve_styles, serve_directory])
}
