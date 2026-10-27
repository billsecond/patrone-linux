# Puts a new build of an ISO in a VM's DVD drive and boots from it. The VM is
# turned off first (whatever it's running is lost).
#
#   .\swap-iso.ps1 -New N:\Hyper-V\ISO\patrone-linux-26.04.1-amd64.new.iso
#
# -New replaces -Iso, the file the drive holds. Inserting a file Hyper-V
# hasn't let go of yet, just after it's replaced, fails ("the object was not
# found") and leaves the drive empty, though Get-VMDvdDrive still shows the
# path for a moment; the VM then falls to network boot. So this tries again
# until the drive really holds an ISO, then puts the DVD first in the boot
# order, which the swap undoes.
param(
    [Parameter(Mandatory)][string]$New,
    [string]$Iso = 'N:\Hyper-V\ISO\patrone-linux-26.04.1-amd64.iso',
    [string]$Name = 'PatroneTest',
    [switch]$NoStart
)
$ErrorActionPreference = 'Stop'

Stop-VM -Name $Name -TurnOff -Force
Set-VMDvdDrive -VMName $Name -ControllerNumber 0 -ControllerLocation 1 -Path $null
Move-Item -Force $New $Iso
foreach ($try in 1..10) {
    try {
        Set-VMDvdDrive -VMName $Name -ControllerNumber 0 -ControllerLocation 1 -Path $Iso
    } catch {
        Start-Sleep -Seconds 3
        continue
    }
    Start-Sleep -Seconds 2
    if ((Get-VMDvdDrive -VMName $Name).DvdMediaType -eq 'ISO') { break }
}
if ((Get-VMDvdDrive -VMName $Name).DvdMediaType -ne 'ISO') { throw "The DVD drive won't take $Iso" }
Set-VMFirmware -VMName $Name -FirstBootDevice (Get-VMDvdDrive -VMName $Name)
if (-not $NoStart) { Start-VM -Name $Name }
Get-VMDvdDrive -VMName $Name | Select-Object Path, DvdMediaType
