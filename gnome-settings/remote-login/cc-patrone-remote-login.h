/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/*
 * Patrone Remote Desktop in GNOME Settings' Remote Login page: the
 * "Enable Remote Desktop" switch and how to connect.
 *
 * Copyright (C) 2026 William Daugherty
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, see <http://www.gnu.org/licenses/>.
 */

#pragma once

#include <adwaita.h>

G_BEGIN_DECLS

#define CC_TYPE_PATRONE_REMOTE_LOGIN (cc_patrone_remote_login_get_type ())
G_DECLARE_FINAL_TYPE (CcPatroneRemoteLogin, cc_patrone_remote_login, CC, PATRONE_REMOTE_LOGIN, AdwBin)

gboolean   cc_patrone_remote_login_available (void);
GtkWidget *cc_patrone_remote_login_new       (void);

G_END_DECLS
