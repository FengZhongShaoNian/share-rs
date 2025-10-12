# Share-rs Nautilus Extension
#
# Place me in ~/.local/share/nautilus-python/extensions/,
# ensure you have python-nautilus package, restart Nautilus, and enjoy :)
#
# This script is released to the public domain.

from gi.repository import Nautilus, GObject
from subprocess import call
import os

# path to share-rs 
SHARE_RS = 'share-rs'

# what name do you want to see in the context menu?
MENUNAME = 'Open in share-rs'


class ShareRsExtension(GObject.GObject, Nautilus.MenuProvider):

    def launch_share_rs(self, menu, files):
        safepaths = ''
        args = ''

        for file in files:
            filepath = file.get_location().get_path()
            safepaths += '"' + filepath + '" '

        call(SHARE_RS + ' ' + args + safepaths + '&', shell=True)

    def get_file_items(self, *args):
        files = args[-1]
        item = Nautilus.MenuItem(
            name='ShareRsOpen',
            label=MENUNAME,
            tip='Opens the selected files with Share-rs'
        )
        item.connect('activate', self.launch_share_rs, files)

        return [item]
