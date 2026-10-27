# Makes a Hyper-V VM for trying a Patrone ISO: Generation 2, Secure Boot on
# with the certificate authority that signs Ubuntu's boot loader (as on a
# real PC), and the ISO in its DVD drive, first in the boot order. Start it
# with Start-VM and watch it with vmconnect.
#
#   .\new-test-vm.ps1 -Iso N:\Hyper-V\ISO\patrone-linux-26.04.1-amd64.iso
param(
    [Parameter(Mandatory)][string]$Iso,
    [string]$Name = 'PatroneTest',
    [string]$Root = 'N:\Hyper-V',
    [int]$Cpus = 4,
    [long]$MemoryGB = 8,
    [long]$DiskGB = 64,
    [string]$Switch = 'Virtual Switch'
)
$ErrorActionPreference = 'Stop'

if (Get-VM -Name $Name -ErrorAction SilentlyContinue) { throw "There's a VM named $Name already." }
$disk = Join-Path $Root "$Name\Virtual Hard Disks\$Name.vhdx"
New-VM -Name $Name -Generation 2 -Path $Root -MemoryStartupBytes ($MemoryGB * 1GB) `
    -NewVHDPath $disk -NewVHDSizeBytes ($DiskGB * 1GB) -SwitchName $Switch | Out-Null
Set-VMProcessor -VMName $Name -Count $Cpus
Set-VMMemory -VMName $Name -DynamicMemoryEnabled $false
Set-VM -Name $Name -AutomaticCheckpointsEnabled $false -CheckpointType Standard
Set-VMFirmware -VMName $Name -EnableSecureBoot On -SecureBootTemplate MicrosoftUEFICertificateAuthority
$dvd = Add-VMDvdDrive -VMName $Name -Path $Iso -Passthru
Set-VMFirmware -VMName $Name -FirstBootDevice $dvd
Enable-VMIntegrationService -VMName $Name -Name 'Guest Service Interface'
Get-VM -Name $Name | Select-Object Name, Generation, ProcessorCount, State
