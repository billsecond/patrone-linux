# Sends keyboard and mouse input to a Hyper-V VM through its virtual devices.
#   -Info                 show the VM's current screen resolution
#   -Text 'abc'           type literal text
#   -Keys Tab,Tab,Enter   press named keys in order (Shift+Tab is "ShiftTab")
#   -Combo Ctrl+Alt+T     hold modifiers and press one key
#   -Click 320,240        left-click at a point in screenshot coordinates
#   -ShotWidth/-ShotHeight  size of the screenshot those coordinates came from
param(
    [string]$VMName = 'PatroneDev',
    [switch]$Info,
    [string]$Text,
    [string[]]$Keys,
    [string]$Combo,
    [int[]]$Click,
    [int]$ShotWidth = 640,
    [int]$ShotHeight = 480
)

$ns = 'root\virtualization\v2'
$vm = Get-CimInstance -Namespace $ns -ClassName Msvm_ComputerSystem -Filter "ElementName='$VMName'"

$head = Get-CimInstance -Namespace $ns -ClassName Msvm_VideoHead |
    Where-Object { $_.SystemName -eq $vm.Name } | Select-Object -First 1
$screenW = [int]$head.CurrentHorizontalResolution
$screenH = [int]$head.CurrentVerticalResolution

if ($Info) { "Screen: ${screenW}x${screenH}"; return }

$vk = @{
    Enter = 0x0D; Tab = 0x09; Esc = 0x1B; Space = 0x20; Backspace = 0x08
    Left = 0x25; Up = 0x26; Right = 0x27; Down = 0x28
    Home = 0x24; End = 0x23; PageUp = 0x21; PageDown = 0x22; Shift = 0x10
    Ctrl = 0x11; Alt = 0x12; Super = 0x5B
}

function Get-KeyCode([string]$name) {
    if ($vk.ContainsKey($name)) { return [uint32]$vk[$name] }
    if ($name -match '^[A-Za-z0-9]$') { return [uint32][char]$name.ToUpper() }
    # F1 to F12 are virtual keys 0x70 to 0x7B
    if ($name -match '^F([1-9]|1[0-2])$') { return [uint32](0x6F + [int]$Matches[1]) }
    throw "Unknown key '$name'"
}

$keyboard = Get-CimAssociatedInstance -InputObject $vm -ResultClassName Msvm_Keyboard

if ($Combo) {
    $parts = $Combo.Split('+')
    $modifiers = $parts[0..($parts.Count - 2)] | ForEach-Object { Get-KeyCode $_ }
    $main = Get-KeyCode $parts[-1]
    foreach ($m in $modifiers) { Invoke-CimMethod -InputObject $keyboard -MethodName PressKey -Arguments @{ keyCode = $m } | Out-Null }
    Invoke-CimMethod -InputObject $keyboard -MethodName TypeKey -Arguments @{ keyCode = $main } | Out-Null
    [array]::Reverse($modifiers)
    foreach ($m in $modifiers) { Invoke-CimMethod -InputObject $keyboard -MethodName ReleaseKey -Arguments @{ keyCode = $m } | Out-Null }
}

if ($Text) {
    # Hyper-V's TypeText drops and garbles characters, so type one key at a time
    # using a US keyboard layout: each character maps to a key and whether Shift is held.
    $plain = @{
        ' ' = 0x20; '-' = 0xBD; '=' = 0xBB; '[' = 0xDB; ']' = 0xDD; '\' = 0xDC
        ';' = 0xBA; "'" = 0xDE; ',' = 0xBC; '.' = 0xBE; '/' = 0xBF; '`' = 0xC0
    }
    $shifted = @{
        '!' = 0x31; '@' = 0x32; '#' = 0x33; '$' = 0x34; '%' = 0x35; '^' = 0x36
        '&' = 0x37; '*' = 0x38; '(' = 0x39; ')' = 0x30; '_' = 0xBD; '+' = 0xBB
        '{' = 0xDB; '}' = 0xDD; '|' = 0xDC; ':' = 0xBA; '"' = 0xDE; '<' = 0xBC
        '>' = 0xBE; '?' = 0xBF; '~' = 0xC0
    }
    foreach ($ch in $Text.ToCharArray()) {
        $s = [string]$ch
        $shift = $false
        if ($s -cmatch '^[a-z]$') { $code = [uint32][char]$s.ToUpper() }
        elseif ($s -cmatch '^[A-Z]$') { $code = [uint32][char]$s; $shift = $true }
        elseif ($s -match '^[0-9]$') { $code = [uint32][char]$s }
        elseif ($plain.ContainsKey($s)) { $code = [uint32]$plain[$s] }
        elseif ($shifted.ContainsKey($s)) { $code = [uint32]$shifted[$s]; $shift = $true }
        else { throw "Can't type character '$s'" }

        if ($shift) { Invoke-CimMethod -InputObject $keyboard -MethodName PressKey -Arguments @{ keyCode = [uint32]$vk.Shift } | Out-Null }
        $r = Invoke-CimMethod -InputObject $keyboard -MethodName TypeKey -Arguments @{ keyCode = $code }
        if ($shift) { Invoke-CimMethod -InputObject $keyboard -MethodName ReleaseKey -Arguments @{ keyCode = [uint32]$vk.Shift } | Out-Null }
        if ($r.ReturnValue -ne 0) { throw "TypeKey for '$s' failed with code $($r.ReturnValue)" }
        Start-Sleep -Milliseconds 15
    }
}

foreach ($name in $Keys) {
    if ($name -eq 'ShiftTab') {
        Invoke-CimMethod -InputObject $keyboard -MethodName PressKey -Arguments @{ keyCode = [uint32]$vk.Shift } | Out-Null
        Invoke-CimMethod -InputObject $keyboard -MethodName TypeKey -Arguments @{ keyCode = [uint32]$vk.Tab } | Out-Null
        Invoke-CimMethod -InputObject $keyboard -MethodName ReleaseKey -Arguments @{ keyCode = [uint32]$vk.Shift } | Out-Null
    }
    else {
        if (-not $vk.ContainsKey($name)) { throw "Unknown key '$name'" }
        $r = Invoke-CimMethod -InputObject $keyboard -MethodName TypeKey -Arguments @{ keyCode = [uint32]$vk[$name] }
        if ($r.ReturnValue -ne 0) { throw "TypeKey $name failed with code $($r.ReturnValue)" }
    }
    Start-Sleep -Milliseconds 150
}

if ($Click) {
    $mouse = Get-CimAssociatedInstance -InputObject $vm -ResultClassName Msvm_SyntheticMouse
    $x = [int][math]::Round($Click[0] * $screenW / $ShotWidth)
    $y = [int][math]::Round($Click[1] * $screenH / $ShotHeight)
    $r = Invoke-CimMethod -InputObject $mouse -MethodName SetAbsolutePosition -Arguments @{ horizontalPosition = $x; verticalPosition = $y }
    if ($r.ReturnValue -ne 0) { throw "SetAbsolutePosition failed with code $($r.ReturnValue)" }
    Start-Sleep -Milliseconds 150
    $r = Invoke-CimMethod -InputObject $mouse -MethodName ClickButton -Arguments @{ buttonIndex = [uint32]1 }
    if ($r.ReturnValue -ne 0) { throw "ClickButton failed with code $($r.ReturnValue)" }
}
