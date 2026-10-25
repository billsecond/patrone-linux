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

## Inside the guest: clipboard and resizing

Hyper-V's basic console has no clipboard sharing and a fixed screen size.
Both come from RDP. GNOME 50 has no X11 session, so the xrdp recipe from
Microsoft's linux-vm-tools can't work. GNOME Remote Desktop is the RDP server
instead. Two things about it matter:

- It only accepts connections that use network-level authentication (NLA).
- It resizes the remote screen only when it gives the client a virtual
  monitor: in **Remote Login** sessions, or in Desktop Sharing's "extend"
  mode. Desktop Sharing normally mirrors the VM's screen at its current size.

### Remote Desktop Connection

GNOME Remote Desktop is installed but off until it's configured. Nothing
listens on port 3389 before that, so Remote Desktop Connection can't reach
the VM. In the VM, run one of these as yourself (not with sudo). Each asks
you to choose an RDP username and password:

```
./scripts/hyperv/guest/setup-remote-desktop.sh sharing   # Desktop Sharing
./scripts/hyperv/guest/setup-remote-desktop.sh login     # Remote Login (uses sudo)
```

- **Desktop Sharing** shows the session you're logged in to, at the VM's
  screen size. Clipboard works; resizing doesn't.
- **Remote Login** starts a separate session whose screen follows the client
  window. After the RDP password you sign in at the login screen with your
  Linux account. Untested: whether it works while you're also logged in on
  the console.

Then on Windows, run `mstsc /v:DevDistro-PatroneLinux.local` (or the VM's IP
address). With both modes on, Remote Login has port 3389 and Desktop Sharing
moves to 3390 (`mstsc /v:DevDistro-PatroneLinux.local:3390`).

Settings → System → Remote Desktop does the same thing through the GUI.

### Hyper-V enhanced session (experimental)

In enhanced-session mode, VMConnect connects to vsock port 3389 in the guest.
`guest/setup-enhanced-session.sh` installs `patrone-hyperv-rdp-bridge.service`,
which forwards that port to GNOME Remote Desktop on TCP 3389:

```
sudo ./scripts/hyperv/guest/setup-enhanced-session.sh   # in the VM
Set-VM -VMName PatroneDev -EnhancedSessionTransportType HvSocket   # on Windows, VM shut down
```

Then start the VM and pick the enhanced session in VMConnect. This hasn't
worked end to end yet. If VMConnect can't do NLA, the next step is to put
`freerdp-proxy` between the bridge and GNOME Remote Desktop: the proxy accepts
TLS without NLA and signs in to GNOME Remote Desktop with NLA itself.
