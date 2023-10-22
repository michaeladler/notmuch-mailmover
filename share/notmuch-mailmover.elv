
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
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'notmuch-mailmover'= {
            cand -c 'Use the provided config file instead of the default'
            cand --config 'Use the provided config file instead of the default'
            cand -l 'Configure the log level'
            cand --log-level 'Configure the log level'
            cand -d 'Enable dry-run mode, i.e. no files are being moved'
            cand --dry-run 'Enable dry-run mode, i.e. no files are being moved'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
    ]
    $completions[$command]
}
