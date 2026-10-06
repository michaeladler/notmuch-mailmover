//! End-to-end tests: generate a Maildir, index and tag it with notmuch, run the
//! notmuch-mailmover binary against that database and check where the mail
//! ended up.
//!
//! Needs the `notmuch` binary on `PATH`.
//!
//! Mail created by `write_mail` below, relative to the maildir, with the tag
//! `MAIL` gives it.
//!
//! | file                                | tag      |
//! |-------------------------------------|----------|
//! | `INBOX/cur/1700000000.0.nmm-it:2,S` | `trash`  |
//! | `INBOX/cur/1700000001.1.nmm-it:2,FS`| `sent`   |
//! | `INBOX/cur/1700000002.2.nmm-it:2,` | `archive`|
//! | `INBOX/new/1700000003.3.nmm-it`     | none     |
//! | `INBOX.lists/rust/new/1700000004.4.nmm-it` | `archive` |

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_notmuch-mailmover");

/// (mailbox, folder under the mailbox, maildir flags, subject, notmuch tags).
/// Deterministic, so assertions can name the exact file names.
const MAIL: &[(&str, &str, &str, &str, &[&str])] = &[
    ("INBOX", "cur", ":2,S", "delete me", &["trash"]),
    ("INBOX", "cur", ":2,FS", "re: your question", &["sent"]),
    ("INBOX", "cur", ":2,", "project notes", &["archive"]),
    ("INBOX", "new", "", "unrelated news", &[]),
    ("INBOX.lists/rust", "new", "", "rust weekly", &["archive"]),
];

/// Writes the mail and the `notmuch tag --batch` commands that tag it. The tag
/// commands select by Message-ID rather than by file name: notmuch reads
/// anything containing a colon as a query, and Maildir flag suffixes (`:2,S`)
/// are full of colons.
fn write_mail(maildir: &Path, taglist: &Path) {
    let mut commands = String::new();
    for (index, (mailbox, subdir, flags, subject, tags)) in MAIL.iter().enumerate() {
        for dir in ["cur", "new", "tmp"] {
            fs::create_dir_all(maildir.join(mailbox).join(dir)).unwrap();
        }

        let message_id = format!("nmm-it-{index}@example.com");
        fs::write(
            maildir
                .join(mailbox)
                .join(subdir)
                .join(format!("17{index:08}.{index}.nmm-it{flags}")),
            format!(
                "From: Sender <sender@example.com>\n\
                 To: Recipient <recipient@example.com>\n\
                 Subject: {subject}\n\
                 Date: Thu, 01 Jan 2024 10:{index:02}:00 +0000\n\
                 Message-ID: <{message_id}>\n\
                 MIME-Version: 1.0\n\
                 Content-Type: text/plain; charset=us-ascii\n\
                 \n\
                 {subject}, this is the body.\n"
            ),
        )
        .unwrap();

        for tag in *tags {
            commands.push_str(&format!("+{tag} -- id:{message_id}\n"));
        }
    }
    fs::write(taglist, commands).unwrap();
}

/// Rules as in `example/config.yaml`: pairwise disjoint, so `unique` is happy.
const RULES: &str = "  - folder: Trash
    query: tag:trash
  - folder: Sent
    query: tag:sent and not tag:trash
  - folder: Archive
    query: tag:archive and not tag:sent and not tag:trash
";

/// Where each mail is expected after a run: the message keeps its own parent
/// directory (`cur` or `new`), which is what the mover appends below the folder.
const EXPECTED: &[(&str, &str)] = &[
    (
        "INBOX/cur/1700000000.0.nmm-it:2,S",
        "Trash/cur/1700000000.0.nmm-it:2,S",
    ),
    (
        "INBOX/cur/1700000001.1.nmm-it:2,FS",
        "Sent/cur/1700000001.1.nmm-it:2,FS",
    ),
    (
        "INBOX/cur/1700000002.2.nmm-it:2,",
        "Archive/cur/1700000002.2.nmm-it:2,",
    ),
    (
        "INBOX.lists/rust/new/1700000004.4.nmm-it",
        "Archive/new/1700000004.4.nmm-it",
    ),
];

/// The mail no rule matches.
const UNTOUCHED: &str = "INBOX/new/1700000003.3.nmm-it";

/// A throwaway Maildir with a notmuch database on it. Everything lives in one
/// temp directory, named after the test so leftovers of a crashed run can be
/// wiped without touching another test's mail.
struct Fixture {
    maildir: PathBuf,
    notmuchrc: PathBuf,
    taglist: PathBuf,
    mailmover_config: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("nmm-it-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let maildir = root.join("mail");
        let notmuchrc = root.join("notmuchrc");
        let taglist = root.join("taglist");
        let mailmover_config = root.join("mailmover.yaml");

        fs::write(
            &notmuchrc,
            format!(
                "[database]\npath={}\nmail_root={}\n\n[new]\ntags=unread;inbox;\n",
                maildir.display(),
                maildir.display()
            ),
        )
        .unwrap();

        // 1) mail
        write_mail(&maildir, &taglist);

        let fixture = Self {
            maildir,
            notmuchrc,
            taglist,
            mailmover_config,
        };

        // 2) import the mail and apply the tags, both in batch mode
        fixture.notmuch(&["new", "--quiet"]);
        fixture.notmuch(&[
            "tag",
            "--batch",
            &format!("--input={}", fixture.taglist.display()),
        ]);

        fixture.write_config(false, RULES);
        fixture
    }

