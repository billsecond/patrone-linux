# Patrone's page (see gnome-settings/README.md in Patrone's repository)
panels_list += cappletname
desktop = 'gnome-@0@-panel.desktop'.format(cappletname)

i18n.merge_file(
  type: 'desktop',
  input: desktop + '.in',
  output: desktop,
  po_dir: po_dir,
  install: true,
  install_dir: control_center_desktopdir
)

# The sidebar's icon: the Patrone P in one color
install_data(
  'org.patrone.Settings-symbolic.svg',
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'symbolic' / 'apps'
)

panels_libs += static_library(
  cappletname,
  sources: files('cc-patrone-panel.c'),
  include_directories: [top_inc, common_inc],
  dependencies: common_deps,
  c_args: cflags
)
