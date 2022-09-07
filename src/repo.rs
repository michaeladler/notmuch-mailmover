use anyhow::Result;
use std::path::PathBuf;

pub trait MailRepo {
    fn search_message(&self, query: &str) -> Result<Vec<PathBuf>>;
}

impl MailRepo for notmuch::Database {
    fn search_message(&self, query: &str) -> Result<Vec<PathBuf>> {
        let nm_query = self.create_query(query)?;
        let messages = nm_query.search_messages()?;
        let mut result: Vec<PathBuf> = Vec::new();
        for msg in messages {
            result.push(msg.filename().to_path_buf());
        }
        Ok(result)
    }
}
