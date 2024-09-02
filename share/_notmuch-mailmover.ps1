
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'notmuch-mailmover' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'notmuch-mailmover'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'notmuch-mailmover' {
            [CompletionResult]::new('-c', '-c', [CompletionResultType]::ParameterName, 'Use the provided config file instead of the default')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Use the provided config file instead of the default')
            [CompletionResult]::new('-l', '-l', [CompletionResultType]::ParameterName, 'Configure the log level')
            [CompletionResult]::new('--log-level', '--log-level', [CompletionResultType]::ParameterName, 'Configure the log level')
            [CompletionResult]::new('-d', '-d', [CompletionResultType]::ParameterName, 'Enable dry-run mode, i.e. no files are being moved')
            [CompletionResult]::new('--dry-run', '--dry-run', [CompletionResultType]::ParameterName, 'Enable dry-run mode, i.e. no files are being moved')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
