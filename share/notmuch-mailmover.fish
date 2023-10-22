complete -c notmuch-mailmover -s c -l config -d 'Use the provided config file instead of the default' -r -F
complete -c notmuch-mailmover -s l -l log-level -d 'Configure the log level' -r -f -a "{trace	'',info	'',debug	'',warn	'',error	''}"
complete -c notmuch-mailmover -s d -l dry-run -d 'Enable dry-run mode, i.e. no files are being moved'
complete -c notmuch-mailmover -s h -l help -d 'Print help'
complete -c notmuch-mailmover -s V -l version -d 'Print version'
