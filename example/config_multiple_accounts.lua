#!/usr/bin/env lua

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
    maildir = "~/mail",
    notmuch_config = "~/.config/notmuch/notmuchrc",
    rename = false,
    max_age_days = 60,
    rule_match_mode = match_modes.FIRST,
    rules = {},
}

local accounts = {
    { prefix = "foo", address = "foo@baz.org" },
    { prefix = "bar", address = "bar@baz.org" },
}
local rules = {
    {
        folder = "Trash",
        query = "tag:deleted",
    },
    {
        folder = "Sent",
        query = "tag:sent and not tag:deleted",
    },
    {
        folder = "Archive",
        query = "tag:archive and not tag:sent and not tag:deleted",
    },
    {
        folder = "Drafts",
        query = "tag:draft and not tag:deleted and not tag:archive",
    },
    {
        folder = "Junk",
        query = "tag:junk and not tag:deleted and not tag:archive",
    },
}
for _, account in ipairs(accounts) do
    local address = account.address
    local query_prefix = string.format("from:%s or to:%s", address, address)
    for _, rule in ipairs(rules) do
        table.insert(config.rules, {
            folder = account.prefix .. "/" .. rule.folder,
            query = string.format("(%s) and (%s)", query_prefix, rule.query),
        })
    end
end

-- for debugging, download https://github.com/jagt/pprint.lua/raw/refs/heads/master/pprint.lua and put it right here
local ok, pprint = pcall(require, "pprint")
if ok then
    pprint(config)
end

return config
