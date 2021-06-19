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

mod range;

#[derive(Debug, StructOpt)]
pub struct Arguments {
    #[structopt(default_value = ".", parse(from_os_str))]
    root_dir: PathBuf,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
    pub is_symlink: bool,
    pub path: String,
}

impl PartialOrd for DirEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.name.partial_cmp(&other.name)
    }
}

impl Ord for DirEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

#[rocket::get("/style.css")]
async fn serve_styles() -> Option<NamedFile> {
    NamedFile::open("./templates/style.css").await.ok()
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

            entries.push(DirEntry {
                name: file_name,
                path,
                is_directory: metadata.is_dir(),
                size: metadata.len(),
                is_symlink: metadata.file_type().is_symlink(),
            })
        }

        entries.sort();
        let entries = json!(entries);
        let base_dir = fs_path.display().to_string();
        let base_dir = json!(if base_dir == "." {
            "/".into()
        } else {
            format!("/{}", base_dir)
        });

        if let Some(parent) = url_path.parent() {
            if !is_root {
                let parent = Path::new("/").join(parent);
                log::info!("parent: {}", parent.display());
                ctx.insert("parent", json!(parent.display().to_string()));
            }
        }

        ctx.insert("entries", entries);
        ctx.insert("base_dir", base_dir);
        Ok(Either::Right(Template::render("index", ctx)))
    }
}

#[rocket::launch]
fn run() -> _ {
    let args = Arguments::from_args();

    env::set_var("RUST_LOG", "info");
    env_logger::init();

    rocket::build()
        .manage(args)
        .attach(Template::fairing())
        .mount("/", rocket::routes![serve_styles, serve_directory])
}
