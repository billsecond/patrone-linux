# Saves a PNG of a Hyper-V VM's console using the Hyper-V WMI thumbnail API.
param(
    [string]$VMName = 'PatroneDev',
    [int]$Width = 1280,
    [int]$Height = 800,
    [Parameter(Mandatory)][string]$Out
)

$ns = 'root\virtualization\v2'
$vm = Get-CimInstance -Namespace $ns -ClassName Msvm_ComputerSystem -Filter "ElementName='$VMName'"
$vssd = Get-CimAssociatedInstance -InputObject $vm -ResultClassName Msvm_VirtualSystemSettingData |
    Where-Object { $_.VirtualSystemType -eq 'Microsoft:Hyper-V:System:Realized' }
$svc = Get-CimInstance -Namespace $ns -ClassName Msvm_VirtualSystemManagementService

$result = Invoke-CimMethod -InputObject $svc -MethodName GetVirtualSystemThumbnailImage -Arguments @{
    TargetSystem = $vssd
    WidthPixels  = [uint16]$Width
    HeightPixels = [uint16]$Height
}
if ($result.ReturnValue -ne 0) { throw "Thumbnail failed with code $($result.ReturnValue)" }

# The API returns raw RGB565 pixels, two bytes per pixel, row after row.
[byte[]]$data = $result.ImageData
$rowBytes = $Width * 2
if ($data.Length -lt $rowBytes * $Height) { throw "Expected $($rowBytes * $Height) bytes, got $($data.Length)" }

Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::new($Width, $Height, [System.Drawing.Imaging.PixelFormat]::Format16bppRgb565)
$rect = [System.Drawing.Rectangle]::new(0, 0, $Width, $Height)
$bits = $bmp.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::WriteOnly, $bmp.PixelFormat)
for ($y = 0; $y -lt $Height; $y++) {
    $dest = [IntPtr]::Add($bits.Scan0, $y * $bits.Stride)
    [System.Runtime.InteropServices.Marshal]::Copy($data, [int]($y * $rowBytes), $dest, [int]$rowBytes)
}
$bmp.UnlockBits($bits)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
$Out
