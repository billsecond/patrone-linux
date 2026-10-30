# Shell setup

Patrone's defaults for bash and zsh, packaged as `patrone-shell-config`: the
prompt (`bashrc`, `bash_aliases`, `zshrc`, `zprofile`; `terminal/README.md`
describes it) and the pointers for Windows commands below.

## Windows commands

Someone coming from Windows who types `ipconfig`, `cls` or `Get-ChildItem`
gets this rather than a bare "command not found":

```
Invalid command: ipconfig
That's a Windows command. The Linux command you might be looking for is:

    ip a

ipconfig /all       ip a, and resolvectl status for DNS servers
ipconfig /flushdns  resolvectl flush-caches
ipconfig /renew     nmcli connection up NAME
```

It exits with an error (127, as any command not found does) and runs nothing
itself. Names match whatever their case, with or without `.exe`.

**How it hooks in.** Accounts have their own copies of `.zshrc` and `.bashrc`,
so changing those wouldn't reach existing accounts. But `/etc/bash.bashrc`
(bash) and `/etc/zsh_command_not_found`, which every Patrone `.zshrc` reads,
both run `/usr/lib/command-not-found`, so that's the one place for all of
them. The package moves Ubuntu's program aside with `dpkg-divert`, to
`/usr/lib/command-not-found.ubuntu`, and puts `command-not-found` from this
folder in its place. It looks the name up in
`/usr/share/patrone/windows-commands.tsv`, and anything that isn't there goes
on to Ubuntu's program, with its suggestions of a package. Ubuntu's updates
to the program land at the new name, and removing the package puts it back.
The two shell files stay untouched: they're configuration files, and
diverting those makes dpkg ask questions.

**Options.** Both shells pass that program the command's name and nothing
else, so it can't tell `ipconfig` from `ipconfig /all`. That's why the message
is the same for both, and lists the usual options each time. Passing the
arguments would take a change in each shell's setup, which wouldn't reach
existing accounts.

**What can't be caught.** The shell has to reach the hook first:

- `ipconfig/all`, with no space: a `/` makes it a path, so the shell says
  "no such file or directory".
- `sudo ipconfig`: that's sudo's own "command not found".
- Scripts, and any shell that isn't interactive.

**What goes in the table.** Common CMD commands and PowerShell cmdlets, and
their aliases, whose name differs from the Linux command:

- A real Linux command of the same name isn't covered, so `ping`, `ls`, `curl`
  or `shutdown` run as usual. `dir`, `type` and `set` are in the table only
  for `DIR`, `TYPE` and `SET`: lower case, they're the Linux commands.
- A name Ubuntu's archive has a well-known program for stays out too (`at`,
  `calc`, `iex`, `certutil`, and PowerShell's `gc`, `sl` and `gm`), so Ubuntu's
  suggestion of that package still shows. `net` and `sc` are in, because
  Windows users type them far more often than anyone wants Samba's `net` or
  the spreadsheet `sc`, and if those are installed, they run.
- Suggest only what's on PatroneOS as installed: `ip a`, since `ifconfig`
  isn't there, `tracepath` rather than `traceroute`, and `wget` rather than
  `curl`.

## Tests

`tests/windows-commands.sh` checks the table's format and the matching, with
a stand-in for Ubuntu's program; the package build runs it. On PatroneOS with
the package installed, `tests/windows-commands.sh --installed` also checks
that everything it suggests is there and that nothing in the table is a real
command on that system.

Windows and PowerShell are trademarks of the Microsoft group of companies.
