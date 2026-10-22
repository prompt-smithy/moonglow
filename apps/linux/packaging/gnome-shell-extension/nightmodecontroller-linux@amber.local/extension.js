/* Night Mode Controller indicator.
 *
 * Talks to the daemon through its CLI: `--status --json` reads state,
 * `--brighter/--dimmer/--warmer/--cooler/--auto` mutate it, `--show-settings`
 * reveals the settings window. No D-Bus API of its own to keep in sync.
 */

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import St from 'gi://St';
import GLib from 'gi://GLib';

const APP_ID = 'nightmodecontroller-linux';
const BIN = 'nightmodecontroller-linux';

function runSync(argv) {
    try {
        const [ok, stdout] = GLib.spawn_sync(
            null, argv, null, GLib.SpawnFlags.SEARCH_PATH, null,
        );
        if (!ok)
            return null;
        return new TextDecoder().decode(stdout);
    } catch {
        return null;
    }
}

function runAsync(argv) {
    try {
        GLib.spawn_async(null, argv, null, GLib.SpawnFlags.SEARCH_PATH, null);
    } catch (e) {
        logError(e, `${APP_ID}: failed to run ${argv.join(' ')}`);
    }
}

function readStatus() {
    const out = runSync([BIN, '--status', '--json']);
    if (!out)
        return null;
    try {
        const status = JSON.parse(out);
        return status.ok ? status : null;
    } catch {
        return null;
    }
}

class Indicator extends PanelMenu.Button {
    constructor() {
        super(0.0, APP_ID, false);
        this.add_child(new St.Icon({
            icon_name: 'weather-clear-night-symbolic',
            style_class: 'system-status-icon',
        }));

        this._statusLabel = new PopupMenu.PopupMenuItem('', {reactive: false});
        this.menu.addMenuItem(this._statusLabel);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        const addAction = (label, args) => {
            const item = new PopupMenu.PopupMenuItem(label);
            item.connect('activate', () => {
                runAsync([BIN, ...args]);
                // Let the daemon apply, then refresh the label.
                GLib.timeout_add(GLib.PRIORITY_DEFAULT, 250, () => {
                    this._refresh();
                    return GLib.SOURCE_REMOVE;
                });
            });
            this.menu.addMenuItem(item);
            return item;
        };

        this._brighter = addAction('Brightness +', ['--brighter']);
        this._dimmer = addAction('Brightness −', ['--dimmer']);
        this._warmer = addAction('Warmer', ['--warmer']);
        this._cooler = addAction('Cooler', ['--cooler']);

        this._autoSwitch = new PopupMenu.PopupSwitchMenuItem('Automatic', false);
        this._autoSwitch.connect('toggled', (_item, state) => {
            runAsync([BIN, '--auto', state ? 'on' : 'off']);
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, 250, () => {
                this._refresh();
                return GLib.SOURCE_REMOVE;
            });
        });
        this.menu.addMenuItem(this._autoSwitch);

        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        const settings = new PopupMenu.PopupMenuItem('Settings…');
        settings.connect('activate', () => runAsync([BIN, '--show-settings']));
        this.menu.addMenuItem(settings);

        const quit = new PopupMenu.PopupMenuItem('Quit');
        quit.connect('activate', () => runAsync([BIN, '--quit']));
        this.menu.addMenuItem(quit);

        this.menu.connect('open-state-changed', (_menu, open) => {
            if (open)
                this._refresh();
        });
        this._refresh();
    }

    _refresh() {
        const status = readStatus();
        if (!status) {
            this._statusLabel.label.text = 'Not running — launch Night Mode Controller';
            for (const item of [this._brighter, this._dimmer, this._warmer, this._cooler])
                item.setSensitive(false);
            this._autoSwitch.setSensitive(false);
            return;
        }
        const mode = status.auto ? 'Automatic' : 'Manual';
        this._statusLabel.label.text =
            `${mode} · ${status.temperature} K · Brightness ${status.brightness}%`;
        const canBrighten = status.hasBrightness === true;
        const canTemp = status.hasColor === true && status.auto === false;
        this._brighter.setSensitive(canBrighten);
        this._dimmer.setSensitive(canBrighten);
        this._warmer.setSensitive(canTemp);
        this._cooler.setSensitive(canTemp);
        this._autoSwitch.setSensitive(status.hasColor === true);
        this._autoSwitch.setToggleState(status.auto === true);
    }
}

export default class NightModeControllerExtension extends Extension {
    enable() {
        this._indicator = new Indicator();
        Main.panel.addToStatusArea(APP_ID, this._indicator);
    }

    disable() {
        this._indicator?.destroy();
        this._indicator = null;
    }
}
