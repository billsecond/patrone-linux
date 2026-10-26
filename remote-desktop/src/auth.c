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
 * With --console it serves /run/patrone-remote-desktop/console.sock instead,
 * which any account may open: the account that signed in on the machine's own
 * screen, asking for its own desktop. The account comes from the connection
 * (SO_PEERCRED), not from anything sent, and needs no password: GDM checked
 * it.
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#define _GNU_SOURCE
#include <errno.h>
#include <pwd.h>
#include <security/pam_appl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#ifndef PREPARE_SESSION
#define PREPARE_SESSION "/usr/libexec/patrone-remote-desktop/prepare-session"
#endif
#define SERVICE_NAME "patrone-remote-desktop"
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

static int check(const Request* request)
{
	struct pam_conv conv = { conversation, (void*)request };
	pam_handle_t* pam = NULL;

	int status = pam_start(SERVICE_NAME, request->user, &conv, &pam);
	if (status == PAM_SUCCESS)
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

int main(int argc, char** argv)
{
	static Request request;

	if (argc > 1 && strcmp(argv[1], "--console") == 0)
		return console();

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

	int ok = check(&request);
	explicit_bzero(request.password, sizeof(request.password));
	if (!ok)
	{
		puts("denied");
		return 1;
	}

	fprintf(stderr, "signed in %s from %s\n", request.user, request.client);
	prepare_session(request.user);
	return 1;
}
