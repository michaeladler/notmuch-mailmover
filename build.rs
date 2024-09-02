use clap::CommandFactory;
use clap_complete::{generate_to, Shell};
use clap_mangen::Man;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{create_dir_all, File};
use std::path::Path;

include!("src/lib/cli.rs");

fn main() {
    println!("cargo::rerun-if-changed=src/lib/cli.rs");

    let out = &Path::new("share");
    create_dir_all(out).unwrap();
    let cmd = &mut Cli::command();

    let f = File::create(out.join("notmuch-mailmover.1.gz")).unwrap();
    let mut encoder = GzEncoder::new(f, Compression::default());

    Man::new(cmd.clone()).render(&mut encoder).unwrap();

    for shell in Shell::value_variants() {
        generate_to(*shell, cmd, "notmuch-mailmover", out).unwrap();
    }
}
