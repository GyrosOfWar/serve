use rocket::form::name::Name;
use rocket::fs::NamedFile;
use rocket::Either;
use rocket_dyn_templates::Template;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::io;
use std::path::PathBuf;

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

#[rocket::get("/<path..>")]
async fn serve_directory(mut path: PathBuf) -> io::Result<Either<NamedFile, Template>> {
    use tokio::fs;

    if path.to_str() == Some("") {
        path.push(".")
    }
    if path.is_file() {
        log::info!("serving file {}", path.display());
        Ok(Either::Left(NamedFile::open(path).await?))
    } else {
        log::info!("listing directory {}", path.display());
        let mut ctx = HashMap::new();
        let mut stream = fs::read_dir(path).await?;

        let mut entries = vec![];
        while let Ok(Some(entry)) = stream.next_entry().await {
            let file_name: String = entry.file_name().to_string_lossy().into();
            if file_name.starts_with(".") {
                continue;
            }
            let metadata = entry.metadata().await?;
            entries.push(DirEntry {
                name: file_name,
                path: entry.path().display().to_string(),
                is_directory: metadata.is_dir(),
                size: metadata.len(),
                is_symlink: metadata.file_type().is_symlink(),
            })
        }

        entries.sort();

        ctx.insert("entries", entries);
        // ctx.insert("baseDir", path.display().to_string());
        Ok(Either::Right(Template::render("index", ctx)))
    }
}

#[rocket::launch]
fn run() -> _ {
    env::set_var("RUST_LOG", "info");
    env_logger::init();

    rocket::build()
        .attach(Template::fairing())
        .mount("/", rocket::routes![serve_styles, serve_directory])
}
