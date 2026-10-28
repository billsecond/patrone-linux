/* patrone-remote-desktop's sign-in helper.
 *
 * systemd runs it as root for each connection to
 * /run/patrone-remote-desktop/auth.sock, which only the proxy's user can open,
 * with the connection as stdin and stdout. It reads a username, a password and
 * the client's address, each ended by a NUL byte, and checks them with PAM
 * (/etc/pam.d/patrone-remote-desktop: the account's own password, and
 * membership of the remote-desktop group). On success it runs prepare-session,
 * which gets the account's desktop ready and answers "ok PORT SECRET";
 * otherwise it answers "denied".
 *
 * With --vmconnect it serves /run/patrone-remote-desktop/vmconnect-auth.sock,
 * which only the login screen of Hyper-V's enhanced session (the greeter, as
 * patrone-vmconnect) can open, the same way but with PAM's
 * patrone-remote-desktop-vmconnect: any account that may sign in at the
 * computer, in the remote-desktop group or not, and whether or not Remote
 * Desktop is on. Only the Hyper-V host can reach that login screen, as only
 * someone at the computer can reach its own.
 *
 * With --console it serves /run/patrone-remote-desktop/console.sock instead,
 * which any account may open: the account that signed in on the machine's own
 * screen, asking for its own desktop. The account comes from the connection
 * (SO_PEERCRED), not from anything sent, and needs no password: GDM checked
 * it.
 *
 * With --remember, pam_exec runs it after every successful password sign-in
 * or password change (see the pam-auth-update profile), with the password on
 * stdin and the account in PAM_USER. For members of the remote-desktop group
 * it keeps the password's NT hash in the SAM file the proxy checks NLA
 * against, as Windows keeps one for its local accounts; it drops the entry of
 * anyone else. So the hash always matches the account's current password.
 *
 * With --screen-sign-in, pam_exec runs it after a successful sign-in on the
 * login screen, before GDM opens the screen's session, and it gets the
 * account's desktop ready then: GDM won't start a background desktop for an
 * account that already has a graphical session open, and the screen's
 * session would be one by the time its viewer asks.
 *
 * With --confirm USER, root's GNOME Settings helper (libexec/settings, which
 * pkexec runs for the account using Settings) checks the account's password,
 * on stdin, as a Remote Desktop sign-in would, and records its NT hash:
 * Settings' "Enable Remote Desktop" asks for it once, so a new member can
 * connect without first signing in again with its password. It answers "ok"
 * or "denied".
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <grp.h>
#include <pwd.h>
#include <security/pam_appl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#include <winpr/ntlm.h>
#include <winpr/ssl.h>
#include <winpr/string.h>

#ifndef PREPARE_SESSION
#define PREPARE_SESSION "/usr/libexec/patrone-remote-desktop/prepare-session"
#endif
#ifndef SAM_FILE
#define SAM_FILE "/var/lib/patrone-remote-desktop/sam"
#endif
#define SAM_DIRECTORY "/var/lib/patrone-remote-desktop"
#define SERVICE_NAME "patrone-remote-desktop"
#define VMCONNECT_SERVICE_NAME "patrone-remote-desktop-vmconnect"
#define REMOTE_GROUP "remote-desktop"
#define PROXY_GROUP "patrone-rdp"
#define FIELD_MAX 512
#define READ_TIMEOUT_SECONDS 30

typedef struct
{
	char user[FIELD_MAX];
	char password[FIELD_MAX];
	char client[FIELD_MAX];
} Request;

/* Reads the three NUL-ended fields, and nothing more. */
static int read_request(Request* request)
{
	char* fields[] = { request->user, request->password, request->client };
	size_t field = 0, length = 0;
	char c;

	while (field < 3)
	{
		ssize_t got = read(STDIN_FILENO, &c, 1);
		if (got < 0 && errno == EINTR)
			continue;
		if (got <= 0)
			return -1;
		if (c == '\0')
		{
			fields[field++][length] = '\0';
			length = 0;
		}
		else if (length + 1 < FIELD_MAX)
			fields[field][length++] = c;
		else
			return -1;
	}
	return 0;
}

/* Accepts the usual Linux account names only, since the name ends up in
 * systemd unit names and file paths. */
