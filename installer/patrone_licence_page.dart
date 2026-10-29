// PatroneOS's licence agreement, shown before installing, with a box to
// accept it. Patrone's own page: installer/patch_source.py copies it into the
// installer's source and adds it after "Try or install". The agreement is
// legal/eula.md, which installer/build.sh puts beside the installer
// (data/patrone-eula.md), in the website's small Markdown: "Version: " on the
// first line, "# " and "## " headings, "- " lists and paragraphs.
//
// It's shown with Accept, not as a lock: the free software's licences let
// anyone run it whether or not they accept (the agreement's section 1), the
// live session isn't behind it, and automated installs, which show no pages,
// skip it (docs/COMPLIANCE.md, item 24).

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:ubuntu_logger/ubuntu_logger.dart';
import 'package:ubuntu_provision/ubuntu_provision.dart';
import 'package:ubuntu_wizard/ubuntu_wizard.dart';
import 'package:yaru/yaru.dart';

final _log = Logger('patrone-licence');

File get _agreementFile =>
    File('${File(Platform.resolvedExecutable).parent.path}/data/patrone-eula.md');

/// A record of the acceptance: the install engine copies /var/log/installer
/// to the new system with its own logs. It goes nowhere else.
const _recordPath = '/var/log/installer/patrone-licence-accepted';

final _agreementProvider =
    FutureProvider<String>((ref) => _agreementFile.readAsString());
final _acceptedProvider = StateProvider<bool>((_) => false);

class PatroneLicencePage extends ConsumerWidget with ProvisioningPage {
  const PatroneLicencePage({super.key});

  // Without the agreement beside it (a build without one), the page is
  // skipped rather than stopping the install
  @override
  Future<bool> load(BuildContext context, WidgetRef ref) async {
    final found = _agreementFile.existsSync();
    if (!found) {
      _log.error('No licence agreement at ${_agreementFile.path}');
    }
    return found;
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final agreement = ref.watch(_agreementProvider);
    final accepted = ref.watch(_acceptedProvider);
    final theme = Theme.of(context);
    return WizardPage(
      title: const YaruWindowTitleBar(title: Text('Licence agreement')),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Please read the licence agreement',
              style: theme.textTheme.titleLarge),
          const SizedBox(height: kWizardSpacing / 2),
          const Text(
            'Each program in PatroneOS keeps its own free software '
            'licence. Accepting this agreement doesn\'t change what those '
            'licences let you do.',
          ),
          const SizedBox(height: kWizardSpacing),
          Expanded(
            child: DecoratedBox(
              decoration: BoxDecoration(
                border: Border.all(color: theme.dividerColor),
                borderRadius: BorderRadius.circular(6),
              ),
              child: agreement.when(
                data: (text) => _Agreement(text: text),
                loading: () => const Center(child: CircularProgressIndicator()),
                error: (error, stackTrace) {
                  _log.error('Couldn\'t read ${_agreementFile.path}', error,
                      stackTrace);
                  return const Center(
                    child: Text('The licence agreement couldn\'t be read.'),
                  );
                },
              ),
            ),
          ),
          const SizedBox(height: kWizardSpacing),
          YaruCheckButton(
            title: Text(
              'I have read and accept the licence agreement',
              style: theme.textTheme.bodyLarge
                  ?.copyWith(fontWeight: FontWeight.bold),
            ),
            value: accepted,
            onChanged: (value) =>
                ref.read(_acceptedProvider.notifier).state = value ?? false,
          ),
        ],
      ),
      bottomBar: WizardBar(
        leading: const BackWizardButton(),
        trailing: [
          NextWizardButton(
            enabled: accepted && agreement.hasValue,
            onNext: () => _record(agreement.value ?? ''),
          ),
        ],
      ),
    );
  }

  void _record(String agreement) {
    final version = RegExp(r'^Version: *(.+)$', multiLine: true)
            .firstMatch(agreement)
            ?.group(1)
            ?.trim() ??
        'unknown';
    final line = 'PatroneOS End User Licence Agreement, version $version, '
        'accepted ${DateTime.now().toUtc().toIso8601String()}';
    _log.info(line);
    try {
      File(_recordPath).writeAsStringSync('$line\n');
    } on FileSystemException catch (error) {
      _log.error('Couldn\'t write $_recordPath', error);
    }
  }
}

/// The agreement's Markdown, drawn as headings, lists and paragraphs, in a
/// box that scrolls.
class _Agreement extends StatefulWidget {
  const _Agreement({required this.text});

  final String text;

  @override
  State<_Agreement> createState() => _AgreementState();
}

class _AgreementState extends State<_Agreement> {
  final _scroll = ScrollController();

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    final blocks = <Widget>[];
    final paragraph = <String>[];

    void endParagraph() {
      if (paragraph.isEmpty) return;
      blocks.add(Padding(
        padding: const EdgeInsets.only(bottom: 10),
        child: Text(paragraph.join(' ')),
      ));
      paragraph.clear();
    }

    for (final line in widget.text.split('\n')) {
      if (line.startsWith('Version:')) {
        // The first line; the agreement says its version again below
      } else if (line.trim().isEmpty) {
        endParagraph();
      } else if (line.startsWith('## ')) {
        endParagraph();
        blocks.add(Padding(
          padding: const EdgeInsets.only(top: 8, bottom: 6),
          child: Text(line.substring(3),
              style: text.titleMedium?.copyWith(fontWeight: FontWeight.bold)),
        ));
      } else if (line.startsWith('# ')) {
        endParagraph();
        blocks.add(Padding(
          padding: const EdgeInsets.only(bottom: 10),
          child: Text(line.substring(2), style: text.headlineSmall),
        ));
      } else if (line.startsWith('- ')) {
        endParagraph();
        blocks.add(Padding(
          padding: const EdgeInsets.only(left: 12, bottom: 6),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text('•  '),
              Expanded(child: Text(line.substring(2))),
            ],
          ),
        ));
      } else {
        paragraph.add(line.trim());
      }
    }
    endParagraph();

    return Scrollbar(
      controller: _scroll,
      thumbVisibility: true,
      child: SingleChildScrollView(
        controller: _scroll,
        padding: const EdgeInsets.all(16),
        child: SelectionArea(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: blocks,
          ),
        ),
      ),
    );
  }
}
