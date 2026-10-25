# Hyper-V helpers (Windows host)

PowerShell scripts for driving a Hyper-V VM from the Windows host without
touching its console window. They use Hyper-V's WMI API, so run them from an
elevated PowerShell 7 prompt. They're useful for scripted install tests of
Patrone ISOs in throwaway VMs.

| Script | What it does |
| --- | --- |
| `vmshot.ps1` | Saves a PNG of the VM's screen at its current resolution. |
| `vminput.ps1` | Types text, presses keys and key combos, and clicks at screen coordinates. `-Info` prints the VM's resolution. |
| `waitssh.ps1` | Finds a VM on the LAN by MAC address and waits until its SSH port opens. |

Examples:

```
.\vminput.ps1 -VMName PatroneDev -Info
.\vmshot.ps1 -VMName PatroneDev -Out screen.png
.\vminput.ps1 -VMName PatroneDev -Combo Ctrl+Alt+T
.\vminput.ps1 -VMName PatroneDev -Text "uname -a" -Keys Enter
.\vminput.ps1 -VMName PatroneDev -Click 942,728 -ShotWidth 1024 -ShotHeight 768
```

Notes:

- `vminput.ps1` types one key at a time with a US keyboard layout. Hyper-V's
  own `TypeText` method drops and garbles characters, and the stray keys can
  trigger desktop shortcuts.
- If the VM console window was opened from an elevated process, desktop
  automation tools can't send input to it, but these scripts still work.
