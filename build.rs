use clap::CommandFactory;
use clap_complete::{Shell, generate_to};
use clap_mangen::Man;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::{File, create_dir_all};
use std::path::Path;

include!("src/cli.rs");

fn main() {
    println!("cargo::rerun-if-changed=src/cli.rs");

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
