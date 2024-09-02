use std::{
    fs::{create_dir_all, File},
    path::Path,
};

use clap::CommandFactory;
use clap_complete::{generate_to, Shell};
use clap_mangen::Man;

include!("src/lib/cli.rs");

fn main() {
    println!("cargo::rerun-if-changed=src/lib/cli.rs");

    let out = &Path::new("share");
    create_dir_all(out).unwrap();
    let cmd = &mut Cli::command();

    Man::new(cmd.clone())
        .render(&mut File::create(out.join("notmuch-mailmover.1")).unwrap())
        .unwrap();

    for shell in Shell::value_variants() {
        generate_to(*shell, cmd, "notmuch-mailmover", out).unwrap();
    }
}
