-- anything goes here, as long as it returns a table (which can be parsed into the corresponding `config::Config` struct)

--- @enum config.match_mode
--- Match modes for rules.
local match_modes = {
    ALL = "all",
    FIRST = "first",
    UNIQUE = "unique",
}

--- Configuration for notmuch-mailmover.
--
--- @class config
--- @field maildir      string                  Path to the maildir
--- @field notmuch_config string                Path to the notmuch configuration
--- @field rename       boolean                 Rename the files when moving
--- @field max_age_days number                  Maximum age (days) of the messages to be procssed
--- @field rule_match_mode config.match_mode    Match mode for rules
--- @field rules        rule[]                  List of rules
---
--- @class rule
--- @field folder string Folder to move the messages to
--- @field query  string Notmuch query to match the messages
local config = {
    maildir = os.getenv("HOME") .. "/mail",
    notmuch_config = "~/.config/notmuch/notmuchrc",
    rename = false,
    max_age_days = 60,
    rule_match_mode = match_modes.FIRST,
    rules = {
        {
            folder = "Trash",
            query = "tag:trash",
        },
        {
            folder = "Spam",
            query = "tag:spam",
        },
        {
            folder = "Sent",
            query = "tag:sent",
        },
        {
            folder = "Archive",
            query = "tag:archive",
        },
        {
            folder = "INBOX",
            query = "tag:inbox",
        },
    },
}

return config
