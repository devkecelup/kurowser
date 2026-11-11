param (
    [string]$ProcessName = "kurowser"
)

Write-Host "Searching for root process: $ProcessName..." -ForegroundColor Cyan
$rootProc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue

if (-not $rootProc) {
    Write-Warning "Process '$ProcessName' not found. Make sure the browser is running via 'cargo run --release'."
    exit 1
}

$rootPid = $rootProc.Id
$allProcs = Get-CimInstance Win32_Process

$collectedPids = New-Object System.Collections.Generic.HashSet[int]
$collectedPids.Add($rootPid) | Out-Null

$queue = New-Object System.Collections.Generic.Queue[int]
$queue.Enqueue($rootPid)

# Traverse process tree to catch all child and grandchild WebView2 processes
while ($queue.Count -gt 0) {
    $currentPid = $queue.Dequeue()
    $children = $allProcs | Where-Object { $_.ParentProcessId -eq $currentPid }
    foreach ($child in $children) {
        if (-not $collectedPids.Contains($child.ProcessId)) {
            $collectedPids.Add($child.ProcessId) | Out-Null
            $queue.Enqueue($child.ProcessId)
        }
    }
}

Write-Host "`nTracked Process Tree:" -ForegroundColor Yellow
$totalWorkingSetMB = 0
$totalPrivateMB = 0

foreach ($pidItem in $collectedPids) {
    $proc = Get-Process -Id $pidItem -ErrorAction SilentlyContinue
    if ($proc) {
        $workingSetMB = [Math]::Round($proc.WorkingSet64 / 1MB, 2)
        $privateMB = [Math]::Round($proc.PrivateMemorySize64 / 1MB, 2)
        $totalWorkingSetMB += $workingSetMB
        $totalPrivateMB += $privateMB

        Write-Host ("  [{0,-6}] {1,-26} | Private: {2,8} MB | WorkingSet: {3,8} MB" -f $proc.Id, $proc.ProcessName, $privateMB, $workingSetMB)
    }
}

Write-Host "------------------------------------------------------------------"
Write-Host ("Total Private Memory : {0} MB" -f [Math]::Round($totalPrivateMB, 2)) -ForegroundColor Green
Write-Host ("Total Working Set    : {0} MB" -f [Math]::Round($totalWorkingSetMB, 2)) -ForegroundColor Green
Write-Host "------------------------------------------------------------------"

