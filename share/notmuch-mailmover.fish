complete -c notmuch-mailmover -s c -l config -d 'use the provided config file instead of the default' -r -F
complete -c notmuch-mailmover -s l -l log-level -d 'configure the log level' -r -f -a "trace debug info warn error"
complete -c notmuch-mailmover -s d -l dry-run -d 'enable dry-run mode, i.e. no files are being moved'
complete -c notmuch-mailmover -s h -l help -d 'print help information'
complete -c notmuch-mailmover -l version -d 'print version information'
