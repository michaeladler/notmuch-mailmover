_notmuch-mailmover() {
    local i cur prev opts cmd
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    cmd=""
    opts=""

    for i in ${COMP_WORDS[@]}
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="notmuch__mailmover"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        notmuch__mailmover)
            opts="-c -l -d -h -V --config --log-level --dry-run --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-level)
                    COMPREPLY=($(compgen -W "trace debug info warn error" -- "${cur}"))
                    return 0
                    ;;
                -l)
                    COMPREPLY=($(compgen -W "trace debug info warn error" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _notmuch-mailmover -o nosort -o bashdefault -o default notmuch-mailmover
else
    complete -F _notmuch-mailmover -o bashdefault -o default notmuch-mailmover
fi