static int valid_user(const char* user)
{
	size_t length = strlen(user);
	if (length == 0 || length > 32)
		return 0;
	if (!(user[0] == '_' || (user[0] >= 'a' && user[0] <= 'z')))
		return 0;
	for (size_t i = 1; i < length; i++)
	{
		char c = user[i];
		if (!((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_' || c == '-' ||
		      c == '.'))
			return 0;
	}
	return 1;
}

static int conversation(int count, const struct pam_message** messages,
                        struct pam_response** responses, void* data)
{
	const Request* request = data;
	struct pam_response* replies = calloc((size_t)count, sizeof(*replies));
	if (!replies)
		return PAM_BUF_ERR;

	for (int i = 0; i < count; i++)
	{
		switch (messages[i]->msg_style)
		{
			case PAM_PROMPT_ECHO_OFF:
				replies[i].resp = strdup(request->password);
				break;
			case PAM_PROMPT_ECHO_ON:
				replies[i].resp = strdup(request->user);
				break;
			case PAM_ERROR_MSG:
			case PAM_TEXT_INFO:
				continue;
			default:
				replies[i].resp = NULL;
				break;
		}
		if (!replies[i].resp)
		{
			for (int j = 0; j < i; j++)
			{
				if (replies[j].resp)
				{
					explicit_bzero(replies[j].resp, strlen(replies[j].resp));
					free(replies[j].resp);
				}
			}
			free(replies);
			return PAM_CONV_ERR;
		}
	}
	*responses = replies;
	return PAM_SUCCESS;
}

/* Hyper-V's console has no remote host for PAM: it's at the computer. */
static int check(const Request* request, const char* service)
{
	struct pam_conv conv = { conversation, (void*)request };
	pam_handle_t* pam = NULL;

	int status = pam_start(service, request->user, &conv, &pam);
	if (status == PAM_SUCCESS && strcmp(service, VMCONNECT_SERVICE_NAME) != 0)
		status = pam_set_item(pam, PAM_RHOST, request->client);
	if (status == PAM_SUCCESS)
		status = pam_authenticate(pam, PAM_DISALLOW_NULL_AUTHTOK);
	if (status == PAM_SUCCESS)
		status = pam_acct_mgmt(pam, PAM_DISALLOW_NULL_AUTHTOK);
	if (status != PAM_SUCCESS)
		fprintf(stderr, "refused %s from %s: %s\n", request->user, request->client,
		        pam ? pam_strerror(pam, status) : "PAM failed to start");
	if (pam)
		pam_end(pam, status);
	return status == PAM_SUCCESS;
}

static void prepare_session(const char* user)
{
	fflush(stdout);
	execl(PREPARE_SESSION, PREPARE_SESSION, user, (char*)NULL);
	fprintf(stderr, "can't run %s: %s\n", PREPARE_SESSION, strerror(errno));
	puts("error");
}

/* The machine's own screen: the account asking is the connecting process's. */
static int console(void)
{
	struct ucred peer;
	socklen_t size = sizeof(peer);

	if (getsockopt(STDIN_FILENO, SOL_SOCKET, SO_PEERCRED, &peer, &size) != 0)
	{
		fprintf(stderr, "can't tell who's asking: %s\n", strerror(errno));
		puts("denied");
		return 1;
	}
	struct passwd* account = getpwuid(peer.uid);
	if (!account || peer.uid < 1000 || !valid_user(account->pw_name))
	{
		fprintf(stderr, "refused a desktop for uid %u\n", (unsigned)peer.uid);
		puts("denied");
		return 1;
	}
	fprintf(stderr, "desktop for %s on this machine's screen\n", account->pw_name);
	prepare_session(account->pw_name);
	return 1;
}

static int may_use_remote_desktop(const struct passwd* account)
{
#ifdef REMEMBER_TEST
	WINPR_UNUSED(account);
	return 1;
#else
	struct group* group = getgrnam(REMOTE_GROUP);
	gid_t groups[256];
	int count = 256;

	if (!group || getgrouplist(account->pw_name, account->pw_gid, groups, &count) < 0)
		return 0;
	for (int i = 0; i < count; i++)
	{
		if (groups[i] == group->gr_gid)
			return 1;
	}
	return 0;
#endif
}

/* The NT hash NLA checks: MD4 of the password in UTF-16, as 32 hex digits */
static int nt_hash_hex(const char* password, char hex[33])
{
	size_t length = 0;
	BYTE hash[16];

	WCHAR* wide = ConvertUtf8ToWCharAlloc(password, &length);
	if (!wide)
		return 0;
	BOOL ok = NTOWFv1W(wide, (UINT32)(length * sizeof(WCHAR)), hash);
	explicit_bzero(wide, length * sizeof(WCHAR));
	free(wide);
	if (!ok)
		return 0;
	for (int i = 0; i < 16; i++)
		snprintf(hex + 2 * i, 3, "%02X", hash[i]);
	explicit_bzero(hash, sizeof(hash));
	return 1;
}

/* Puts the account's entry ("user:::NTHASH:") in the SAM file in place of any
 * older one, or with no hash, takes it out. Root owns the file; the proxy's
 * group may read it. */
static void update_sam(const char* user, const char* nt_hash)
{
	char entry[FIELD_MAX + 48] = "";
	char line[FIELD_MAX + 64];
	char temp[] = SAM_FILE ".XXXXXX";
	size_t user_length = strlen(user);
	int found = 0, changed = 0;

	if (nt_hash)
		snprintf(entry, sizeof(entry), "%s:::%s:\n", user, nt_hash);

	mkdir(SAM_DIRECTORY, 0755);
	int lock = open(SAM_FILE ".lock", O_RDWR | O_CREAT | O_CLOEXEC, 0600);
	if (lock < 0 || flock(lock, LOCK_EX) != 0)
		goto out;

	FILE* old = fopen(SAM_FILE, "re");
	int fd = mkostemp(temp, O_CLOEXEC);
	FILE* new = fd >= 0 ? fdopen(fd, "w") : NULL;
	if (!new)
	{
		if (fd >= 0)
		{
			close(fd);
			unlink(temp);
		}
		if (old)
			fclose(old);
		goto out;
	}

	while (old && fgets(line, sizeof(line), old))
	{
		if (strncmp(line, user, user_length) == 0 && line[user_length] == ':')
		{
			found = 1;
			if (!nt_hash || strcmp(line, entry) != 0)
				changed = 1;
			continue;
		}
		fputs(line, new);
	}
	if (old)
		fclose(old);
	if (nt_hash)
	{
		fputs(entry, new);
		if (!found)
			changed = 1;
	}

	struct group* readers = getgrnam(PROXY_GROUP);
	if (readers && fchown(fd, 0, readers->gr_gid) == 0)
		fchmod(fd, 0640);
	else
		fchmod(fd, 0600);
	if (changed && fflush(new) == 0 && fsync(fd) == 0)
	{
		fclose(new);
		if (rename(temp, SAM_FILE) != 0)
			unlink(temp);
	}
	else
	{
		fclose(new);
		unlink(temp);
	}

out:
	explicit_bzero(entry, sizeof(entry));
	explicit_bzero(line, sizeof(line));
	if (lock >= 0)
		close(lock);
}

/* pam_exec's call after a successful password sign-in or change. It never
 * fails the sign-in. */
static int remember(void)
{
	const char* user = getenv("PAM_USER");
	char password[FIELD_MAX];
	char hex[33];
	size_t length = 0;

#ifndef REMEMBER_TEST
	/* Not root: the account unlocking its own screen, say. */
	if (geteuid() != 0)
		return 0;
#endif
	if (!user || !valid_user(user))
		return 0;
	struct passwd* account = getpwnam(user);
	if (!account || account->pw_uid < 1000)
		return 0;
	if (!may_use_remote_desktop(account))
	{
		update_sam(user, NULL);
		return 0;
	}

	while (length + 1 < sizeof(password))
	{
		ssize_t got = read(STDIN_FILENO, password + length, sizeof(password) - 1 - length);
		if (got < 0 && errno == EINTR)
			continue;
		if (got <= 0)
			break;
		length += (size_t)got;
	}
	password[length] = '\0';
	if (password[0] && winpr_InitializeSSL(WINPR_SSL_INIT_DEFAULT) && nt_hash_hex(password, hex))
		update_sam(user, hex);
	explicit_bzero(password, sizeof(password));
	explicit_bzero(hex, sizeof(hex));
	return 0;
}

/* pam_exec's call after a successful sign-in, from any PAM service: acts only
 * for the login screen's own (not gdm-autologin, which is how GDM starts the
 * background desktop itself). It never fails the sign-in. */
static int screen_sign_in(void)
{
	const char* service = getenv("PAM_SERVICE");
	const char* user = getenv("PAM_USER");

	if (geteuid() != 0 || !service || !user)
		return 0;
	if (strcmp(service, "gdm-password") != 0 && strcmp(service, "gdm-fingerprint") != 0 &&
	    strcmp(service, "gdm-smartcard") != 0)
		return 0;
	if (!valid_user(user))
		return 0;
	struct passwd* account = getpwnam(user);
	if (!account || account->pw_uid < 1000)
		return 0;

	/* prepare-session's answer holds the secret: keep it off any output. */
	pid_t child = fork();
	if (child == 0)
	{
		int null = open("/dev/null", O_RDWR | O_CLOEXEC);
		if (null >= 0)
		{
			dup2(null, STDIN_FILENO);
			dup2(null, STDOUT_FILENO);
		}
		execl(PREPARE_SESSION, PREPARE_SESSION, user, (char*)NULL);
		_exit(127);
	}
	if (child > 0)
		waitpid(child, NULL, 0);
	return 0;
}

/* Settings' check of the account's own password, from stdin: PAM through
 * Remote Desktop's own service (the group, the password, the account not
 * locked), then the hash. The PAM hook records it too; this doesn't rely on
 * that. */
static int confirm(const char* user)
{
	static Request request;
	size_t length = 0;
	char hex[33];
	int ok = 0;

	if (geteuid() != 0 || !user || !valid_user(user))
	{
		puts("denied");
		return 1;
	}
	snprintf(request.user, sizeof(request.user), "%s", user);
	snprintf(request.client, sizeof(request.client), "localhost");

	alarm(READ_TIMEOUT_SECONDS);
	while (length + 1 < sizeof(request.password))
	{
		ssize_t got = read(STDIN_FILENO, request.password + length,
		                   sizeof(request.password) - 1 - length);
		if (got < 0 && errno == EINTR)
			continue;
		if (got <= 0)
			break;
		length += (size_t)got;
	}
	alarm(0);
	/* As long as the buffer holds: longer than any sign-in takes */
	if (length + 1 >= sizeof(request.password))
		length = 0;
	request.password[length] = '\0';

	if (request.password[0] && check(&request, SERVICE_NAME))
	{
		ok = 1;
		if (winpr_InitializeSSL(WINPR_SSL_INIT_DEFAULT) && nt_hash_hex(request.password, hex))
			update_sam(user, hex);
		fprintf(stderr, "confirmed the password of %s for Remote Desktop, in Settings\n", user);
	}
	explicit_bzero(&request, sizeof(request));
	explicit_bzero(hex, sizeof(hex));
	puts(ok ? "ok" : "denied");
	return ok ? 0 : 1;
}

int main(int argc, char** argv)
{
	static Request request;
	const char* service = SERVICE_NAME;

	if (argc > 1 && strcmp(argv[1], "--vmconnect") == 0)
		service = VMCONNECT_SERVICE_NAME;
	else if (argc > 1 && strcmp(argv[1], "--console") == 0)
		return console();
	if (argc > 1 && strcmp(argv[1], "--remember") == 0)
		return remember();
	if (argc > 1 && strcmp(argv[1], "--screen-sign-in") == 0)
		return screen_sign_in();
	if (argc > 1 && strcmp(argv[1], "--confirm") == 0)
		return confirm(argc > 2 ? argv[2] : NULL);

	alarm(READ_TIMEOUT_SECONDS);
	if (read_request(&request) != 0)
	{
		fprintf(stderr, "a malformed request\n");
		puts("denied");
		return 1;
	}
	alarm(0);

	if (!valid_user(request.user))
	{
		fprintf(stderr, "refused an unusual account name from %s\n", request.client);
		explicit_bzero(&request, sizeof(request));
		puts("denied");
		return 1;
	}

	int ok = check(&request, service);
	explicit_bzero(request.password, sizeof(request.password));
	if (!ok)
	{
		puts("denied");
		return 1;
	}

	if (strcmp(service, VMCONNECT_SERVICE_NAME) == 0)
		fprintf(stderr, "signed in %s at Hyper-V's console\n", request.user);
	else
		fprintf(stderr, "signed in %s from %s\n", request.user, request.client);
	prepare_session(request.user);
	return 1;
}
