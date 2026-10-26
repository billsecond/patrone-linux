// Patrone's login screen (patrone-remote-desktop).
//
// Signing in on the screen joins the account's one desktop, which runs with
// no seat (GDM's headless session) so that the screen and Remote Desktop can
// both show it. GNOME's login screen would call that desktop a conflicting
// session, offer to end it, and end it. This leaves sessions without a seat
// out of that check. Should the check fail, it falls back to GNOME's own.
//
// Copyright 2026 William Daugherty
// SPDX-License-Identifier: GPL-3.0-or-later

import {Extension, InjectionManager} from 'resource:///org/gnome/shell/extensions/extension.js';
import {LoginDialog} from 'resource:///org/gnome/shell/gdm/loginDialog.js';
import * as LoginManager from 'resource:///org/gnome/shell/misc/loginManager.js';

// GNOME's _findConflictingSession, without sessions that have no seat
async function findConflictingSession(startingSessionId) {
    const loginManager = LoginManager.getLoginManager();
    const sessions = await loginManager.listSessions();
    const starting = sessions.find(([id]) => id === startingSessionId);
    if (!starting)
        return null;
    const owner = starting[2];

    for (const [id, , user, seat, path] of sessions) {
        if (id === startingSessionId || user !== owner)
            continue;
        // The account's own desktop, which this sign-in is about to join
        if (!seat)
            continue;

        const session = loginManager.getSession(path);
        if (session.Type !== 'wayland' && session.Type !== 'x11')
            continue;
        if (session.State !== 'active' && session.State !== 'online')
            continue;
        return session;
    }
    return null;
}

export default class PatroneLoginExtension extends Extension {
    enable() {
        this._injections = new InjectionManager();
        this._injections.overrideMethod(LoginDialog.prototype, '_findConflictingSession',
            original => async function (startingSessionId) {
                try {
                    return await findConflictingSession(startingSessionId);
                } catch (error) {
                    logError(error, 'patrone-login: using GNOME\'s own conflict check');
                    return original.call(this, startingSessionId);
                }
            });
    }

    disable() {
        this._injections.clear();
        this._injections = null;
    }
}
