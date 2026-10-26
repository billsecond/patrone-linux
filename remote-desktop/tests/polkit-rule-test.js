// Runs data/patrone-desktops.rules under plain gjs with polkit stood in for:
// gjs -m tests/polkit-rule-test.js data/patrone-desktops.rules

import GLib from 'gi://GLib';
import System from 'system';

const [ok, contents] = GLib.file_get_contents(ARGV[0]);
if (!ok)
    throw new Error(`can't read ${ARGV[0]}`);
const source = new TextDecoder().decode(contents);

// logind's sessions, by ID: what "loginctl show-session ID -p Type" says
const sessionTypes = {desktop: 'wayland', x11desktop: 'x11', ssh: 'tty'};
let rule = null;
const polkit = {
    Result: {YES: 'yes', NO: 'no', NOT_HANDLED: 'not handled'},
    addRule(fn) {
        rule = fn;
    },
    spawn(argv) {
        if (argv[0] !== '/usr/bin/loginctl' || argv[1] !== 'show-session')
            throw new Error(`unexpected command ${argv.join(' ')}`);
        const type = sessionTypes[argv[2]];
        if (!type)
            throw new Error('no such session');
        return `${type}\n`;
    },
};
new Function('polkit', source)(polkit);

function subject(params) {
    const groups = params.groups ?? [];
    return {
        local: false,
        active: true,
        seat: '',
        session: 'desktop',
        ...params,
        isInGroup: group => groups.includes(group),
    };
}

let failures = 0;
function expect(test, actionId, params, expected) {
    const result = rule({id: actionId}, subject(params));
    if (result === expected) {
        print(`ok   polkit: ${test}`);
    } else {
        print(`FAIL polkit: ${test} (got "${result}", expected "${expected}")`);
        failures++;
    }
}

const NM = 'org.freedesktop.NetworkManager';
expect('the desktop may control the network', `${NM}.network-control`, {}, 'yes');
expect('an X11 desktop too', `${NM}.network-control`, {session: 'x11desktop'}, 'yes');
expect('the desktop may shut down', 'org.freedesktop.login1.power-off', {}, 'yes');
expect('the desktop may mount a drive', 'org.freedesktop.udisks2.filesystem-mount', {}, 'yes');
expect('not with other sessions signed in', 'org.freedesktop.login1.power-off-multiple-sessions',
    {}, 'not handled');
expect('administrator-only actions stay so', `${NM}.reload`, {}, 'not handled');
expect('system connections: administrators', `${NM}.settings.modify.system`,
    {groups: ['sudo']}, 'yes');
expect('system connections: netdev', `${NM}.settings.modify.system`,
    {groups: ['netdev']}, 'yes');
expect('system connections: not others', `${NM}.settings.modify.system`, {}, 'not handled');
expect('not an SSH login', `${NM}.network-control`, {session: 'ssh'}, 'not handled');
expect('not a session with a seat (local: the defaults)', `${NM}.network-control`,
    {local: true, seat: 'seat0'}, 'not handled');
expect('not a seat it was given anyway', `${NM}.network-control`, {seat: 'seat0'}, 'not handled');
expect('not an inactive desktop', `${NM}.network-control`, {active: false}, 'not handled');
expect('not with no session', `${NM}.network-control`, {session: ''}, 'not handled');
expect('not when logind doesn\'t know the session', `${NM}.network-control`,
    {session: 'gone'}, 'not handled');

if (failures)
    System.exit(1);
