# PatroneOS

An Ubuntu 26.04.1 LTS-based OS with its own brand-new Wayland desktop,
**Patrone Desktop**. Renamed PatroneOS on 2026-10-29; its website is
https://patroneos.com. The name is **PatroneOS**, one word, wherever users
or the public see it; "Patrone" stays the short name (Patrone Terminal,
`patrone-*` packages, app IDs, paths, `ID=patrone`). Public repo:
`billsecond/patroneos` (renamed on 2026-10-29; GitHub redirects its old
address), public since 2026-10-28. Its history before then,
website included, is in the private `billsecond/patroneos-archive`. The
website has its own private repo, `billsecond/patrone-website`.

Read these before starting work:

- `docs/NEXT.md`: what to do next, and findings from the last session
- `docs/PLAN.md`: architecture, decided stack, milestones
- `branding/README.md`: logo, colors, wallpaper

## Rules

- **Never credit Claude.** No `Co-Authored-By: Claude`, no "Generated with Claude
  Code", no AI attribution in commits, PRs, tags, release notes or code comments.
- Commit as `William Daugherty (billsecond) <31995458+billsecond@users.noreply.github.com>`
  (already set in both clones' local git config).
- **This repo is public.** Nothing private goes in it: not the user's email
  address, and not the website, its `.env` or its download data, which stay
  in the website's private repo and folder.
- The stack is decided: Rust + Smithay for the compositor, GTK4 (gtk4-rs,
  gtk4-layer-shell) for the shell and apps. Don't revisit it without asking.
- **Merges publish themselves, within a day.** Installs get only what's in
  Patrone's apt repository (`billsecond/patrone-apt`). Once a day, while the
  user is signed in on the VM, the VM builds every package whose version on
  `main` is newer than the published one and publishes it
  (`packaging/main-updates.sh`), so a change that should reach installs
  needs a version bump. A build that fails becomes an issue in
  billsecond/patrone-apt. To publish one sooner, on the VM from `main`:
  `./packaging/build.sh PACKAGE`, then `./packaging/publish.sh PACKAGE`
  (`packaging/README.md`, "Publishing"). Never publish a build from a branch.
- Never type passwords or approve sign-ins for the user. Root work on the VM
  goes through the `claude` account (`ssh patronedev-claude`), which has
  passwordless sudo: the user's choice, 2026-10-27
  (`scripts/vm/add-claude-account.sh`, which also says how to undo it). Build
  as `wdaugherty` (`ssh patronedev`); use `claude` to install the packages
  you build, read system logs and debug. Don't sign the user out, end their
  session, reboot the VM, or change their settings without asking.

## Machines and paths

| What | Where |
| --- | --- |
| Windows clone | `N:\Source\PatroneLinux` |
| Dev VM (Hyper-V) | `PatroneDev`, files in `N:\Hyper-V\PatroneDev`, 12 vCPU, 32 GB, nested virtualization on |
| VM login | user `wdaugherty`, hostname `DevDistro-PatroneLinux` (`DevDistro-PatroneLinux.local` on the LAN) |
| VM clone | `~/src/patrone-linux` |
| SSH from Windows | `ssh patronedev` (key `~/.ssh/patronedev_ed25519`, alias in `~/.ssh/config`) |
| Root on the VM | `ssh patronedev-claude`: user `claude` (no password, key `~/.ssh/patronedev_claude_ed25519`), passwordless sudo, in `adm` and `systemd-journal` |
| Ubuntu ISO | `N:\Hyper-V\ISO\ubuntu-26.04.1-desktop-amd64.iso` (SHA256 verified) |
| Website clone | `N:\Source\PatroneWebsite` (private `billsecond/patrone-website`). The site's container runs from there, with its `.env`, `data/` and `admin/data/`, which aren't committed. Its build reads this repo's `legal/` from `N:\Source\PatroneLinux` |

Keep the two clones in sync through GitHub: push from one, pull in the other.
Both `gh` and `git` are signed in on the VM; the VM's `gh` is 2.46, which is
older and lacks some newer flags.

## Working on the VM

- Build and test Linux code **on the VM**. From a Windows session, run commands
  with `ssh patronedev "..."`.
- To run GUI commands in the VM's desktop session over SSH, export:
  `XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus WAYLAND_DISPLAY=wayland-0`.
  `gio open <url>` from SSH opened Text Editor instead of Firefox; launch
  `firefox <url>` directly if needed.
- To see the VM's screen or type into it from Windows, use `scripts/hyperv/`
  (`vmshot.ps1`, `vminput.ps1`). They need an **elevated** PowerShell 7.
- **Don't type into the VM's screen while the user is using it.** Twice, input
  collided with what they were doing. Take a screenshot first, and prefer SSH
  for anything that doesn't need the GUI.
- On Windows, the user's global instructions ask that shell commands run
  through MCPTerminal shared terminals when it's available.
