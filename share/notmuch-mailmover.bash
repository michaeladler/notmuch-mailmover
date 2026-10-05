_notmuch-mailmover_completion() {
    local cur prev
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    case "${prev}" in
        -c|--config)
            COMPREPLY=($(compgen -f "${cur}"))
            return 0
            ;;
        -l|--log-level)
            COMPREPLY=($(compgen -W "trace debug info warn error" "${cur}"))
            return 0
            ;;
    esac
    COMPREPLY=($(compgen -W "-c -l -d -h --config --log-level --dry-run --help --version" "${cur}"))
}
complete -F _notmuch-mailmover_completion -o bashdefault -o default notmuch-mailmover