    fn write_config(&self, rename: bool, rules: &str) -> &Self {
        fs::write(
            &self.mailmover_config,
            format!(
                "maildir: {}\nnotmuch_config: {}\nrename: {}\nrule_match_mode: unique\nrules:\n{}",
                self.maildir.display(),
                self.notmuchrc.display(),
                rename,
                rules
            ),
        )
        .unwrap();
        self
    }

    fn notmuch(&self, args: &[&str]) -> Output {
        let output = Command::new("notmuch")
            .args(args)
            .env("NOTMUCH_CONFIG", &self.notmuchrc)
            .current_dir(&self.maildir)
            .output()
            .expect("failed to run notmuch");
        assert_success(&output);
        output
    }

    /// Runs the binary under test, which is expected to succeed.
    fn mailmover(&self, args: &[&str]) -> Output {
        let output = self.mailmover_raw(args);
        assert_success(&output);
        output
    }

    /// Runs the binary under test, which is expected to fail.
    fn mailmover_fails(&self, args: &[&str]) -> Output {
        let output = self.mailmover_raw(args);
        assert!(
            !output.status.success(),
            "expected a failure\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn mailmover_raw(&self, args: &[&str]) -> Output {
        Command::new(BIN)
            .args(["--config", &self.mailmover_config.to_string_lossy()])
            .args(args)
            .output()
            .expect("failed to run notmuch-mailmover")
    }

    /// All mail files below the maildir, sorted and relative to it.
    fn listing(&self) -> Vec<String> {
        let mut files = Vec::new();
        let mut dirs = vec![self.maildir.clone()];
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|n| n == ".notmuch") {
                        continue;
                    }
                    dirs.push(path);
                } else {
                    files.push(
                        path.strip_prefix(&self.maildir)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        files.sort();
        files
    }

    fn relative(&self, path: &str) -> PathBuf {
        self.maildir.join(path)
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed ({})\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn moves_tagged_mail_into_its_folder() {
    let fixture = Fixture::new("moves-tagged-mail");

    fixture.mailmover(&[]);

    for (from, to) in EXPECTED {
        assert!(
            !fixture.relative(from).exists(),
            "{from} should have been moved away"
        );
        assert!(fixture.relative(to).exists(), "{from} should be at {to}");
    }
    assert!(
        fixture.relative(UNTOUCHED).exists(),
        "untagged mail must stay put"
    );
    assert_eq!(
        vec![
            "Archive/cur/1700000002.2.nmm-it:2,",
            "Archive/new/1700000004.4.nmm-it",
            "INBOX/new/1700000003.3.nmm-it",
            "Sent/cur/1700000001.1.nmm-it:2,FS",
            "Trash/cur/1700000000.0.nmm-it:2,S",
        ],
        fixture.listing(),
        "no mail may be lost or copied"
    );
}

#[test]
fn notmuch_follows_the_moved_mail() {
    let fixture = Fixture::new("notmuch-follows");

    fixture.mailmover(&[]);
    // what a `pre-new` hook would do after the moves
    fixture.notmuch(&["new", "--quiet"]);

    let files = stdout(&fixture.notmuch(&["search", "--output=files", "tag:trash"]));
    assert_eq!(
        format!(
            "{}\n",
            fixture
                .relative("Trash/cur/1700000000.0.nmm-it:2,S")
                .display()
        ),
        files,
        "notmuch must know the new location"
    );
}

#[test]
fn second_run_moves_nothing() {
    let fixture = Fixture::new("second-run");

    fixture.mailmover(&[]);
    fixture.notmuch(&["new", "--quiet"]);
    let after_first_run = fixture.listing();

    let output = fixture.mailmover(&[]);

    assert_eq!(after_first_run, fixture.listing(), "nothing may move twice");
    assert!(
        stderr(&output).contains("nothing to do"),
        "expected nothing to do, got: {}",
        stderr(&output)
    );
}

#[test]
fn dry_run_leaves_mail_in_place() {
    let fixture = Fixture::new("dry-run");
    let before = fixture.listing();

    let output = fixture.mailmover(&["--dry-run"]);

    assert_eq!(before, fixture.listing(), "dry-run must not move anything");
    let log = stderr(&output);
    for (from, to) in EXPECTED {
        assert!(
            log.contains(&format!(
                "would move {} to {}",
                fixture.relative(from).display(),
                fixture.relative(to).display()
            )),
            "dry-run should log the intended move of {from}, got: {log}"
        );
    }
}

#[test]
fn rename_keeps_maildir_flags() {
    let fixture = Fixture::new("rename");
    fixture.write_config(true, RULES);

    fixture.mailmover(&[]);

    let mut names: Vec<String> = fs::read_dir(fixture.relative("Trash/cur"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(1, names.len(), "one mail in Trash: {names:?}");
    assert_ne!("1700000000.0.nmm-it:2,S", names[0], "must get a new name");
    assert!(
        names[0].ends_with(":2,S"),
        "Maildir flags must survive the rename: {}",
        names[0]
    );
    assert!(
        !fixture
            .relative("INBOX/cur/1700000000.0.nmm-it:2,S")
            .exists(),
        "the old name must be gone"
    );
}

#[test]
fn overlapping_rules_are_rejected_before_anything_moves() {
    let fixture = Fixture::new("overlapping-rules");
    fixture.write_config(
        false,
        "  - folder: Trash
    query: tag:trash
  - folder: Deleted
    query: tag:trash
",
    );
    let before = fixture.listing();

    let output = fixture.mailmover_fails(&[]);

    assert!(
        stderr(&output).contains("Rules overlap (1 messages)"),
        "expected the overlap to be reported, got: {}",
        stderr(&output)
    );
    assert_eq!(
        before,
        fixture.listing(),
        "nothing may be moved when the rules overlap"
    );
}
