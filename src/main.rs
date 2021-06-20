use crate::index::DirectoryIndex;
use rocket::fs::NamedFile;
use rocket::{Either, State};
use rocket_dyn_templates::Template;
use std::{env, io};
use std::path::PathBuf;
use structopt::StructOpt;

mod index;

#[derive(Debug, StructOpt)]
pub struct Arguments {
    #[structopt(short, long, default_value = "8000")]
    port: u16,

    // TODO: basic auth
    #[structopt(default_value = ".", parse(from_os_str))]
    root_dir: PathBuf,
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
