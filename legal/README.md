# PatroneOS's licence agreement

`eula.md` is PatroneOS's End User Licence Agreement: the one text the
website shows before a download (`src/licence.rs` in the website's private
repo, `billsecond/patrone-website`, whose build reads this folder) and the installer
shows on its licence page. The user asked for it on 2026-10-28 ("a strong
EULA in the website for the software that the user must click before
downloading, and ... that same one up on the install page"), with Texas law.

## Its format

The website renders it with a small part of Markdown, and escapes
everything:

- The first line is `Version: X.Y`. The website records that version with
  each download, so change it, and the date in the line under the title,
  with every change to the text. A new version applies only to new
  downloads and new disc images (section 12).
- Keep every version people have accepted readable. The website serves
  only the current one, at `/licence/X.Y`; before publishing a new version,
  copy the old text to `eula-X.Y.md` here and have the website serve it at
  its own address too.
- `# ` for the title, `## ` for section headings, `- ` for list items, and
  plain paragraphs, each separated by a blank line. No bold, links, HTML or
  comments: they'd show as typed. Disclaimers are in capitals instead
  (section 5 and 7's headings and text), which is what makes them
  "conspicuous" under the UCC (Texas Business and Commerce Code 1.201 and
  2.316).

Notes for reviewers stay here, not in `eula.md`, because the website shows
every line of it.

## Versions

- **1.0**, 28 October 2026, under the system's old name: `eula-1.0.md`.
- **1.1**, 29 October 2026: the same terms, with the system's new name,
  PatroneOS, and the website's, patroneos.com.

## What it may and may not say

Almost everything in PatroneOS is free software under its own licence,
and those licences forbid adding restrictions: GPL-3 section 10 ("You may
not impose any further restrictions on the exercise of the rights granted
or affirmed under this License"), GPL-2 section 6, LGPL, MPL-2.0 section
3.3, and Creative Commons 4.0 section 2(a)(5)(B). So the agreement sets
terms only where Patrone can: its name and logos, no warranty, limits on
liability, the promises people make when they share it, fair use of the
download service, and governing law. Section 1 says the free licences come
first and win in any conflict, and that declining the agreement doesn't
affect them. The source code is never behind the click-through.

Never add the usual commercial EULA terms: a ban on reverse engineering
(LGPL-2.1 section 6 and LGPL-3 section 4 require allowing it for
debugging), limits on copying, devices or commercial use, a ban on
publishing benchmarks, fees, confidentiality, patent non-assertion, or a
ban on uses such as high-risk ones (section 6 shifts that risk instead of
banning it).

## How it was checked

Drafted for the compliance review (`docs/COMPLIANCE.md`, item 24), then
reviewed by an independent agent against GPL-2, GPL-3, MPL-2.0 and
Apache-2.0 (the texts in `/usr/share/common-licenses`), the NVIDIA and
Broadcom licences in the disc's driver packages, clickwrap case law and
Texas, EU and UK consumer law. Its seven must-fixes are in version 1.0.
Its renderer check (a copy of `licence.rs`'s `to_html`) gives one title, 16
section headings and the lists intact. Not legal advice.

## For a lawyer, before it's relied on

1. **The free licences.** Whether sections 3 (modified versions drop the
   name and logos), 8 (the indemnity), 9 (fair use of the download) and 13
   (ending) are "further restrictions" under GPL-2 section 6 or GPL-3
   section 10, especially for GPL-2-only code such as the kernel. Whether
   the installer may require acceptance before installing (GPL-3 section 2
   "affirms your unlimited permission to run"); the live session should
   stay ungated, and an unattended install skips the page.
2. **The indemnity** (section 8): enforceable, consistent with GPL-3 section
   7(f), and worth keeping against consumers at all.
3. **NVIDIA and Broadcom** (section 2): whether a licence inside each
   `.deb` meets NVIDIA's "this Agreement is provided to each SOFTWARE
   recipient" and Broadcom's "a copy of this Agreement accompanies such
   distribution" (a plain copy on the disc and on the download page
   removes the doubt); the owner's own indemnities to both, NVIDIA's U.S.
   Government clause and arbitration terms; or dropping the disc's
   `pool/restricted` (compliance review item 17), which would mean a new
   version of section 2.
4. **Texas**: whether the "fair notice" rule (express negligence and
   conspicuousness, Dresser Industries v. Page Petroleum, 853 S.W.2d 505,
   Tex. 1993) is met by section 7 as worded; which county to name; whether
   to add a jury waiver. The cap of US$10 is within practice for free
   software.
5. **Trademarks**: the rights in unregistered marks, a clearance search for
   PATRONE (Patrón, the tequila, among others), the AI-made logo
   (compliance review item 9), and the Linux Foundation's sublicence for a
   name containing "Linux" (item 4).
6. **The EU and UK**: whether the owner is a "trader", which decides
   whether unfair-terms law, the UK Consumer Rights Act and the Digital
   Content Directive apply; whether the Product Liability Directive
   2024/2853 (from 9 December 2026) and the Cyber Resilience Act exclude a
   free disc that also offers non-free drivers, and whether the optional
   download details make the supply "commercial".
7. **Elsewhere**: New Jersey's TCCWNA on "where the law doesn't allow"
   wording, and Quebec's rule that standard contracts come in French first.
8. **Export and sanctions**: whether the encryption is outside the EAR as
   publicly available (15 CFR 734.3(b)(3), 742.15(b)), and OFAC for
   downloads from embargoed places; section 9 reminds users, but doesn't
   discharge the owner's own duties.
9. **The source code**: section 1 says it's "published alongside the disc
   image"; that has to be literally true before downloads open (GPL-2
   section 3's "same place", compliance review item 1).
