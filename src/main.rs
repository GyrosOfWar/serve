use std::collections::HashMap;
use std::fs;
use rocket_dyn_templates::Template;

pub struct DirEntry<'a> {
    pub name: &'a str,
    pub is_directory: bool,
    pub size: u64,
    pub is_symlink: bool,
}

#[rocket::get("/")]
fn hello() -> Template {
    let mut ctx = HashMap::new();
    ctx.insert("name", "Martin");
    Template::render("index", ctx)
}

#[rocket::launch]
fn run() -> _ {
    rocket::build()
        .attach(Template::fairing())
        .mount("/", rocket::routes![hello])
}
