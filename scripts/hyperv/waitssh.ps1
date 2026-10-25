# Finds a VM on the LAN by MAC address and waits until its SSH port accepts connections.
param(
    [string]$Mac = '00-15-5D-0B-01-04',
    [string]$Prefix = '10.0.0.',
    [int]$TimeoutSec = 540
)

$deadline = (Get-Date).AddSeconds($TimeoutSec)
$ip = $null
while ((Get-Date) -lt $deadline) {
    if (-not $ip) {
        $n = Get-NetNeighbor -AddressFamily IPv4 -LinkLayerAddress $Mac -ErrorAction SilentlyContinue |
            Where-Object State -ne 'Unreachable' | Select-Object -First 1
        if ($n) {
            $ip = $n.IPAddress
            "Found VM at $ip"
        }
        else {
            # Ping the subnet so Windows learns every device's MAC address.
            1..254 | ForEach-Object -Parallel {
                Test-Connection -TargetName "$using:Prefix$_" -Count 1 -TimeoutSeconds 1 -Quiet | Out-Null
            } -ThrottleLimit 64
            continue
        }
    }
    if (Test-Connection -TargetName $ip -TcpPort 22 -TimeoutSeconds 2 -Quiet) {
        "SSH is up at $ip"
        exit 0
    }
    Start-Sleep -Seconds 5
}
"Timed out waiting for SSH. Last known IP: $ip"
exit 1
