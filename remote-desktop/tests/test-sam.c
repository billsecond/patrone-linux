/* Reads a SAM file the way FreeRDP's NTLM does (WinPR's SamLookupUserA, with
 * the client's domain and then without), to check that what the sign-in
 * helper's --remember writes is what NLA will find.
 *
 *   test-sam FILE USER DOMAIN EXPECTED-NT-HASH
 *
 * Copyright 2026 William Daugherty
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include <stdio.h>
#include <string.h>

#include <winpr/sam.h>

int main(int argc, char** argv)
{
	if (argc != 5)
		return 2;
	const char* user = argv[2];
	const char* domain = argv[3];
	char hex[33] = "";

	WINPR_SAM* sam = SamOpen(argv[1], TRUE);
	if (!sam)
		return 1;
	WINPR_SAM_ENTRY* entry =
	    SamLookupUserA(sam, user, (UINT32)strlen(user), domain, (UINT32)strlen(domain));
	if (!entry)
		entry = SamLookupUserA(sam, user, (UINT32)strlen(user), NULL, 0);
	if (entry)
	{
		for (int i = 0; i < 16; i++)
			snprintf(hex + 2 * i, 3, "%02X", entry->NtHash[i]);
		SamFreeEntry(sam, entry);
	}
	SamClose(sam);
	return strcmp(hex, argv[4]) == 0 ? 0 : 1;
}
