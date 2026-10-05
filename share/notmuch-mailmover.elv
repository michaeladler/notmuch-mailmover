
use builtin;
use str;

set edit:completion:arg-completer[notmuch-mailmover] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'notmuch-mailmover'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            continue
        }
        set command = $command';'$word
    }
    var completions = [
        &'notmuch-mailmover'= {
            cand -c 'use the provided config file instead of the default'
            cand --config 'use the provided config file instead of the default'
            cand -l 'configure the log level'
            cand --log-level 'configure the log level'
            cand -d 'enable dry-run mode, i.e. no files are being moved'
            cand --dry-run 'enable dry-run mode, i.e. no files are being moved'
            cand -h 'print help information'
            cand --help 'print help information'
            cand --version 'print version information'
        }
    ]
    $completions[$command]
}
