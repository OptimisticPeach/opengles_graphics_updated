extern crate gl_generator;

use gl_generator::{Registry, Api, Profile, Fallbacks, GlobalGenerator};
use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    let dest = env::var("OUT_DIR").unwrap();
    let mut file = File::create(&Path::new(&dest).join("gl.rs")).unwrap();

    Registry::new(Api::Gles2, (3, 1), Profile::Compatibility, Fallbacks::All, [])
        .write_bindings(GlobalGenerator, &mut file)
        .unwrap();
}
