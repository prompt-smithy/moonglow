# Moonglow Privacy Policy

Last updated: 19 November 2026

This policy applies to every platform version of Moonglow: Windows (including its Microsoft Store,
installer, and portable editions), macOS (including the Mac App Store, Homebrew, and direct
downloads), and Linux. It describes how the app and its maintainers handle information.

## Information handled by the app

Moonglow adjusts screen brightness and color temperature on your device. It works offline
and does not require an account. The app does not send your settings, display information,
or diagnostic logs to its maintainers or to an external server. It contains no advertising,
tracking, usage analytics, or automatic crash-reporting service.

To provide its features, Moonglow accesses the following information locally:

- Connected display names, identifiers, capabilities, and brightness or color settings,
  so it can detect and control supported displays.
- Your device's local time and appearance setting, so it can apply your schedule
  and match light or dark mode.
- Running process names and process identifiers, solely to check whether another copy of
  Moonglow is already running. This information is not saved as an application-usage history.
- Keyboard shortcuts configured for Moonglow, so its brightness and temperature controls
  work while you use other apps. Moonglow does not record text typed in other applications.

Moonglow does not capture your screen contents or access your browsing history, contacts,
camera, microphone, or geographic location.

## Information stored on your device

### Windows

Moonglow saves your brightness, color-temperature, schedule, startup, and keyboard-shortcut
preferences in your Windows user profile. Settings are normally stored at
`%APPDATA%\Moonglow\settings.json`; Windows may redirect app-data locations for Microsoft
Store installations. Earlier settings from the app's former name, Amber, may be imported
from `%APPDATA%\Amber\settings.json`.

The app also writes diagnostic logs in the Windows temporary folder, using the filename
`Moonglow-color.log` and numbered backup copies. These logs record timestamps, process and
thread identifiers, requested color temperatures, color-adjustment results, and technical
errors. They help troubleshoot display behavior and are rotated to limit their size.
They are not uploaded automatically.

If you enable launch at sign-in, Moonglow stores its startup registration and executable
path in your Windows user configuration.

These files rely on Windows file permissions for protection; Moonglow does not encrypt
them separately.

### macOS

On macOS, settings are stored at `~/Library/Application Support/Moonglow/settings.json`. In the
Mac App Store build, which runs in the macOS App Sandbox, the same files are kept inside the app's
container at `~/Library/Containers/com.prompt-smithy.Moonglow/Data/Library/Application Support/Moonglow/`.

Diagnostic logs are written to `~/Library/Logs/Moonglow/Moonglow.log`, with up to three rotated
backup copies. The logs record brightness and colour-temperature routing, display detection, and
technical errors. They are not uploaded automatically. In the Mac App Store build they are kept
inside the app's container.

If you enable launch at login, macOS records the registration through its `SMAppService` login-item
system.

### Linux

On Linux, settings are stored at `~/.config/moonglow/settings.json` (or under `$XDG_CONFIG_HOME`).
The application log is stored at `~/.local/state/moonglow/moonglow.log` (or under `$XDG_STATE_HOME`),
and launch at login is recorded as a desktop autostart entry in your user configuration folder.

## Your choices and deletion

You can change preferences, disable automatic scheduling, clear shortcuts, and turn off
launch at sign-in in the app's settings.

Settings remain on your device until removed. Diagnostic logs remain until rotated or deleted.
Uninstalling the app may leave local files. To remove them, first disable launch at sign-in and
exit Moonglow, then delete the settings folder and diagnostic logs, including backup copies, at
the locations listed above. For the Mac App Store build, these files are inside the app's container.
If you previously used Amber, remove its old settings too if you do not want them imported again.

## Support and third-party services

If you choose to contact the maintainers, they receive the information you submit, such as
your GitHub username, message, and any attachments. This information is used to respond to
your request and investigate or fix reported problems. You choose whether to share logs.

The [Moonglow issue tracker](https://github.com/prompt-smithy/moonglow/issues) is public.
Do not post private information there. Public reports and attachments may remain visible
in the project's history. GitHub handles its platform data under the
[GitHub Privacy Statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

Windows and Microsoft Store may independently process information for installation,
updates, and system diagnostics under the
[Microsoft Privacy Statement](https://www.microsoft.com/en-us/privacy/privacystatement).
Their data practices are separate from the app's local processing described above.

## Children

The app's local data practices apply to all users. Moonglow does not request personal
information from children through the app.

## Changes and contact

This policy will be updated if Moonglow's data practices change. The date above identifies
the latest revision.

For questions about this policy, contact the Moonglow maintainers through the
[project issue tracker](https://github.com/prompt-smithy/moonglow/issues). For a sensitive
question, request a private contact method without posting the sensitive information.
