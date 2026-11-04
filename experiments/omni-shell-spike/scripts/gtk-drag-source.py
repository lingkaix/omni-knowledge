#!/usr/bin/env python3
"""Show a GTK 4 window that can drag one local file.

The drag starts only from a pointer gesture on this window. The script does
not call NSH and does not synthesize an in-process drop.
"""

import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, Gtk

path = sys.argv[1]
with open(path, "w", encoding="utf-8") as handle:
    handle.write("spike drop\n")


class App(Gtk.Application):
    def do_activate(self):
        window = Gtk.ApplicationWindow(application=self, title="Spike Drag Source")
        window.set_default_size(180, 180)
        label = Gtk.Label(label="DRAG")
        source = Gtk.DragSource()

        def prepare(*_args):
            file = Gio.File.new_for_path(path)
            files = Gdk.FileList.new_from_list([file])
            return Gdk.ContentProvider.new_for_value(files)

        source.connect("prepare", prepare)
        label.add_controller(source)
        window.set_child(label)
        window.present()
        print("spike-drag-ready", flush=True)


application = App(application_id="dev.omniknowledge.spike-drag-source")
raise SystemExit(application.run(None))
