use anyhow::Result;
use std::path::PathBuf;

/// Source of messages for the rule engine.
///
/// The rule engine only ever asks questions in notmuch query syntax; it never
/// parses or rewrites them, so an implementation must evaluate the full syntax
/// (as [`notmuch::Database`] does) rather than, say, match tags literally.
pub trait MailRepo {
    /// Returns the file of every message matching `query`.
    ///
    /// A message stored in several files yields one path per file; the engine
    /// maps paths to folders, so duplicate files are moved individually.
    fn search_message(&self, query: &str) -> Result<Vec<PathBuf>>;
}

impl MailRepo for notmuch::Database {
    fn search_message(&self, query: &str) -> Result<Vec<PathBuf>> {
        let nm_query = self.create_query(query)?;
        let messages = nm_query.search_messages()?;
        let mut result: Vec<PathBuf> = Vec::new();
        for msg in messages {
            for fname in msg.filenames() {
                result.push(fname);
            }
        }
        Ok(result)
    }
}
