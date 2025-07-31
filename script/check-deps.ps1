function Test-Command($cmd) {
    $result = Get-Command $cmd -ErrorAction SilentlyContinue
    if (-not $result) {
        Write-Host "$cmd is not installed"
        exit 1
    }
}
Test-Command cargo
Test-Command go
Test-Command python
Test-Command jq
Write-Host "All pre-requisite dependencies are installed"