use crate::index::DirectoryIndex;
use rocket::fs::NamedFile;
use rocket::{Either, State};
use rocket_dyn_templates::Template;
use std::path::PathBuf;
use std::{env, io};
use structopt::StructOpt;

mod index;

#[derive(Debug, StructOpt)]
pub struct Arguments {
    #[structopt(short, long, default_value = "8000")]
    port: u16,

    #[structopt(default_value = ".", parse(from_os_str))]
    root_dir: PathBuf,

    #[structopt(short = "a", long = "auth")]
    basic_auth_credentials: Option<String>
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
    let is_root = url_path.components().count() == 0;
    let fs_path = args.root_dir.join(&url_path);

    if fs_path.is_file() {
        log::info!("serving file {}", fs_path.display());
        Ok(Either::Left(NamedFile::open(fs_path).await?))
    } else {
        let index = DirectoryIndex::new(url_path, fs_path, args.root_dir.clone());
        let template = index.render_template(is_root).await?;
        Ok(Either::Right(template))
    }
}

#[rocket::launch]
fn run() -> _ {
    use rocket::Config;

    let args = Arguments::from_args();

    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let config = Config {
        port: args.port,
        address: "0.0.0.0".parse().unwrap(),
        ..Config::release_default()
    };

    rocket::build()
        .configure(config)
        .manage(args)
        .attach(Template::fairing())
        .mount("/", rocket::routes![serve_styles, serve_directory])
}
